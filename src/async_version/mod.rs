extern crate smol;

use smol::channel::{unbounded, Sender};
use smol::fs::{read, read_dir};
use smol::lock::Semaphore;
use smol::stream::StreamExt;
use smol::LocalExecutor;
use smol::{pin, Task};
use std::collections::HashMap;
use std::fmt::Write;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::atomic::Ordering;

use sha3::{Digest, Sha3_256};

use super::{Comparison, CD, CF};

pub mod multi_async;

const MAX_OPEN_FILES: usize = 10;

pub fn find_doubles(comp: Comparison, dir: PathBuf) -> HashMap<String, Vec<PathBuf>> {
    // Should be possible of getting rid of Rc and just use references, but it seems
    // the reference in or of the executor outlives here...
    let ex = Rc::new(LocalExecutor::new());
    smol::block_on(ex.run(find_doubles_async(ex.clone(), comp, dir)))
}

async fn find_doubles_async(
    ex: Rc<LocalExecutor<'_>>,
    comp: Comparison,
    dir: PathBuf,
) -> HashMap<String, Vec<PathBuf>> {
    let mut files: HashMap<String, Vec<PathBuf>> = HashMap::new();

    let (tx, rx) = unbounded();

    let semaphore = Rc::new(Semaphore::new(MAX_OPEN_FILES));

    enter_dir(ex, semaphore, tx, dir, comp).await;

    pin!(rx);

    while let Some((file_id, file_path)) = rx.next().await {
        files.entry(file_id).or_default().push(file_path);
    }

    files
}

async fn enter_file(
    semaphore: Rc<Semaphore>,
    known_names: Sender<(String, PathBuf)>,
    file_path: PathBuf,
    comp: Comparison,
) {
    let _lock = semaphore.acquire().await;

    CF.fetch_add(1, Ordering::Relaxed);

    /*
    if !file_path.is_file() {
        panic!("Not a file : `{}`!", file_path.to_string_lossy());
    }
    */

    // println!("file {}", file_path.to_string_lossy());
    let file_id = match comp {
        Comparison::FileName => get_file_id_by_file_name(&file_path).await,
        Comparison::Hash => get_file_id_by_hash(&file_path).await,
        Comparison::Both => get_file_id_by_both(&file_path).await,
    };

    match file_id {
        Ok(file_id) => {
            known_names.send((file_id, file_path)).await.unwrap();
        }
        Err(err) => eprintln!(
            "Error when getting file identifier for `{}` : {}",
            file_path.to_string_lossy(),
            err
        ),
    }

    // CF.fetch_sub(1, Ordering::Relaxed);
}

async fn enter_dir(
    ex: Rc<LocalExecutor<'_>>,
    semaphore: Rc<Semaphore>,
    known_names: Sender<(String, PathBuf)>,
    dir_path: PathBuf,
    comp: Comparison,
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
    if !dir_path.is_dir() {
        panic!("Not a directory : `{}`!", dir_path.to_string_lossy());
    }
    */

    CD.fetch_add(1, Ordering::Relaxed);

    // println!("{:?} dir  {}", semaphore, dir_path.to_string_lossy());

    let mut dirs = Vec::new();
    let mut files = Vec::new();
    match read_dir(&dir_path).await {
        Ok(mut entries) => {
            while let Some(entry_res) = entries.next().await {
                match entry_res {
                    Ok(entry) => match entry.metadata().await {
                        Ok(metadata) => {
                            if metadata.is_dir() {
                                dirs.push(enter_dir(
                                    ex.clone(),
                                    semaphore.clone(),
                                    known_names.clone(),
                                    entry.path(),
                                    comp,
                                ));
                            } else if metadata.is_file() {
                                files.push(enter_file(
                                    semaphore.clone(),
                                    known_names.clone(),
                                    entry.path(),
                                    comp,
                                ));
                            }
                        }
                        Err(err) => eprintln!(
                            "Error when reading entry metadata `{}` : {}",
                            entry.path().to_string_lossy(),
                            err
                        ),
                    },
                    Err(err) => eprintln!(
                        "Error when reading dir entry `{}` : {}",
                        dir_path.to_string_lossy(),
                        err
                    ),
                }
            }
        }
        Err(err) => {
            eprintln!(
                "{:?} Error when reading dir `{}` : {}",
                semaphore,
                dir_path.to_string_lossy(),
                err
            );
        }
    }

    if !dirs.is_empty() {
        let mut dirs_tasks = Vec::with_capacity(dirs.len());
        ex.spawn_many(dirs, &mut dirs_tasks);
        dirs_tasks.into_iter().for_each(Task::detach);
    }
    if !files.is_empty() {
        let mut files_tasks = Vec::with_capacity(files.len());
        ex.spawn_many(files, &mut files_tasks);
        files_tasks.into_iter().for_each(Task::detach);
    }

    // CD.fetch_sub(1, Ordering::Relaxed);
}

async fn get_file_id_by_file_name(file: &Path) -> Result<String, String> {
    if let Some(name) = file.file_name() {
        Ok(name.to_string_lossy().into_owned())
    } else {
        Err("No name for given path.".to_string())
    }
}

async fn get_file_id_by_hash(file: &Path) -> Result<String, String> {
    let mut hasher = Sha3_256::new();
    let file_content = read(file).await.map_err(|e| e.to_string())?;

    hasher.update(file_content);

    let hash = hasher.finalize();
    let mut hash_str = "0x".to_string();
    for i in hash.iter() {
        write!(hash_str, "{:02x}", i).unwrap();
    }
    Ok(hash_str)
}

async fn get_file_id_by_both(file: &Path) -> Result<String, String> {
    let name = get_file_id_by_file_name(file).await?;
    let hash = match get_file_id_by_hash(file).await {
        Ok(hash) => hash,
        Err(err) => return Err(err.to_string()),
    };

    Ok(format!("{}:{}", name, hash))
}
