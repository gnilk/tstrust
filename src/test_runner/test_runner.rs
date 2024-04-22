use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use crate::test_runner::{Config, Singleton, DynLibraryRef, Module, ModuleRef, TestFunction, TestFunctionRef, TestScope, TestType};

pub struct TestRunner {
    library : DynLibraryRef,
    modules : HashMap<String, ModuleRef>,
    global_main : Option<TestFunctionRef>,
    global_exit : Option<TestFunctionRef>,
}
impl TestRunner {
    pub fn new(library: &DynLibraryRef) -> TestRunner {
        Self {
            library : library.clone(),
            modules : HashMap::new(),
            global_main : None,
            global_exit : None,
        }
    }



    pub fn prescan(&mut self) {
        // FIXME: Implement
        //let lib = self.library.borrow_mut();
        let lib = self.library.borrow();
        for x in &lib.exports {
            let res = Self::create_test_function(x);
            if res.is_err() {
                continue;
            }
            let func = res.unwrap();

            // if func.borrow().symbol == "test_exit" {
            //     println!("WEFWeffwef");
            // }
            // println!("symbol='{}'    module='{}'    case='{}'", x, func.borrow().module_name, func.borrow().case_name);

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
                        let m = Rc::new(RefCell::new(Module::new(&func.borrow().module_name, &self.library)));
                        self.modules.insert(func.borrow().module_name.to_string(),m);
                    }
                    let m = self.modules.get(&func.borrow().module_name).expect("get");

                    match func.borrow().test_type {
                        TestType::Main => m.borrow_mut().main_func = Some(func.clone()),
                        TestType::Exit => m.borrow_mut().exit_func = Some(func.clone()),
                        _ => m.borrow_mut().test_cases.push(func.clone()),
                    }; // match test_type
                },
            }; // match test_scope
        };
    }

    pub fn dump(&self) {
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

            let module_exec = module.borrow().should_execute();
            println!("{} Module: {}", self.module_exec_prefix(module), &name);

            // Move main/exit out of here...
            if module.borrow().main_func.is_some() {
                // Need to clone here - also I really start to dislike working with references...
                // it would be nice with some shared pointers..
                let main_func = module.borrow().main_func.as_ref().unwrap().clone();
                println!("  {}{} {}::{} ({})",
                         self.func_exec_prefix(&main_func, module_exec),
                         self.func_qualifier(&main_func),
                         &name,
                         main_func.borrow().case_name,
                         main_func.borrow().symbol);

            }


            if module.borrow().exit_func.is_some() {
                // Need to clone here - also I really start to dislike working with references...
                // it would be nice with some shared pointers..
                let exit_func = module.borrow().exit_func.as_ref().unwrap().clone();
                println!("  {}{} {}::{} ({})",
                         self.func_exec_prefix(&exit_func, module_exec),
                         self.func_qualifier(&exit_func),
                         &name,
                         exit_func.borrow().case_name,
                         exit_func.borrow().symbol);

            }


            for func in &module.borrow().test_cases {
                println!("  {}  {}::{} ({})",
                         self.func_exec_prefix(func, module_exec),
                         &name,
                         func.borrow().case_name,
                         func.borrow().symbol);
            }
        }
    }
    fn module_exec_prefix(&self, module : &ModuleRef) -> &str {
        if module.borrow().should_execute() {
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


}