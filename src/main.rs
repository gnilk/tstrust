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
    app.scan_libraries(&cfg.inputs);
    if cfg.list_tests {
        app.list_tests();
    }
    if cfg.execute_tests {
        app.execute_tests();
    }
}



struct App {
    modules_to_test : HashMap<String, ModuleRef>,
}
impl App {

    pub fn new<'a>() -> App {
        let instance = App {
            modules_to_test : HashMap::new(),
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
        let modules = modules_from_dynlib(&library);
        for (name, module) in modules.into_iter() {
            // I don't like the fact that this must be unsafe.. really need to consider putting this in a context object..

            if !self.modules_to_test.contains_key(&name) {
                // Don't want clone here...
                self.modules_to_test.insert(name, module);
            }
        }
    }

    fn list_tests(&self) {
        for (_, module) in &self.modules_to_test {
            match module.borrow().should_execute() {
                true => print!("*"),
                false=> print!("-"),
            }
            println!(" Module: {}",module.borrow().name);
            for tc in &module.borrow().test_cases {
                print!("  "); // indent
                match module.borrow().should_execute() && tc.borrow().should_execute() {
                    true => print!("*"),
                    false => print!("-"),
                }
                println!("  {}::{} ({})", module.borrow().name, tc.borrow().name, tc.borrow().export);
            }
        }
    }

    fn execute_tests(&mut self) {
        for (_, module) in &self.modules_to_test {
            if !module.borrow().should_execute() {
                continue;
            }
            println!(" Module: {}",module.borrow().name);
            module.borrow().execute();
        }
    }


}

//
// helper
//

fn modules_from_dynlib(dynlibref: &DynLibraryRef) -> HashMap<String, ModuleRef> {
    let dynlib = dynlibref.borrow();
    let module_names: Vec<&str> = dynlib.exports
        .iter()
        .map(|e| e.split('_').nth(1).unwrap())
        .collect();

    let mut module_map: HashMap<String, ModuleRef> = HashMap::new();

    // I've struggled to turn this into a filter/map chain..  didn't get it to work...
    for m in module_names {
        if module_map.contains_key(m) {
            continue;
        }
        let module = Rc::new(RefCell::new(Module::new(m, dynlibref)));
        module.borrow_mut().find_test_cases(module.clone());
        module_map.insert(m.to_string(), module.clone());
    }

    return module_map;
}
