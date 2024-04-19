use std::ffi::{c_char, c_int, c_void, CStr};

// Can most likely transform this...
pub const K_TR_PASS: u32 = 0;
pub const K_TR_FAIL: u32 = 16;
pub const K_TR_FAIL_MODULE: u32 = 32;
pub const K_TR_FAIL_ALL: u32 = 48;

pub enum TestResult {
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

pub type AssertErrorHandler = extern "C" fn(exp : *const c_char, file : *const c_char, line : c_int);
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
pub type PrePostTestcaseFunction = unsafe extern "C" fn(*mut TestRunnerInterface) -> c_void;

extern "C" fn assert_error_impl(exp : *const c_char, file : *const c_char, line : c_int) {

    let str_exp = unsafe { CStr::from_ptr(exp).to_str().expect("assert error impl, exp error") };
    let str_file = unsafe { CStr::from_ptr(file).to_str().expect("assert error impl, file error") };

    // NOTE: This is normally printed with the logger
    println!("Assert Error: {}:{}\t'{}'", str_file, line, str_exp);
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
