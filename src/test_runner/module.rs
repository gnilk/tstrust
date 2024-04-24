use crate::test_runner::*;

//
// A module is a semantic grouping of tests. The module name is derived from the name, like:
//  test_<module>_<name_with_whatever>
//
// Only from main is it possible to call 'depends' (in the C/C++ version we allow this from everywhere)
//
#[derive(Debug)]
pub struct Module {
    pub name : String,
    // Set through 'case_depends'
    pub pre_case_func : Option<PrePostCaseHandler>,
    pub post_case_func : Option<PrePostCaseHandler>,

    // main is: 'test_<module>()'
    pub main_func : Option<TestFunctionRef>,
    // exit is: 'test_<module>_exit()'  <- ergo, exit is a reserved function name...
    pub exit_func : Option<TestFunctionRef>,
    // regular test cases
    pub test_cases : Vec<TestFunctionRef>,
}

impl Module {
    pub fn new(name : &str) -> Module {
        let module = Module {
            name : name.to_string(),
            main_func : None,
            exit_func : None,
            post_case_func : None,
            pre_case_func : None,
            test_cases : Vec::new(),
        };

        return module;
    }

    // Checks if we should execute
    // FIXME: Could also check 'flags' - this needs better impl - support for '!module' and '-m a*' etc..
    pub fn should_execute(&self) -> bool {
        let cfg = Config::instance();
        if cfg.modules.contains(&"-".to_string()) || cfg.modules.contains(&self.name) {
            return true;
        }
        return false;
    }

    // Execute all functions in a module (incl. main/exit)
    pub fn execute(&mut self, dynlib : &DynLibraryRef) {
        // Execute main first, main can define various dependens plus pre/post functions
        self.execute_main(dynlib);

        // Execute actual test cases
        for tc in &self.test_cases {
            if !tc.borrow().should_execute() {
                continue;
            }
            self.execute_test(tc,dynlib);
        }

        self.execute_exit(dynlib);
    }

    // Execute module main, test_<module>
    fn execute_main(&mut self, dynlib : &DynLibraryRef) {
        if !self.main_func.is_some() {
            return;
        }

        let func = self.main_func.as_ref().unwrap();
        func.borrow_mut().execute(self, dynlib);

        // Grab hold of the context and verify test-cases...
        let ctx = CONTEXT.lock().unwrap();

        self.pre_case_func = ctx.pre_case_handler;
        self.post_case_func = ctx.post_case_handler;

        // handle dependencies
        if ctx.dependencies.is_empty() {
            return;
        }

        // move dependencies over to their correct test-case
        // note: the dependency list contains a case and a list of dependencies for that case..
        //       thus we must have two loops..  one for the 'cases' and one for the dependencies..
        //       plus this also checks if they are valid (if let...)
        for casedep in &ctx.dependencies {
            if let Some(tc) = self.get_test_case(casedep.case.as_str()).ok() {
                for dep in &casedep.dependencies {
                    if let Some(tc_dep) = self.get_test_case(dep).ok() {
                        tc.borrow_mut().dependencies.push(tc_dep.clone());
                    }
                }
            }
        }
    }

    // Execute the module exit, test_<module>_exit
    fn execute_exit(&mut self, dynlib : &DynLibraryRef) {
        if !self.exit_func.is_some() {
            return;
        }

        let func = self.exit_func.as_ref().unwrap();
        func.borrow_mut().execute(self, dynlib);
    }

    // Execute a test, including pre/post cases
    fn execute_test(&self, tc : &TestFunctionRef, dynlib : &DynLibraryRef) {
        if self.pre_case_func.is_some() {
            let mut trun_interface = get_truninterface_ptr(); //TestRunnerInterface::new();
            self.pre_case_func.as_ref().unwrap()(&mut trun_interface);
        }

        tc.borrow_mut().execute(self, dynlib);

        if self.post_case_func.is_some() {
            let mut trun_interface = get_truninterface_ptr(); //TestRunnerInterface::new();
            self.post_case_func.as_ref().unwrap()(&mut trun_interface);
        }
    }


    fn get_test_case(&self, case : &str) -> Result<&TestFunctionRef, ()> {
        for tc in &self.test_cases {
            if tc.borrow().case_name == case {
                return Ok(tc);
            }
        }
        return Err(());
    }

    pub fn gather_test_results(&self) -> Vec<TestResult> {
        let mut test_results: Vec<TestResult> = Vec::new();

        match &self.main_func {
            Some(x) if x.borrow().is_finished() => {
               test_results.push(x.borrow().test_result.clone());
            },
            _ => (),
        }

        match &self.exit_func {
            Some(x) if x.borrow().is_finished() => {
                test_results.push(x.borrow().test_result.clone());
            },
            _ => (),
        }

        for tc in &self.test_cases {
            test_results.push(tc.borrow().test_result.clone());
        }

        return test_results;

    }

}
