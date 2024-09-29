extern crate smol;

use smol::channel::{bounded, unbounded, Sender};
use smol::fs::read_dir;
use smol::lock::Semaphore;
use smol::stream::StreamExt;
use smol::Executor;
use smol::{pin, Task};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{atomic::Ordering, Arc};
use std::thread;

use super::{
    get_file_id_by_both, get_file_id_by_file_name, get_file_id_by_hash, Comparison, CD, CF,
    MAX_OPEN_FILES,
};

pub fn find_doubles(comp: Comparison, dir: PathBuf) -> HashMap<String, Vec<PathBuf>> {
    thread::scope(|s| {
        // TODO: remove Arc by moving executer outside ?
        let ex = Arc::new(Executor::new());
        let num_threads = thread::available_parallelism().unwrap().into();
        let mut txs = Vec::with_capacity(num_threads);

        for _ in 0..=num_threads {
            let ex = ex.clone();
            let (tx, rx) = bounded(1);
            s.spawn(move || {
                smol::block_on(ex.run(async {
                    rx.recv().await.unwrap();
                }))
            });
            txs.push(tx);
        }

        let ex2 = ex.clone();
        smol::block_on(ex.run(async {
            let files = find_doubles_async(ex2, comp, dir).await;

            // TODO: might not need to send, just to drop, but it creates a receive err.
            for tx in txs {
                tx.send(()).await.unwrap();
            }

            files
        }))
    })
}

async fn find_doubles_async(
    ex: Arc<Executor<'_>>,
    comp: Comparison,
    dir: PathBuf,
) -> HashMap<String, Vec<PathBuf>> {
    let mut files: HashMap<String, Vec<PathBuf>> = HashMap::new();

    let (tx, rx) = unbounded();

    let semaphore = Arc::new(Semaphore::new(MAX_OPEN_FILES));

    ex.spawn(enter_dir(ex.clone(), semaphore, tx, dir, comp))
        .detach();

    pin!(rx);

    while let Some((file_id, file_path)) = rx.next().await {
        files.entry(file_id).or_default().push(file_path);
    }

    files
}

async fn enter_file(
    semaphore: Arc<Semaphore>,
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

// Have to create a special function so compiler doesn't detect recursion and keeps future  send ?
fn spawn_dir(
    ex: Arc<Executor<'_>>,
    semaphore: Arc<Semaphore>,
    known_names: Sender<(String, PathBuf)>,
    entry_path: PathBuf,
    comp: Comparison,
) {
    ex.spawn(enter_dir(
        ex.clone(),
        semaphore.clone(),
        known_names.clone(),
        entry_path,
        comp,
    ))
    .detach();
}

async fn enter_dir(
    ex: Arc<Executor<'_>>,
    semaphore: Arc<Semaphore>,
    known_names: Sender<(String, PathBuf)>,
    dir_path: PathBuf,
    comp: Comparison,
) {
    /*
    // Affiche un message quand le sémaphore va bloquer.
    let is_zero = format!("{:?}", semaphore)
        .chars()
        .nth("Semaphore { count: ".len())
        .filter(|e| *e == '0')
        .is_some();
    if is_zero {
        eprintln!(
            "Semaphore will block | f {}, d {} | {:?}",
            CF.load(Ordering::Acquire),
            CD.load(Ordering::Acquire),
            semaphore
        )
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

    let mut files = Vec::new();
    match read_dir(&dir_path).await {
        Ok(mut entries) => {
            while let Some(entry_res) = entries.next().await {
                match entry_res {
                    Ok(entry) => match entry.metadata().await {
                        Ok(metadata) => {
                            if metadata.is_dir() {
                                spawn_dir(
                                    ex.clone(),
                                    semaphore.clone(),
                                    known_names.clone(),
                                    entry.path(),
                                    comp,
                                );
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

    if !files.is_empty() {
        let mut files_tasks = Vec::with_capacity(files.len());
        ex.spawn_many(files, &mut files_tasks);
        files_tasks.into_iter().for_each(Task::detach);
    }

    // CD.fetch_sub(1, Ordering::Relaxed);
}
