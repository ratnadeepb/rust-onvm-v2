/*
 * Created on Tue Sep 29 2020:20:19:39
 * Created by Ratnadeep Bhattacharya
 */

use super::constants;
use super::global;
use crate::error_handling::exit_on_failure;
use crate::nflib;
use exitfailure::ExitFailure;
use failure;
use fragile::Fragile;
use std::ffi::c_void;
use std::os::raw::{c_char, c_int};

// DPDK structures
use capsule_ffi::{
	rte_eth_conf, rte_eth_conf__bindgen_ty_1, rte_eth_rss_conf, rte_eth_rxmode, rte_eth_txmode,
	rte_mbuf, rte_mempool, rte_memzone, rte_ring,
};
// DPDK functions
use capsule_ffi::{
	rte_calloc, rte_eal_init, rte_eth_dev_count_avail, rte_exit, rte_memzone_reserve, rte_socket_id,
};
// DPDK constants
use capsule_ffi::{
	rte_eth_tx_mq_mode, DEV_RX_OFFLOAD_IPV4_CKSUM, DEV_RX_OFFLOAD_TCP_CKSUM,
	DEV_RX_OFFLOAD_UDP_CKSUM, DEV_TX_OFFLOAD_IPV4_CKSUM, DEV_TX_OFFLOAD_TCP_CKSUM,
	DEV_TX_OFFLOAD_UDP_CKSUM, ETH_MQ_RX_RSS_FLAG, ETH_RSS_IP, ETH_RSS_L2_PAYLOAD, ETH_RSS_TCP,
	ETH_RSS_UDP, RTE_ETHER_MAX_LEN,
};

// Initialise the default onvm config structure
fn set_default_config(config: &mut nflib::structs::OnvmConfiguration) {
	match nflib::constants::ONVM_NF_SHARE_CORES_DEFAULT {
		true => config.set_flag(0),
		false => config.set_flag(1),
	};
}

/// Start the OpenNetVM manager
pub fn init(mut argc: c_int, mut argv: *mut *mut c_char) -> Result<(), ExitFailure> {
	// the entire global state struct is wrapped inside fragile
	// REVIEW: Do they need to be thread-safe (Fragile)?
	println!("Inside init"); // DEBUG
	let global_state: Fragile<global::GlobalState> = Fragile::default();

	let retval: i32;
	let mz_nf: rte_memzone;
	let mz_port: rte_memzone;
	let mz_cores: rte_memzone;
	let mz_scp: rte_memzone;
	let mz_services: rte_memzone;
	let mz_nf_per_service: rte_memzone;
	let mz_onvm_config: rte_memzone;
	let total_ports: u16;
	let port_id: u8;
	let i: u8;

	unsafe {
		println!("Inside init: argc = {} and argv = {:?}", &argc, &*(*argv)); // DEBUG
		println!("Inside init unsafe block"); // DEBUG
		retval = rte_eal_init(argc, argv);
		println!("return from \"rte_eal_init\":{}", retval); // DEBUG
		if retval < 0 {
			return Ok(exit_on_failure("EAL failed", "In the init function")?);
		}
		
	}
	Ok(())
}
