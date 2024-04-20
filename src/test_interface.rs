use std::ffi::{c_char, c_int, c_void, CStr};
use std::time::Duration;

// Can most likely transform this...
pub const K_TR_PASS: u32 = 0;
pub const K_TR_FAIL: u32 = 16;
pub const K_TR_FAIL_MODULE: u32 = 32;
pub const K_TR_FAIL_ALL: u32 = 48;

#[derive(Debug)]
pub enum TestResultClass {
    Pass = 0,
    Fail = 16,
    FailModule = 32,
    FailAll = 48,
    NotExecuted = 64,
    InvalidReturnCode = 65,
}







impl TryFrom<c_int> for TestResultClass {
    type Error = ();
    fn try_from(v : c_int) -> Result<Self, Self::Error> {
        match v {
            x if x == TestResultClass::Pass as c_int => Ok(TestResultClass::Pass),
            x if x == TestResultClass::Fail as c_int => Ok(TestResultClass::Fail),
            x if x == TestResultClass::FailModule as c_int => Ok(TestResultClass::FailModule),
            x if x == TestResultClass::FailAll as c_int => Ok(TestResultClass::FailAll),
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

    pub assert_error : Option<AssertErrorHandler>,

    pub set_pre_case_callback : Option<CaseHandler>,
    pub set_post_case_callback : Option<CaseHandler>,

    pub case_depends : Option<DependsHandler>,
}
pub type TestableFunction = unsafe extern "C" fn(*mut TestRunnerInterface) -> c_int;
pub type PrePostTestcaseFunction = unsafe extern "C" fn(*mut TestRunnerInterface) -> c_void;



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

            assert_error: None,

            set_pre_case_callback : None,
            set_post_case_callback : None,

            case_depends : None,

        };
        return trun;
    }
}
