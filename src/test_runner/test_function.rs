use std::cell::RefCell;
use std::ffi::{c_char, c_int, CStr};
use std::rc::Rc;
use std::time::{Duration, Instant};
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
    test_result: TestResult,
}
pub type TestFunctionRef = Rc<RefCell<TestFunction>>;

#[derive(Debug)]
pub struct TestResult {
    exec_duration: Duration,

    raw_return_code: c_int,     // The return code from the C/C++ function, use 'TestReturnCode::try_from' to transform and verify
    return_code: Result<TestReturnCode,()>, // raw c_int enum from ITestInterface converted to internal enum after execution

    assert_error: Option<AssertError>,        // Holds the assert error msg if any..
    num_error : u32,            // Note: Always one, if an error occurs - however, embedded or single-threading can have several
    num_assert : u32,           // Note: Always one, if an error occurs - however, embedded or single-threading can have several

    symbol : String,            // The actual exported symbol
}

//
// The context is a global variable which is set fresh for each execution
// It contains the everything happening during a single test-function execution...
//
thread_local! {
    pub static CONTEXT: RefCell<Context> = RefCell::new(Context::new());
}

extern "C" fn set_pre_case_handler(case_handler: PrePostCaseHandler) {
    println!("PRE_CASE_CALLED!");
    CONTEXT.with(|ctx| ctx.borrow_mut().pre_case_handler = Some(case_handler));
}
extern "C" fn set_post_case_handler(case_handler: PrePostCaseHandler) {
    println!("POST_CASE_CALLED!");
    CONTEXT.with(|ctx| ctx.borrow_mut().post_case_handler = Some(case_handler));
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


impl TestResult {
    pub fn new() -> TestResult {
        Self {
            return_code: Ok(TestReturnCode::NotExecuted),
            assert_error : None,
            exec_duration: Duration::new(0, 0),
            num_assert : 0,
            num_error : 0,
            symbol : String::default(),
            raw_return_code: 0,
        }
    }
    pub fn print(&self) {
        //
        // Asserts are not printed here - they are printed as they come up..
        //
        match self.return_code {
            Ok(TestReturnCode::Pass) => println!("=== PASS:\t{}, {} sec, {}", self.symbol, self.exec_duration.as_secs_f32(), self.raw_return_code),
            Ok(TestReturnCode::Fail) => println!("=== FAIL:\t{}, {} sec, {}", self.symbol, self.exec_duration.as_secs_f32(), self.raw_return_code),
            Ok(TestReturnCode::FailModule) => println!("=== FAIL:\t{}, {} sec, {}", self.symbol, self.exec_duration.as_secs_f32(), self.raw_return_code),
            Ok(TestReturnCode::FailAll) => println!("=== FAIL:\t{}, {} sec, {}", self.symbol, self.exec_duration.as_secs_f32(), self.raw_return_code),
            _ => println!("=== INVALID RETURN CODE ({}) for {}", self.raw_return_code, self.symbol),
        }
        // Empty line in the console output
        println!("");
    }
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

    pub fn execute_no_module(&mut self, dynlib : &DynLibrary) {
        match self.state {
            State::Idle => (),
            _ => return,
        }
        self.change_state(State::Executing);

        // Spawn thread here, need to figure out what happens with the Context (since it is a thread-local) variable
        println!("=== RUN \t{}",self.symbol);

        // Start the timer - do NOT include 'dependencies' in the timing - they are just a way of controlling execution
        let t_start = Instant::now();

        // Create the test runner interface...
        let mut trun_interface = self.get_truninterface_ptr(); //TestRunnerInterface::new();

        // And the context..
        CONTEXT.set(Context::new());

        // Look up the symbol and exeecute
        //let lib = dynlib.borrow();
        let func = dynlib.get_testable_function(&self.symbol);
        let raw_result = unsafe { func(&mut trun_interface) };

        // Stop timer
        self.test_result.exec_duration = t_start.elapsed();

        // Create test result, note: DO NOT take the Context here - we do this in the module later on!
        // mainly because some 'setters' are module-level setters while some getters are module level getters...
        CONTEXT.with_borrow_mut(|ctx| self.handle_result_from_ctx(ctx));
        self.handle_test_return(raw_result);
        self.test_result.symbol = self.symbol.clone();

        self.test_result.print();
        self.change_state(State::Finished);
    }

    pub fn get_truninterface_ptr(&self) -> TestRunnerInterface {
        let mut trun_interface = TestRunnerInterface::new();
        trun_interface.case_depends = Some(dependency_handler);
        trun_interface.assert_error = Some(assert_error_handler);
        trun_interface.set_pre_case_callback = Some(set_pre_case_handler);
        trun_interface.set_post_case_callback = Some(set_post_case_handler);

        return trun_interface;
    }

    // FIXME: Should return result<>
    pub fn execute(&mut self, module : &Module, dynlib : &DynLibrary) {
        match self.state {
            State::Idle => (),
            _ => return,
        }

        self.change_state(State::Executing);
        self.execute_dependencies(module, dynlib);

        // Spawn thread here, need to figure out what happens with the Context (since it is a thread-local) variable
        println!("=== RUN \t{}",self.symbol);

        // Start the timer - do NOT include 'dependencies' in the timing - they are just a way of controlling execution
        let t_start = Instant::now();

        // Create the test runner interface...

        let mut trun_interface = self.get_truninterface_ptr(); //TestRunnerInterface::new();

        // And the context..
        CONTEXT.set(Context::new());


        // Note: We do this here - as we align to the existing C/C++ test runner
        //       otherwise we could simply run in the module it-self (which might have been more prudent)
        // Execute pre case handler - if any has been assigned
        if module.pre_case_func.is_some() {
            module.pre_case_func.as_ref().unwrap()(&mut trun_interface);
        }

        // Look up the symbol and exeecute
        //let lib = dynlib.borrow();
        let func = dynlib.get_testable_function(&self.symbol);
        let raw_result = unsafe { func(&mut trun_interface) };

        // Execute post case handler - if any...
        if module.post_case_func.is_some() {
            module.post_case_func.as_ref().unwrap()(&mut trun_interface);
        }

        // Stop timer
        self.test_result.exec_duration = t_start.elapsed();

        // Create test result, note: DO NOT take the Context here - we do this in the module later on!
        // mainly because some 'setters' are module-level setters while some getters are module level getters...
        CONTEXT.with_borrow_mut(|ctx| self.handle_result_from_ctx(ctx));
        self.handle_test_return(raw_result);
        self.test_result.symbol = self.symbol.clone();

        self.test_result.print();

        self.change_state(State::Finished);
    }

    fn execute_dependencies(&mut self, module : &Module, dynlib : &DynLibrary) {
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

    fn handle_result_from_ctx(&mut self, context: &mut Context) {
        self.test_result.assert_error = context.assert_error.take();
    }
    fn handle_test_return(&mut self, raw_result : c_int) {
        self.test_result.raw_return_code = raw_result;
        // Assert takes predence..
        if self.test_result.assert_error.is_some() {
            self.test_result.return_code = Ok(TestReturnCode::Fail);
        } else {
            self.test_result.return_code = TestReturnCode::try_from(raw_result);
        }
    }
    fn print_result(&self) {
        self.test_result.print();
    }
}


