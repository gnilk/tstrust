use crate::test_runner::{DynLibraryRef, TestFunction};

pub struct TestRunner {

}
impl TestRunner {
    pub fn new() -> TestRunner {
        Self {}
    }

    pub fn prepare_tests(&mut self, dynlib : &DynLibraryRef) {
        for x in &dynlib.borrow().exports {

        }
    }
}