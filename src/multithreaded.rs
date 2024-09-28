use std::collections::HashMap;
use std::fmt::Display;
use std::fs::read_dir;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicIsize, Ordering};
use std::sync::mpsc;
use std::thread;

extern crate loole;
use loole::{unbounded, Sender};

use super::{
    display_doubles, get_file_id_by_both, get_file_id_by_file_name, get_file_id_by_hash,
    Comparison, FnGetFileId,
};

static CF: AtomicIsize = const { AtomicIsize::new(0) };
static CD: AtomicIsize = const { AtomicIsize::new(0) };

pub fn find_doubles<P: AsRef<Path>>(comp: Comparison, dir: &P) {
    let files = thread::scope(move |s| {
        let (tx, rx) = unbounded::<PathBuf>();
        let frx = {
            let (ftx, frx) = mpsc::channel::<(String, PathBuf)>();

            for _ in 0..=thread::available_parallelism().unwrap().into() {
                let ftx = ftx.clone();
                let rx = rx.clone();
                s.spawn(move || {
                    for p in rx {
                        let id = match comp {
                            Comparison::FileName => enter_file(&p, &get_file_id_by_file_name),
                            Comparison::Hash => enter_file(&p, &get_file_id_by_hash),
                            Comparison::Both => enter_file(&p, &get_file_id_by_both),
                        };
                        if let Some(id) = id {
                            ftx.send((id, p)).unwrap();
                        }
                    }
                });
            }

            frx
        };

        let handle = s.spawn(move || {
            let mut files: HashMap<String, Vec<PathBuf>> = HashMap::new();
            for (file_id, file_path) in frx {
                let vec_opt = files.entry(file_id).or_default();
                vec_opt.push(file_path);
            }
            files
        });

        enter_dir(tx, dir.as_ref().to_path_buf());

        handle.join().unwrap()
    });
    println!(
        "f {}, d {}",
        CF.load(Ordering::Acquire),
        CD.load(Ordering::Acquire)
    );
    display_doubles(&files);
}

fn enter_file<E: Display>(file_path: &Path, get_file_id: &FnGetFileId<E>) -> Option<String> {
    /*
    if !file_path.is_file() {
        panic!("Not a file : `{}`!", file_path.to_string_lossy());
    }
    */

    CF.fetch_add(1, Ordering::Relaxed);

    // println!("file {}", file_path.to_string_lossy());
    get_file_id(file_path)
        .inspect_err(|err| {
            eprintln!(
                "Error when getting file identifier for `{}` : {}",
                file_path.to_string_lossy(),
                err
            )
        })
        .ok()
}

fn enter_dir(known_names: Sender<PathBuf>, dir_path: PathBuf) {
    /*
    if !dir_path.is_dir() {
        panic!("Not a directory : `{}`!", dir_path.to_string_lossy());
    }
    */

    CD.fetch_add(1, Ordering::Relaxed);

    // println!("dir  {}", dir_path.to_string_lossy());
    match read_dir(&dir_path) {
        Ok(entries) => entries.for_each(|entry_res| match entry_res {
            Ok(entry) => match entry.metadata() {
                Ok(metadata) => {
                    if metadata.is_dir() {
                        enter_dir(known_names.clone(), entry.path());
                    } else if metadata.is_file() {
                        known_names.send(entry.path()).unwrap();
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
        }),
        Err(err) => eprintln!(
            "Error when reading dir `{}` : {}",
            dir_path.to_string_lossy(),
            err
        ),
    }
}
