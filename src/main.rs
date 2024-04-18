pub mod test_interface;
pub mod dir_scanner;
pub mod dyn_library;

use libloading;
use std::{env};
use std::cell::{Ref, RefCell};

use std::rc::Rc;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::ffi::{c_char, CStr, CString};
use std::convert::TryFrom;
use std::mem::MaybeUninit;
use std::ops::DerefMut;
use std::ptr::null;
use std::string::ToString;
use std::sync::{Once, Mutex, Arc};
use clap::{Parser};

use crate::test_interface::{TestRunnerInterface, TestableFunction, TestResult};
use crate::dir_scanner::*;
use crate::dyn_library::*;


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


#[derive(Debug)]
struct Module {
    name : String,
    dynlib : Rc<DynLibrary>,
    main_func : Option<TestFunctionRef>,
    test_cases : Vec<TestFunctionRef>,
}

//pub type ModuleRef = Rc<RefCell<<Module>>;
pub type ModuleRef = Rc<RefCell<Module>>;

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

            if module.try_borrow_mut().is_err() {
                println!("module::execute, borrow active");
            }
            CURRENT_TEST_MODULE.set(Some(module.clone()));
            if module.try_borrow_mut().is_err() {
                println!("module::execute, borrow active");
            }

            module.borrow().execute();
        }
    }


}

//
// helper
//

fn modules_from_dynlib(dyn_library: &Rc<DynLibrary>) -> HashMap<String, ModuleRef> {
    let module_names: Vec<&str> = dyn_library.exports
        .iter()
        .map(|e| e.split('_').nth(1).unwrap())
        .collect();

    let mut module_map: HashMap<String, ModuleRef> = HashMap::new();

    // I've struggled to turn this into a filter/map chain..  didn't get it to work...
    for m in module_names {
        if module_map.contains_key(m) {
            continue;
        }
        let module = Rc::new(RefCell::new(Module::new(m, dyn_library)));
        module.borrow_mut().find_test_cases(module.clone());
        module_map.insert(m.to_string(), module.clone());
    }

    return module_map;
}

fn testcases_for_module(dynlib: &DynLibrary, module : &mut ModuleRef) {
    // println!("parsing testcase, module={}", self.name);
    for e in &dynlib.exports {
        let parts:Vec<&str> = e.split('_').collect();
        if parts.len() < 2 {
            panic!("Invalid export={} in dynlib={}",e,dynlib.name);
        }
        // Skip everything not belonging to us..
        if parts[1] != module.borrow().name {
            continue;
        }
        //Rc::get_mut(&mut module).unwrap().main_func = None;

        // special handling for 'test_<module>' => CaseType::ModuleMain
        if (parts.len() == 2) && (parts[1] == module.borrow().name) {
            // println!("  main, func={},  export={}", parts[1], e);
            //module.borrow_mut().main_func = Some(TestFunction::new(parts[1], CaseType::ModuleMain, e.to_string(), module));
            //self.test_cases.push(TestFunction::new(parts[1], CaseType::ModuleMain, e.to_string()));
        } else {
            // join the case name together again...
            let case_name = parts[2..].join("_");
            // println!("  case, func={},  export={}", case_name, e);
            //module.borrow_mut().test_cases.push(TestFunction::new(&case_name, CaseType::Regular, e.to_string(), module));
        }
    }

}




//
// Move to own module/file
//

impl Module {
    //pub fn new<'a>(name : &'a str, dyn_library: &'a DynLibrary) -> Module<'a> {
    pub fn new(name : &str, dyn_library: &Rc<DynLibrary>) -> Module {
        let module = Module {
            name : name.to_string(),
            dynlib : Rc::clone(dyn_library),
            main_func : None,
            test_cases : Vec::new(),
        };

        return module;
    }

    pub fn find_test_cases(&mut self, module : ModuleRef) {
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
                self.main_func = Some(TestFunction::new(parts[1], CaseType::ModuleMain, e.to_string(), module.clone()));
                //self.test_cases.push(TestFunction::new(parts[1], CaseType::ModuleMain, e.to_string()));
            } else {
                // join the case name together again...
                let case_name = parts[2..].join("_");
                // println!("  case, func={},  export={}", case_name, e);
                self.test_cases.push(TestFunction::new(&case_name, CaseType::Regular, e.to_string(), module.clone()));
            }
        }
    }


    pub fn should_execute(&self) -> bool {
        let cfg = Config::instance();
        if cfg.modules.contains(&"-".to_string()) || cfg.modules.contains(&self.name) {
            return true;
        }
        return false;
    }

    pub fn execute(&self) {
        // Execute main first, this will setup dependencies

        if self.main_func.is_some() {
            println!("Execute main!");

            let func = self.main_func.as_ref().unwrap();

            func.borrow().execute(&self.dynlib);
        }
        // FIXME: Execute dependencies

        println!("Execute test cases!");
        for tc in &self.test_cases {
            if tc.borrow().should_execute() {
                tc.borrow().execute(&self.dynlib)
            }
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


// Testable function
#[derive(Debug)]
enum CaseType {
    Main,
    Exit,
    ModuleMain,
    ModuleExit,
    Regular,
}
#[derive(Debug)]
struct TestFunction {
    name : String,
//    module : ModuleRef,
    case_type: CaseType,
    export : String,
    executed : bool,    // state?
    dependencies : Vec<String>,
}

pub type TestFunctionRef = Rc<RefCell<TestFunction>>;


thread_local! {
    //pub type ModuleRef = Rc<RefCell<Module>>;
    // so this final type would be:  RefCell<Option<Rc<RefCell<Module
    pub static CURRENT_TEST_MODULE: RefCell<Option<ModuleRef>> = RefCell::new(None);
    //pub static CURRENT_TEST_MODULE: *mut ModuleRef = null();
}
fn dep_handler(glb_opt : &Option<ModuleRef>, str_name : &str, str_deplist : &str) {
    let glb_module_ref = glb_opt.as_ref().unwrap();

    println!("Deps handler; func={}, depends={}", str_name, str_deplist);

    if glb_module_ref.try_borrow_mut().is_err() {
        println!("Borrow active!");
    }

    // I can't get this one to be a mutable object!!!
    let glb_module = glb_module_ref.borrow();


    println!("  Module => {}", glb_module.name);

}
extern "C" fn dependency_handler(name : *const c_char, dep_list: *const c_char) {
    // This needs access to a global variable!!

    let str_name = unsafe { CStr::from_ptr(name).to_str().expect("assert error impl, exp error") };
    let str_deplist = unsafe { CStr::from_ptr(dep_list).to_str().expect("assert error impl, file error") };

    //CURRENT_TEST_MODULE.with_borrow(|crt| dep_handler(crt, str_name, str_deplist));
    let mod_ref = CURRENT_TEST_MODULE.take().unwrap();
    {
        // Can't take this as 'mut' for some reason - I don't quite understand why...
        let module = mod_ref.borrow();
        println!("wef => {}", module.name);
    }
    // Set it again - needed??
    CURRENT_TEST_MODULE.set(Some(mod_ref));



}


impl TestFunction {
    pub fn new(name :&str, case_type: CaseType, export : String, module : ModuleRef) -> TestFunctionRef {
        let test_function = TestFunction {
            name : name.to_string(),
            //module : module,
            case_type : case_type,
            export : export.to_string(),
            executed : false,
            dependencies : Vec::new(),
        };
        return Rc::new(RefCell::new(test_function));
    }
    pub fn should_execute(&self) -> bool {
        let cfg = Config::instance();
        // already executed?
        if self.executed {
            return false;
        }

        // Are we part of execution chain?
        if cfg.testcases.contains(&"-".to_string()) || cfg.testcases.contains(&self.name) {
            return true;
        }

        return false;
    }
    pub fn execute(&self, dynlib : &DynLibrary) {


        let mut trun_interface = TestRunnerInterface::new();
        trun_interface.case_depends = Some(dependency_handler);

        //current_test_module.set("wef");

        //println!("dynlib is='{}'", dynlib.name);

        //CURRENT_TEST_MODULE = Some(self.module.clone());

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
// Just an experimental function to load and excute something from dylib
// bulk of this is now in 'dynlib'
/*
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

 */


pub trait Singleton {
    fn instance() -> &'static Self;
}

struct CurrentRunningTestCase {
    test_function: Option<Rc<TestFunction>>,
}

impl Singleton for CurrentRunningTestCase {
    fn instance() -> &'static Self {
        static mut GLB_CRT_SINGLETON: MaybeUninit<CurrentRunningTestCase> = MaybeUninit::uninit();
        static ONCE: Once = Once::new();
        unsafe {
            ONCE.call_once(|| {
                let singleton = CurrentRunningTestCase {
                    test_function : None,
                };
                GLB_CRT_SINGLETON.write(singleton);
            });
            GLB_CRT_SINGLETON.assume_init_ref()
        }
    }
}
impl CurrentRunningTestCase {
    fn set_current_test(&mut self, test_function: &Rc<TestFunction>) {
        self.test_function = Some(Rc::clone(test_function));
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


