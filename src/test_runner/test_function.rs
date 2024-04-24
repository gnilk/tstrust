use std::cell::RefCell;
use std::ffi::{c_char, c_int, c_void, CStr};
use std::ptr;
use std::rc::Rc;
use std::sync::Mutex;
use std::time::{Instant};
use libc::pthread_exit;
use libloading::Symbol;
use once_cell::sync::Lazy;
use crate::test_runner::*;

// Testable function
#[derive(Debug)]
pub enum TestScope {
    Global,
    Module,
}

#[derive(Debug)]
pub enum TestType {
    Main,
    Exit,
    Regular,
}

#[derive(Debug)]
enum State {
    Idle,
    Executing,
    Finished,
}
#[derive(Debug)]
pub struct TestFunction {
    pub case_name: String,
    pub module_name : String,
    pub symbol: String,

    pub test_scope : TestScope,
    pub test_type: TestType,

    state : State,
    pub dependencies : Vec<TestFunctionRef>,
    pub test_result: TestResult,
}
pub type TestFunctionRef = Rc<RefCell<TestFunction>>;

struct ThreadArg {
    symbol : String,
    dynlib : DynLibraryRef,
}

impl ThreadArg {
    pub fn new(dynlib : &DynLibraryRef) -> ThreadArg {
        Self {
            symbol : String::new(),
            dynlib : dynlib.clone(),
        }
    }
}

//
// The context is a global variable which is set fresh for each execution
// It contains the everything happening during a single test-function execution...
//
//pub static CONTEXT: RefCell<Context> = RefCell::new(Context::new());
pub static CONTEXT: Lazy<Mutex<Context>> = Lazy::new(|| {
    let ctx = Context::new();
    Mutex::new(ctx)
});


extern "C" fn set_pre_case_handler(case_handler: PrePostCaseHandler) {
    CONTEXT.lock().unwrap().pre_case_handler = Some(case_handler);
    //CONTEXT.with(|ctx| ctx.borrow_mut().pre_case_handler = Some(case_handler));
}
extern "C" fn set_post_case_handler(case_handler: PrePostCaseHandler) {
    CONTEXT.lock().unwrap().post_case_handler = Some(case_handler);
    //CONTEXT.with(|ctx| ctx.borrow_mut().post_case_handler = Some(case_handler));
}

extern "C" fn dependency_handler(name : *const c_char, dep_list: *const c_char) {
    // This needs access to a global variable!!

    let str_name = unsafe { CStr::from_ptr(name).to_str().expect("assert error impl, exp error") };
    let str_deplist = unsafe { CStr::from_ptr(dep_list).to_str().expect("assert error impl, file error") };

    CONTEXT.lock().unwrap().add_dependency(str_name, str_deplist);
}

extern "C" fn all_log_handlers(line : c_int, file: *const c_char, format: *const c_char) {
    let str_file = unsafe { CStr::from_ptr(file).to_str().expect("assert error impl, file error") };
    let str_msg = unsafe { CStr::from_ptr(format).to_str().expect("assert error impl, exp error") };

    println!("log: {}:{}:{}", str_file, line, str_msg);

}
// FIXME: when rust support c-variadic's
// Change to: unsafe extern "C" fn fatal_handler(line : c_int, file: *const c_char, format: *const c_char, ...) {
// see: https://github.com/rust-lang/rust/issues/44930
extern "C" fn fatal_handler(line : c_int, file: *const c_char, format: *const c_char) {
    let str_exp = unsafe { CStr::from_ptr(format).to_str().expect("assert error impl, exp error") };
    let str_file = unsafe { CStr::from_ptr(file).to_str().expect("assert error impl, file error") };

    let func_error = TestFuncError::new(TestFuncErrorClass::Fatal, str_file, line as u32, str_exp);
    func_error.print();
    CONTEXT.lock().unwrap().func_error = Some(func_error);

    unsafe {
        pthread_exit(ptr::null_mut());
    }
}
extern "C" fn error_handler(line : c_int, file: *const c_char, format: *const c_char) {
    let str_exp = unsafe { CStr::from_ptr(format).to_str().expect("assert error impl, exp error") };
    let str_file = unsafe { CStr::from_ptr(file).to_str().expect("assert error impl, file error") };

    let func_error = TestFuncError::new(TestFuncErrorClass::Error, str_file, line as u32, str_exp);
    func_error.print();
    CONTEXT.lock().unwrap().func_error = Some(func_error);

    unsafe {
        pthread_exit(ptr::null_mut());
    }
}

extern "C" fn abort_handler(line : c_int, file: *const c_char, format: *const c_char) {
    let str_exp = unsafe { CStr::from_ptr(format).to_str().expect("assert error impl, exp error") };
    let str_file = unsafe { CStr::from_ptr(file).to_str().expect("assert error impl, file error") };

    let func_error = TestFuncError::new(TestFuncErrorClass::Abort, str_file, line as u32, str_exp);
    func_error.print();
    CONTEXT.lock().unwrap().func_error = Some(func_error);

    unsafe {
        pthread_exit(ptr::null_mut());
    }
}


extern "C" fn assert_error_handler(exp : *const c_char, file : *const c_char, line : c_int) {

    let str_exp = unsafe { CStr::from_ptr(exp).to_str().expect("assert error impl, exp error") };
    let str_file = unsafe { CStr::from_ptr(file).to_str().expect("assert error impl, file error") };

    // NOTE: This is normally printed with the logger
    //println!("Assert Error: {}:{}\t'{}'", str_file, line, str_exp);

    let func_error = TestFuncError::new(TestFuncErrorClass::Error, str_file, line as u32, str_exp);
    func_error.print();
    CONTEXT.lock().unwrap().func_error = Some(func_error);

    unsafe {
        pthread_exit(ptr::null_mut());
    }

}

pub fn get_truninterface_ptr() -> TestRunnerInterface {
    let mut trun_interface = TestRunnerInterface::new();
    trun_interface.debug = Some(all_log_handlers);
    trun_interface.info = Some(all_log_handlers);
    trun_interface.warning = Some(all_log_handlers);

    trun_interface.error = Some(error_handler);
    trun_interface.fatal = Some(fatal_handler);
    trun_interface.abort = Some(abort_handler);
    trun_interface.case_depends = Some(dependency_handler);
    trun_interface.assert_error = Some(assert_error_handler);
    trun_interface.set_pre_case_callback = Some(set_pre_case_handler);
    trun_interface.set_post_case_callback = Some(set_post_case_handler);

    return trun_interface;
}

extern "C" fn pthread_execute_async(ptr_arg: *mut c_void) -> *mut c_void {
    let thread_arg : &mut ThreadArg = unsafe { &mut *(ptr_arg as *mut ThreadArg)};

    // do this on a two liner - otherwise the borrow checker will terminate at the end of the statement
    // and since we take out a function from the dynlib, we need that later..
    // ergo, first borrow the dynlib (allows borrow for dynlib to end of function)
    // then we take the test-function out of it..
    let dynlib = thread_arg.dynlib.as_ref().borrow();
    let func : Symbol<TestableFunction> = dynlib.get_testable_function(&thread_arg.symbol);

    // Fetch a callback interface instance, treat as a pointer and off we go...
    let mut trun_interface = get_truninterface_ptr(); //TestRunnerInterface::new();
    let raw_result = unsafe {
        func(&mut trun_interface)
    };

    // Set the raw result - if any...
    // note: in case of errors, the thread is terminated, the result handling will first check if we
    //       have any errors before checking the resulting test-code..
    let mut ctx = CONTEXT.lock().unwrap();
    ctx.raw_result = raw_result;

    return std::ptr::null_mut();
}

impl TestFunction {
    pub fn new(symbol : &str, module : &str, case : &str) -> TestFunctionRef {
        let mut test_scope = TestScope::Module;
        let mut test_type = TestType::Regular;
        let mut new_module_name = module.to_string();

        if (module == "-") && (case == Config::instance().main_func_name) {
            // global main is: 'test_main'
            test_type = TestType::Main;
            test_scope = TestScope::Global;
        } else if module == "-" && (case == Config::instance().exit_func_name) {
            // global exit is: 'test_exit'
            test_type = TestType::Exit;
            test_scope = TestScope::Global;
        } else {
            // scope is already 'module' so skip that...
            if module == "-" {
                // test_<module>  <- module main
                new_module_name = case.to_string();
                test_type = TestType::Main;
            } else if case == Config::instance().exit_func_name {
                // Module exit is: 'test_<module>_<exit>
                test_type = TestType::Exit;
            }
        }

        let test_function = TestFunction {
            case_name: case.to_string(),
            module_name: new_module_name,
            symbol : symbol.to_string(),
            test_scope,
            test_type,

            state : State::Idle,
            dependencies : Vec::new(),
            test_result : TestResult::new(),
        };
        return Rc::new(RefCell::new(test_function));
    }
    pub fn should_execute(&self) -> bool {
        let cfg = Config::instance();
        // already executed?
        match self.state {
            State::Finished => return false,
            State::Executing => return false,
            _ => (),
        }

        // Are we part of execution chain?
        if cfg.testcases.contains(&"-".to_string()) || cfg.testcases.contains(&self.case_name) {
            return true;
        }

        return false;
    }

    pub fn is_finished(&self) -> bool {
        match self.state {
            State::Finished => return true,
            _ => return false,
        }
    }
    pub fn is_idle(&self) -> bool {
        match self.state {
            State::Idle => return true,
            _ => return false,
        }
    }

    fn change_state(&mut self, new_state : State) {
        self.state = new_state;
    }

    pub fn is_global(&self) -> bool {
        return self.module_name == "-";
    }
    pub fn is_global_main(&self) -> bool {
        return self.is_global() && (self.case_name == Config::instance().main_func_name);
    }

    pub fn is_global_exit(&self) -> bool {
        return self.is_global() && (self.case_name == Config::instance().exit_func_name);
    }

    pub fn execute_no_module(&mut self, library : &DynLibraryRef) {
        match self.state {
            State::Idle => (),
            _ => return,
        }

        // just create a dummy module - everything is None here - doesn't cost much...
        let module = Module::new("dummy");
        self.execute(&module, library);
    }




    // FIXME: Should return result<>
    // Need to simplify this...
    pub fn execute(&mut self, module : &Module, library : &DynLibraryRef) {
        match self.state {
            State::Idle => (),
            _ => return,
        }

        self.change_state(State::Executing);
        self.execute_dependencies(module, library);

        // Spawn thread here, need to figure out what happens with the Context (since it is a thread-local) variable
        println!("=== RUN \t{}",self.symbol);

        // Start the timer - we do NOT include 'dependencies' in the timing - they are just a way of controlling execution
        let t_start = Instant::now();


        // Reset the context, this must be done before the pre/post cases are executed, they all use the context to communicate!
        let mut ctx = CONTEXT.lock().unwrap();
        ctx.reset();
        drop(ctx);


        // Note: We do this here - as we align to the existing C/C++ test runner
        //       otherwise we could simply run in the module it-self (which might have been more prudent)
        // Execute pre case handler - if any has been assigned
        if module.pre_case_func.is_some() {
            // FIXME: V2 have 'int' as return codes for the pre/post cases
            let mut trun_interface = get_truninterface_ptr(); //TestRunnerInterface::new();
            module.pre_case_func.as_ref().unwrap()(&mut trun_interface);
        }

        // Set up the thread argument..
        let mut thread_arg = ThreadArg::new(library);
        thread_arg.symbol = self.symbol.clone();

        // Spawn execution thread
        let mut mthread = PThread::<ThreadArg>::new(thread_arg);
        // FIXME: better error handling, this will just panic if something goes wrong...
        mthread.spawn(pthread_execute_async).ok();
        mthread.join().ok();

        // Execute post case handler - if any...
        if module.post_case_func.is_some() {
            let mut trun_interface = get_truninterface_ptr(); //TestRunnerInterface::new();
            module.post_case_func.as_ref().unwrap()(&mut trun_interface);
        }

        // Stop timer
        self.test_result.exec_duration = t_start.elapsed();

        // Create test result
        let mut ctx = CONTEXT.lock().unwrap();
        self.test_result.func_error = ctx.func_error.take();


        self.handle_test_return(ctx.raw_result);
        self.test_result.symbol = self.symbol.clone();

        self.test_result.print();
        self.change_state(State::Finished);
    }


    fn execute_dependencies(&mut self, module : &Module, dynlib : &DynLibraryRef) {
        for func in &self.dependencies {
            if func.try_borrow().is_err() {
                // circular dependency - we are probably already executing this - as we have a borrow on it while executing..
                continue;
            }
            if !func.borrow().is_idle() {
                continue;
            }
            func.borrow_mut().execute(module, dynlib)
        }
    }

    fn handle_test_return(&mut self, raw_result : c_int) {
        self.test_result.raw_return_code = raw_result;
        // Assert takes predence..
        if self.test_result.func_error.is_some() {
            self.test_result.return_code = Some(TestReturnCode::Fail);
        } else {
            self.test_result.return_code = TestReturnCode::try_from(raw_result).ok();
        }
    }
}
