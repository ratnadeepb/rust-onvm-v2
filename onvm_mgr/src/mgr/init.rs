/*
 * Created on Tue Sep 29 2020:20:19:39
 * Created by Ratnadeep Bhattacharya
 */

use super::{constants, get_args, global};
use crate::error_handling::exit_on_failure;
use crate::nflib;
use exitfailure::ExitFailure;
use failure;
use fragile::Fragile;
use libc::fflush;
use num_cpus;
use std::ffi::{c_void, CString};
use std::os::raw::{c_char, c_int};
// use std::rc::Rc;
use std::cell::RefCell;
use std::sync::Arc;
use std::{mem, ptr};
// NOTE: don't depend on the actual values of ENOTSUP and ENODEV. These two are required in the init_port function
use libc::{EINVAL, EIO, ENODEV, ENOMEM, ENOTSUP};

// DPDK structures
use capsule_ffi::{
	rte_eth_conf, rte_eth_conf__bindgen_ty_1, rte_eth_dev_info, rte_eth_link, rte_eth_rss_conf,
	rte_eth_rxconf, rte_eth_rxmode, rte_eth_txconf, rte_eth_txmode, rte_ether_addr, rte_mbuf,
	rte_mempool, rte_memzone, rte_pktmbuf_pool_private, rte_ring,
};
// DPDK functions
use capsule_ffi::{
	_rte_errno, rte_calloc, rte_delay_us_sleep, rte_eal_init, rte_eth_dev_adjust_nb_rx_tx_desc,
	rte_eth_dev_configure, rte_eth_dev_count_avail, rte_eth_dev_info_get, rte_eth_dev_socket_id,
	rte_eth_dev_start, rte_eth_link_get_nowait, rte_eth_macaddr_get, rte_eth_promiscuous_enable,
	rte_eth_rx_queue_setup, rte_eth_tx_queue_setup, rte_exit, rte_lcore_count, rte_mempool_create,
	rte_memzone_reserve, rte_pktmbuf_init, rte_pktmbuf_pool_init, rte_ring_create, rte_socket_id,
	rte_strerror,
};
// DPDK constants
use capsule_ffi::{
	rte_eth_tx_mq_mode, DEV_RX_OFFLOAD_IPV4_CKSUM, DEV_RX_OFFLOAD_TCP_CKSUM,
	DEV_RX_OFFLOAD_UDP_CKSUM, DEV_TX_OFFLOAD_IPV4_CKSUM, DEV_TX_OFFLOAD_MBUF_FAST_FREE,
	DEV_TX_OFFLOAD_TCP_CKSUM, DEV_TX_OFFLOAD_UDP_CKSUM, ETH_LINK_FULL_DUPLEX, ETH_MQ_RX_RSS_FLAG,
	ETH_RSS_IP, ETH_RSS_L2_PAYLOAD, ETH_RSS_TCP, ETH_RSS_UDP, RTE_ETHER_MAX_LEN,
};

/// Start the OpenNetVM manager
pub fn init(mut args: Vec<String>) -> Result<(), ExitFailure> {
	// the entire global state struct is wrapped inside fragile
	// REVIEW: Do they need to be thread-safe (Fragile)?
	// NOTE: Fragile marker is taken out because GlobalState is now marked as Sync
	println!("Inside init"); // DEBUG
	let mut global_state: global::GlobalNFState = Default::default();

	let retval: i32;
	let mut mz_nf: rte_memzone;
	let mut mz_port: rte_memzone;
	let mut mz_cores: rte_memzone;
	let mut mz_scp: rte_memzone;
	let mut mz_services: rte_memzone;
	let mut mz_nf_per_service: rte_memzone;
	let mut mz_onvm_config: rte_memzone;
	let total_ports: u16;
	let mut port_id: u8;
	let i: u8;

	unsafe {
		// println!("Inside init: argc = {} and argv = {:?}", &argc, &*(*argv)); // DEBUG
		println!("Inside init unsafe block");
		// DEBUG
		// let mut _v: Vec<*mut i8> = args
		// 	.iter_mut()
		// 	.map(|arg| CString::new(&arg[..]).unwrap().into_raw())
		// 	.collect();
		// let mut argc = args.len() as c_int;
		// _v.shrink_to_fit();
		// let mut argv = _v.as_mut_ptr();
		// mem::forget(_v);
		let len = args.len() as c_int;
		let args = args
			.into_iter()
			.map(|s| CString::new(s))
			.collect::<Vec<_>>();
		let mut ptrs = args
			.iter()
			.map(|_s| {
				if let Ok(s) = _s {
					s.as_ptr() as *mut c_char
				} else {
					"".as_ptr() as *mut c_char // We expect this to never run
				}
			})
			.collect::<Vec<_>>();
		// let mut ptrs = args
		// 	.iter()
		// 	.map(|s| s.as_ptr() as *mut c_char)
		// 	.collect::<Vec<_>>();
		let mut argc = len;
		let mut argv = ptrs.as_mut_ptr();
		retval = rte_eal_init(argc, argv);
		println!("return from \"rte_eal_init\": {}", retval); // DEBUG
		if retval != 0 {
			rte_exit(1, "EAL init failed\n\n" as *const _ as *const i8);
			return Ok(exit_on_failure(
				"EAL failed\n".into(),
				"In the init function\n",
			)?);
		}
		argc -= retval;
		// let cs = CString::from_raw(*argv);
		// NOTE: Generally this style of pointer arithmatic looks like a bad idea in Rust, but for char (1 byte) array, this is probably okay
		argv = (argv as usize + retval as usize) as *mut *mut i8;

		/* get total number of ports */
		total_ports = rte_eth_dev_count_avail();

		/* set up array for NF tx data */
		let mut tmp = rte_memzone_reserve(
			nflib::constants::MZ_NF_INFO as *const _ as *const i8,
			(mem::size_of::<nflib::structs::OnvmNF>() * nflib::constants::MAX_NFS as usize) as u64,
			rte_socket_id() as i32,
			constants::NO_FLAGS.into(),
		);
		if tmp.is_null() {
			rte_exit(
				1,
				"Cannot reserve memory zone for nf information\n" as *const _ as *const i8,
			);
		}
		mz_nf = *tmp;
		mz_nf.__bindgen_anon_2.addr = mem::zeroed();
		// convert this memory chunk into a vector of OnvmNF referenes
		let mut addr = mz_nf.__bindgen_anon_2.addr as usize;
		let mut v_tmp = vec![];
		let sz = mem::size_of::<nflib::structs::OnvmNF>();
		for i in 0..nflib::constants::MAX_NFS {
			// v_tmp.push(RefCell::from(&mut *(addr as *mut nflib::structs::OnvmNF)));
			v_tmp.push(Arc::new(addr as *mut nflib::structs::OnvmNF));
			addr += sz;
		}
		global_state.nfs = v_tmp;

		/* set up ports info */
		tmp = rte_memzone_reserve(
			nflib::constants::MZ_PORT_INFO as *const _ as *const i8,
			mem::size_of::<nflib::structs::PortInfo>() as u64,
			rte_socket_id() as i32,
			constants::NO_FLAGS.into(),
		);
		if tmp.is_null() {
			rte_exit(
				1,
				"Cannot reserve memory zone for port information\n" as *const _ as *const i8,
			);
		}
		mz_port = *tmp;
		mz_port.__bindgen_anon_2.addr = mem::zeroed();
		global_state.ports =
			Arc::from_raw(mz_port.__bindgen_anon_2.addr as *mut nflib::structs::PortInfo);

		/* set up core status */
		let cores = num_cpus::get();
		tmp = rte_memzone_reserve(
			nflib::constants::MZ_CORES_STATUS as *const _ as *const i8,
			(mem::size_of::<nflib::structs::CoreStatus>() * cores) as u64,
			rte_socket_id() as i32,
			constants::NO_FLAGS.into(),
		);
		if tmp.is_null() {
			rte_exit(
				1,
				"Cannot reserve memory zone for core information\n" as *const _ as *const i8,
			);
		}
		mz_cores = *tmp;
		mz_cores.__bindgen_anon_2.addr = mem::zeroed();
		// convert this memory chunk into a vector of CoreStatus referenes
		addr = mz_cores.__bindgen_anon_2.addr as usize;
		let mut v_tmp = vec![];
		let sz = mem::size_of::<nflib::structs::CoreStatus>();
		for i in 0..cores {
			let a = Arc::new(addr as *mut nflib::structs::CoreStatus);
			// v_tmp.push(&mut *(addr as *mut nflib::structs::CoreStatus));
			v_tmp.push(a);
			addr += sz;
		}
		global_state.cores = v_tmp;

		/* set up array for NF tx data */
		// NOTE: get a chunk of memory for the services
		tmp = rte_memzone_reserve(
			nflib::constants::MZ_SERVICES_INFO as *const _ as *const i8,
			(mem::size_of::<nflib::structs::CoreStatus>() * nflib::constants::MAX_SERVICES as usize)
				as u64,
			rte_socket_id() as i32,
			constants::NO_FLAGS.into(),
		);
		if tmp.is_null() {
			rte_exit(
				1,
				"Cannot reserve memory zone for services information\n" as *const _ as *const i8,
			);
		}
		mz_services = *tmp;
		mz_services.__bindgen_anon_2.addr = mem::zeroed();
		// convert this memory chunk into a vector of c_void pointers
		addr = mz_services.__bindgen_anon_2.addr as usize;
		let mut v_tmp = vec![];
		let sz = mem::size_of::<c_void>();
		for i in 0..cores {
			v_tmp.push(RefCell::new(addr as *mut c_void));
			addr += sz;
		}
		global_state.services = v_tmp;
		// NOTE: allocate memory for NFs per service
		for i in 0..nflib::constants::MAX_SERVICES {
			global_state.services[i as usize] = RefCell::from(rte_calloc(
				"one service NFs" as *const _ as *const i8,
				nflib::constants::MAX_NFS_PER_SERVICE as u64,
				mem::size_of::<c_void>() as u64,
				0,
			));
		}

		tmp = rte_memzone_reserve(
			nflib::constants::MZ_NF_PER_SERVICE_INFO as *const _ as *const i8,
			mem::size_of::<c_void>() as u64,
			rte_socket_id() as i32,
			constants::NO_FLAGS.into(),
		);
		if tmp.is_null() {
			rte_exit(
				1,
				"Cannot reserve memory zone for NF per service information.\n" as *const _
					as *const i8,
			);
		}
		mz_nf_per_service = *tmp;
		// REVIEW: Is nf_per_service supposed to be a vector of u32s?
		mz_nf_per_service.__bindgen_anon_2.addr = mem::zeroed();
		global_state.nf_per_service_count = Vec::from_raw_parts(
			&mut RefCell::from(*(mz_nf_per_service.__bindgen_anon_2.addr as *mut u32)),
			0,
			nflib::constants::MAX_NFS_PER_SERVICE as usize,
		);

		/* set up custom flags */
		tmp = rte_memzone_reserve(
			nflib::constants::MZ_ONVM_CONFIG as *const _ as *const i8,
			mem::size_of::<nflib::structs::OnvmConfiguration>() as u64,
			rte_socket_id() as i32,
			constants::NO_FLAGS.into(),
		);
		if tmp.is_null() {
			rte_exit(
				1,
				"Cannot reserve memory zone for ONVM custom flags.\n" as *const _ as *const i8,
			);
		}
		mz_onvm_config = *tmp;
		mz_onvm_config.__bindgen_anon_2.addr = mem::zeroed();
		// NOTE: OnvmConfiguration can be marked Copy for the next line to work but this will copy a memory region - not a wise move
		// So use Arc
		global_state.onvm_config = Arc::from_raw(
			mz_onvm_config.__bindgen_anon_2.addr as *mut nflib::structs::OnvmConfiguration,
		);
		// global_state
		// .onvm_config
		// .set_flag(nflib::constants::ONVM_NF_SHARE_CORES_DEFAULT);
		set_default_config(global_state.onvm_config.clone());

		/* parse additional, application arguments */
		// NOTE: parse_app_args return an ExitFailure and so does init. Thus we can simply use ? to pass an error up to whichever executable uses this lib
		get_args::parse_app_args(total_ports, &mut global_state, argc, argv)?;

		/* initialise mbuf pools */
		init_mbuf_pools(&mut global_state)?;

		/* initialise nf info pool */
		init_nf_init_cfg_pool(&mut global_state)?;

		/* initialise pool for NF messages */
		init_nf_msg_pool(&mut global_state)?;

		/* now initialise the ports we will use */
		let end = *(global_state.ports.num_ports).borrow() as usize; // immutable borrow has to end before init_port can borrow global_state mutably
		for i in 0..end {
			port_id = (*global_state.ports.id.borrow())[i];
			let mut r = rte_ether_addr {
				addr_bytes: (*global_state.ports.mac[port_id as usize].borrow_mut()).addr_bytes,
			};
			rte_eth_macaddr_get(port_id.into(), &mut r as *mut rte_ether_addr);
			init_port(&mut global_state, port_id)?;
			// onvm_stats_gen_event_info(event_msg_buf, ONVM_EVENT_PORT_INFO, NULL);
		}

		check_all_ports_link_status(!0x0, &mut global_state)?;

		/* initialise a queue for newly created NFs */
		init_info_queue(&mut global_state);

		/* initialise the shared memory for shared core mode */
		// init_shared_sem();
		/*initialize a default service chain*/
		// TODO: Implement service chains
		// default_chain = onvm_sc_create();
		// retval = onvm_sc_append_entry(default_chain, ONVM_NF_ACTION_TONF, 1);
		/* set up service chain pointer shared to NFs*/
		// TODO:
		// mz_scp = rte_memzone_reserve(MZ_SCP_INFO, sizeof(struct onvm_service_chain *), rte_socket_id(), NO_FLAGS);
		// if (mz_scp == NULL)
		// rte_exit(EXIT_FAILURE, "Cannot reserve memory zone for service chain pointer\n");
		// memset(mz_scp->addr, 0, sizeof(struct onvm_service_chain *));
		// default_sc_p = mz_scp->addr;
		// *default_sc_p = default_chain;
		// onvm_sc_print(default_chain);

		// onvm_flow_dir_init();
	} // unsafe ends
	Ok(())
}

// Initialise the default onvm config structure
fn set_default_config(config: Arc<nflib::structs::OnvmConfiguration>) {
	match nflib::constants::ONVM_NF_SHARE_CORES_DEFAULT {
		true => config.set_flag(0),
		false => config.set_flag(1),
	};
}

/// Initialise the mbuf pool for packet reception for the NIC, and any other buffer pools needed by the app - currently none.
fn init_mbuf_pools(global_state: &mut global::GlobalNFState) -> Result<(), ExitFailure> {
	println!(
		"Creating mbuf pool '{}' [{} mbufs] ...\n",
		nflib::constants::PKTMBUF_POOL_NAME,
		nflib::constants::NUM_MBUFS
	);

	*global_state.pktmbuf_pool.borrow_mut() = unsafe {
		rte_mempool_create(
			nflib::constants::PKTMBUF_POOL_NAME as *const _ as *const i8,
			nflib::constants::NUM_MBUFS.into(),
			constants::MBUF_SIZE as u32,
			constants::MBUF_CACHE_SIZE as u32,
			mem::size_of::<rte_pktmbuf_pool_private>() as u32,
			Some(rte_pktmbuf_pool_init),
			ptr::null_mut() as *mut c_void,
			Some(rte_pktmbuf_init),
			ptr::null_mut() as *mut c_void,
			rte_socket_id() as i32,
			nflib::constants::NO_FLAGS,
		)
	};
	if (*global_state.pktmbuf_pool.borrow()).is_null() {
		return Ok(exit_on_failure(
			"Cannot create needed mbuf pools".into(),
			"Failed in the init_mbuf function",
		)?);
	}
	Ok(())
}

/// Set up a mempool to store nf_msg structs
fn init_nf_msg_pool(global_state: &mut global::GlobalNFState) -> Result<(), ExitFailure> {
	/* don't pass single-producer/single-consumer flags to mbuf
		* create as it seems faster to use a cache instead */
	println!(
		"Creating mbuf pool '{}' ...\n",
		nflib::constants::_NF_MSG_POOL_NAME
	);
	*global_state.nf_msg_pool.borrow_mut() = unsafe {
		rte_mempool_create(
			nflib::constants::_NF_MSG_POOL_NAME as *const _ as *const i8,
			(nflib::constants::MAX_NFS as u32 * constants::NF_MSG_QUEUE_SIZE as u32).into(),
			constants::NF_INFO_SIZE as u32,
			constants::NF_MSG_CACHE_SIZE as u32,
			0,
			None,
			ptr::null_mut() as *mut c_void,
			None,
			ptr::null_mut() as *mut c_void,
			rte_socket_id() as i32,
			nflib::constants::NO_FLAGS,
		)
	};
	if (*global_state.pktmbuf_pool.borrow()).is_null() {
		let f = unsafe {
			format!(
				"Cannot create nf info mbuf pool: {:?}",
				rte_strerror(_rte_errno())
			)
		};
		return Ok(exit_on_failure(
			f,
			"Failed in the init_nf_msg_pool function",
		)?);
	}
	Ok(())
}

/// Set up a mempool to store nf_init_cfg structs
fn init_nf_init_cfg_pool(global_state: &mut global::GlobalNFState) -> Result<(), ExitFailure> {
	println!(
		"Creating mbuf pool '{}' ...\n",
		nflib::constants::_NF_MEMPOOL_NAME
	);

	*global_state.nf_init_cfg_pool.borrow_mut() = unsafe {
		rte_mempool_create(
			nflib::constants::_NF_MEMPOOL_NAME as *const _ as *const i8,
			nflib::constants::MAX_NFS.into(),
			constants::NF_INFO_SIZE as u32,
			0,
			0,
			None,
			ptr::null_mut() as *mut c_void,
			None,
			ptr::null_mut() as *mut c_void,
			rte_socket_id() as i32,
			nflib::constants::NO_FLAGS,
		)
	};
	if (*global_state.nf_init_cfg_pool.borrow()).is_null() {
		let f = unsafe {
			format!(
				"Cannot create nf info mbuf pool: {:?}\n",
				rte_strerror(_rte_errno())
			)
		};
		return Ok(exit_on_failure(
			f,
			"Failed in the init_nf_msg_pool function",
		)?);
	}
	// println!("Cannot create nf message pool: %s\n", rte_strerror(rte_errno));
	Ok(())
}

/// Initialise an individual port:
/// - configure number of rx and tx rings
/// - set up each rx ring, to pull from the main mbuf pool
/// - set up each tx ring
/// - start the port and report its status to stdout
fn init_port(global_state: &mut global::GlobalNFState, port_num: u8) -> Result<(), ExitFailure> {
	let rx_rings = constants::ONVM_NUM_RX_THREADS;
	let mut rx_ring_size = constants::RTE_MP_RX_DESC_DEFAULT;
	/* Set the number of tx_rings equal to the tx threads. This mimics the onvm_mgr tx thread calculation. */
	let tx_rings =
		unsafe { rte_lcore_count() - rx_rings as u32 - constants::ONVM_NUM_MGR_AUX_THREADS as u32 };
	let mut tx_ring_size = constants::RTE_MP_TX_DESC_DEFAULT;
	let mut rxq_conf: rte_eth_rxconf;
	let mut txq_conf: rte_eth_txconf;
	let mut dev_info = unsafe { mem::MaybeUninit::<rte_eth_dev_info>::uninit().assume_init() };
	let mut local_port_conf: rte_eth_conf = global_state.port_conf;
	let q: u16;
	let mut retval: i32;

	println!("Port {} init ... ", port_num);
	unsafe {
		println!(
			"Port {} socket id {} ... ",
			port_num,
			rte_eth_dev_socket_id(port_num.into())
		);
	}
	println!("Port {} Rx rings {} ... ", port_num, rx_rings);
	println!("Port {} Tx rings {} ... ", port_num, tx_rings);

	/* Standard DPDK port initialisation - config port, then set up rx and tx rings */
	// NOTE: dev_info is uninitialised and rte_eth_dev_info_get is supposed to initialise it
	let check =
		unsafe { rte_eth_dev_info_get(port_num.into(), &mut dev_info as *mut rte_eth_dev_info) };
	if check == -ENOTSUP {
		return Ok(exit_on_failure(
			"support for dev_infos_get() does not exist for the device".to_string(),
			"Failed in the init_port function",
		)?);
	} else if check == -ENODEV {
		return Ok(exit_on_failure(
			"port_id invalid".into(),
			"Failed in the init_port function",
		)?);
	}
	if (dev_info.tx_offload_capa as i64 & DEV_TX_OFFLOAD_MBUF_FAST_FREE as i64) as u64 != 0 {
		local_port_conf.txmode.offloads =
			(local_port_conf.txmode.offloads as i64 | DEV_TX_OFFLOAD_MBUF_FAST_FREE as i64) as u64;
	}
	local_port_conf.rx_adv_conf.rss_conf.rss_hf = (local_port_conf.rx_adv_conf.rss_conf.rss_hf
		as i64 & dev_info.flow_type_rss_offloads as i64)
		as u64;
	if local_port_conf.rx_adv_conf.rss_conf.rss_hf
		!= global_state.port_conf.rx_adv_conf.rss_conf.rss_hf
	{
		println!("Port {} modified RSS hash function based on hardware support, requested: {} configured: {}", port_num, global_state.port_conf.rx_adv_conf.rss_conf.rss_hf, local_port_conf.rx_adv_conf.rss_conf.rss_hf);
	}

	retval = unsafe {
		rte_eth_dev_configure(
			port_num.into(),
			rx_rings.into(),
			tx_rings as u16,
			&local_port_conf,
		)
	};
	if retval != 0 {
		return Ok(exit_on_failure(
			" Error code returned by the driver configuration function.".into(),
			"Failed in the init_port function",
		)?);
	}

	/* Adjust rx,tx ring sizes if not allowed by ethernet device
		* TODO: if this is ajusted store the new values for future reference */
	retval = unsafe {
		rte_eth_dev_adjust_nb_rx_tx_desc(port_num.into(), &mut rx_ring_size, &mut tx_ring_size)
	};
	if retval < 0 {
		// REVIEW: Is this a panic or should this also be a exit_on_failure Result
		panic!(
			"Cannot adjust number of descriptors for port {} ({})",
			port_num, retval,
		);
	}
	rxq_conf = dev_info.default_rxconf;
	rxq_conf.offloads = local_port_conf.rxmode.offloads;

	for q in 0..rx_rings as usize {
		retval = unsafe {
			rte_eth_rx_queue_setup(
				port_num.into(),
				q as u16,
				rx_ring_size,
				rte_eth_dev_socket_id(port_num.into()) as u32,
				&rxq_conf,
				&mut global_state.pktmbuf_pool as *mut _ as *mut rte_mempool,
			)
		};
		if retval == -EIO {
			return Ok(exit_on_failure(
				"device is removed".to_string(),
				"Failed in the init_port function",
			)?);
		} else if retval == -EINVAL {
			return Ok(exit_on_failure(
				"The memory pool pointer is null or the size of network buffers which can be allocated from this memory pool does not fit the various buffer sizes allowed by the device controller.".to_string(),
				"Failed in the init_port function",
			)?);
		} else if retval == -ENOMEM {
			return Ok(exit_on_failure(
				"Unable to allocate the receive ring descriptors or to allocate network memory buffers from the memory pool when initializing receive descriptors.".to_string(),
				"Failed in the init_port function",
			)?);
		}
	}

	txq_conf = dev_info.default_txconf;
	txq_conf.offloads = global_state.port_conf.txmode.offloads;
	for q in 0..rx_rings as usize {
		retval = unsafe {
			rte_eth_tx_queue_setup(
				port_num.into(),
				q as u16,
				tx_ring_size,
				rte_eth_dev_socket_id(port_num.into()) as u32,
				&txq_conf,
			)
		};
		if retval == -ENOMEM {
			return Ok(exit_on_failure(
				"Unable to allocate the transmit ring descriptors.".to_string(),
				"Failed in the init_port function",
			)?);
		}
	}

	retval = unsafe { rte_eth_promiscuous_enable(port_num.into()) };
	if retval == -ENOTSUP {
		return Ok(exit_on_failure(
			"support for promiscuous_enable() does not exist for the device.".to_string(),
			"Failed in the init_port function",
		)?);
	} else if retval == -ENODEV {
		return Ok(exit_on_failure(
			"port id is invalid.".to_string(),
			"Failed in the init_port function",
		)?);
	}

	retval = unsafe { rte_eth_dev_start(port_num.into()) };
	if retval < 0 {
		return Ok(exit_on_failure(
			"device driver start function failed.".to_string(),
			"Failed in the init_port function",
		)?);
	}

	global_state.ports.init.borrow_mut()[port_num as usize] = 1;

	println!("Initialised ports");
	Ok(())
}

/// Check the link status of all ports in up to 9s, and print them finally
fn check_all_ports_link_status(
	port_mask: u32,
	global_state: &global::GlobalNFState,
) -> Result<(), ExitFailure> {
	let port_num = *global_state.ports.num_ports.borrow();
	let portid: usize;
	let count: u8;
	let mut all_ports_up;
	let mut retval: i32;
	let mut print_flag = 0;

	let mut link = unsafe { mem::MaybeUninit::<rte_eth_link>::uninit().assume_init() };

	println!("Checking link status");
	for count in 0..constants::MAX_CHECK_TIME as usize {
		all_ports_up = 1;
		for portid in 0..port_num {
			if (port_mask as i64) & (1 << (*global_state.ports.id.borrow_mut())[portid as usize])
				!= 0
			{
				link = unsafe { mem::zeroed() };
				retval = unsafe {
					rte_eth_link_get_nowait(
						(*global_state.ports.id.borrow_mut())[portid as usize].into(),
						&mut link,
					)
				};
				if retval == -ENOTSUP {
					return Ok(exit_on_failure(
						"function is not supported in PMD driver.".to_string(),
						"Failed in the check_all_ports_link_status function",
					)?);
				} else if retval == -ENODEV {
					return Ok(exit_on_failure(
						"Port id is invalid.".to_string(),
						"Failed in the check_all_ports_link_status function",
					)?);
				}
				if print_flag == 1 {
					if link.link_status() != 0 {
						let duplex;
						if link.link_duplex() == ETH_LINK_FULL_DUPLEX as u16 {
							duplex = "full-duplex";
						} else {
							duplex = "half-duplex";
						}
						println!(
							"Port {} link up - speed {} Mbps - {}",
							(*global_state.ports.id.borrow_mut())[portid as usize],
							link.link_status(),
							"full-duplex"
						);
					} else {
						println!(
							"Port {} link down",
							(*global_state.ports.id.borrow_mut())[portid as usize]
						);
						continue;
					}
				}
				/* clear all_ports_up flag if any link down */
				if link.link_status() == 0 {
					all_ports_up = 0;
					break;
				}
			} // if loop
		} // inner for loop
		if print_flag == 1 {
			break;
		}
		if all_ports_up == 0 {
			println!(".");
			unsafe {
				rte_delay_us_sleep(constants::CHECK_INTERVAL as u32);
			}
		}
		/* set the print_flag if all ports up or timeout */
		if all_ports_up == 0 || count == constants::MAX_CHECK_TIME as usize - 1 {
			print_flag = 1;
			println!("check_all_ports_link_status done");
		}
	} // outer for loop
	Ok(())
}

/// Allocate a rte_ring for newly created NFs
fn init_info_queue(global_state: &mut global::GlobalNFState) {
	*global_state.incoming_msg_queue.borrow_mut() = unsafe {
		rte_ring_create(
			nflib::constants::_MGR_MSG_QUEUE_NAME as *const _ as *const i8,
			nflib::constants::MAX_NFS.into(),
			rte_socket_id() as i32,
			constants::RING_F_SC_DEQ,
		)
	}; // MP enqueue (default), SC dequeue
	if (*global_state.incoming_msg_queue.borrow()).is_null() {
		unsafe {
			rte_exit(
				1,
				"Cannot create incoming msg queue" as *const _ as *const i8,
			);
		}
	}
}
