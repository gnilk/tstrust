use std::{fs, io};
use std::path::Path;

pub struct DirScanner {
    pub libraries : Vec<String>,
}

impl DirScanner {
    pub fn new() -> DirScanner {
        let dir_scanner = DirScanner {
            libraries: Vec::new(),
        };
        return dir_scanner;
    }

    // Recursively scan and add potential files to list
    pub fn scan(&mut self, dir: &Path) -> io::Result<()> {
        if dir.is_dir() {
            for entry in fs::read_dir(dir)? {
                let entry = entry?;
                let path = entry.path();
                if path.is_dir() {
                    self.scan(&path)?;
                } else {
                    match entry.path().to_str() {
                        None => {},
                        Some(v) => self.check_and_add(v),
                    }
                }
            }
        }
        Ok(())
    }

    fn check_and_add(&mut self, filename : &str) {
        if !self.is_extension_ok(filename) {
            return;
        }

        self.libraries.push(filename.to_string());
    }

    fn is_extension_ok(&mut self, filename : &str) -> bool {
        // Might need to extend this one..
        return filename.ends_with(".so")
    }
}
