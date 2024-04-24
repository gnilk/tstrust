use std::ffi::{c_char, c_int, c_void};

// Can most likely transform this...
pub const K_TR_PASS: u32 = 0;
pub const K_TR_FAIL: u32 = 16;
pub const K_TR_FAIL_MODULE: u32 = 32;
pub const K_TR_FAIL_ALL: u32 = 48;

#[derive(Debug, Clone)]
pub enum TestReturnCode {
    Pass = 0,
    Fail = 16,
    FailModule = 32,
    FailAll = 48,
}


impl TryFrom<c_int> for TestReturnCode {
    type Error = ();
    fn try_from(v : c_int) -> Result<TestReturnCode,()> {
        match v {
            x if x == TestReturnCode::Pass as c_int => Ok(TestReturnCode::Pass),
            x if x == TestReturnCode::Fail as c_int => Ok(TestReturnCode::Fail),
            x if x == TestReturnCode::FailModule as c_int => Ok(TestReturnCode::FailModule),
            x if x == TestReturnCode::FailAll as c_int => Ok(TestReturnCode::FailAll),
            _ => Err(()),
        }
    }
}

pub type TestableFunction = unsafe extern "C" fn(*mut TestRunnerInterface) -> c_int;
pub type PrePostCaseHandler = extern "C" fn(*mut TestRunnerInterface) -> c_void;
pub type AssertErrorHandler = extern "C" fn(exp : *const c_char, file : *const c_char, line : c_int);
pub type LogHandlerNonVar =  extern "C" fn (line : c_int, file: *const c_char, format: *const c_char);
pub type LogHandler =  extern "C" fn (line : c_int, file: *const c_char, format: *const c_char, ...) -> c_void;
//pub type CaseHandler = extern "C" fn(case_handler: *mut TestRunnerInterface);
pub type CaseHandler = extern "C" fn(case_handler: PrePostCaseHandler);
pub type DependsHandler = extern "C" fn(name : *const c_char, dep_list: *const c_char);
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct TestRunnerInterface {
    pub debug : Option<LogHandlerNonVar>,
    pub info : Option<LogHandlerNonVar>,
    pub warning : Option<LogHandlerNonVar>,
    // FIXME: Change once rust support variadic arg handling
    pub error: Option<LogHandlerNonVar>,
    pub fatal : Option<LogHandlerNonVar>,
    pub abort : Option<LogHandlerNonVar>,

    pub assert_error : Option<AssertErrorHandler>,

    pub set_pre_case_callback : Option<CaseHandler>,
    pub set_post_case_callback : Option<CaseHandler>,

    pub case_depends : Option<DependsHandler>,
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

            assert_error: None,

            set_pre_case_callback : None,
            set_post_case_callback : None,

            case_depends : None,

        };
        return trun;
    }
}
