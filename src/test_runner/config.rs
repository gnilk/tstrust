use std::mem::MaybeUninit;
use std::sync::Once;
use crate::test_runner::Singleton;

use clap::{Parser};



#[derive(clap::Parser, Debug)]
#[command(name = "tstrust")]
#[command(version = "0.0.1")]
#[command(about = "C/C++ Test Runner in Rust", long_about = None)]
pub struct Config {
    /// Verbose, specify multiple times to increase
    #[arg(short='v', default_value_t = 0, action = clap::ArgAction::Count)]
    pub verbose : u8,

    /// Specify test modules
    #[arg(short='m', value_parser, value_delimiter= ',', default_values=["-"].to_vec())]
    pub modules : Vec<String>,

    /// Specify test cases
    #[arg(short='t', value_parser, value_delimiter= ',', default_values=["-"].to_vec())]
    pub testcases : Vec<String>,

    /// Specify global main function name
    #[arg(long, default_value_t = ("main").to_string())]
    pub main_func_name : String,

    /// Specify global exit function name
    #[arg(long, default_value_t = ("exit").to_string())]
    pub exit_func_name : String,

    /// Specify reporting module
    #[arg(short='R', default_value_t = ("console").to_string())]
    pub reporting_module : String,


    /// Specify reporting output file, default is stdout
    #[arg(short='O', default_value_t = ("-").to_string())]
    pub reporting_file : String,

    /// Specify reporting indent
    #[arg(long, default_value_t = 8)]
    pub report_indent : i32,

    /// Execute tests
    #[arg(short='x', default_value_t = true)]
    pub execute_tests : bool,

    /// List available tests
    #[arg(short='l', default_value_t = false)]
    pub list_tests : bool,

    /// Print test-passes in summary
    #[arg(short='S', default_value_t = false)]
    pub print_pass_summary : bool,

    /// Execute module globals when executing
    #[arg(short='g', default_value_t = true)]
    pub test_module_globals : bool,

    /// Execute globals when executing
    #[arg(short='G', default_value_t = true)]
    pub test_global_main : bool,

    // /// Filter logs from tests
    // #[arg(short, default_value_t = false)]
    // test_log_filter : bool,

    /// Skip module on result FailModule from case
    #[arg(short='c', default_value_t = true)]
    pub skip_on_module_fail : bool,

    /// Skip all on result AllFail from case
    #[arg(short='C', default_value_t = true)]
    pub stop_on_all_fail : bool,


    /// Suppress progress messages
    #[arg(short='s', default_value_t = false)]
    pub suppress_progress : bool,

    /// Discard test result code handling
    #[arg(short='r', default_value_t = false)]
    pub discard_test_return_code : bool,

    /// files/directories to scan for tests
    #[arg(default_values = ["."].to_vec())]
    pub inputs : Vec<String>,
}


impl Singleton for Config {
    fn instance() -> &'static Self {
        static mut GLB_CONFIG_SINGLETON: MaybeUninit<Config> = MaybeUninit::uninit();
        static ONCE: Once = Once::new();
        unsafe {
            ONCE.call_once(|| {
                let singleton = Config::parse();
                GLB_CONFIG_SINGLETON.write(singleton);
            });
            GLB_CONFIG_SINGLETON.assume_init_ref()
        }
    }
}
