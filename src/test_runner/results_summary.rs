use crate::test_runner::{Module, TestResult};

pub struct ResultSummary {
    pub module_name : String,
    pub tests_executed : i32,
    pub tests_failed : i32,
    pub duration_sec : f32,
    pub test_results : Vec<TestResult>,
}

impl ResultSummary {

    // Consider supplying a module here
    pub fn new(module : &Module) -> Self {
        let mod_results = module.gather_test_results();

        let mut instance = ResultSummary {
            module_name : module.name.clone(),
            tests_executed : 0,
            tests_failed : 0,
            duration_sec : 0f32,        // This should not be here???  [it is in the C/C++ version]
            test_results : mod_results,
        };

        for r in &instance.test_results {
            instance.tests_executed += 1;
            if r.did_fail() {
                instance.tests_failed += 1;
            }
        }
        return instance;
    }

    pub fn print_failures(&self) {
        for r in &self.test_results {
            if r.did_fail() {
                r.print_failure();
            }
        }
    }

}
