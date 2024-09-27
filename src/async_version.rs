extern crate smol;

use smol::channel::{unbounded, Sender};
use smol::fs::{read, read_dir};
use smol::lock::Semaphore;
use smol::pin;
use smol::stream::StreamExt;
use smol::{io, LocalExecutor};
use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt::{Display, Write};
use std::path::{Path, PathBuf};
use std::rc::Rc;

use sha3::{Digest, Sha3_256};

use super::Comparison;

const MAX_OPEN_FILES: usize = 900;

pub fn find_doubles<P: AsRef<Path>>(comp: Comparison, dir: &P) {
    let ex = Rc::new(LocalExecutor::new());

    smol::block_on(ex.run(find_doubles_async(ex.clone(), comp, dir)));
}

async fn find_doubles_async<P: AsRef<Path>>(ex: Rc<LocalExecutor<'_>>, comp: Comparison, dir: &P) {
    let mut files: HashMap<String, Vec<PathBuf>> = HashMap::new();

    let (tx, rx) = unbounded();

    let semaphore = Rc::new(Semaphore::new(MAX_OPEN_FILES));

    match comp {
        Comparison::FileName => {
            enter_dir(
                ex,
                semaphore,
                tx,
                dir.as_ref().to_path_buf(),
                &get_file_id_by_file_name,
            )
            .await
        }
        Comparison::Hash => {
            enter_dir(
                ex,
                semaphore,
                tx,
                dir.as_ref().to_path_buf(),
                &get_file_id_by_hash,
            )
            .await
        }
        Comparison::Both => {
            enter_dir(
                ex,
                semaphore,
                tx,
                dir.as_ref().to_path_buf(),
                &get_file_id_by_both,
            )
            .await
        }
    }

    pin!(rx);

    while let Some((file_id, file_path)) = rx.next().await {
        files.entry(file_id).or_default().push(file_path);
    }

    CF.with_borrow(|cf| CD.with_borrow(|cd| println!("f {}, d {}", cf, cd)));

    // display_doubles(&files);
}

async fn enter_dir_or_file<'a, E: Display + 'a>(
    ex: Rc<LocalExecutor<'a>>,
    semaphore: Rc<Semaphore>,
    known_names: Sender<(String, PathBuf)>,
    path: PathBuf,
    get_file_id: &'a impl async Fn(&PathBuf) -> Result<String, E>,
) {
    // TODO: is_file is sync
    if path.is_file() {
        ex.spawn(enter_file(semaphore, known_names, path, get_file_id))
            .detach();
    } else if path.is_dir() {
        ex.spawn(enter_dir(
            ex.clone(),
            semaphore,
            known_names,
            path,
            get_file_id,
        ))
        .detach();
    }
}

async fn enter_file<E: Display>(
    semaphore: Rc<Semaphore>,
    known_names: Sender<(String, PathBuf)>,
    file_path: PathBuf,
    get_file_id: &impl async Fn(&PathBuf) -> Result<String, E>,
) {
    let _lock = semaphore.acquire().await;

    CF.with_borrow_mut(|cf| *cf += 1);

    /*
    // TODO: sync ope
    if !file_path.is_file() {
        panic!("Not a file : `{}`!", file_path.to_string_lossy());
    }
    */

    // println!("file {}", file_path.to_string_lossy());
    match get_file_id(&file_path).await {
        Ok(file_id) => {
            known_names.send((file_id, file_path)).await.unwrap();
        }
        Err(err) => eprintln!(
            "Error when getting file identifier for `{}` : {}",
            file_path.to_string_lossy(),
            err
        ),
    }

    // CF.with_borrow_mut(|cf| *cf -= 1);
}

thread_local! {
static CF: RefCell<isize> = RefCell::new(0);
static CD: RefCell<isize> = RefCell::new(0);
}

async fn enter_dir<'a, E: Display + 'a>(
    ex: Rc<LocalExecutor<'a>>,
    semaphore: Rc<Semaphore>,
    known_names: Sender<(String, PathBuf)>,
    dir_path: PathBuf,
    get_file_id: &'a impl async Fn(&PathBuf) -> Result<String, E>,
) {
    /*
    let is_zero = format!("{:?}", semaphore)
        .chars()
        .nth("Semaphore { count: ".len())
        .filter(|e| *e == '0')
        .is_some();
    if is_zero {
        CF.with_borrow(|cf| {
            CD.with_borrow(|cd| {
                eprintln!(
                    "Semaphore will block | f {}, d {} | {:?}",
                    cf, cd, semaphore
                )
            })
        });
    }
    */

    let _lock = semaphore.acquire().await;
    /*
    // TODO: sync ope
    if !dir_path.is_dir() {
        panic!("Not a directory : `{}`!", dir_path.to_string_lossy());
    }
    */

    CD.with_borrow_mut(|cd| *cd += 1);

    // println!("{:?} dir  {}", semaphore, dir_path.to_string_lossy());

    // Reading all at once to close de FS handle (I hope).
    // let mut c = 0;
    let entries: Vec<io::Result<PathBuf>> = match read_dir(&dir_path).await {
        Ok(entries) => entries.map(|r| r.map(|e| e.path())).collect().await,
        Err(err) => {
            eprintln!(
                "{:?} Error when reading dir `{}` : {}",
                semaphore,
                dir_path.to_string_lossy(),
                err
            );
            return ();
        }
    };

    for entry_res in entries.into_iter() {
        match entry_res {
            Ok(entry) => {
                enter_dir_or_file(
                    ex.clone(),
                    semaphore.clone(),
                    known_names.clone(),
                    entry,
                    get_file_id,
                )
                .await
            }
            Err(err) => eprintln!(
                "Error when reading dir entry `{}` : {}",
                dir_path.to_string_lossy(),
                err
            ),
        }
    }

    // CD.with_borrow_mut(|cd| *cd -= 1);
}

async fn get_file_id_by_file_name(file: &PathBuf) -> Result<String, String> {
    if let Some(name) = file.file_name() {
        Ok(name.to_string_lossy().into_owned())
    } else {
        Err("No name for given path.".to_string())
    }
}

async fn get_file_id_by_hash(file: &PathBuf) -> io::Result<String> {
    let mut hasher = Sha3_256::new();
    let file_content = read(file).await?;

    hasher.update(file_content);

    let hash = hasher.finalize();
    let mut hash_str = "0x".to_string();
    for i in hash.iter() {
        write!(hash_str, "{:02x}", i).unwrap();
    }
    Ok(hash_str)
}

async fn get_file_id_by_both(file: &PathBuf) -> Result<String, String> {
    let name = get_file_id_by_file_name(file).await?;
    let hash = match get_file_id_by_hash(file).await {
        Ok(hash) => hash,
        Err(err) => return Err(err.to_string()),
    };

    Ok(format!("{}:{}", name, hash))
}
