pub mod test_interface;
pub mod dir_scanner;
pub mod dyn_library;

use libloading;
use std::{env};
use std::cell::{Ref, RefCell};

use std::rc::Rc;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::ffi::{c_char, c_int, CStr, CString};
use std::convert::TryFrom;
use std::mem::MaybeUninit;
use std::ops::{Deref, DerefMut};
use std::ptr::null;
use std::string::ToString;
use std::sync::{Once, Mutex, Arc};
use std::time::{Duration, Instant};
use clap::{Parser};

use crate::test_interface::{TestRunnerInterface, TestableFunction, TestResultClass, PrePostTestcaseFunction};
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
    dynlib : DynLibraryRef,
    main_func : Option<TestFunctionRef>,
    pre_case_func : Option<PrePostTestcaseFunction>,
    post_case_func : Option<PrePostTestcaseFunction>,
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
    pub fn new(name : &str, dyn_library: &DynLibraryRef) -> Module {
        let module = Module {
            name : name.to_string(),
            dynlib : dyn_library.clone(),
            main_func : None,
            post_case_func : None,
            pre_case_func : None,
            test_cases : Vec::new(),
        };

        return module;
    }

    pub fn find_test_cases(&mut self, module : ModuleRef) {
        // println!("parsing testcase, module={}", self.name);
        for e in &self.dynlib.borrow().exports {
            let parts:Vec<&str> = e.split('_').collect();
            if parts.len() < 2 {
                panic!("Invalid export={} in dynlib={}",e,self.dynlib.borrow().name);
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
        // Execute main first, main can define various dependens plus pre/post functions
        self.execute_main();

        // Execute actual test cases
        for tc in &self.test_cases {
            if !tc.borrow().should_execute() {
                continue;
            }
            self.execute_test(tc);
        }
    }
    fn execute_test(&self, tc : &TestFunctionRef) {

        // FIXME: Need to protect against recursiveness here!
        self.execute_dependencies(tc);
        // FIXME: Execute dependencies, should now be in the 'tc.dependencies'
        //        note: from this point - 'depends' is an illegal callback - we should probably verify this
        tc.borrow_mut().execute(&self.dynlib);

        // We set this to true before actual execution to avoid recursive..
        tc.borrow_mut().executed = true;

    }

    fn execute_main(&self) {
        if !self.main_func.is_some() {
            return;
        }

//        println!("Execute main!");
        let func = self.main_func.as_ref().unwrap();
        func.borrow_mut().execute(&self.dynlib);
        func.borrow_mut().executed = true;

        // Grab hold of the context and verify test-cases...
        let ctx = CONTEXT.take();
        if ctx.dependencies.is_empty() {
//            println!("No dependencies");
            return;
        }
//        println!("Dependencies");
//        ctx.dump();
        // FIXME: Look up the correct test-case (use case names) and then append the dependency list to the test-case

        for casedep in &ctx.dependencies {
            if let Some(tc) = self.get_test_case(casedep.case.as_str()).ok() {
                for dep in &casedep.dependencies {
                    if let Some(tc_dep) = self.get_test_case(dep).ok() {
                        tc.borrow_mut().dependencies.push(tc_dep.clone());
                    }
                }
            }
        }
    }

    fn execute_dependencies(&self, test_function: &TestFunctionRef) {
        for func in &test_function.borrow().dependencies {
            if func.borrow().executed {
                continue;
            }
            self.execute_test(func);
        }
    }

    fn get_test_case(&self, case : &str) -> Result<&TestFunctionRef, ()> {
        for tc in &self.test_cases {
            if tc.borrow().name == case {
                return Ok(tc);
            }
        }
        return Err(());
    }

    pub fn dump(&self) {
        // Smarter way to filter??
        let lib = self.dynlib.borrow();
        let dummy : Vec<&String> = lib.exports.iter().filter(|x| x.contains("casefilter")).collect();

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
    case_type: CaseType,
    export : String,
    executed : bool,    // state?
    dependencies : Vec<TestFunctionRef>,
    test_result: TestResult,
}
pub type TestFunctionRef = Rc<RefCell<TestFunction>>;

#[derive(Debug)]
pub struct TestResult {
    result_class : Result<TestResultClass,()>, // raw c_int enum from ITestInterface converted to internal enum after execution
    assert_error: Option<AssertError>,
    raw_result : c_int,
    t_elapsed : Duration,
    num_error : u32,
    num_assert : u32,
    symbol : String,
}
#[derive(Debug)]
pub enum AssertClass {
    Error,
    Abort,
    Fatal,
}
#[derive(Debug)]
pub struct AssertError {
    assert_class: AssertClass,
    file : String,
    line : u32,
    message : String,
}
impl AssertError {
    pub fn new(file : &str, line : u32, message : &str) -> AssertError {
        Self {
            assert_class : AssertClass::Error,
            file : file.to_string(),
            line : line,
            message : message.to_string(),
        }
    }
    pub fn print(&self) {
        // Ensure equal spacing with the logger from original test-runner
        print!("                                                                                     ");
        // Now print the error code..
        println!("Assert Error: {}:{}\t'{}'", self.file, self.line, self.message);
    }
}

impl TestResult {
    pub fn new() -> TestResult {
        Self {
            result_class : Ok(TestResultClass::NotExecuted),
            assert_error : None,
            t_elapsed : Duration::new(0,0),
            num_assert : 0,
            num_error : 0,
            symbol : String::default(),
            raw_result : 0,
        }
    }
    pub fn print(&self) {
        //
        // Asserts are not printed here - they are printed as they come up..
        //
        match self.result_class {
            Ok(TestResultClass::Pass) => println!("=== PASS:\t{}, {} sec, {}", self.symbol, self.t_elapsed.as_secs_f32(), self.raw_result),
            Ok(TestResultClass::Fail) => println!("=== FAIL:\t{}, {} sec, {}", self.symbol, self.t_elapsed.as_secs_f32(), self.raw_result),
            Ok(TestResultClass::FailModule) => println!("=== FAIL:\t{}, {} sec, {}", self.symbol, self.t_elapsed.as_secs_f32(), self.raw_result),
            Ok(TestResultClass::FailAll) => println!("=== FAIL:\t{}, {} sec, {}", self.symbol, self.t_elapsed.as_secs_f32(), self.raw_result),
            _ => println!("=== INVALID RETURN CODE ({}) for {}", self.raw_result,self.symbol),
        }
        // Empty line in the console output
        println!("");
    }
}

//
// The context is a global variable which is set fresh for each execution
// It contains the everything happening during a single test-execution function...
//
struct Context {
    dependencies : Vec<CaseDependency>,
    assert_error : Option<AssertError>,
}
struct CaseDependency {
    case : String,
    dependencies : Vec<String>,
}

impl Default for Context {
    fn default() -> Self {
        Self {
            dependencies : Vec::new(),
            assert_error : None,
        }
    }
}
impl Context {
    pub fn new() -> Context {
        Self {
            dependencies : Vec::new(),
            assert_error : None,
        }
    }
    pub fn add_dependency(&mut self, case: &str, deplist: &str) {
        let parts: Vec<_> = deplist.split(",").collect();
        let mut case_dep = CaseDependency {
            case : case.to_string(),
            dependencies : Vec::new(),
        };

        for p in &parts {

            case_dep.dependencies.push(p.trim().to_string());
        }
        self.dependencies.push(case_dep);

    }
    pub fn set_assert_error(&mut self, assert_class: AssertClass, line : u32, file : &str, message : &str) {
        let assert_error = AssertError {
          assert_class : assert_class,
            line : line,
            file : file.to_string(),
            message : message.to_string(),
        };
        self.assert_error = Some(assert_error);
    }

    pub fn dump(&self) {
        println!("Context, dependencies");
        for dep in &self.dependencies {
            println!("  test case: {}", dep.case);
            for case in &dep.dependencies {
                println!("    {}", case);
            }
        }
    }
}


thread_local! {
    pub static CONTEXT: RefCell<Context> = RefCell::new(Context::new());
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

    CONTEXT.with(|ctx| ctx.borrow_mut().add_dependency(str_name, str_deplist));
}

extern "C" fn assert_error_handler(exp : *const c_char, file : *const c_char, line : c_int) {

    let str_exp = unsafe { CStr::from_ptr(exp).to_str().expect("assert error impl, exp error") };
    let str_file = unsafe { CStr::from_ptr(file).to_str().expect("assert error impl, file error") };

    // NOTE: This is normally printed with the logger
    //println!("Assert Error: {}:{}\t'{}'", str_file, line, str_exp);

    let assert_error = AssertError::new(str_file, line as u32, str_exp);
    assert_error.print();
    CONTEXT.with_borrow_mut(|ctx| ctx.assert_error = Some(assert_error));
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
            test_result : TestResult::new(),
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

    // FIXME: Should return result<>
    pub fn execute(&mut self, dynlib : &DynLibraryRef) {

        if self.executed {
            return;
        }

        // Spawn thread here, need to figure out what happens with the Context (since it is a thread-local) variable

        let mut trun_interface = TestRunnerInterface::new();
        trun_interface.case_depends = Some(dependency_handler);
        trun_interface.assert_error = Some(assert_error_handler);
        CONTEXT.set(Context::new());

        let lib = dynlib.borrow();
        let func = lib.get_testable_function(&self.export);

        let ptr_trun = &mut trun_interface; //std::ptr::addr_of!(trun_interface);


        println!("=== RUN \t{}",self.export);
        let t_start = Instant::now();
        let raw_result = unsafe { func(ptr_trun) };
        let duration = t_start.elapsed();

        // Create test result

        self.test_result.t_elapsed = duration;

        CONTEXT.with_borrow_mut(|ctx| self.handle_result_from_ctx(ctx));
        //self.test_result.result_class = Ok(TestResultClass::Pass);
        self.handle_test_return(raw_result);
        self.test_result.symbol = self.export.clone();

        //
        self.test_result.print();
    }
    fn handle_result_from_ctx(&mut self, context: &mut Context) {
        self.test_result.assert_error = context.assert_error.take();
    }
    fn handle_test_return(&mut self, raw_result : c_int) {
        self.test_result.raw_result = raw_result;
        // Assert takes predence..
        if (self.test_result.assert_error.is_some()) {
            self.test_result.result_class = Ok(TestResultClass::Fail);
        } else {
            self.test_result.result_class = TestResultClass::try_from(raw_result);
        }
    }
    fn print_result(&self) {
        self.test_result.print();
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


