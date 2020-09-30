/*
 * Created on Mon Sep 28 2020:11:29:30
 * Created by Ratnadeep Bhattacharya
 */

use getopts::Options;
use lazy_static::lazy_static;
use std::os::raw::{c_char, c_int};
use std::sync::{Arc, Mutex, RwLock};

pub static mut ONVM_NF_SHARE_CORES: Option<RwLock<u8>> = None;

static mut PROGNAME: Option<String> = None;

lazy_static! {
	// pub static ref ONVM_NF_SHARE_CORES: Arc<RwLock<u8>> = Arc::new(RwLock::new(0));
	pub static ref GLOBAL_VERBOSITY_LEVEL: Arc<RwLock<u8>> = Arc::new(RwLock::new(0));
	pub static ref GLOBAL_PKT_LIMIT: Arc<RwLock<u8>> = Arc::new(RwLock::new(0));
	pub static ref GLOBAL_TIME_TO_LIVE: Arc<RwLock<u8>> = Arc::new(RwLock::new(0));
	pub static ref NUM_NFS: Arc<RwLock<u16>> = Arc::new(RwLock::new(0));
	pub static ref NUM_SERVICES: Arc<RwLock<u16>> = Arc::new(RwLock::new(0));
	pub static ref DEFAULT_SERVICE: Arc<RwLock<u16>> = Arc::new(RwLock::new(0));
	pub static ref DEFAULT_SERVICE_ID: Arc<RwLock<u32>> = Arc::new(RwLock::new(1));
}

fn parse_app_args(max_ports: u8, mut argc: c_int, mut argv: *mut *mut c_char) -> u8 {
	// let option_index;
	// let opt;
	let mut lgopts = Options::new();
	lgopts.reqopt("p", "port-mask", "", "");
	lgopts.reqopt("r", "num-services", "", "");
	lgopts.reqopt("n", "nf-cores", "", "");
	let a = unsafe { std::slice::from_raw_parts(*argv, 1) };
	// std::mem::replace(&mut *PROGNAME, a[0].to_string());
	unsafe { PROGNAME = Some(a[0].to_string()) };
	0
}
