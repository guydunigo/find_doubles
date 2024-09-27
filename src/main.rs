extern crate find_doubles;

use std::env::{args, current_dir};
use std::path::PathBuf;
use std::process::exit;

use find_doubles::{async_version, find_doubles, Comparison};

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
        if args().nth(3).is_some() {
            async_version::find_doubles(comp, &dir);
            println!("Async finished");
        } else {
            find_doubles(comp, &dir);
            println!("Sync finished");
        }
    } else {
        eprintln!(
            "Error: provided argument `{}` is not a directory.",
            dir.to_string_lossy()
        );
        exit(1);
    }
}
