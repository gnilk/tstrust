//use sharedlib::Lib;
//use std::io;
use std::process::Command;
use std::{fs, io, env};
use std::fs::DirEntry;
use std::path::Path;


struct DirScanner {
    libraries : Vec<String>,    
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

#[derive(Debug)]
struct DynLibrary {
    name : String,
    exports : Vec<String>,
}

impl DynLibrary {
    pub fn new(dynlib_name : &str) -> DynLibrary {
        let dynlib = DynLibrary {
            name: dynlib_name.to_string(),
            exports: Vec::new(),
        };
        return dynlib;            
    }
    
    // I should probably not return 'bool' here...
    pub fn scan(&mut self) -> bool {
        let output = Command::new("nm")
                        .arg(self.name.as_str())
                        .output()
                        .expect("failed to execute");

        if !output.status.success() {
            println!("Error while scanning library");
            return false;
        }
        let str = String::from_utf8(output.stdout).expect("error");
        // This can most likely be rewritten as a chain of map's and filters..
        let lines = str.split("\n");       

        for line in lines {
            let parts: Vec<&str> = line.split_whitespace().collect();            
            if parts.len() != 3 {
                continue;
            }

            let fn_type = parts[1];
            if fn_type != "T" {
                continue;
            }

            let fn_name = parts[2];
            if !DynLibrary::is_testfun(fn_name) {
                continue;
            }
            self.exports.push(fn_name.to_string());
        }

        return true;
    } // scan    

    fn is_testfun(name : &str) -> bool {    
        if name.starts_with("test_") {
            return true;
        }
        return false;
    }
}   // impl...


struct Module {
    dynlib : DynLibrary,
}
impl Module {
    pub fn new(filename : &str) -> Module {
        let mut module = Module {
            dynlib : DynLibrary::new(filename),
        };
        module.dynlib.scan();
        return module;
    }

    pub fn dump(&self) {

        // Smarter way to filter??
        let dummy : Vec<&String> = self.dynlib.exports.iter().filter(|x| x.contains("casefilter")).collect();

        for d in dummy {
            println!("{}", d);
        }

        // println!("Lib: {}",self.dynlib.name);
        // for export in &self.dynlib.exports {
        //     println!("  {}", export);
        // } 
    }
}


fn main() {    
    println!("Hello, world!");
    let cdir = env::current_dir().expect("wef");


    let mut dir_scanner = DirScanner::new();
    dir_scanner.scan(cdir.as_path()).expect("wef");
    for lib in dir_scanner.libraries {
        println!("{}", lib);

        let module = Module::new(&lib);
        module.dump();
    }



}
