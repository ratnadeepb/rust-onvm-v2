/*
 * Created on Fri Oct 02 2020:17:40:44
 * Created by Ratnadeep Bhattacharya
 */

use onvm_mgr;
// use onvm_mgr::mgr::init;
use std::env;
// use std::ffi::CString;
use std::os::raw::c_int;

fn main() {
    let mut args: Vec<String> = env::args().collect();
    println!("Args received: {:?}", &args);
    // onvm_mgr::main_run(args.len() as c_int, &mut (&mut args as *mut _ as *mut i8));
    // match onvm_mgr::mgr::init::init(args.len() as c_int, &mut (&mut args as *mut _ as *mut i8)) {
    match onvm_mgr::mgr::init::init(args) {
        Ok(()) => println!("Successfully init"),
        Err(e) => println!("Init failed: {:?}", e),
    }
    println!("Hello, world!");
}
