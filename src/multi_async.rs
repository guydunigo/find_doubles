extern crate smol;

use smol::channel::{bounded, unbounded, Sender};
use smol::fs::read_dir;
use smol::lock::Semaphore;
use smol::stream::StreamExt;
use smol::Executor;
use smol::{pin, Task};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{atomic::Ordering, Arc};
use std::thread;

use super::{
    async_version::{get_file_id_by_both, get_file_id_by_file_name, get_file_id_by_hash},
    display_doubles, Comparison, CD, CF,
};

const MAX_OPEN_FILES: usize = 1000;

pub fn find_doubles<P: AsRef<Path>>(comp: Comparison, dir: &P) {
    thread::scope(|s| {
        let ex = Arc::new(Executor::new());
        let num_threads = thread::available_parallelism().unwrap().into();
        let mut txs = Vec::with_capacity(num_threads);

        for _ in 0..=num_threads {
            let ex = ex.clone();
            let (tx, rx) = bounded(1);
            s.spawn(move || smol::block_on(ex.run(rx.recv())));
            txs.push(tx);
        }

        let ex2 = ex.clone();
        smol::block_on(ex.run(async {
            find_doubles_async(ex2, comp, dir).await;

            for tx in txs {
                tx.send(()).await.unwrap();
            }
        }));
    });
}

async fn find_doubles_async<P: AsRef<Path>>(ex: Arc<Executor<'_>>, comp: Comparison, dir: &P) {
    let mut files: HashMap<String, Vec<PathBuf>> = HashMap::new();

    let (tx, rx) = unbounded();

    let semaphore = Arc::new(Semaphore::new(MAX_OPEN_FILES));

    enter_dir(ex, semaphore, tx, dir.as_ref().to_path_buf(), comp).await;

    pin!(rx);

    while let Some((file_id, file_path)) = rx.next().await {
        files.entry(file_id).or_default().push(file_path);
    }

    println!(
        "f {}, d {}",
        CF.load(Ordering::Acquire),
        CD.load(Ordering::Acquire)
    );

    display_doubles(&files);
}

async fn enter_file(
    semaphore: Arc<Semaphore>,
    known_names: Sender<(String, PathBuf)>,
    file_path: PathBuf,
    comp: Comparison,
) {
    let _lock = semaphore.acquire().await;

    CF.fetch_add(1, Ordering::Acquire);

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

    // CF.fetch_sub(1, Ordering::Release);
}

async fn enter_dir(
    ex: Arc<Executor<'_>>,
    semaphore: Arc<Semaphore>,
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

    CD.fetch_add(1, Ordering::Acquire);

    // println!("{:?} dir  {}", semaphore, dir_path.to_string_lossy());

    // TODO: let mut dirs = Vec::new();
    let mut files = Vec::new();
    match read_dir(&dir_path).await {
        Ok(mut entries) => {
            while let Some(entry_res) = entries.next().await {
                match entry_res {
                    Ok(entry) => match entry.metadata().await {
                        Ok(metadata) => {
                            if metadata.is_dir() {
                                /*
                                // TODO
                                dirs.push(enter_dir(
                                    ex.clone(),
                                    semaphore.clone(),
                                    known_names.clone(),
                                    entry.path(),
                                    comp,
                                ));
                                */
                                Box::pin(enter_dir(
                                    ex.clone(),
                                    semaphore.clone(),
                                    known_names.clone(),
                                    entry.path(),
                                    comp,
                                ))
                                .await;
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

    /*
    // TODO
    if !dirs.is_empty() {
        let mut dirs_tasks = Vec::with_capacity(dirs.len());
        ex.spawn_many(dirs, &mut dirs_tasks);
        dirs_tasks.into_iter().for_each(Task::detach);
    }
    */
    if !files.is_empty() {
        let mut files_tasks = Vec::with_capacity(files.len());
        ex.spawn_many(files, &mut files_tasks);
        files_tasks.into_iter().for_each(Task::detach);
    }

    // CD.fetch_sub(1, Ordering::Release);
}
