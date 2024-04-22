use std::cell::RefCell;
use std::ffi::{c_char, c_int, CStr};
use std::rc::Rc;
use std::time::{Duration, Instant};
use crate::test_runner::*;

// Testable function
#[derive(Debug)]
pub enum CaseType {
    Main,
    Exit,
    ModuleMain,
    ModuleExit,
    Regular,
}
#[derive(Debug)]
pub struct TestFunction {
    pub name : String,
    pub export : String,
    case_type: CaseType,
    executed : bool,    // state?
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

    pub fn set_executed(&mut self) {
        self.executed = true;
    }

    pub fn is_executed(&self) -> bool {
        return self.executed;
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

        self.test_result.exec_duration = duration;

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


