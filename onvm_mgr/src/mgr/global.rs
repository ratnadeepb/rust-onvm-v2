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
	rte_eth_txmode,
};
// DPDK constants
use capsule_ffi::{
	DEV_RX_OFFLOAD_IPV4_CKSUM, DEV_RX_OFFLOAD_TCP_CKSUM, DEV_RX_OFFLOAD_UDP_CKSUM,
	DEV_TX_OFFLOAD_IPV4_CKSUM, DEV_TX_OFFLOAD_TCP_CKSUM, DEV_TX_OFFLOAD_UDP_CKSUM,
	ETH_MQ_RX_RSS_FLAG, ETH_RSS_IP, ETH_RSS_L2_PAYLOAD, ETH_RSS_TCP, ETH_RSS_UDP,
	RTE_ETHER_MAX_LEN,
};
use fragile::Fragile;
use std::ffi::c_void;
use std::ptr;
use std::sync::{Arc, RwLock};

/* the struct denoting the global state */
pub struct GlobalState {
	// REVIEW: is the type correct? Do they need to be thread-safe (RwLock)?
	// REVIEW: Still debating if global state fields should be Arc<RwLock<_>> or not. A speed vs safety debate.
	// NOTE: the lifetime is static since we expect the global state to last throughout the program
	incoming_msg_queue: *mut rte_ring,
	pktmbuf_pool: *mut rte_ring,
	nf_msg_pool: *mut rte_ring,
	services: Vec<*mut c_void>,
	nf_per_service_count: *mut c_void,
	num_sockets: RwLock<u16>,
	default_chain: RwLock<nflib::structs::OnvmServiceChain>,
	onvm_config: nflib::structs::OnvmConfiguration,
	nfs: Vec<&'static mut nflib::structs::OnvmNF>,
	ports: Vec<&'static nflib::structs::PortInfo>,
	cores: Vec<&'static nflib::structs::CoreStatus>,
	port_conf: rte_eth_conf,
}

impl Default for GlobalState {
	fn default() -> Self {
		Self {
			incoming_msg_queue: ptr::null_mut(),
			pktmbuf_pool: ptr::null_mut(),
			nf_msg_pool: ptr::null_mut(),
			services: vec![],
			nf_per_service_count: ptr::null_mut(),
			num_sockets: RwLock::new(0),
			default_chain: RwLock::new(Default::default()),
			onvm_config: Default::default(),
			nfs: Vec::with_capacity(nflib::constants::MAX_NFS.into()),
			ports: vec![],
			cores: vec![],
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
