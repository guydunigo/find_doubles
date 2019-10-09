extern crate find_doubles;

use std::env::args;
use std::env::current_dir;
use std::path::PathBuf;
use std::process::exit;

use find_doubles::{find_doubles, Comparison};

fn main() {
    let (comp, dir) = if let Some(comp) = args().nth(1) {
        let comp: Comparison = match comp.parse() {
            Ok(comp) => comp,
            Err(err) => {
                eprintln!("{}", err);
                exit(3);
            }
        };

        let dir = if let Some(dir) = args().nth(2) {
            PathBuf::from(&dir)
        } else {
            eprintln!("No folder was provided, using current working directory...");
            current_dir().unwrap()
        };

        (comp, dir)
    } else {
        eprintln!("No arguments provided, using file name to find duplicates in current working directory.");
        (Comparison::FileName, current_dir().unwrap())
    };

    if dir.is_dir() {
        find_doubles(comp, &dir);
    } else {
        eprintln!(
            "Error: provided argument `{}` is not a directory.",
            dir.to_string_lossy()
        );
        exit(1);
    }
}
