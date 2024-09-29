#![feature(async_closure)]
extern crate sha3;

use std::collections::HashMap;
use std::fmt::{Display, Write};
use std::fs::read;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::atomic::{AtomicIsize, Ordering};
use std::time::Instant;

use sha3::{Digest, Sha3_256};

mod async_version;
mod sync;
use async_version::multi_async;
mod multithreaded;
mod threaded;

static CF: AtomicIsize = const { AtomicIsize::new(0) };
static CD: AtomicIsize = const { AtomicIsize::new(0) };

pub const COMP_NAME: &str = "name";
pub const COMP_HASH: &str = "hash";
pub const COMP_BOTH: &str = "both";
pub const BACK_SYNC: &str = "sync";
pub const BACK_ASYNC: &str = "async";
pub const BACK_MULTI_ASYNC: &str = "multi_async";
pub const BACK_THREADED: &str = "thread";
pub const BACK_MULTI_THREADED: &str = "multi_thread";
pub const BACK_ALL: &str = "all";

#[derive(Clone, Copy, Debug)]
pub enum Comparison {
    FileName,
    Hash,
    Both,
}

impl FromStr for Comparison {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (comp, msg) = match s.to_lowercase().as_ref() {
            COMP_NAME => (Comparison::FileName, "name"),
            COMP_HASH => (Comparison::Hash, "SHA3-256 hash"),
            COMP_BOTH => (Comparison::Both, "name and SHA3-256 hash"),
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

#[derive(Debug)]
pub enum Backend {
    Sync,
    Async,
    MultiAsync,
    Threaded,
    MultiThreaded,
    All,
}

impl FromStr for Backend {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let comp = match s.to_lowercase().as_ref() {
            BACK_SYNC => Backend::Sync,
            BACK_ASYNC => Backend::Async,
            BACK_MULTI_ASYNC => Backend::MultiAsync,
            BACK_THREADED => Backend::Threaded,
            BACK_MULTI_THREADED => Backend::MultiThreaded,
            BACK_ALL => Backend::All,
            _ => {
                return Err(format!(
                    "Could not parse `{}` as backend, please use `{}`, `{}`, `{}`, `{}`, or `{}`.",
                    s, BACK_SYNC, BACK_ASYNC, BACK_MULTI_ASYNC, BACK_THREADED, BACK_MULTI_THREADED,
                ));
            }
        };

        eprintln!("Using backend {}.", s);

        Ok(comp)
    }
}

pub fn find_doubles(enable_output: bool, comp: Comparison, backend: Backend, dir: PathBuf) {
    if let Backend::All = backend {
        eprintln!("Useless first try that should be much longer if the system hasn't already cached files.");
        find_doubles(enable_output, comp, Backend::MultiThreaded, dir.clone());
        eprintln!("--------------------------------------------------------------------------------------------------------------------------------\n");
        find_doubles(enable_output, comp, Backend::Sync, dir.clone());
        find_doubles(enable_output, comp, Backend::Async, dir.clone());
        find_doubles(enable_output, comp, Backend::Threaded, dir.clone());
        find_doubles(enable_output, comp, Backend::MultiThreaded, dir.clone());
        find_doubles(enable_output, comp, Backend::MultiAsync, dir.clone());
        return;
    }

    eprintln!("Backend {:?}", backend);

    let backend = match backend {
        Backend::Sync => sync::find_doubles,
        Backend::Async => async_version::find_doubles,
        Backend::MultiAsync => multi_async::find_doubles,
        Backend::Threaded => threaded::find_doubles,
        Backend::MultiThreaded => multithreaded::find_doubles,
        Backend::All => unreachable!(),
    };

    // Reset file and directory counters.
    CF.store(0, Ordering::Relaxed);
    CD.store(0, Ordering::Relaxed);

    let start = Instant::now();
    let files = backend(comp, dir);
    let end = Instant::now();

    if enable_output {
        display_doubles(&files);
    }

    eprintln!(
        "    Stats : files {}, dirs {}",
        CF.load(Ordering::Acquire),
        CD.load(Ordering::Acquire)
    );

    eprintln!("    Finished in {}s\n", end.duration_since(start).as_secs());
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

fn get_file_id_by_hash(file: &Path) -> Result<String, String> {
    let mut hasher = Sha3_256::new();
    let file_content = read(file).map_err(|e| e.to_string())?;

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
