extern crate sha3;

use std::collections::HashMap;
use std::fs::read_dir;
use std::hash::Hash;
use std::path::{Path, PathBuf};

type FnGetFileId<T> = (dyn Fn(&PathBuf) -> T);

pub fn find_doubles<P: AsRef<Path>>(dir: &P) {
    let mut files = HashMap::new();
    enter_dir_or_file(
        &mut files,
        dir.as_ref().to_path_buf(),
        &get_file_id_by_file_name,
    );
    display_doubles(&files);
}

fn enter_dir_or_file<T: Eq + Hash>(
    known_names: &mut HashMap<T, Vec<PathBuf>>,
    path: PathBuf,
    get_file_id: &FnGetFileId<T>,
) {
    if path.is_file() {
        enter_file(known_names, path, get_file_id);
    } else if path.is_dir() {
        enter_dir(known_names, path, get_file_id);
    }
}

fn enter_file<T: Eq + Hash>(
    known_names: &mut HashMap<T, Vec<PathBuf>>,
    file_path: PathBuf,
    get_file_id: &FnGetFileId<T>,
) {
    if !file_path.is_file() {
        panic!("Not a file : `{}`!", file_path.to_string_lossy());
    }

    // println!("file {}", file_path.to_string_lossy());
    let file_id = get_file_id(&file_path);
    let vec_opt = known_names.entry(file_id).or_default();
    vec_opt.push(file_path);
}

fn enter_dir<T: Eq + Hash>(
    known_names: &mut HashMap<T, Vec<PathBuf>>,
    dir_path: PathBuf,
    get_file_id: &FnGetFileId<T>,
) {
    if !dir_path.is_dir() {
        panic!("Not a directory : `{}`!", dir_path.to_string_lossy());
    }

    // println!("dir  {}", dir_path.to_string_lossy());
    match read_dir(&dir_path) {
        Ok(entries) => entries.for_each(|entry_res| match entry_res {
            Ok(entry) => enter_dir_or_file(known_names, entry.path(), get_file_id),
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

fn display_doubles(files: &HashMap<String, Vec<PathBuf>>) {
    files
        .iter()
        .filter(|(_, vec)| vec.len() > 1)
        .for_each(|(f, vec)| {
            println!("{} :", f);
            vec.iter()
                .for_each(|path| println!("    - {}", path.to_string_lossy()));
        });
}

fn get_file_id_by_file_name(file: &PathBuf) -> String {
    file.file_name().unwrap().to_string_lossy().into_owned()
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
