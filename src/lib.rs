use std::io;
use std::path::Path;

pub fn find_doubles<P: AsRef<Path>>(_dir: &P) -> io::Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
