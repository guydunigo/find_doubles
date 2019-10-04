extern crate find_doubles;

use std::env::args;
use std::env::current_dir;
use std::path::PathBuf;
use std::process::exit;

use find_doubles::find_doubles;

fn main() {
    let dir = if let Some(dir) = args().nth(1) {
        PathBuf::from(&dir)
    } else {
        eprintln!("No folder was provided, using current working directory...");
        current_dir().unwrap()
    };

    if dir.is_dir() {
        let res = find_doubles(&dir);

        if let Err(err) = res {
            eprintln!("{}", err);
            exit(2);
        }
    } else {
        eprintln!(
            "Error: provided argument `{}` is not a directory.",
            dir.to_string_lossy()
        );
        exit(1);
    }
}
