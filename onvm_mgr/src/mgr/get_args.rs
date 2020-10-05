/*
 * Created on Mon Sep 28 2020:11:29:30
 * Created by Ratnadeep Bhattacharya
 */

use super::global;
use crate::error_handling;
use exitfailure::ExitFailure;
use getopts::Options;
// use lazy_static::lazy_static;
// use log;
use num_cpus;
use std::ffi::{CString, OsStr};
use std::os::raw::{c_char, c_int};
use std::sync::{Arc, Mutex, RwLock};
use std::{i64, u8};
// use std::os::unix::ffi::OsStrExt;

// pub static mut ONVM_NF_SHARE_CORES: Option<RwLock<u8>> = None;

static mut PROGNAME: Option<String> = None;

// lazy_static! {
// 	// pub static ref ONVM_NF_SHARE_CORES: Arc<RwLock<u8>> = Arc::new(RwLock::new(0));
// 	pub static ref GLOBAL_VERBOSITY_LEVEL: Arc<RwLock<u8>> = Arc::new(RwLock::new(0));
// 	pub static ref GLOBAL_PKT_LIMIT: Arc<RwLock<u8>> = Arc::new(RwLock::new(0));
// 	pub static ref GLOBAL_TIME_TO_LIVE: Arc<RwLock<u8>> = Arc::new(RwLock::new(0));
// 	pub static ref NUM_NFS: Arc<RwLock<u16>> = Arc::new(RwLock::new(0));
// 	pub static ref NUM_SERVICES: Arc<RwLock<u16>> = Arc::new(RwLock::new(0));
// 	pub static ref DEFAULT_SERVICE: Arc<RwLock<u16>> = Arc::new(RwLock::new(0));
// 	pub static ref DEFAULT_SERVICE_ID: Arc<RwLock<u32>> = Arc::new(RwLock::new(1));
// }

pub fn parse_app_args(
	max_ports: u16,
	global_state: &mut global::GlobalNFState,
	args: Vec<String>,
) -> Result<(), ExitFailure> {
	// let option_index;
	// let opt;
	let mut lgopts = Options::new();
	lgopts.optopt("p", "port-mask", "", "");
	lgopts.optopt("r", "num-services", "", "");
	lgopts.optopt("n", "nf-cores", "", "");
	// let a = unsafe { std::slice::from_raw_parts(*argv, argc as usize) };
	// std::mem::replace(&mut *PROGNAME, a[0].to_string());
	// unsafe { PROGNAME = Some(a[0].to_string()) }

	// let cs = unsafe { CString::from_raw(*argv) };
	// let cs =

	// let matches = match lgopts.parse(cs.to_str()) {
	let matches = match lgopts.parse(args) {
		Ok(matches) => matches,
		Err(e) => {
			let e = format!("No arguments provided for the switch: {}", e);
			return Ok(error_handling::exit_on_failure(
				"Failed to find any arguments".into(),
				&e[..],
			)?);
		}
	};

	if let Some(p) = matches.opt_str("p") {
		parse_portmask(max_ports, p, global_state);
	}
	if let Some(r) = matches.opt_str("r") {
		parse_num_services(r, global_state);
	}
	if let Some(n) = matches.opt_str("n") {
		parse_nf_cores(n, global_state);
	}
	Ok(())
}

fn parse_portmask(max_ports: u16, portmask: String, global_state: &mut global::GlobalNFState) {
	let mut count = 0;
	let mut pm = i64::from_str_radix(&portmask[..], 16).unwrap();

	if pm == 0 {
		println!("WARNING: No ports are being used.\n");
		return;
	}
	/* loop through bits of the mask and mark ports */
	while pm != 0 {
		if pm & 0x01 != 0 {
			/* bit is set in mask, use port */
			{
				if count >= max_ports {
					println!("Ignoring port: {}", count);
				} else {
					let n = *(global_state.ports.num_ports).borrow() as usize;
					(*global_state.ports.id.borrow_mut())[n] = count as u8;
				}
			}
			pm = pm >> 1;
			count += 1;
		}
	}
}

fn parse_num_services(services: String, global_state: &mut global::GlobalNFState) {
	let r = u8::from_str_radix(&services[..], 10).unwrap();
	*global_state.num_services.borrow_mut() = r;
}

fn parse_nf_cores(nf_coremask: String, global_state: &mut global::GlobalNFState) {
	let max_cores = num_cpus::get();
	let mut num_cores = 0;
	let mut count = 0;
	let mut pm = u8::from_str_radix(&nf_coremask[..], 10).unwrap();
	if pm == 0 {
		println!("WARNING: No NF cores are being used.\n");
		println!("         Restart onvm_mgr with a valid coremask to run NFs.\n");
		return;
	}
	while pm != 0 {
		if pm & 0x01 != 0 {
			if count >= max_cores {
				println!(
					"WARNING: requested core {} out of cpu bounds - ignoring\n",
					count
				);
			} else {
				// let core = unsafe { global_state.cores.clone()[count] };
				// let core = **global_state.cores.clone()[count].borrow_mut();
				unsafe {
					*(*(*global_state.cores[count].clone())).enabled.borrow_mut() = true;
					*(*(*global_state.cores[count].clone()))
						.nf_count
						.borrow_mut() = 0;
				}
				num_cores += 1;
			}
		}
		pm = pm >> 1;
		count += 1;
		if count == max_cores {
			break;
		}
	}

	count = 0;
	println!("Registered {} cores for NFs: ", num_cores);
	for i in 0..max_cores {
		let enabled = unsafe { *(*(*global_state.cores[count].clone())).enabled.borrow() };
		if enabled {
			print!("{}", i);
			if count != num_cores - 1 {
				print!(", ");
			}
			count += 1;
		}
	}
	println!("");
}
