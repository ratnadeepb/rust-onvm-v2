/*
 * Created on Thu Oct 01 2020:22:55:03
 * Created by Ratnadeep Bhattacharya
 */

use super::global;
use crate::nflib;

// DPDK functions
use capsule_ffi::{
	_rte_atomic16_dec, _rte_mempool_get, _rte_mempool_put, _rte_pktmbuf_free, _rte_ring_count,
	_rte_ring_dequeue, _rte_ring_dequeue_bulk, _rte_ring_dequeue_burst, _rte_ring_enqueue,
	rte_exit, rte_free, rte_log, rte_mempool_lookup,
};

// DPDK constants
use capsule_ffi::{RTE_LOGTYPE_USER1, RTE_LOG_ERR, RTE_LOG_INFO};

// DPDK structures
use capsule_ffi::{rte_mbuf, rte_mempool};

use crate::error_handling::exit_on_failure;
use exitfailure::ExitFailure;
use std::cell::RefCell;
use std::ffi::c_void;
use std::sync::Arc;
use std::{mem, ptr};

const NXT_INSTANCE_ID: RefCell<u32> = RefCell::new(1);
const STARTING_INSTANCE_ID: RefCell<u32> = RefCell::new(1);

/******************************Internal functions*****************************/
/// Function starting a NF
/// Input  : a pointer to the NF's informations
/// Output : an error code
fn onvm_nf_start(
	nf_init_cfg: &mut nflib::structs::OnvmNfInitCfg,
	global_state: &global::GlobalNFState,
) -> Result<(), ExitFailure> {
	let spawned_nf: &nflib::structs::OnvmNF;
	let nf_id: u32;
	let ret: i32;

	if nf_init_cfg.service_id != nflib::constants::NF_WAITING_FOR_ID {
		return Ok(exit_on_failure(
			"NF waiting for ID".into(),
			"In the onvm_nf_start function",
		)?);
	}
	// NOTE: In this case, user can't pass NF IDs but everything is assigned by the system
	nf_id = onvm_nf_next_instance_id(global_state);
	let spawned_nf = unsafe { &*global_state.nfs.clone()[nf_id as usize] };
	// let spawned_nf = *(&global_state.nfs).clone()[nf_id as usize].borrow_mut();

	// spawned_nf = *global_state.nfs[nf_id as usize].borrow_mut();

	if nf_id >= nflib::constants::MAX_NFS {
		// Service ID must be less than MAX_SERVICES and greater than 0
		nf_init_cfg.status = nflib::constants::NF_SERVICE_MAX;
		return Ok(exit_on_failure(
			"NF Service Max".into(),
			"In the onvm_nf_start function",
		)?);
	}

	if *global_state.nf_per_service_count[nf_init_cfg.service_id as usize].borrow()
		>= nflib::constants::MAX_NFS_PER_SERVICE
	{
		nf_init_cfg.status = nflib::constants::NF_SERVICE_COUNT_MAX;
		nf_init_cfg.status = nflib::constants::NF_SERVICE_MAX;
		return Ok(exit_on_failure(
			"Service per NF Count Max".into(),
			"In the onvm_nf_start function",
		)?);
	}

	// REVIEW: Since the system only is assigning IDs, is this truly necessary?
	unsafe {
		if nflib::funcs_macros::onvm_nf_is_valid(&*(*spawned_nf)) {
			// This NF is trying to declare an ID already in use
			nf_init_cfg.status = nflib::constants::NF_ID_CONFLICT;
			return Ok(exit_on_failure(
				"NF ID Conflict".into(),
				"In the onvm_nf_start function",
			)?);
		}
	}

	// Keep reference to this NF in the manager
	nf_init_cfg.instance_id = nf_id as u16;

	Ok(())
}

/// Function to mark a NF as ready.
/// Input  : a pointer to the NF's informations
/// Output : an error code
fn onvm_nf_ready(
	ready: *mut nflib::structs::OnvmNF,
	global_state: &global::GlobalNFState,
) -> Result<(), ExitFailure> {
	Ok(())
}

/// Function stopping a NF.
/// Input  : a pointer to the NF's informations
/// Output : an error code
fn onvm_nf_stop(
	stop: *mut nflib::structs::OnvmNF,
	global_state: &global::GlobalNFState,
) -> Result<(), ExitFailure> {
	let nf_id: u16;
	let nf_status: u16;
	let service_id: u16;
	let mut nb_pkts: u16;
	let mut msg: nflib::structs::OnvmNFMsg =
		unsafe { mem::MaybeUninit::<nflib::structs::OnvmNFMsg>::uninit().assume_init() };
	let nf_info_mp: *mut rte_mempool;
	let mut pkts: Vec<*mut rte_mbuf> = Vec::with_capacity(nflib::constants::PACKET_READ_SIZE);
	let candidate_nf_id: u16;
	let candidate_core: u16;
	let map_index: i32;

	let nf_id = unsafe { *(*stop).instance_id.borrow_mut() };
	// nf_id = unsafe { *(*stop).instance_id.borrow_mut() };
	service_id = unsafe { *(*stop).service_id.borrow_mut() };
	nf_status = unsafe { *(*stop).status.borrow_mut() };
	candidate_core = unsafe { (*(*stop).thread_info.borrow_mut()).core };

	/* Cleanup the allocated tag */
	unsafe {
		rte_free(&mut (*(*stop).tag.borrow_mut())[..] as *mut _ as *mut c_void);

		/* Cleanup should only happen if NF was starting or running */
		if *(*stop).status.borrow() != nflib::constants::NF_STARTING
			&& *(*stop).status.borrow() != nflib::constants::NF_RUNNING
			&& *(*stop).status.borrow() != nflib::constants::NF_PAUSED
		{
			return Ok(exit_on_failure(
				"NF is not running or starting".into(),
				"In the onvm_nf_stop function",
			)?);
		}

		*(*stop).status.borrow_mut() = nflib::constants::NF_STOPPED;
		let parent: usize;
		let mut children_cnt;
		// unsafe {
		*(*(*global_state.nfs[*(*stop).instance_id.borrow() as usize]))
			.status
			.borrow_mut() = nflib::constants::NF_STOPPED;
		// *(*global_state.nfs.clone()[*stop.instance_id.borrow() as usize])
		// 	.status
		// 	.borrow_mut() = nflib::constants::NF_STOPPED
		/* Tell parent we stopped running */
		parent = (*(*(*global_state.nfs[nf_id as usize].clone()))
			.thread_info
			.borrow())
		.parent as usize;
		// let parent = (*(*global_state.nfs.clone()[nf_id as usize])
		// 	.thread_info
		// 	.borrow())
		// .parent as usize;
		children_cnt =
			(*(*(*global_state.nfs[parent].clone())).thread_info.borrow()).children_count;
		// let mut children_cnt =
		// 	(*(*global_state.nfs.clone()[parent]).thread_info.borrow()).children_count;

		if parent != 0 {
			_rte_atomic16_dec(&mut children_cnt);
		}

		/* Remove the NF from the core it was running on */
		// unsafe {
		// let mut core = *global_state.cores.clone()[(*stop.thread_info.borrow()).core as usize];
		// let mut core = unsafe {**global_state.cores.clone()[stop.thread_info.core as usize].borrow_mut()};
		// *core.nf_count.borrow_mut() -= 1;
		// *core.is_dedicated_core.borrow_mut() = 0;
		// }
		*(*(*global_state.cores[(*(*stop).thread_info.borrow()).core as usize].clone()))
			.nf_count
			.borrow_mut() -= 1;
		// *(*global_state.cores.clone()[(*(*stop).thread_info.borrow()).core as usize])
		// 	.nf_count
		// 	.borrow_mut() -= 1;
		*(*(*global_state.cores[(*(*stop).thread_info.borrow()).core as usize].clone()))
			.is_dedicated_core
			.borrow_mut() = 0;

		/* Clean up possible left over objects in rings */
		let rx_ring_opt = *(**global_state.nfs[nf_id as usize]).rx_q.borrow_mut();
		match rx_ring_opt {
			Some(rx_ring) => {
				nb_pkts = _rte_ring_dequeue_burst(
					rx_ring,
					&mut (pkts.as_mut_ptr() as *mut c_void),
					nflib::constants::PACKET_READ_SIZE as u32,
					ptr::null_mut(),
				) as u16;
				while nb_pkts > 0 {
					for i in 0..nb_pkts as usize {
						_rte_pktmbuf_free(pkts[i]);
					}
					nb_pkts = _rte_ring_dequeue_burst(
						rx_ring,
						&mut (pkts.as_mut_ptr() as *mut c_void),
						nflib::constants::PACKET_READ_SIZE as u32,
						ptr::null_mut(),
					) as u16;
				}
			}
			None => {
				rte_exit(1, "Missing rx packet mempool\n" as *const _ as *const i8);
			}
		}

		let tx_ring_opt = *(**global_state.nfs[nf_id as usize]).tx_q.borrow_mut();
		match tx_ring_opt {
			Some(tx_ring) => {
				nb_pkts = _rte_ring_dequeue_burst(
					tx_ring,
					&mut (pkts.as_mut_ptr() as *mut c_void),
					nflib::constants::PACKET_READ_SIZE as u32,
					ptr::null_mut(),
				) as u16;
				while nb_pkts > 0 {
					for i in 0..nb_pkts as usize {
						_rte_pktmbuf_free(pkts[i]);
					}
					nb_pkts = _rte_ring_dequeue_burst(
						tx_ring,
						&mut (pkts.as_mut_ptr() as *mut c_void),
						nflib::constants::PACKET_READ_SIZE as u32,
						ptr::null_mut(),
					) as u16;
				}
			}
			None => {
				rte_exit(1, "Missing tx packet mempool\n" as *const _ as *const i8);
			}
		}

		*global_state.nf_msg_pool.borrow_mut() =
			*rte_mempool_lookup(&nflib::constants::_NF_MSG_POOL_NAME[..] as *const _ as *const i8);
		// let a = *global_state.nf_msg_pool.borrow_mut();
		let _msg_q = *(*(*global_state.nfs[nf_id as usize].clone()))
			.msg_q
			.borrow_mut();
		// let _msg_q = unsafe { *((*global_state.nfs.clone()[dest as usize]).msg_q).borrow_mut() };
		match _msg_q {
			Some(msg_q) => {
				let mut m = match ptr::NonNull::new(&mut msg) {
					Some(p) => (p.as_ptr() as *mut _ as *mut c_void),
					None => (ptr::null_mut() as *mut c_void),
				};
				while _rte_ring_dequeue(msg_q, &mut m) == 0 {
					// while _rte_ring_dequeue(msg_q, m) == 0 {
					// while _rte_ring_dequeue(msg_q, &mut (msg as *mut _ as *mut c_void)) == 0 {
					// if let Some(msg_pool) = global_state.nf_msg_pool {
					// 	_rte_mempool_put(msg_pool, m);
					// _rte_mempool_put(global_state.nf_msg_pool, &mut msg as *mut _ as *mut c_void)
					_rte_mempool_put(&mut *global_state.nf_msg_pool.borrow_mut(), m);
					// }
				}
			}
			None => rte_exit(1, "NF Msg pool unavailable" as *const _ as *const i8),
		};

		/* Free info struct */
		/* Lookup mempool for nf struct */
		nf_info_mp =
			rte_mempool_lookup(nflib::constants::_NF_MEMPOOL_NAME as *const _ as *const i8);
		if nf_info_mp.is_null() {
			return Ok(exit_on_failure(
				"Failed to fetch memory for NF info pool\n".into(),
				"In the onvm_nf_stop function",
			)?);
		}

		_rte_mempool_put(nf_info_mp, stop as *mut _ as *mut c_void);

		/* Further cleanup is only required if NF was succesfully started */
		if nf_status != nflib::constants::NF_RUNNING && nf_status != nflib::constants::NF_PAUSED {
			return Ok(());
		}

		/* Decrease the total number of RUNNING NFs */
		*global_state.num_nfs.borrow_mut() -= 1;

		/* Reset stats */
		// onvm_stats_clear_nf(nf_id);
		// TODO: Rest of the function
		// NOTE: Incomplete because I still don't understand the purpose of void **service
		/* Remove this NF from the service map.
		 * Need to shift all elements past it in the array left to avoid gaps */
		*global_state.nf_per_service_count[service_id as usize].borrow_mut() -= 1;
		for map_index in 0..nflib::constants::MAX_NFS_PER_SERVICE as usize {}
	} // end of unsafe block

	Ok(())
}

/// Function to move a NF to another core.
// pub fn onvm_nf_relocate_nf(&global_state::global::GlobalState) {}

/// Function that initializes an LPM object
// pub fn onvm_nf_init_lpm_region(&global_state::global::GlobalState) {}

/// Function that initializes a hashtable for a flow_table struct
// pub fn onvm_nf_init_ft(&global_state::global::GlobalState) {}

/// Set up the DPDK rings which will be used to pass packets, via
/// pointers, between the multi-process server and NF processes.
/// Each NF needs one RX queue.
/// Input: An nf struct
/// Output: rte_exit if failed, none otherwise
fn onvm_nf_init_rings(global_state: &global::GlobalNFState) {}

//******************************Interfaces*****************************/
pub fn onvm_nf_next_instance_id(global_state: &global::GlobalNFState) -> u32 {
	let mut nf: *mut nflib::structs::OnvmNF;
	let mut instance_id: u32;

	if *global_state.num_nfs.borrow() >= nflib::constants::MAX_NFS {
		return nflib::constants::MAX_NFS.into();
	}
	/* Do a first pass for NF IDs bigger than current next_instance_id */
	while *NXT_INSTANCE_ID.borrow() < nflib::constants::MAX_NFS {
		instance_id = *NXT_INSTANCE_ID.borrow();
		*NXT_INSTANCE_ID.borrow_mut() += 1;
		/* Check if this id is occupied by another NF */
		let nf = unsafe { *global_state.nfs.clone()[instance_id as usize] };
		// let nf = *(&global_state.nfs).clone()[instance_id as usize].borrow_mut();
		// let g = &global_state.nfs.clone();
		// let nf = g[instance_id as usize].borrow_mut();
		unsafe {
			if nflib::funcs_macros::onvm_nf_is_valid(&*nf) {
				return instance_id;
			}
		}
	}
	/* This should never happen, means our num_nfs counter is wrong */
	unsafe {
		rte_log(
			RTE_LOG_ERR,
			RTE_LOGTYPE_USER1,
			"Tried to allocated a next instance ID but num_nfs is corrupted" as *const _
				as *const i8,
		)
	};
	nflib::constants::MAX_NFS
}

// struct _MSGS<T: nflib::structs::OnvmMfgTrait> {
// 	data: T,
// }

// FIXME: This fiunction is not simple to code in Rust
// REVIEW: Look at the OnvmMfgTrait
pub fn onvm_nf_check_status(global_state: &global::GlobalNFState) {
	// NOTE: We expect the data in the messages to be either OnvmNF or OnvmNfInitCfg which have been marked as OnvmMfgTrait. So here the msg variable is declared as a OnvmMfgTrait trait object
	// let mut msg: mem::MaybeUninit<Arc<dyn nflib::structs::OnvmMfgTrait>> =
	// mem::MaybeUninit::<Arc<dyn nflib::structs::OnvmMfgTrait>>::uninit();
	// NOTE: A simpler approach is to declare an enum and have a vector of the enums
	// One can't have a vec of trait objects because trait objects are not sized
	let mut msgs =
		Vec::<nflib::structs::OnvmNFMsg>::with_capacity(nflib::constants::MAX_NFS as usize);
	let num_msgs = unsafe { _rte_ring_count(&*global_state.incoming_msg_queue.borrow_mut()) };

	if num_msgs == 0 {
		return;
	}

	unsafe {
		if _rte_ring_dequeue_bulk(
			&mut *global_state.incoming_msg_queue.borrow_mut(),
			&mut (msgs.as_mut_ptr() as *mut _ as *mut c_void),
			num_msgs,
			ptr::null_mut(),
		) == 0
		{
			return;
		}
	}

	for i in 0..num_msgs as usize {
		let msg = &mut msgs[i];
		match msg {
			nflib::structs::OnvmNFMsg::NfStarting(start) => {
				let retval = onvm_nf_start(start, global_state);
				match retval {
					Ok(()) => {
						let f = format!("NF {} Starting\n", start.instance_id);
						unsafe {
							rte_log(
								RTE_LOG_INFO,
								RTE_LOGTYPE_USER1,
								&f[..] as *const _ as *const i8,
							);
						}
					} // successfully started NF
					Err(e) => {
						let f = format!("NF {} failed to start: {:?}\n", start.instance_id, e);
						unsafe {
							rte_log(
								RTE_LOG_INFO,
								RTE_LOGTYPE_USER1,
								&f[..] as *const _ as *const i8,
							);
						}
					} // error in starting NF
				} // inner match
			} // NF starting case
			nflib::structs::OnvmNFMsg::NfReady(ready) => {
				let retval = onvm_nf_ready(*ready, global_state);
				match retval {
					Ok(()) => unsafe {
						let f = format!("NF {} Ready\n", *(*(*ready)).instance_id.borrow());
						rte_log(
							RTE_LOG_INFO,
							RTE_LOGTYPE_USER1,
							&f[..] as *const _ as *const i8,
						);
					}, // successfully registered NF
					Err(e) => unsafe {
						let f = format!(
							"NF {} has a problem: {:?}\n",
							*(*(*ready)).instance_id.borrow(),
							e
						);
						rte_log(
							RTE_LOG_INFO,
							RTE_LOGTYPE_USER1,
							&f[..] as *const _ as *const i8,
						);
					}, // error in getting NF status
				} // inner match
			} // NF ready case
			nflib::structs::OnvmNFMsg::NfStopping(stop) => {
				let retval = onvm_nf_stop(*stop, global_state);
				match retval {
					Ok(()) => unsafe {
						let f = format!("NF {} Stopping\n", *(*(*stop)).instance_id.borrow());
						rte_log(
							RTE_LOG_INFO,
							RTE_LOGTYPE_USER1,
							&f[..] as *const _ as *const i8,
						);
					}, // successfully stopped NF
					Err(e) => unsafe {
						let f = format!(
							"NF {} failed to stop: {:?}\n",
							*(*(*stop)).instance_id.borrow(),
							e
						);
						rte_log(
							RTE_LOG_INFO,
							RTE_LOGTYPE_USER1,
							&f[..] as *const _ as *const i8,
						);
					}, // error stopping NF
				} // inner match
			} // NF Stopping case
		} // msg matching end
		unsafe {
			_rte_mempool_put(
				&mut *global_state.nf_msg_pool.borrow_mut(),
				msgs.as_mut_ptr() as *mut _ as *mut c_void,
			);
		}
	} // end of for loop
}

pub fn onvm_nf_send_msg(
	dest: u16,
	msg: nflib::structs::OnvmNFMsg,
	global_state: &global::GlobalNFState,
) -> i32 {
	let mut msg = Box::from(msg);
	let ret = unsafe {
		_rte_mempool_get(
			&mut *global_state.nf_msg_pool.borrow_mut(),
			&mut (msg.as_mut() as *mut _ as *mut c_void),
		)
	};
	if ret != 0 {
		unsafe {
			rte_log(
				RTE_LOG_INFO,
				RTE_LOGTYPE_USER1,
				"Oh the huge manatee! Unable to allocate msg from pool" as *const _ as *const i8,
			)
		};
		return ret;
	}
	unsafe {
		let _msg_q = *(*(*global_state.nfs[dest as usize].clone()))
			.msg_q
			.borrow_mut();
		// let _msg_q = unsafe { *((*global_state.nfs.clone()[dest as usize]).msg_q).borrow_mut() };
		match _msg_q {
			Some(msg_q) => {
				// this is what we want to happen
				return _rte_ring_enqueue(msg_q, msg.as_mut() as *mut _ as *mut c_void);
			}
			None => rte_exit(1, "NF Msg pool unavailable" as *const _ as *const i8),
		};
	}
	1_i32 // it gets here only if it did not hit the return statements
}
