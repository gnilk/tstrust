pub mod test_interface;
pub mod dir_scanner;
pub mod dyn_library;

//use sharedlib::{Lib, Func, Symbol};
use libloading;
use std::process::Command;
use std::{fs, io, env};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::ffi::{c_int, c_void,c_char, CStr, CString};
use std::convert::TryFrom;
use std::iter::Map;
use std::mem::MaybeUninit;
use std::ops::Deref;
use std::string::ToString;
use std::sync::{Mutex, Once};
use log::info;
use clap::{Parser, Subcommand};

use crate::test_interface::{TestRunnerInterface, TestableFunction, TestResult};
use crate::dir_scanner::*;
use crate::dyn_library::*;


//
// well - main...
//
fn main() {
    let cfg = Config::instance();

    // This is populated during the 'scan' sequence and used later...
    // There might be more for the context of one 'execution' round - consider creating a Context object..

    let mut app = App::new();
    app.scan_libraries(&cfg.inputs);

    // for input in &cfg.inputs {
    //     execute_tests_for(&input);
    // }

/*
    let cdir = env::current_dir().expect("wef");
    let mut dir_scanner = DirScanner::new();
    dir_scanner.scan(cdir.as_path()).expect("wef");
    for lib in dir_scanner.libraries {
        println!("{}", lib);

        let dynlib = DynLibrary::new(&lib);
        // if !dynlib.scan() {
        //     println!("Scan failed on {}", lib);
        //     break;
        // }
        let modules = modules_from_dynlib(&dynlib);
        for (_, module) in modules.into_iter() {
            //
            if cfg.modules.contains(&("-").to_string()) || cfg.modules.contains(&module.name) {
                module.execute();
            }
        }
    }

 */
}
struct App<'a> {
    modules_to_test : HashMap<String, Module<'a>>,
}
impl App<'_> {

    pub fn new<'a>() -> App<'a> {
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
        let library = DynLibrary::new(filename);
        let modules = modules_from_dynlib(&library);
        for (name, module) in modules.into_iter() {
            // I don't like the fact that this must be unsafe.. really need to consider putting this in a context object..

            if !self.modules_to_test.contains_key(&name) {
                // Don't want clone here...
                self.modules_to_test.insert(name, module);
            }
        }
    }

    fn list_tests(&self) {}


    //
// Executes tests for libraries and/or modules
//
    fn execute_tests_for(&self, input: &str) {
        if input == "." {
            let cdir = env::current_dir().unwrap();
            self.execute_tests_in_directory(&cdir);
        } else {
            let path = Path::new(input);
            match path {
                x if x.is_dir() => self.execute_tests_in_directory(&x.to_path_buf()),
                x if x.is_file() => self.execute_tests_in_lib(x.to_str().unwrap()),
                _ => println!("ERR: Unsupported file type")
            }
        }
    }

    fn execute_tests_in_directory(&self, dirname: &PathBuf) {
        let mut dir_scanner = DirScanner::new();
        dir_scanner.scan(dirname.as_path()).expect("wef");
        for library in &dir_scanner.libraries {
            self.execute_tests_in_lib(library)
        }
    }

    fn execute_tests_in_lib(&self, filename: &str) {
        let library = DynLibrary::new(filename);
        let modules = modules_from_dynlib(&library);

        let cfg = Config::instance();
        for (_, module) in modules.into_iter() {
            //
            if cfg.modules.contains(&("-").to_string()) || cfg.modules.contains(&module.name) {
                module.execute();
            }
        }
    }

    //
// Returns a hashmap of unique modules found in a shared library
// a testable module is defined through 'test_<module>_<case>'
//
}


fn modules_from_dynlib(dyn_library: &DynLibrary) -> HashMap<String, Module> {
    let module_names: Vec<&str> = dyn_library.exports
        .iter()
        .map(|e| e.split('_').nth(1).unwrap())
        .collect();

    let mut module_map: HashMap<String, Module> = HashMap::new();

    // I've struggled to turn this into a filter/map chain..  didn't get it to work...
    for m in module_names {
        if module_map.contains_key(m) {
            continue;
        }
        let mut module = Module::new(m, dyn_library);
        module.find_test_cases();
        module_map.insert(m.to_string(), module);
    }

    return module_map;
}



// FIXME: Move these to own 'module'

// Testable function
enum CaseType {
    Main,
    Exit,
    ModuleMain,
    ModuleExit,
    Regular,
}
struct TestFunction {
    name : String,
    case_type: CaseType,
    export : String,
}

impl TestFunction {
    pub fn new(name :&str, case_type: CaseType, export : String) -> TestFunction {
        let test_function = TestFunction {
            name : name.to_string(),
            case_type : case_type,
            export : export.to_string(),
        };
        return test_function;
    }
    pub fn execute(&self, dynlib : &DynLibrary) {

        let mut trun_interface = TestRunnerInterface::new();

        //println!("dynlib is='{}'", dynlib.name);

        let func = dynlib.get_testable_function(&self.export);

        let ptr_trun = &mut trun_interface; //std::ptr::addr_of!(trun_interface);
        println!("=== RUN \t{}",self.export);
        let raw_result = unsafe { func(ptr_trun) };
        let test_result = TestResult::try_from(raw_result); //.unwrap();
        match test_result {
            Ok(TestResult::Pass) => println!("=== PASS:\t{}, 0.00 sec, 0",self.export),
            Ok(TestResult::Fail) => println!("=== FAIL:\t{}, 0.00 sec, 0",self.export),
            Ok(TestResult::FailModule) => println!("=== FAIL:\t{}, 0.00 sec, 0",self.export),
            Ok(TestResult::FailAll) => println!("=== FAIL:\t{}, 0.00 sec, 0",self.export),
            _ => println!("=== INVALID RETURN CODE ({}) for {}", raw_result,self.export),
        }
    }
}

//
// Move to own module/file
//
struct Module<'a> {
    name : String,
    dynlib : &'a DynLibrary,
    test_cases : Vec<TestFunction>,
}

impl Module<'_> {
    pub fn new<'a>(name : &'a str, dyn_library: &'a DynLibrary) -> Module<'a> {
        let mut module = Module {
            name : name.to_string(),
            dynlib : dyn_library,
            test_cases : Vec::new(),
        };

        return module;
    }

    pub fn find_test_cases(&mut self) {
        // println!("parsing testcase, module={}", self.name);
        for e in &self.dynlib.exports {
            let parts:Vec<&str> = e.split('_').collect();
            if parts.len() < 2 {
                panic!("Invalid export={} in dynlib={}",e,self.dynlib.name);
            }
            // Skip everything not belonging to us..
            if parts[1] != self.name {
                continue;
            }

            // special handling for 'test_<module>' => CaseType::ModuleMain
            if (parts.len() == 2) && (parts[1] == self.name) {
                // println!("  main, func={},  export={}", parts[1], e);
                self.test_cases.push(TestFunction::new(parts[1], CaseType::ModuleMain, e.to_string()));
            } else {
                // join the case name together again...
                let case_name = parts[2..].join("_");
                // println!("  case, func={},  export={}", case_name, e);
                self.test_cases.push(TestFunction::new(&case_name, CaseType::ModuleMain, e.to_string()));
            }
        }
    }

    pub fn execute(&self) {
        // FIXME: Execute main first...

        for tc in &self.test_cases {
            tc.execute(self.dynlib)
        }
    }

    pub fn dump(&self) {
        // Smarter way to filter??
        let dummy : Vec<&String> = self.dynlib.exports.iter().filter(|x| x.contains("casefilter")).collect();

        for d in dummy {
            println!("{}", d);
        }
    }
}


//
// Just an experimental function to load and excute something from dylib
// bulk of this is now in 'dynlib'
fn call_func(module : &Module, fname : &str) {
        let mut trun_interface = TestRunnerInterface::new();

        let lib = unsafe { libloading::Library::new(&module.dynlib.name).expect("load lib") } ;
        //let func_symbol: Func<extern "C" fn(*mut c_void) -> c_int> = lib.find_func("test_strutil_trim");

        let str_export = CString::new(fname).unwrap();
        let func : libloading::Symbol<TestableFunction> = unsafe { lib.get(str_export.as_bytes_with_nul()).expect("find_func") };
        let ptr_trun = &mut trun_interface; //std::ptr::addr_of!(trun_interface);
        println!("data");
        println!("  trun = {:#?}",ptr_trun);
        println!("  sz trun = {}",std::mem::size_of::<TestRunnerInterface>());
        println!("calling func {fname}");
        let res = unsafe { func(ptr_trun) };

        println!("res={res}");
        println!("done!");

}

struct Context<'a> {
    modules_to_test : Mutex<HashMap<String, Module<'a>>>,
}
pub trait Singleton {
    fn instance() -> &'static Self;
}

impl Context<'_> {
    fn instance() -> &'static Self {
        static mut GLB_CONTEXT_SINGLETON: MaybeUninit<Context<'_>> = MaybeUninit::uninit();
        static ONCE: Once = Once::new();
        unsafe {
            ONCE.call_once(|| {
                let context = Context {
                    modules_to_test: Mutex::new(HashMap::new()),
                };
                GLB_CONTEXT_SINGLETON.write(context);
            });
            GLB_CONTEXT_SINGLETON.assume_init_ref()
        }
    }

    fn merge_modules(&self, modules : HashMap<String, Module>) {
        let cfg = Config::instance();
        for (name, module) in modules.into_iter() {
            // I don't like the fact that this must be unsafe.. really need to consider putting this in a context object..

            let mut data = self.modules_to_test.lock().unwrap();
            if !data.contains_key(&name) {
                // Don't want clone here...
                data.insert(name, module);
            }
        }

    }
}

impl Singleton for Config {
    fn instance() -> &'static Self {
        static mut GLB_CONFIG_SINGLETON: MaybeUninit<Config> = MaybeUninit::uninit();
        static ONCE: Once = Once::new();
        unsafe {
            ONCE.call_once(|| {
                let singleton = Config::parse();
                GLB_CONFIG_SINGLETON.write(singleton);
            });
            GLB_CONFIG_SINGLETON.assume_init_ref()
        }
    }
}


#[derive(clap::Parser, Debug)]
#[command(name = "tstrust")]
#[command(version = "0.0.1")]
#[command(about = "C/C++ Test Runner in Rust", long_about = None)]
struct Config {
    /// Verbose, specify multiple times to increase
    #[arg(short='v', default_value_t = 0, action = clap::ArgAction::Count)]
    verbose : u8,

    /// Specify test modules
    #[arg(short='m', value_parser, value_delimiter= ',', default_values=["-"].to_vec())]
    modules : Vec<String>,

    /// Specify test cases
    #[arg(short='t', value_parser, value_delimiter= ',', default_values=["-"].to_vec())]
    testcases : Vec<String>,

    /// Specify global main function name
    #[arg(long, default_value_t = ("main").to_string())]
    main_func_name : String,

    /// Specify global exit function name
    #[arg(long, default_value_t = ("exit").to_string())]
    exit_func_name : String,

    /// Specify reporting module
    #[arg(short='R', default_value_t = ("console").to_string())]
    reporting_module : String,


    /// Specify reporting output file, default is stdout
    #[arg(short='O', default_value_t = ("-").to_string())]
    reporting_file : String,

    /// Specify reporting indent
    #[arg(long, default_value_t = 8)]
    report_indent : i32,

    /// Execute tests
    #[arg(short='x', default_value_t = true)]
    execute_tests : bool,

    /// List available tests
    #[arg(short='l', default_value_t = false)]
    list_tests : bool,

    /// Print test-passes in summary
    #[arg(short='S', default_value_t = false)]
    print_pass_summary : bool,

    /// Execute module globals when executing
    #[arg(short='g', default_value_t = true)]
    test_module_globals : bool,

    /// Execute globals when executing
    #[arg(short='G', default_value_t = true)]
    test_global_main : bool,

    // /// Filter logs from tests
    // #[arg(short, default_value_t = false)]
    // test_log_filter : bool,

    /// Skip module on result FailModule from case
    #[arg(short='c', default_value_t = true)]
    skip_on_module_fail : bool,

    /// Skip all on result AllFail from case
    #[arg(short='C', default_value_t = true)]
    stop_on_all_fail : bool,


    /// Suppress progress messages
    #[arg(short='s', default_value_t = false)]
    suppress_progress : bool,

    /// Discard test result code handling
    #[arg(short='r', default_value_t = false)]
    discard_test_return_code : bool,

    /// files/directories to scan for tests
    #[arg(default_values = ["."].to_vec())]
    inputs : Vec<String>,

}


