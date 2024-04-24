use crate::test_runner::{TestFuncErrorClass, TestFuncError, PrePostCaseHandler};

pub struct Context {
    pub raw_result : i32,
    pub dependencies : Vec<CaseDependency>,
    pub func_error : Option<TestFuncError>,
    pub pre_case_handler : Option<PrePostCaseHandler>,
    pub post_case_handler : Option<PrePostCaseHandler>,
}
pub struct CaseDependency {
    pub case : String,
    pub dependencies : Vec<String>,
}

impl Default for Context {
    fn default() -> Self {
        Self {
            raw_result : 0,
            dependencies : Vec::new(),
            func_error : None,
            pre_case_handler : None,
            post_case_handler : None,
        }
    }
}
impl Context {
    pub fn new() -> Context {
        Self {
            raw_result : 0,
            dependencies : Vec::new(),
            func_error : None,
            pre_case_handler : None,
            post_case_handler : None,
        }
    }

    // FIXME: check if possible to use 'default' instead..
    pub fn reset(&mut self) -> Context {
        Self {
            raw_result : 0,
            dependencies : Vec::new(),
            func_error : None,
            pre_case_handler : None,
            post_case_handler : None,
        }
    }
    pub fn add_dependency(&mut self, case: &str, deplist: &str) {
        let parts: Vec<_> = deplist.split(",").collect();
        let mut case_dep = CaseDependency {
            case : case.to_string(),
            dependencies : Vec::new(),
        };

        for p in &parts {

            case_dep.dependencies.push(p.trim().to_string());
        }
        self.dependencies.push(case_dep);

    }
    pub fn set_assert_error(&mut self, eclass: TestFuncErrorClass, line : u32, file : &str, message : &str) {
        let func_error = TestFuncError {
            eclass,
            line,
            file : file.to_string(),
            message : message.to_string(),
        };
        self.func_error = Some(func_error);
    }

    pub fn dump(&self) {
        println!("Context, dependencies");
        for dep in &self.dependencies {
            println!("  test case: {}", dep.case);
            for case in &dep.dependencies {
                println!("    {}", case);
            }
        }
    }
}
