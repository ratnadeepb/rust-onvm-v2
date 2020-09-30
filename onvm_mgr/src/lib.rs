/*
 * Created on Fri Sep 25 2020:03:38:32
 * Created by Ratnadeep Bhattacharya
 */

// `

pub mod error_handling;
pub mod mgr;
pub mod nflib;

#[allow(unused_imports)] // remove when code stabilises
use mgr::get_args;
// use nflib::{common, msg_common};

#[cfg(test)]
mod tests {
    use crate::mgr;
    use std::ffi::CString;
    use std::mem;
    use std::os::raw::{c_char, c_int};
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
    #[test]
    fn onvm_run_init() {
        // FIXME: How to get environment arguments in a tes?
        println!("Starting test init"); // DEBUG
                                        // let mut args = vec!["-p", "1"];
        let args = std::env::args();
        println!("Environment args: {:?}", &args);
        let mut _v: Vec<*mut c_char> = args
            .map(|arg| CString::new(&arg[..]).unwrap().into_raw())
            .collect();
        let argc = (_v.len() + 1) as c_int;
        let argv = _v.as_mut_ptr();
        mem::forget(_v);
        unsafe {
            println!(
                "Calling init with argc: {} and argv: {:?}",
                &argc,
                &*(*argv)
            )
        }; // DEBUG
        let _ = mgr::init::init(argc, argv);
    }
}
