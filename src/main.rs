use sharedlib::{Lib, Func, Symbol};
use std::process::Command;
use std::{fs, io, env};
use std::path::Path;
use std::ffi::{c_int, c_void,c_char};
use std::convert::TryFrom;

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

        // No sure this will ever happen...
        if !output.status.success() {
            println!("Error while scanning library");
            return false;
        }
        let str = String::from_utf8(output.stdout).expect("error");
        // This can most likely be rewritten as a chain of map's and filters..
        let lines = str.split("\n");

        // transform each valid line containing a valid test function to our internal list of plausible exports
        self.exports = lines
            .filter(|s| s.split_whitespace().count() == 3)
            .filter_map(|s| DynLibrary::is_valid_testfunc(s).ok())
            .map(str::to_string)
            .collect();

        return true;
    } // scan

    fn is_valid_line(line : &str) -> Result<&str,()>{
        if line.split_whitespace().count() != 3 {
            return Err(());
        }
        Ok(line)
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


// Can most likely transform this...
pub const K_TR_PASS: u32 = 0;
pub const K_TR_FAIL: u32 = 16;
pub const K_TR_FAIL_MODULE: u32 = 32;
pub const K_TR_FAIL_ALL: u32 = 48;

enum TestResult {
    Pass = 0,
    Fail = 16,
    FailModule = 32,
    FailAll = 48,
}


impl TryFrom<c_int> for TestResult {
    type Error = ();
    fn try_from(v : c_int) -> Result<Self, Self::Error> {
        match v {
            x if x == TestResult::Pass as c_int => Ok(TestResult::Pass),
            x if x == TestResult::Fail as c_int => Ok(TestResult::Fail),
            x if x == TestResult::FailModule as c_int => Ok(TestResult::FailModule),
            x if x == TestResult::FailAll as c_int => Ok(TestResult::FailAll),
            _ => Err(())
        }
    }
}

pub type AssertErrorHandler = unsafe extern "C" fn(exp : *const c_char, file : *const c_char, line : c_int);
pub type LogHandler =  extern "C" fn (line : c_int, file: *const c_char, format: *const c_char, ...) -> c_void;
pub type CaseHandler = extern "C" fn(case: *mut TestRunnerInterface);
pub type DependsHandler = extern "C" fn(name : *const c_char, dep_list: *const c_char);
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct TestRunnerInterface {
    pub debug : Option<LogHandler>,
    pub info : Option<LogHandler>,
    pub warning : Option<LogHandler>,
    pub error: Option<LogHandler>,
    pub fatal : Option<LogHandler>,
    pub abort : Option<LogHandler>,

    pub assert_error : AssertErrorHandler,

    pub set_pre_case_callback : Option<CaseHandler>,
    pub set_post_case_callback : Option<CaseHandler>,

    pub case_depends : Option<DependsHandler>,
}
pub type TestableFunction = unsafe extern "C" fn(*mut TestRunnerInterface) -> c_int;

unsafe extern "C" fn assert_error_impl(exp : *const c_char, file : *const c_char, line : c_int) {
    println!("AssertError called");
}

impl TestRunnerInterface {
    pub fn new() -> TestRunnerInterface {
//        let ptr_assert_error = AssertError as *const ();
        let trun = TestRunnerInterface {
            debug: None,
            info: None,
            warning: None,
            error: None,
            fatal: None,
            abort: None,

            assert_error: assert_error_impl,

            set_pre_case_callback : None,
            set_post_case_callback : None,

            case_depends : None,

        };
        return trun;
    }
}

fn call_func(module : &Module, fname : &str) {
    unsafe {
        let mut trun_interface = TestRunnerInterface::new();



        let lib = Lib::new(&module.dynlib.name).expect("load lib");
        //let func_symbol: Func<extern "C" fn(*mut c_void) -> c_int> = lib.find_func("test_strutil_trim");
        let func_symbol : Func<TestableFunction> = lib.find_func(fname).expect("find_func");
        let func = func_symbol.get();
        let ptr_trun = &mut trun_interface; //std::ptr::addr_of!(trun_interface);
        println!("data");
        println!("  trun = {:#?}",ptr_trun);
        println!("  sz trun = {}",std::mem::size_of::<TestRunnerInterface>());
        println!("calling func {fname}");
        let res = func(ptr_trun);


        println!("res={res}");
        println!("done!");
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
        call_func(&module, "test_rust_fail");
    }
}
