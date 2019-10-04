# Find duplicate files

## Description

This crate lists all files sharing the same name in the given directory.

## Usage

```shell
find_duplicates [directory]
```

If no `directory` is provided, the program will looking into the current directory.

## Exit codes

- `1` : argument is not a directory
- `2` : input-output error (not read or write right on the directory for instance)
