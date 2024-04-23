use std::{env};
use std::cell::{RefCell};

use std::rc::Rc;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::string::ToString;

// Bring in everything - this is just our way to split things...
use tstrust::test_runner::*;

//
// well - main...
//
fn main() {
    let cfg = Config::instance();


    // Putting stuff in an 'app' instance - this 'solves' global variable problems..
    // Tried having a 'context' but was constantly battling life-time handling - this made it much easier...
    let mut app = App::new();
    let runner = app.scan_libraries(&cfg.inputs);

    if cfg.list_tests {
        // app.list_tests();
    }
    if cfg.execute_tests {
        app.execute_tests();
    }
}



struct App {
    runners : Vec<TestRunner>,
}
impl App {

    pub fn new<'a>() -> App {
        let instance = App {
            runners : Vec::new(),
        };
        return instance;
    }
    fn scan_libraries(&mut self, inputs: &Vec<String>) {
        for x in inputs {
            self.scan_path_or_library(&x);
        }
    }

    fn scan_path_or_library(&mut self, input: &str) {
        if input == "." {
            let cdir = env::current_dir().unwrap();
            self.scan_directory(&cdir);
        } else {
            let path = Path::new(input);
            match path {
                x if x.is_dir() => self.scan_directory(&x.to_path_buf()),
                x if x.is_file() => self.scan_library(x.to_str().unwrap()),
                _ => println!("ERR: Unsupported file type")
            }
        }
    }

    fn scan_directory(&mut self, dirname: &PathBuf) {
        let mut dir_scanner = DirScanner::new();
        dir_scanner.scan(dirname.as_path()).expect("wef");
        for library in &dir_scanner.libraries {
            self.scan_library(library)
        }
    }
    fn scan_library(&mut self, filename: &str) {
        let library = Rc::new(DynLibrary::new(filename));

        // TEST TEST
        let mut tr = TestRunner::new(&library);
        tr.prescan();
        self.runners.push(tr);
    }
    fn list_tests(&self) {
        for runner in &self.runners {
            runner.dump();
        }
    }

    fn execute_tests(&mut self) {
        for runner in &mut self.runners {
            runner.execute_tests();
        }
    }

}

