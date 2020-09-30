/*
 * Created on Tue Sep 29 2020:20:05:42
 * Created by Ratnadeep Bhattacharya
 */
// import constants from submodules
use crate::get_args::{
	DEFAULT_SERVICE, GLOBAL_PKT_LIMIT, GLOBAL_TIME_TO_LIVE, GLOBAL_VERBOSITY_LEVEL, NUM_NFS,
	NUM_SERVICES, ONVM_NF_SHARE_CORES,
};
use crate::nflib;
use std::mem;
use capsule_ffi::rte_mbuf;

/* Manager constants */
const MBUF_CACHE_SIZE: usize = 512;
const MBUF_OVERHEAD: usize = mem::size_of::<rte_mbuf>() + mem::size_of::<u32>(); // RTE_PKTMBUF_HEADROOM is of type u32
const RX_MBUF_DATA_SIZE: usize = 2048;
const MBUF_SIZE: usize = RX_MBUF_DATA_SIZE + MBUF_OVERHEAD;
const NF_INFO_SIZE: usize = mem::size_of::<nflib::structs::OnvmNfInitCfg>();
const NF_MSG_SIZE: usize = mem::size_of::<nflib::structs::OnvmNfMsg>();
const NF_MSG_CACHE_SIZE: u8 = 8;
const RTE_MP_RX_DESC_DEFAULT: u16 = 512;
const RTE_MP_TX_DESC_DEFAULT: u16 = 512;
const NF_MSG_QUEUE_SIZE: u8 = 128;
const NO_FLAGS: u8 = 0;
const ONVM_NUM_RX_THREADS: u8 = 1;
// Number of auxiliary threads in manager, 1 reserved for stats
const ONVM_NUM_MGR_AUX_THREADS: u8 = 1;
const ONVM_NUM_WAKEUP_THREADS: u8 = 1; // Enabled when using shared core mode

/// RX and TX Prefetch, Host, and Write-back threshold values should be carefully set for optimal performance. Consult the network controller's datasheet and supporting DPDK documentation for guidance on how these parameters should be set.
const RX_PTHRESH: usize = 8; // Default values of RX prefetch threshold reg
const RX_HTHRESH: usize = 8; // Default values of RX host threshold reg
const RX_WTHRESH: usize = 4; // Default values of RX write-back threshold reg

/// These default values are optimized for use with the Intel(R) 82599 10 GbE Controller and the DPDK ixgbe PMD. Consider using other values for other network controllers and/or network drivers.
const TX_PTHRESH: usize = 36; // Default values of TX prefetch threshold reg
const TX_HTHRESH: usize = 0; // Default values of TX host threshold reg
const TX_WTHRESH: usize = 0; // Default values of TX write-back threshold reg
