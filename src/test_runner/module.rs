use std::cell::RefCell;
use std::rc::Rc;
use crate::test_runner::*;

#[derive(Debug)]
pub struct Module {
    pub name : String,
    dynlib : DynLibraryRef,
    main_func : Option<TestFunctionRef>,
    pre_case_func : Option<PrePostTestcaseFunction>,
    post_case_func : Option<PrePostTestcaseFunction>,
    pub test_cases : Vec<TestFunctionRef>,
}

//pub type ModuleRef = Rc<RefCell<<Module>>;
pub type ModuleRef = Rc<RefCell<Module>>;


impl Module {
    //pub fn new<'a>(name : &'a str, dyn_library: &'a DynLibrary) -> Module<'a> {
    pub fn new(name : &str, dyn_library: &DynLibraryRef) -> Module {
        let module = Module {
            name : name.to_string(),
            dynlib : dyn_library.clone(),
            main_func : None,
            post_case_func : None,
            pre_case_func : None,
            test_cases : Vec::new(),
        };

        return module;
    }

    pub fn find_test_cases(&mut self, module : ModuleRef) {
        // println!("parsing testcase, module={}", self.name);
        for e in &self.dynlib.borrow().exports {
            let parts:Vec<&str> = e.split('_').collect();
            if parts.len() < 2 {
                panic!("Invalid export={} in dynlib={}",e,self.dynlib.borrow().name);
            }
            // Skip everything not belonging to us..
            if parts[1] != self.name {
                continue;
            }

            // special handling for 'test_<module>' => CaseType::ModuleMain
            if (parts.len() == 2) && (parts[1] == self.name) {
                // println!("  main, func={},  export={}", parts[1], e);
                self.main_func = Some(TestFunction::new(parts[1], CaseType::ModuleMain, e.to_string(), module.clone()));
                //self.test_cases.push(TestFunction::new(parts[1], CaseType::ModuleMain, e.to_string()));
            } else {
                // join the case name together again...
                let case_name = parts[2..].join("_");
                // println!("  case, func={},  export={}", case_name, e);
                self.test_cases.push(TestFunction::new(&case_name, CaseType::Regular, e.to_string(), module.clone()));
            }
        }
    }


    pub fn should_execute(&self) -> bool {
        let cfg = Config::instance();
        if cfg.modules.contains(&"-".to_string()) || cfg.modules.contains(&self.name) {
            return true;
        }
        return false;
    }

    pub fn execute(&self) {
        // Execute main first, main can define various dependens plus pre/post functions
        self.execute_main();

        // Execute actual test cases
        for tc in &self.test_cases {
            if !tc.borrow().should_execute() {
                continue;
            }
            self.execute_test(tc);
        }
    }
    fn execute_test(&self, tc : &TestFunctionRef) {

        // FIXME: Need to protect against recursiveness here!
        self.execute_dependencies(tc);
        // FIXME: Execute dependencies, should now be in the 'tc.dependencies'
        //        note: from this point - 'depends' is an illegal callback - we should probably verify this
        tc.borrow_mut().execute(&self.dynlib);

        // FIXME: Set this in 'execute' directly?
        tc.borrow_mut().set_executed();

    }

    fn execute_main(&self) {
        if !self.main_func.is_some() {
            return;
        }

//        println!("Execute main!");
        let func = self.main_func.as_ref().unwrap();
        func.borrow_mut().execute(&self.dynlib);
        func.borrow_mut().set_executed();

        // Grab hold of the context and verify test-cases...
        let ctx = CONTEXT.take();
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

    fn execute_dependencies(&self, test_function: &TestFunctionRef) {
        for func in &test_function.borrow().dependencies {
            if func.borrow().is_executed() {
                continue;
            }
            self.execute_test(func);
        }
    }

    fn get_test_case(&self, case : &str) -> Result<&TestFunctionRef, ()> {
        for tc in &self.test_cases {
            if tc.borrow().name == case {
                return Ok(tc);
            }
        }
        return Err(());
    }

    pub fn dump(&self) {
        // Smarter way to filter??
        let lib = self.dynlib.borrow();
        let dummy : Vec<&String> = lib.exports.iter().filter(|x| x.contains("casefilter")).collect();

        for d in dummy {
            println!("{}", d);
        }
    }
}
