use std::collections::HashMap;
use std::fmt::Display;
use std::fs::read_dir;
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;

use super::{
    get_file_id_by_both, get_file_id_by_file_name, get_file_id_by_hash, Comparison, CD, CF,
};

type FnGetFileId<E> = (dyn Fn(&Path) -> Result<String, E>);

pub fn find_doubles(comp: Comparison, dir: PathBuf) -> HashMap<String, Vec<PathBuf>> {
    let mut files = HashMap::new();
    match comp {
        Comparison::FileName => enter_dir(&mut files, dir, &get_file_id_by_file_name),
        Comparison::Hash => enter_dir(&mut files, dir, &get_file_id_by_hash),
        Comparison::Both => enter_dir(&mut files, dir, &get_file_id_by_both),
    }

    files
}

fn enter_file<E: Display>(
    known_names: &mut HashMap<String, Vec<PathBuf>>,
    file_path: PathBuf,
    get_file_id: &FnGetFileId<E>,
) {
    /*
    if !file_path.is_file() {
        panic!("Not a file : `{}`!", file_path.to_string_lossy());
    }
    */

    CF.fetch_add(1, Ordering::Relaxed);

    // println!("file {}", file_path.to_string_lossy());
    match get_file_id(&file_path) {
        Ok(file_id) => {
            let vec_opt = known_names.entry(file_id).or_default();
            vec_opt.push(file_path);
        }
        Err(err) => eprintln!(
            "Error when getting file identifier for `{}` : {}",
            file_path.to_string_lossy(),
            err
        ),
    }
}

fn enter_dir<E: Display>(
    known_names: &mut HashMap<String, Vec<PathBuf>>,
    dir_path: PathBuf,
    get_file_id: &FnGetFileId<E>,
) {
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
                        enter_dir(known_names, entry.path(), get_file_id);
                    } else if metadata.is_file() {
                        enter_file(known_names, entry.path(), get_file_id);
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
