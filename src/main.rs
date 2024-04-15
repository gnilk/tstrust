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
pub const kTR_Pass: u32 = 0;
pub const kTR_Fail: u32 = 16;
pub const kTR_FailModule: u32 = 32;
pub const kTR_FailAll: u32 = 48;

enum kResultCode {
    kTR_Pass = 0,
    kTR_Fail = 16,
    kTR_FailModule = 32,
    kTR_FailAll = 48,
}


impl TryFrom<c_int> for kResultCode {
    type Error = ();
    fn try_from(v : c_int) -> Result<Self, Self::Error> {
        match v {
            x if x == kResultCode::kTR_Pass as c_int => Ok(kResultCode::kTR_Pass),
            x if x == kResultCode::kTR_Fail as c_int => Ok(kResultCode::kTR_Fail),
            x if x == kResultCode::kTR_FailModule as c_int => Ok(kResultCode::kTR_FailModule),
            x if x == kResultCode::kTR_FailAll as c_int => Ok(kResultCode::kTR_FailAll),
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
    pub Debug : Option<LogHandler>,
    pub Info : Option<LogHandler>,
    pub Warning : Option<LogHandler>,
    pub Error: Option<LogHandler>,
    pub Fatal : Option<LogHandler>,
    pub Abort : Option<LogHandler>,

    pub AssertError : AssertErrorHandler,

    pub SetPreCaseCallback : Option<CaseHandler>,
    pub SetPostCaseCallback : Option<CaseHandler>,

    pub CaseDepends : Option<DependsHandler>,
}
pub type TestableFunction = unsafe extern "C" fn(*mut TestRunnerInterface) -> c_int;

unsafe extern "C" fn AssertErrorImpl(exp : *const c_char, file : *const c_char, line : c_int) {
    println!("AssertError called");
}

impl TestRunnerInterface {
    pub fn new() -> TestRunnerInterface {
//        let ptr_assert_error = AssertError as *const ();
        let mut trun = TestRunnerInterface {
            Debug: None,
            Info: None,
            Warning: None,
            Error: None,
            Fatal: None,
            Abort: None,

            AssertError: AssertErrorImpl,

            SetPreCaseCallback : None,
            SetPostCaseCallback : None,

            CaseDepends : None,

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
