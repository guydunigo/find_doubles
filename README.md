# Find duplicate files

## Description

This crate lists all files sharing the same name, same hash or both in the given directory.

## Usage

```shell
find_duplicates [comparison_kind [directory]]
```

- `comparison_kind` should be one of `name`|`hash`|`both`. It defines whether files will be considered duplicates if they have the same name, hash (SHA3-256) or both.
- If no argument are given, it will search duplicate only by comparing file-names into the current directory (as if `find_duplicates name .` was called).
- If no `directory` is provided, the program will looking into the current directory.

## Exit codes

- `1` : argument is not a directory
- `2` : input-output error (not read or write right on the directory for instance)
- `3` : could not parse `comparison_kind` into one of the authorised values
