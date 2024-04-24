use std::ffi::c_void;
use std::{mem, ptr};
use libc::{pthread_attr_init, pthread_attr_t, pthread_create, pthread_join, pthread_t};

pub struct PThread<T> {
    thread_arg : T,
    h_thread : pthread_t,
}
pub type PThreadFunc = extern "C" fn(*mut c_void) -> *mut c_void;

impl<T> PThread<T> {
    pub fn new(arg : T) -> PThread<T> {
        Self {
            thread_arg : arg,
            h_thread : unsafe { mem::zeroed() },
        }
    }

    pub fn spawn(&mut self, func : PThreadFunc) -> Result<(),&str>{
        let mut attr : pthread_attr_t = unsafe { mem::zeroed() };
        //let mut h_thread : pthread_t = unsafe { mem::zeroed() };

        let attr_ptr : *mut pthread_attr_t = &mut attr;
        let h_thread_ptr : *mut pthread_t = &mut self.h_thread;

        let ptr_arg : *mut c_void = &mut self.thread_arg as *mut _ as *mut c_void;

        unsafe {
            pthread_attr_init(attr_ptr);

            let err = pthread_create(h_thread_ptr, attr_ptr, func, ptr_arg);
            if err != 0 {
                return Err("pthread create failed!");
            }
        }
        return Ok(());
    }

    pub fn join(&mut self) -> Result<(), &str> {
        unsafe {
            let err = pthread_join(self.h_thread, ptr::null_mut());
            if err != 0 {
                return Err("pthread join failed!");
            }
        }
        return Ok(());
    }
}
