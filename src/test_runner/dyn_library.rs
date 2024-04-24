use std::cell::RefCell;
use std::ffi::CString;
use std::process::Command;
use std::rc::Rc;
use libloading::{Symbol};
use crate::test_runner::TestableFunction;

#[derive(Debug)]
pub struct DynLibrary {
    pub name : String,
    pub exports : Vec<String>,
    pub library : libloading::Library,
}

pub type DynLibraryRef = Rc<RefCell<DynLibrary>>;


impl DynLibrary {
    // Should return result...
    pub fn new(dynlib_name : &str) -> DynLibrary {

        let exports = Self::prescan(dynlib_name);
        let name = dynlib_name.to_string();
        let library =  Self::cache_library(dynlib_name);

        return Self {name, exports, library};
    }

    pub fn get_testable_function(&self, symbol : &str) -> Symbol<TestableFunction> {
        let str_export = CString::new(symbol).unwrap();
        let func : libloading::Symbol<TestableFunction> = unsafe { self.library.get(str_export.as_bytes_with_nul()).expect("find_func") };
        return func;
    }

    fn cache_library(name : &str) -> libloading::Library {
        let lib = unsafe { libloading::Library::new(name).expect("load lib") } ;
        return lib;
    }
    fn prescan(name : &str) -> Vec<String> {
        let output = Command::new("nm")
            .arg(name)
            .output()
            .expect("failed to execute");

        // No sure this will ever happen...
        if !output.status.success() {
            println!("Error while scanning library");
            return Vec::new();  // FIXME!
        }
        let str = String::from_utf8(output.stdout).expect("error");
        // This can most likely be rewritten as a chain of map's and filters..
        let lines = str.split("\n");

        // transform each valid line containing a valid test function to our internal list of plausible exports
        let exports: Vec<String> = lines
            .filter(|s| s.split_whitespace().count() == 3)
            .filter_map(|s| DynLibrary::is_valid_testfunc(s).ok())
            .map(str::to_string)
            .collect();

        // Did we get exports???
        return exports;
    }

    fn is_valid_testfunc(line : &str) -> Result<&str, ()> {

        let parts : Vec<&str> = line.split_whitespace().collect();
        if parts[1] != "T" {
            return Err(())
        }
        if !parts[2].starts_with("test_") {
            return Err(())
        }
        Ok(parts[2])
    }

}   // impl...
