use std::cell::RefCell;
use std::rc::Rc;
use std::collections::{HashMap};
use crate::test_runner::{Config, Singleton, DynLibrary, Module, TestFunction, TestFunctionRef, TestScope, TestType, ResultSummary, DynLibraryRef};

//
// The runner holds all test details for a single library..
//
pub struct TestRunner {
    //library : DynLibrary,
    library : DynLibraryRef,
    modules : HashMap<String, Module>,
    global_main : Option<TestFunctionRef>,
    global_exit : Option<TestFunctionRef>,
    global_results : ResultSummary,
    test_results : Vec<ResultSummary>,
}

impl TestRunner {

    // filename should be a shared library
    pub fn new(filename : &str) -> TestRunner {
        let mut inst = TestRunner {
            library : Rc::new(RefCell::new(DynLibrary::new(filename))),
            modules : HashMap::new(),
            global_main : None,
            global_exit : None,
            global_results : ResultSummary::new("-"),       // special 'global' name
            test_results : Vec::new(),

        };
        inst.prescan();
        return inst;
    }

    //
    // Internal, called during ctor - creates the test-functions and the modules
    //
    fn prescan(&mut self) {

        for x in &self.library.borrow_mut().exports {
            let res = Self::create_test_function(x);
            if res.is_err() {
                continue;
            }
            let func = res.unwrap();

            match func.borrow().test_scope {
                TestScope::Global => {
                    match func.borrow().test_type {
                        TestType::Main => self.global_main = Some(func.clone()),
                        TestType::Exit => self.global_exit = Some(func.clone()),
                        _ => {
                            println!("Global regular test cases not allowed");
                            return;
                        }
                    }; // match test_type
                },
                TestScope::Module => {
                    if !self.modules.contains_key(&func.borrow().module_name) {
                        let m = Module::new(&func.borrow().module_name);
                        self.modules.insert(func.borrow().module_name.to_string(),m);
                    }
                    let m = self.modules.get_mut(&func.borrow().module_name).expect("get");

                    match func.borrow().test_type {
                        TestType::Main => m.main_func = Some(func.clone()),
                        TestType::Exit => m.exit_func = Some(func.clone()),
                        _ => m.test_cases.push(func.clone()),
                    }; // match test_type
                },
            }; // match test_scope
        };
    }

    //
    // List tests available (with proper grouping) in this runner and if they are scheduled for execution
    //
    pub fn list_tests(&self) {
        if Config::instance().test_global_main {
            println!("* Globals:");
        } else {
            println!("- Globals:");
        }
        // TJOHO!
        if self.global_main.is_some() {
            let main_func = self.global_main.as_ref().unwrap();
            println!("  {} ::{} ({})",
                     self.func_exec_prefix(main_func, Config::instance().test_global_main),
                     main_func.borrow().case_name,
                     main_func.borrow().symbol);
        }
        if self.global_exit.is_some() {
            let exit_func = self.global_exit.as_ref().unwrap();
            println!("  {} ::{} ({})",
                     self.func_exec_prefix(exit_func, Config::instance().test_global_main),
                     exit_func.borrow().case_name,
                     exit_func.borrow().symbol);
        }
        for (name, module) in &self.modules {

            let module_exec = module.should_execute();
            println!("{} Module: {}", self.module_exec_prefix(module), &name);

            // Move main/exit out of here...
            if module.main_func.is_some() {
                // Need to clone here - also I really start to dislike working with references...
                // it would be nice with some shared pointers..
                let main_func = module.main_func.as_ref().unwrap().clone();
                println!("  {}{} {}::{} ({})",
                         self.func_exec_prefix(&main_func, module_exec),
                         self.func_qualifier(&main_func),
                         &name,
                         main_func.borrow().case_name,
                         main_func.borrow().symbol);

            }


            if module.exit_func.is_some() {
                // Need to clone here - also I really start to dislike working with references...
                // it would be nice with some shared pointers..
                let exit_func = module.exit_func.as_ref().unwrap().clone();
                println!("  {}{} {}::{} ({})",
                         self.func_exec_prefix(&exit_func, module_exec),
                         self.func_qualifier(&exit_func),
                         &name,
                         exit_func.borrow().case_name,
                         exit_func.borrow().symbol);

            }


            for func in &module.test_cases {
                println!("  {}  {}::{} ({})",
                         self.func_exec_prefix(func, module_exec),
                         &name,
                         func.borrow().case_name,
                         func.borrow().symbol);
            }
        }
    }
    fn module_exec_prefix(&self, module : &Module) -> &str {
        if module.should_execute() {
            return "*"
        }
        return "-"
    }
    fn func_exec_prefix(&self, func : &TestFunctionRef, module_exec : bool) -> &str {
        if module_exec && func.borrow().should_execute() {
            return "*";
        }
        return " ";
    }
    fn func_qualifier(&self, func: &TestFunctionRef) -> &str {
        match func.borrow().test_type {
            TestType::Main => return "m",
            TestType::Exit => return "e",
            _ => " ",
        }
    }

    //
    // Creates a test function from an export symbol...
    // This could perhaps be baked into 'new' for TestFunctionRef - but won't do that until this whole thing can replace the current execution path...
    //
    fn create_test_function(symbol : &str) -> Result<TestFunctionRef,()> {
        let parts:Vec<String> = symbol.split('_').map(|x| x.to_string()).collect();

        if parts.len() <= 1 {
            return Err(());
        }
        if parts.len() == 2 {
            let func = TestFunction::new(symbol, "-", &parts[1]);
            return Ok(func);
        }
        let case_name = parts[2..].join("_");
        let func = TestFunction::new(symbol, &parts[1], &case_name);


        return Ok(func);
    }
    //
    // Execution
    //
    pub fn execute_tests(&mut self) {

        println!("---> Start Library  \t{}", self.library.borrow().name);

        self.execute_library_main();
        self.execute_all_modules();
        self.execute_library_exit();

        // Merge results
        self.test_results.push(self.global_results.clone());

        println!("<--- Start Library  \t{}", self.library.borrow().name);
    }

    //
    // I would rather have a single function doing this like 'execute_opt_func(&mut self, &Option<TestFunctionRef>)'
    //
    fn execute_library_main(&mut self) {
        match &self.global_main {
            None => (),
            Some(x) => {
                if x.borrow().should_execute() {
                    x.borrow_mut().execute_no_module(&self.library);
                    self.global_results.add_test_result(&x.borrow().test_result);
                }
            }
        }
    }

    fn execute_library_exit(&mut self) {
        match &self.global_exit {
            None => (),
            Some(x) => {
                if x.borrow().should_execute() {
                    x.borrow_mut().execute_no_module(&self.library);
                    self.global_results.add_test_result(&x.borrow().test_result);
                }
            }
        }
    }

    //
    // Execute tests in all modules
    //
    fn execute_all_modules(&mut self) {
        for (_, module) in self.modules.iter_mut() {
            if !module.should_execute() {
                continue;
            }
            // HOW SHOULD THIS LINE WORK!!!
            //self.execute_module_tests(module);
            let dynlibref = Rc::new(RefCell::new(&self.library));
            module.execute(&self.library);

            // Gather and append results...
            let results = ResultSummary::from_module(&module);
            self.test_results.push(results);


        };
    }

    pub fn print_results(&self) {
        let mut num_failed = 0;
        let mut num_executed = 0;


        for r in &self.test_results {
            // We only gather number of executed
            num_executed += r.tests_executed;
            num_failed += r.tests_failed;
        } // for

        println!("Tests Executed: {}", num_executed);
        println!("Tests Failed..: {}", num_failed);

        if num_failed > 0 {
            println!("Failed:");
            for r in &self.test_results {
                r.print_failures();
            }
        }

    }
}