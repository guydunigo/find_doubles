extern crate find_doubles;

use std::env::{args, current_dir};
use std::path::PathBuf;
use std::process::exit;

use find_doubles::{find_doubles, Backend, Comparison};

const DEFAULT_COMP: &str = find_doubles::COMP_NAME;
const DEFAULT_BACK_FILENAME: &str = find_doubles::BACK_SYNC;
const DEFAULT_BACK_HASH: &str = find_doubles::BACK_MULTI_THREADED;

const ERROR_CODE_BAD_COMP: i32 = 1;
const ERROR_CODE_BAD_DIR: i32 = 2;
const ERROR_CODE_BAD_BACK: i32 = 3;

fn main() {
    let mut args = args().skip(1);
    let comp_arg1 = args.next();
    let dir_arg2 = args.next();
    let backend_arg3 = args.next();

    let comp: Comparison = match comp_arg1.as_ref().map_or(DEFAULT_COMP, |e| &e[..]).parse() {
        Ok(comp) => comp,
        Err(err) => {
            eprintln!("{}", err);
            exit(ERROR_CODE_BAD_COMP);
        }
    };

    let dir = if let Some(dir) = dir_arg2 {
        PathBuf::from(&dir)
    } else {
        let dir = current_dir().unwrap();
        eprintln!(
            "No folder was provided, using current working directory : {}",
            dir.to_string_lossy()
        );
        dir
    };
    if !dir.is_dir() {
        eprintln!(
            "Error: provided argument `{}` is not a directory.",
            dir.to_string_lossy()
        );
        exit(ERROR_CODE_BAD_DIR);
    }

    let backend: Backend = match backend_arg3
        .as_ref()
        .map_or(
            if let Comparison::FileName = comp {
                DEFAULT_BACK_FILENAME
            } else {
                DEFAULT_BACK_HASH
            },
            |e| &e[..],
        )
        .parse()
    {
        Ok(backend) => backend,
        Err(err) => {
            eprintln!("{}", err);
            exit(ERROR_CODE_BAD_BACK);
        }
    };

    let enable_output = if backend_arg3.is_some() {
        eprintln!("A backend was provided, we disable output.");
        false
    } else {
        true
    };

    find_doubles(enable_output, comp, backend, dir);
}
