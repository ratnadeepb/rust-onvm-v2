/*
 * Created on Fri Sep 25 2020:00:24:57
 * Created by Ratnadeep Bhattacharya
 */

use crate::{common, msg_common};
// structures
use capsule_ffi::{
	rte_eth_conf, rte_eth_conf__bindgen_ty_1, rte_eth_rss_conf, rte_eth_rxmode, rte_eth_txmode,
	rte_mbuf, rte_mempool, rte_memzone, rte_ring,
};
// functions
use capsule_ffi::{
	rte_calloc, rte_eal_init, rte_eth_dev_count_avail, rte_exit, rte_memzone_reserve, rte_socket_id,
};
// constants
use capsule_ffi::{
	rte_eth_tx_mq_mode, DEV_RX_OFFLOAD_IPV4_CKSUM, DEV_RX_OFFLOAD_TCP_CKSUM,
	DEV_RX_OFFLOAD_UDP_CKSUM, DEV_TX_OFFLOAD_IPV4_CKSUM, DEV_TX_OFFLOAD_TCP_CKSUM,
	DEV_TX_OFFLOAD_UDP_CKSUM, ETH_MQ_RX_RSS_FLAG, ETH_RSS_IP, ETH_RSS_L2_PAYLOAD, ETH_RSS_TCP,
	ETH_RSS_UDP, RTE_ETHER_MAX_LEN,
};
// use std::cell::RefCell;
use exitfailure::ExitFailure;
use failure;
use std::ffi::c_void;
use std::os::raw::{c_char, c_int};
use std::sync::{Arc, Mutex, RwLock};
use std::{mem, ptr};

const MBUF_CACHE_SIZE: usize = 512;
const MBUF_OVERHEAD: usize = mem::size_of::<rte_mbuf>() + mem::size_of::<u32>(); // RTE_PKTMBUF_HEADROOM is of type u32
const RX_MBUF_DATA_SIZE: usize = 2048;
const MBUF_SIZE: usize = RX_MBUF_DATA_SIZE + MBUF_OVERHEAD;
const NF_INFO_SIZE: usize = mem::size_of::<common::OnvmNfInitCfg>();
const NF_MSG_SIZE: usize = mem::size_of::<msg_common::OnvmNfMsg>();
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

// FIXME: Raw pointers cannot be shared across threads - rte_ring contains *const rte_memzone
// So:
// 		option 1 (safe but limited): use thread local static variables and run mgr in a single thread
//		option 2 (safe but valid only within the init function unless ownership is transferred to main): define global variables inside init as Arc<Mutex<>>
//		option 3 (unsafe? but can remain in global scope): wrap structs that contain a raw pointer like rte_ring and implement std::marker::sync and std::marker::send for the wrapper structs

// TODO:
// 1. OnvmFT type SDN_FT needs to be declared when and if sdn is implemented
// 2. ONVM_STATS_OUTPUT type stats_destination when onvm_stats has been implemented
// thread_local!(
// 	/// NF to Manager data flow
// 	static INCOMING_MSG_QUEUE: RefCell<Option<rte_ring>> = RefCell::new(None);
// 	/// the shared port information: port numbers, rx and tx stats etc
// 	static PORTS: RefCell<Option<common::PortInfo>> = RefCell::new(None);

// 	static CORES: RefCell<Option<common::CoreStatus>> = RefCell::new(None);
// 	static PKTMBUF_POOL: RefCell<Option<rte_mempool>> = RefCell::new(None);

// 	static NF_MSG_POOL: RefCell<Option<rte_mempool>> = RefCell::new(None);

// 	static NUM_NFS: RefCell<u16> = RefCell::new(0);

// 	static NUM_SERVICES: RefCell<u16> = RefCell::new(0);

// 	static DEFAULT_SERVICES: RefCell<u16> = RefCell::new(0);

// 	// virtual memory address for each service
// 	static SERVICES: RefCell<Vec<*mut c_void>> = RefCell::new(vec![]);

// 	// virtual address per service of the nf
// 	static NF_PER_SERVICE_COUNT: RefCell<*mut c_void>> = RefCell::new(Default::default());

// 	static NUM_SOCETS: RefCell<u8> = RefCell::new(0);

// 	static DEFAULT_CHAIN: RefCell<Option<common::OnvmServiceChain>> = RefCell::new(None);

// 	static GLOBAL_STATS_SLEEP_TIME: RefCell<u16> = RefCell::new(0);

// 	static GLOBAL_TIME_TO_LIVE: RefCell<u16> = RefCell::new(0);

// 	static GLOBAL_PKT_LIMIT: RefCell<u32> = RefCell::new(0);

// 	static GLOBAL_VERBOSITY_LEVEL: RefCell<u8> = RefCell::new(0);
// );

// Initialise the default onvm config structure
fn set_default_config(config: &mut common::OnvmConfiguration) {
	match common::ONVM_NF_SHARE_CORES_DEFAULT {
		true => config.set_flag(0),
		false => config.set_flag(1),
	};
}

// Show exit error
fn exit_on_failure(msg: &'static str, context: &str) -> Result<(), failure::Error> {
	let err = failure::err_msg(msg);
	Ok(Err(err.context(context.to_string()))?)
}

/// Start the OpenNetVM manager
fn init(mut argc: c_int, mut argv: *mut *mut c_char) -> Result<(), ExitFailure> {
	// NF to Manager data flow
	let incoming_msg_queue: Arc<RwLock<rte_ring>> = Arc::new(RwLock::new(Default::default()));

	// the shared port information: port numbers, rx and tx stats etc
	let ports: Arc<RwLock<common::PortInfo>> = Arc::new(RwLock::new(Default::default()));
	let cores: Arc<RwLock<common::CoreStatus>> = Arc::new(RwLock::new(Default::default()));
	let pktmbuf_pool: Arc<RwLock<rte_mempool>> = Arc::new(RwLock::new(Default::default()));
	let nf_msg_pool: Arc<RwLock<rte_mempool>> = Arc::new(RwLock::new(Default::default()));
	let num_nfs: Arc<RwLock<u16>> = Arc::new(RwLock::new(Default::default()));
	let num_services: Arc<RwLock<u16>> = Arc::new(RwLock::new(Default::default()));
	let default_services: Arc<RwLock<u16>> = Arc::new(RwLock::new(Default::default()));
	let services: Arc<RwLock<Vec<*mut c_void>>> = Arc::new(RwLock::new(vec![]));
	let nf_per_service_count: Arc<RwLock<*mut c_void>> = Arc::new(RwLock::new(unsafe {
		mem::MaybeUninit::<*mut c_void>::uninit().assume_init()
	}));
	let num_sockets: Arc<RwLock<u8>> = Arc::new(RwLock::new(0));
	let default_chain: Arc<RwLock<common::OnvmServiceChain>> =
		Arc::new(RwLock::new(Default::default()));
	let nfs: Arc<RwLock<common::OnvmNF>> = Arc::new(RwLock::new(Default::default()));
	let nf_init_cfg_pool: Arc<RwLock<rte_mempool>> = Arc::new(RwLock::new(Default::default()));
	// extern struct onvm_ft *sdn_ft;
	// extern ONVM_STATS_OUTPUT stats_destination;
	let global_stats_sleep_time: Arc<RwLock<u16>> = Arc::new(RwLock::new(Default::default()));
	let global_time_to_live: Arc<RwLock<u32>> = Arc::new(RwLock::new(Default::default()));
	let global_pkt_limit: Arc<RwLock<u32>> = Arc::new(RwLock::new(Default::default()));
	let global_verbosity_level: Arc<RwLock<u8>> = Arc::new(RwLock::new(Default::default()));

	// Custom flags for onvm
	let onvm_config: Arc<RwLock<common::OnvmConfiguration>> =
		Arc::new(RwLock::new(Default::default()));
	let onvm_nf_share_cores: Arc<RwLock<u8>> = Arc::new(RwLock::new(0));

	// For handling shared core logic
	// let nf_wakeup_infos: Arc<RwLock<common::OnvMNfWakeupInfo>> = Arc::new(RwLock::new(Default::default()));

	// let rss_symmetric_key: [u8; 40] = [
	// 	0x6d, 0x5a, 0x6d, 0x5a, 0x6d, 0x5a, 0x6d, 0x5a, 0x6d, 0x5a, 0x6d, 0x5a, 0x6d, 0x5a, 0x6d,
	// 	0x5a, 0x6d, 0x5a, 0x6d, 0x5a, 0x6d, 0x5a, 0x6d, 0x5a, 0x6d, 0x5a, 0x6d, 0x5a, 0x6d, 0x5a,
	// 	0x6d, 0x5a, 0x6d, 0x5a, 0x6d, 0x5a, 0x6d, 0x5a, 0x6d, 0x5a,
	// ];

	let port_conf: rte_eth_conf = rte_eth_conf {
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
				rss_key: common::RSS_SYMMETRIC_KEY.get_mut() as *mut _ as *mut u8,
				// rss_key: rss_symmetric_key.as_mut_ptr(),
				rss_hf: (ETH_RSS_IP | ETH_RSS_UDP | ETH_RSS_TCP | ETH_RSS_L2_PAYLOAD) as u64,
				..Default::default()
			},
			..Default::default()
		},
		..Default::default()
	};

	let retval: i32;
	let mz_nf: Arc<RwLock<rte_memzone>>;
	let mz_port: Arc<RwLock<rte_memzone>>;
	let mz_cores: Arc<RwLock<rte_memzone>>;
	let mz_scp: Arc<RwLock<rte_memzone>>;
	let mz_services: Arc<RwLock<rte_memzone>>;
	let mz_nf_per_service: Arc<RwLock<rte_memzone>>;
	let mz_onvm_config: Arc<RwLock<rte_memzone>>;
	let total_ports: Arc<RwLock<u16>>;
	let port_id: Arc<RwLock<u8>>;
	let i: u8;

	// init EAL, parsing EAL args
	unsafe {
		retval = rte_eal_init(argc, argv);
	}
	if retval < 0 {
		return Ok(exit_on_failure("EAL failed", "In the init function")?);
	}
	argc -= retval;
	unsafe {
		argv = argv.offset(retval as isize);
		// get total number of ports
		total_ports = Arc::new(RwLock::new(rte_eth_dev_count_avail()));
		// set up array for NF tx data
		// rte_memzone_reserve() returns a `*const capsule-ffi::rte_memzone` ptr, which is held in `m_n`
		let m_n = rte_memzone_reserve(
			common::MZ_NF_INFO as *const _ as *const c_char,
			(mem::size_of::<common::OnvmNF>() as u64) * common::MAX_NFS as u64,
			rte_socket_id() as i32,
			common::NO_FLAGS,
		);
		// `m_n` is a raw pointer so its validity need to be tested
		if m_n.is_null() {
			rte_exit(
				1,
				"Cannot reserve memory zone for nf information\n" as *const _ as *const i8,
			);
		}
		// this convoluted line is the equivalent of C `memset(0)` - zero out the memory in the `addr` field of `rte_memzone`
		(*(m_n as *mut rte_memzone)).__bindgen_anon_2.addr = mem::zeroed();
		// assign the contents of the memzone held by `m_n` to `mz_nf`
		// encapsulates the memzone on a RwLock inside an atomically reference counter memory block
		// mz_nf can be easily, efficiently and safely shared between threads
		mz_nf = Arc::new(RwLock::new(*m_n));
		{
			// get a write lock on `OnvmNF nfs`
			let mut nfs_w = &*nfs.write().unwrap();
			// get a read lock on `mz_nf`
			let mz_nf_r = *mz_nf.read().unwrap();
			// forcefully, cast the addr field of mz_nf as an OnvmNf
			// we assume this operation is valid since it is valid in the equivalent C code
			// https://github.com/sdnfv/openNetVM/blob/27d9ed5d06ebcc987f36f8860b1935aaddc8cf1c/onvm/onvm_mgr/onvm_init.c#L165
			nfs_w = &*(mz_nf_r.__bindgen_anon_2.addr as *mut common::OnvmNF);
		} // locks are dropped at end of scope

		// set up ports info
		let m_p = rte_memzone_reserve(
			common::MZ_PORT_INFO as *const _ as *const c_char,
			mem::size_of::<common::PortInfo>() as u64,
			rte_socket_id() as i32,
			common::NO_FLAGS,
		);
		if m_p.is_null() {
			rte_exit(
				1,
				"Cannot reserve memory zone for port information\n" as *const _ as *const i8,
			);
		}
		mz_port = Arc::new(RwLock::new(*m_p));
		{
			let mut ports_w = &*ports.write().unwrap();
			let mz_port_r = *mz_port.read().unwrap();
			ports_w = &*(mz_port_r.__bindgen_anon_2.addr as *mut common::PortInfo);
		} // locks are dropped

		// set up core status
		let m_c = rte_memzone_reserve(
			common::MZ_CORES_STATUS as *const _ as *const c_char,
			mem::size_of::<common::CoreStatus>() as u64,
			rte_socket_id() as i32,
			common::NO_FLAGS,
		);
		if m_c.is_null() {
			rte_exit(
				1,
				"Cannot reserve memory zone for core information\n" as *const _ as *const i8,
			);
		}
		mz_cores = Arc::new(RwLock::new(*m_p));
		{
			let mut cores_w = &*cores.write().unwrap();
			let mz_cores_r = *mz_cores.read().unwrap();
			cores_w = &*(mz_cores_r.__bindgen_anon_2.addr as *mut common::CoreStatus);
		} // locks are dropped

		// set up array for NF tx data
		let m_s = rte_memzone_reserve(
			common::MZ_SERVICES_INFO as *const _ as *const c_char,
			(mem::size_of::<u16>() as u64) * (*num_services.read().unwrap() as u64),
			rte_socket_id() as i32,
			common::NO_FLAGS,
		);
		if m_s.is_null() {
			rte_exit(
				1,
				"Cannot reserve memory zone for services information\n" as *const _ as *const i8,
			);
		}
		mz_services = Arc::new(RwLock::new(*m_s));
		{
			let mut nf_w = &*services.write().unwrap();
			let mz_cores_r = *mz_services.read().unwrap();
			nf_w = &*(mz_cores_r.__bindgen_anon_2.addr as *mut Vec<*mut c_void>);
		} // locks are dropped

		let services = &mut *services.write().unwrap();
		for i in 0..(*num_services.read().unwrap()) {
			services[i as usize] = rte_calloc(
				"one service NFs" as *const _ as *const i8,
				common::MAX_NFS_PER_SERVICE as u64,
				mem::size_of::<u16>() as u64,
				0,
			);
		}
		let m_s = rte_memzone_reserve(
			common::MZ_NF_PER_SERVICE_INFO as *const _ as *const c_char,
			(mem::size_of::<u16>() as u64) * (*num_services.read().unwrap() as u64),
			rte_socket_id() as i32,
			common::NO_FLAGS,
		);
		if m_s.is_null() {
			rte_exit(
				1,
				"Cannot reserve memory zone for NF per service information.\n" as *const _
					as *const i8,
			);
		}
		mz_nf_per_service = Arc::new(RwLock::new(*m_s));
		{
			let mut nf_w = &*nf_per_service_count.write().unwrap();
			let mz_cores_r = *mz_nf_per_service.read().unwrap();
			nf_w = &mz_cores_r.__bindgen_anon_2.addr;
		} // locks are dropped

		// set up custom flags
		let m_f = rte_memzone_reserve(
			common::MZ_ONVM_CONFIG as *const _ as *const c_char,
			mem::size_of::<u16>() as u64,
			rte_socket_id() as i32,
			common::NO_FLAGS,
		);
		if m_f.is_null() {
			rte_exit(
				1,
				"Cannot reserve memory zone for ONVM custom flags.\n" as *const _ as *const i8,
			);
		}
		mz_onvm_config = Arc::new(RwLock::new(*m_f));
		{
			// mut reference is required by set_default_config
			let mut nf_cfg_w = &mut *onvm_config.write().unwrap();
			let mz_conf_r = *mz_onvm_config.read().unwrap();
			// since `nf_cfg_w` was mut we need this reference to be mut too
			nf_cfg_w =
				&mut *(mz_conf_r.__bindgen_anon_2.addr as *mut _ as *mut common::OnvmConfiguration);
			set_default_config(nf_cfg_w);
		} // locks dropped

		// parse additional, application arguments
	}
	Ok(())
}
