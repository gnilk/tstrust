use std::ffi::c_int;
use std::time::Duration;
use crate::test_runner::{TestFuncError, TestReturnCode};

#[derive(Debug, Clone)]
pub struct TestResult {
    pub exec_duration: Duration,

    pub raw_return_code: c_int,     // The return code from the C/C++ function, use 'TestReturnCode::try_from' to transform and verify
    pub return_code: Option<TestReturnCode>, // raw c_int enum from ITestInterface converted to internal enum after execution

    pub func_error: Option<TestFuncError>,        // Holds the assert error msg if any..
    pub num_error : u32,            // Note: Always one, if an error occurs - however, embedded or single-threading can have several
    pub num_assert : u32,           // Note: Always one, if an error occurs - however, embedded or single-threading can have several

    pub symbol : String,            // The actual exported symbol
}

impl TestResult {
    pub fn new() -> TestResult {
        Self {
            return_code: None,
            func_error: None,
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
        match &self.return_code {
            None => println!("=== INVALID RETURN CODE ({}) for {}", self.raw_return_code, self.symbol),
            Some(rc) => {
                match rc {
                    TestReturnCode::Pass => println!("=== PASS:\t{}, {} sec, {}", self.symbol, self.exec_duration.as_secs_f32(), self.raw_return_code),
                    TestReturnCode::Fail => println!("=== FAIL:\t{}, {} sec, {}", self.symbol, self.exec_duration.as_secs_f32(), self.raw_return_code),
                    TestReturnCode::FailModule => println!("=== FAIL:\t{}, {} sec, {}", self.symbol, self.exec_duration.as_secs_f32(), self.raw_return_code),
                    TestReturnCode::FailAll => println!("=== FAIL:\t{}, {} sec, {}", self.symbol, self.exec_duration.as_secs_f32(), self.raw_return_code),
                }
            }
        }
        // Empty line in the console output
        println!("");
    }

    pub fn did_pass(&self) -> bool {
        match &self.return_code {
            Some(rc) => {
                match rc {
                    TestReturnCode::Pass => {
                        return true;
                    },
                    _ => (),
                } // inner match
            },
            _ => (),
        } // outer match

        return false;
    }
    pub fn did_fail(&self) -> bool {
        match &self.return_code {
            Some(rc) => {
                match rc {
                    TestReturnCode::Pass => {
                        return false;
                    },
                    _ => (),
                } // inner match
            },
            _ => (),
        } // outer match

        return true;
    }
    pub fn print_failure(&self) {
        if self.func_error.is_some() {
            let ass_err = &self.func_error.as_ref().unwrap();
            println!("  [Tma]: {}, {}:{}, {}", self.symbol, ass_err.file, ass_err.line, ass_err.message);
            return;
        }
        if self.return_code.is_some() {
            match &self.return_code.as_ref().unwrap() {
                TestReturnCode::Fail => {
                    println!("  [Tma]: {}", self.symbol);
                },
                TestReturnCode::FailModule => {
                    println!("  [tMa]: {}", self.symbol);
                },
                TestReturnCode::FailAll => {
                    println!("  [tmA]: {}", self.symbol);
                },
                _ => (),
            }
        } else {
            println!("  [tma]: {}", self.symbol);
        }
    }

}
