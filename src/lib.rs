use std::collections::HashMap;
use std::fs::read_dir;
use std::path::{Path, PathBuf};

pub fn find_doubles<P: AsRef<Path>>(dir: &P) {
    let mut files = HashMap::new();
    enter_dir_or_file(&mut files, dir.as_ref().to_path_buf());
    display_doubles(&files);
}

fn enter_dir_or_file(known_names: &mut HashMap<String, Vec<PathBuf>>, path: PathBuf) {
    if path.is_file() {
        enter_file(known_names, path);
    } else if path.is_dir() {
        enter_dir(known_names, path);
    }
}

fn enter_file(known_names: &mut HashMap<String, Vec<PathBuf>>, file_path: PathBuf) {
    if !file_path.is_file() {
        panic!("Not a file : `{}`!", file_path.to_string_lossy());
    }

    // println!("file {}", file_path.to_string_lossy());
    let file_name = file_path
        .file_name()
        .unwrap()
        .to_string_lossy()
        .into_owned();
    let vec_opt = known_names.entry(file_name).or_default();
    vec_opt.push(file_path);
}

fn enter_dir(known_names: &mut HashMap<String, Vec<PathBuf>>, dir_path: PathBuf) {
    if !dir_path.is_dir() {
        panic!("Not a directory : `{}`!", dir_path.to_string_lossy());
    }

    // println!("dir  {}", dir_path.to_string_lossy());
    match read_dir(&dir_path) {
        Ok(entries) => entries.for_each(|entry_res| match entry_res {
            Ok(entry) => enter_dir_or_file(known_names, entry.path()),
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
            println!("`{}` :", f);
            vec.iter()
                .for_each(|path| println!("    - {}", path.to_string_lossy()));
        });
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
