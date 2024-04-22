#[derive(Debug)]
pub enum AssertClass {
    Error,
    Abort,
    Fatal,
}
#[derive(Debug)]
pub struct AssertError {
    pub assert_class: AssertClass,
    pub file : String,
    pub line : u32,
    pub message : String,
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
