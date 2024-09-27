#![feature(async_closure)]
extern crate sha3;

use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt::{Display, Write};
use std::fs::{read, read_dir};
use std::io;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use sha3::{Digest, Sha3_256};

pub mod async_version;
pub mod threaded;

type FnGetFileId<E> = (dyn Fn(&Path) -> Result<String, E>);

const COMP_NAME: &str = "name";
const COMP_HASH: &str = "hash";
const COMP_BOTH: &str = "both";

thread_local! {
static CF: RefCell<isize> = const {RefCell::new(0)};
static CD: RefCell<isize> = const {RefCell::new(0)};
}

pub enum Comparison {
    FileName,
    Hash,
    Both,
}

impl FromStr for Comparison {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (comp, msg) = match s.to_lowercase().as_ref() {
            COMP_NAME => (Comparison::FileName, "file name"),
            COMP_HASH => (Comparison::Hash, "SHA3-256 hash"),
            COMP_BOTH => (Comparison::Both, "file name and SHA3-256 hash"),
            _ => {
                return Err(format!(
                    "Could not parse `{}` as comparison method, please use `{}`, `{}`, or `{}`.",
                    s, COMP_NAME, COMP_HASH, COMP_BOTH
                ));
            }
        };

        eprintln!("Using file {} to compare files and detect duplicates.", msg);

        Ok(comp)
    }
}

pub fn find_doubles<P: AsRef<Path>>(comp: Comparison, dir: &P) {
    let mut files = HashMap::new();
    match comp {
        Comparison::FileName => enter_dir(
            &mut files,
            dir.as_ref().to_path_buf(),
            &get_file_id_by_file_name,
        ),
        Comparison::Hash => enter_dir(&mut files, dir.as_ref().to_path_buf(), &get_file_id_by_hash),
        Comparison::Both => enter_dir(&mut files, dir.as_ref().to_path_buf(), &get_file_id_by_both),
    }
    CF.with_borrow(|cf| CD.with_borrow(|cd| println!("f {}, d {}", cf, cd)));
    display_doubles(&files);
}

fn enter_file<E: Display>(
    known_names: &mut HashMap<String, Vec<PathBuf>>,
    file_path: PathBuf,
    get_file_id: &FnGetFileId<E>,
) {
    /*
    // TODO
    if !file_path.is_file() {
        panic!("Not a file : `{}`!", file_path.to_string_lossy());
    }
    */

    CF.with_borrow_mut(|cf| *cf += 1);

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
    // TODO
    if !dir_path.is_dir() {
        panic!("Not a directory : `{}`!", dir_path.to_string_lossy());
    }
    */

    CD.with_borrow_mut(|cd| *cd += 1);

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

fn display_doubles<String: Display>(files: &HashMap<String, Vec<PathBuf>>) {
    files
        .iter()
        .filter(|(_, vec)| vec.len() > 1)
        .for_each(|(f, vec)| {
            println!("{} :", f);
            vec.iter()
                .for_each(|path| println!("    - {}", path.to_string_lossy()));
        });
}

fn get_file_id_by_file_name(file: &Path) -> Result<String, String> {
    if let Some(name) = file.file_name() {
        Ok(name.to_string_lossy().into_owned())
    } else {
        Err("No name for given path.".to_string())
    }
}

fn get_file_id_by_hash(file: &Path) -> io::Result<String> {
    let mut hasher = Sha3_256::new();
    let file_content = read(file)?;

    hasher.update(file_content);

    let hash = hasher.finalize();
    let mut hash_str = "0x".to_string();
    for i in hash.iter() {
        write!(hash_str, "{:02x}", i).unwrap();
    }
    Ok(hash_str)
}

fn get_file_id_by_both(file: &Path) -> Result<String, String> {
    let name = get_file_id_by_file_name(file)?;
    let hash = match get_file_id_by_hash(file) {
        Ok(hash) => hash,
        Err(err) => return Err(err.to_string()),
    };

    Ok(format!("{}:{}", name, hash))
}
