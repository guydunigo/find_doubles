use std::collections::HashMap;
use std::fs::read_dir;
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::sync::mpsc;
use std::thread;

extern crate loole;
use loole::{unbounded, Sender};

use super::{
    get_file_id_by_both, get_file_id_by_file_name, get_file_id_by_hash, Comparison, CD, CF,
};

pub fn find_doubles(comp: Comparison, dir: PathBuf) -> HashMap<String, Vec<PathBuf>> {
    thread::scope(move |s| {
        let (tx, rx) = unbounded::<PathBuf>();
        let frx = {
            let (ftx, frx) = mpsc::channel::<(String, PathBuf)>();

            for _ in 0..=thread::available_parallelism().unwrap().into() {
                let ftx = ftx.clone();
                let rx = rx.clone();
                s.spawn(move || {
                    for p in rx {
                        if let Some(id) = enter_file(&p, comp) {
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

        enter_dir(tx, dir);

        handle.join().unwrap()
    })
}

fn enter_file(file_path: &Path, comp: Comparison) -> Option<String> {
    /*
    if !file_path.is_file() {
        panic!("Not a file : `{}`!", file_path.to_string_lossy());
    }
    */

    CF.fetch_add(1, Ordering::Relaxed);

    // println!("file {}", file_path.to_string_lossy());
    let file_id = match comp {
        Comparison::FileName => get_file_id_by_file_name(file_path),
        Comparison::Hash => get_file_id_by_hash(file_path),
        Comparison::Both => get_file_id_by_both(file_path),
    };

    file_id
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
