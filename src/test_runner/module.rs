use crate::test_runner::*;

#[derive(Debug)]
pub struct Module {
    pub name : String,
    pub pre_case_func : Option<PrePostCaseHandler>,
    pub post_case_func : Option<PrePostCaseHandler>,

    pub main_func : Option<TestFunctionRef>,
    pub exit_func : Option<TestFunctionRef>,
    pub test_cases : Vec<TestFunctionRef>,
}

impl Module {
    //pub fn new<'a>(name : &'a str, dyn_library: &'a DynLibrary) -> Module<'a> {
    //pub fn new(name : &str, dyn_library: &DynLibraryRef) -> Module {
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

    pub fn should_execute(&self) -> bool {
        let cfg = Config::instance();
        if cfg.modules.contains(&"-".to_string()) || cfg.modules.contains(&self.name) {
            return true;
        }
        return false;
    }

    pub fn execute(&mut self, dynlib : &DynLibrary) {
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

    fn execute_main(&mut self, dynlib : &DynLibrary) {
        if !self.main_func.is_some() {
            return;
        }

//        println!("Execute main!");
        let func = self.main_func.as_ref().unwrap();
        func.borrow_mut().execute(self, dynlib);

        // Grab hold of the context and verify test-cases...
        let ctx = CONTEXT.take();

        self.pre_case_func = ctx.pre_case_handler;
        self.post_case_func = ctx.post_case_handler;

        // handle dependencies
        if ctx.dependencies.is_empty() {
//            println!("No dependencies");
            return;
        }
//        println!("Dependencies");
//        ctx.dump();
        // FIXME: Look up the correct test-case (use case names) and then append the dependency list to the test-case

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

    fn execute_exit(&mut self, dynlib : &DynLibrary) {
        if !self.exit_func.is_some() {
            return;
        }

        let func = self.exit_func.as_ref().unwrap();
        func.borrow_mut().execute(self, dynlib);

        // Grab hold of the context and verify test-cases...
        CONTEXT.take();
    }


    fn execute_test(&self, tc : &TestFunctionRef, dynlib : &DynLibrary) {
        if self.pre_case_func.is_some() {
            let mut trun_interface = tc.borrow().get_truninterface_ptr(); //TestRunnerInterface::new();
            self.pre_case_func.as_ref().unwrap()(&mut trun_interface);
        }

        tc.borrow_mut().execute(self, dynlib);

        if self.post_case_func.is_some() {
            let mut trun_interface = tc.borrow().get_truninterface_ptr(); //TestRunnerInterface::new();
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

}
