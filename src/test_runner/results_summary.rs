use crate::test_runner::{Module, TestResult};


#[derive(Debug, Clone)]
pub struct ResultSummary {
    pub module_name : String,
    pub tests_executed : i32,
    pub tests_failed : i32,
    pub duration_sec : f32,
    pub test_results : Vec<TestResult>,
}

impl ResultSummary {

    pub fn new(module_name : &str) -> Self {
        Self {
            module_name : module_name.to_string(),
            tests_executed : 0,
            tests_failed : 0,
            duration_sec : 0f32,        // This should not be here???  [it is in the C/C++ version]
            test_results : Vec::new(),
        }
    }
    pub fn from_module(module : &Module) -> Self {
        let mod_results = module.gather_test_results();

        let mut instance = ResultSummary {
            module_name : module.name.clone(),
            tests_executed : 0,
            tests_failed : 0,
            duration_sec : 0f32,        // This should not be here???  [it is in the C/C++ version]
            test_results : mod_results,
        };
        instance.count_stats();
        return instance;
    }

    pub fn add_test_result(&mut self, test_result : &TestResult) {
        self.test_results.push(test_result.clone());
        self.count_stats();
    }

    fn count_stats(&mut self) {
        // reset
        self.tests_executed = 0;
        self.tests_failed = 0;

        // re-count
        for r in &self.test_results {
            self.tests_executed += 1;
            if r.did_fail() {
                self.tests_failed += 1;
            }
        }
    }

    pub fn print_failures(&self) {
        for r in &self.test_results {
            if r.did_fail() {
                r.print_failure();
            }
        }
    }

}
