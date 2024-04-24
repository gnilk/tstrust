#[derive(Debug, Clone)]
pub enum TestFuncErrorClass {
    Error,
    Abort,
    Fatal,
}
#[derive(Debug, Clone)]
pub struct TestFuncError {
    pub eclass: TestFuncErrorClass,
    pub file : String,
    pub line : u32,
    pub message : String,
}
impl TestFuncError {
    pub fn new(eclass : TestFuncErrorClass, file : &str, line : u32, message : &str) -> TestFuncError {
        Self {
            eclass,
            file : file.to_string(),
            line,
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
