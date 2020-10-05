/*
 * Created on Tue Sep 29 2020:20:35:51
 * Created by Ratnadeep Bhattacharya
 */

use crate::nflib;
// DPDK functions
use capsule_ffi::{rte_eth_conf, rte_ring};
// DPDK structs
use capsule_ffi::{
	rte_eth_conf__bindgen_ty_1, rte_eth_rss_conf, rte_eth_rxmode, rte_eth_tx_mq_mode,
	rte_eth_txmode, rte_mempool,
};
// DPDK constants
use capsule_ffi::{
	DEV_RX_OFFLOAD_IPV4_CKSUM, DEV_RX_OFFLOAD_TCP_CKSUM, DEV_RX_OFFLOAD_UDP_CKSUM,
	DEV_TX_OFFLOAD_IPV4_CKSUM, DEV_TX_OFFLOAD_TCP_CKSUM, DEV_TX_OFFLOAD_UDP_CKSUM,
	ETH_MQ_RX_RSS_FLAG, ETH_RSS_IP, ETH_RSS_L2_PAYLOAD, ETH_RSS_TCP, ETH_RSS_UDP,
	RTE_ETHER_MAX_LEN,
};
use fragile::Fragile;
use std::cell::RefCell;
use std::ffi::c_void;
use std::ptr;
use std::sync::{Arc, RwLock};

/* the struct denoting the global state */
pub struct GlobalNFState {
	// REVIEW: is the type correct? Do they need to be thread-safe (RwLock)?
	// REVIEW: Still debating if global state fields should be Arc<RwLock<_>> or not. A speed vs safety debate.
	// NOTE: the lifetime is static since we expect the global state to last throughout the program
	pub incoming_msg_queue: RefCell<*mut rte_ring>,
	pub pktmbuf_pool: RefCell<*mut rte_mempool>,
	pub nf_msg_pool: RefCell<*mut rte_mempool>,
	pub nf_init_cfg_pool: RefCell<*mut rte_mempool>,
	pub services: Vec<RefCell<*mut c_void>>,
	pub nf_per_service_count: Vec<RefCell<u32>>,
	pub num_sockets: RwLock<u16>,
	pub default_chain: RwLock<nflib::structs::OnvmServiceChain>,
	pub onvm_config: Arc<nflib::structs::OnvmConfiguration>,
	pub nfs: Vec<Arc<*mut nflib::structs::OnvmNF>>, // Arc<Vec<RefCell<&'static mut nflib::structs::OnvmNF>>>,
	// pub ports: Vec<&'static nflib::structs::PortInfo>,
	pub ports: Arc<nflib::structs::PortInfo>,
	pub cores: Vec<Arc<*mut nflib::structs::CoreStatus>>,
	pub num_services: RefCell<u8>,
	// pub global_stats_sleep_time: u8, // also used to run the main thread of onvm
	// pub global_verbosity_level: u8,
	// pub global_pkt_limit: u8,
	// pub global_time_to_live: u8,
	pub num_nfs: RefCell<u32>,
	pub default_service: u16,
	pub default_service_id: u16,
	pub onvm_nf_share_cores: bool,
	pub port_conf: rte_eth_conf,
}

impl Default for GlobalNFState {
	fn default() -> Self {
		GlobalNFState {
			incoming_msg_queue: RefCell::new(ptr::null_mut()),
			pktmbuf_pool: RefCell::new(ptr::null_mut()),
			nf_msg_pool: RefCell::new(ptr::null_mut()),
			// nf_msg_pool: Some(ptr::null_mut()),
			nf_init_cfg_pool: RefCell::new(ptr::null_mut()),
			services: vec![],
			nf_per_service_count: vec![],
			num_sockets: RwLock::new(0),
			default_chain: RwLock::new(Default::default()),
			onvm_config: unsafe {
				Arc::from_raw(ptr::null_mut() as *mut nflib::structs::OnvmConfiguration)
			},
			nfs: Vec::with_capacity(nflib::constants::MAX_NFS as usize),
			ports: unsafe { Arc::from_raw(ptr::null_mut() as *mut nflib::structs::PortInfo) },
			// cores: Arc::new(vec![]),
			cores: vec![],
			num_services: RefCell::new(nflib::constants::MAX_SERVICES),
			// global_stats_sleep_time: 1,
			// global_verbosity_level: 0,
			// global_pkt_limit: 0,
			// global_time_to_live: 0,
			num_nfs: RefCell::new(0),
			default_service: 0,
			default_service_id: 0,
			onvm_nf_share_cores: false,
			port_conf: rte_eth_conf {
				rxmode: rte_eth_rxmode {
					mq_mode: ETH_MQ_RX_RSS_FLAG,
					max_rx_pkt_len: RTE_ETHER_MAX_LEN,
					split_hdr_size: 0,
					offloads: (DEV_RX_OFFLOAD_IPV4_CKSUM
						| DEV_RX_OFFLOAD_UDP_CKSUM
						| DEV_RX_OFFLOAD_TCP_CKSUM) as u64,
					..Default::default()
				},
				txmode: rte_eth_txmode {
					mq_mode: rte_eth_tx_mq_mode::ETH_MQ_TX_NONE,
					offloads: (DEV_TX_OFFLOAD_IPV4_CKSUM
						| DEV_TX_OFFLOAD_UDP_CKSUM
						| DEV_TX_OFFLOAD_TCP_CKSUM) as u64,
					..Default::default()
				},
				rx_adv_conf: rte_eth_conf__bindgen_ty_1 {
					rss_conf: rte_eth_rss_conf {
						rss_key: nflib::constants::RSS_SYMMETRIC_KEY.get_mut() as *mut _ as *mut u8,
						// rss_key: rss_symmetric_key.as_mut_ptr(),
						rss_hf: (ETH_RSS_IP | ETH_RSS_UDP | ETH_RSS_TCP | ETH_RSS_L2_PAYLOAD)
							as u64,
						..Default::default()
					},
					..Default::default()
				},
				..Default::default()
			},
		}
	}
}

unsafe impl std::marker::Sync for GlobalNFState {}
unsafe impl std::marker::Send for GlobalNFState {}
