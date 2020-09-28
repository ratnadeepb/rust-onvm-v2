/*
 * Created on Fri Sep 25 2020:00:40:19
 * Created by Ratnadeep Bhattacharya
 */

// 'nf denotes the lifetime of the NF
// So most of these data structures will live as long as the NF lives
// Is that true?
use bit_field::BitField;
// Functions
use capsule_ffi::{rte_eth_dev_is_valid_port, rte_eth_macaddr_get};
// Structures
use capsule_ffi::{rte_atomic16_t, rte_ether_addr, rte_mbuf, rte_ring};
// Constants
use capsule_ffi::{RTE_LOGTYPE_USER1, RTE_MAX_ETHPORTS};
// use capsule::Mbuf;
// use std::{mem, ptr};
use std::cell::RefCell;

// true when NFs pass packets to each other
pub const ONVM_NF_HANDLE_TX: bool = true;
// should be true if on NF shutdown onvm_mgr tries to reallocate cores
pub const ONVM_NF_SHUTDOWN_CORE_REASSIGNMENT: bool = false;
// the maximum chain length
pub const ONVM_MAX_CHAIN_LENGTH: u8 = 4;
// total number of concurrent NFs allowed (-1 because ID 0 is reserved)
pub const MAX_NFS: u8 = 128;
// total number of unique services allowed
pub const MAX_SERVICES: u8 = 32;
// max number of NFs per service.
pub const MAX_NFS_PER_SERVICE: u8 = 32;
// total number of mbufs (2^15 - 1)
pub const NUM_MBUFS: u16 = 32767;
// size of queue for NFs
pub const NF_QUEUE_RINGSIZE: usize = 16384;
pub const PACKET_READ_SIZE: usize = 32;
// default value for shared core logic, if true NFs sleep while waiting for packets
pub const ONVM_NF_SHARE_CORES_DEFAULT: bool = false;

pub enum OnvmAction {
	DROP, // drop packet
	NEXT, // to whatever the next action is configured
	TONF, // // send to the NF specified in the argument field, if on the same host
	OUT,  // send the packet out the NIC port set in the argument field
}

// for shared core mode, how many packets are required to wake up the NF
const PKT_WAKEUP_THRESHOLD: u8 = 1;
// for shared core mode, how many messages on an NF's ring are required to wake up the NF
const MSG_WAKEUP_THRESHOLD: u8 = 1;

// Used in setting bit flags for core options
const MANUAL_CORE_ASSIGNMENT_BIT: bool = false;
const SHARE_CORE_BIT: bool = true;

const ONVM_SIGNAL_TERMINATION: i16 = -999;

// Maximum length of NF_TAG
const TAG_SIZE: usize = 15;

#[inline]
fn onvm_check_bit(flags: &mut u8, n: usize) -> bool {
	flags.get_bit(n)
}

#[inline]
fn onvm_set_bit(flags: &mut u8, n: usize) {
	flags.set_bit(n, true);
}

#[inline]
fn onvm_clear_bit(flags: &mut u8, n: usize) {
	flags.set_bit(n, false);
}

// Measured in millions of packets
const PKT_TTL_MULTIPLIER: u32 = 1000000;

// Measured in seconds
const TIME_TTL_MULTIPLIER: u8 = 1;

// For NF termination handling
const NF_TERM_WAIT_TIME: u8 = 1;
const NF_TERM_INIT_ITER_TIMES: u8 = 3;
const NF_TERM_STOP_ITER_TIMES: u8 = 10;

pub struct OnvmPktMeta {
	action: OnvmAction, // Action to be performed
	destination: u16,   // where to go next
	src: u16,           // who processed the packet last
	chain_index: u8,    // index of the current step in the service chain
	flags: u8, // bits for custom NF data. Use with caution to prevent collisions from different NFs
}

#[inline]
pub fn onvm_get_pkt_name(pkt: &rte_mbuf) -> Option<&OnvmPktMeta> {
	unsafe {
		let p = pkt.__bindgen_anon_5.udata64 as *const u64;
		if !p.is_null() {
			Some(&(*(p as *const OnvmPktMeta)))
		} else {
			None
		}
	}
}

#[inline]
pub fn onvm_get_pkt_chain_index(pkt: &rte_mbuf) -> Option<u8> {
	if let Some(meta) = onvm_get_pkt_name(pkt) {
		Some(meta.chain_index)
	} else {
		None
	}
}

// Shared port info, including statistics information for display by server.
// Structure will be put in a memzone.
// - All port id values share one cache line as this
// data will be read-only
// during operation.
// - All rx statistic values share cache lines, as this data is written only
// by the server process. (rare reads by stats display)
// - The tx statistics have values for all ports per cache line, but the stats
// themselves are written by the NFs, so we have a distinct set, on different
// cache lines for each NF to use.
// Data Structures

// Data Structures

/// Local buffers to put packets in, used to send packets in bursts to the NFs or to the NIC
#[derive(Default)]
pub struct PacketBuf {
	// these packets are to be sent out
	// so we are going to take over ownsership
	// however, it might make more sense to use Rc/Weak
	// buffer: [rte_mbuf; PACKET_READ_SIZE],
	buffer: Vec<rte_mbuf>,
	count: u16,
}

impl PacketBuf {
	pub fn new() -> Self {
		Self {
			buffer: Vec::with_capacity(PACKET_READ_SIZE),
			count: 0,
		}
	}

	pub fn add_mbuf(&mut self, pkt: rte_mbuf) {
		self.buffer.push(pkt);
		self.count += 1;
	}
}

/// Packets may be transported by a tx thread or by an NF. This data structure encapsulates data specific to tx threads.
pub struct TxThreadInfo<'nf> {
	first_nf: u8,
	last_nf: u8,
	// the port tx bufs exist as long
	port_tx_bufs: Option<&'nf mut PacketBuf>,
}

impl<'nf> TxThreadInfo<'nf> {
	pub fn new(first_nf: u8, last_nf: u8, port_tx_bufs: &'nf mut PacketBuf) -> Self {
		Self {
			first_nf,
			last_nf,
			port_tx_bufs: Some(port_tx_bufs),
		}
	}

	pub fn add_mbuf(&mut self, pkt: rte_mbuf) {
		match self.port_tx_bufs.take() {
			Some(tx_buf) => {
				tx_buf.add_mbuf(pkt);
				// mem::replace(&mut self.port_tx_bufs, Some(tx_buf));
				self.port_tx_bufs = Some(tx_buf);
			}
			None => {
				let mut tx_buf = Box::new(PacketBuf::new());
				tx_buf.add_mbuf(pkt);
				self.port_tx_bufs = unsafe { Some(&mut *Box::into_raw(tx_buf)) };
			}
		}
	}
}

pub enum QmgrType {
	NF,
	MGR,
}

type MgrTypeT = QmgrType;

pub enum Qmgr<'nf> {
	Mgr(&'nf TxThreadInfo<'nf>),
	NF(&'static PacketBuf),
}

/// Generic data struct that tx threads and nfs both use. Allows pkt functions to be shared
pub struct QueueMgr<'nf> {
	id: u8,
	mgr_type: MgrTypeT,
	buf: Qmgr<'nf>,
	nf_rx_buf: Option<&'nf PacketBuf>,
}

impl<'nf> QueueMgr<'nf> {
	#[inline]
	fn get_self(id: u8, mgr_type: MgrTypeT, buf: Qmgr<'nf>, nf_rx_buf: &'nf PacketBuf) -> Self {
		Self {
			id,
			mgr_type,
			buf,
			nf_rx_buf: Some(nf_rx_buf),
		}
	}
	pub fn new(
		id: u8,
		mgr_type: MgrTypeT,
		buf: Qmgr<'nf>,
		nf_rx_buf: &'nf PacketBuf,
	) -> Option<Self> {
		// QmgrType mgr_type should always have the correct associated buffer type
		match mgr_type {
			MgrTypeT::MGR => match buf {
				Qmgr::Mgr(_) => Some(Self::get_self(id, mgr_type, buf, nf_rx_buf)),
				Qmgr::NF(_) => None,
			},
			MgrTypeT::NF => match buf {
				Qmgr::NF(_) => Some(Self::get_self(id, mgr_type, buf, nf_rx_buf)),
				Qmgr::Mgr(_) => None,
			},
		}
	}
}

/// NFs wakeup Info: used by manager to update NFs pool and wakeup stats
pub struct WakeupThreadContext {
	first_nf: u8,
	last_nf: u8,
}

// pub struct NfWakeupInfo {
// 	sem_name: String,
// 	mutex: &RwLock,
// }

#[derive(Default)]
pub struct RxStats {
	rx: [u64; RTE_MAX_ETHPORTS as usize],
}

#[derive(Default)]
pub struct TxStats {
	tx: [u64; RTE_MAX_ETHPORTS as usize],
	tx_drop: [u64; RTE_MAX_ETHPORTS as usize],
}

#[derive(Default)]
struct EtherAddr {
	addr_bytes: [u8; 6],
}

impl EtherAddr {
	fn new(addr_bytes: [u8; 6]) -> Self {
		Self { addr_bytes }
	}

	fn from_rte_ether_addr(&self, addr: rte_ether_addr) -> Self {
		Self::new(addr.addr_bytes)
	}
}

#[derive(Default)]
pub struct PortInfo {
	num_ports: u8,
	id: [u8; RTE_MAX_ETHPORTS as usize],
	init: [u8; RTE_MAX_ETHPORTS as usize],
	mac: [EtherAddr; RTE_MAX_ETHPORTS as usize],
	rx_stats: RxStats,
	tx_stats: TxStats,
}

#[derive(Default)]
struct Flag {
	onvm_nf_share_cores: u8,
}

#[derive(Default)]
pub struct OnvmConfiguration {
	flags: Flag,
}

#[derive(Default)]
pub struct CoreStatus {
	enabled: bool,
	is_dedicated_core: bool,
	nf_count: u16,
}

/// Function prototype for NF packet handlers
type NfPktHandlerFn = fn(pkt: &rte_mbuf, meta: &OnvmPktMeta, _: &OnvmNfLocalCtx) -> i8;

/// Function prototype for NFs that want extra initalization/setup before running
type NfSetupFn = fn(nf_local_ctx: &OnvmNfLocalCtx);

/// Function prototype for NF the callback
type NfUserActionsFn = fn(_: &OnvmNfLocalCtx) -> i8;

/// Function prototype for NFs to handle custom messages
type NfMsgHandlerFn = fn(msg_data: &str, nf_local_ctx: &OnvmNfLocalCtx);

/// Function prototype for NFs to signal handling
type HandleSignalFn = fn(i8);

/// Contains all functions the NF might use
pub struct OnvmFunctionTable {
	setup: NfSetupFn,
	nf_msg_handler_fn: NfMsgHandlerFn,
	nf_user_actions_fn: NfUserActionsFn,
	nf_pkt_handler_fn: NfPktHandlerFn,
}

/// Information needed to initialize a new NF child thread
pub struct OnvmScaleInfo {}

pub struct OnvmNfLocalCtx<'nf> {
	nf: Option<&'nf OnvmNF<'nf>>,
	nf_init_finished: rte_atomic16_t,
	keep_running: rte_atomic16_t,
	nf_stopped: rte_atomic16_t,
}

struct Stats {
	rx: u64,
	rx_drop: u64,
	tx: u64,
	tx_drop: u64,
	tx_buffer: u64,
	tx_returned: u64,
	act_out: u64,
	act_tonf: u64,
	act_drop: u64,
	act_next: u64,
	act_buffer: u64,
}

#[derive(Default)]
struct Flags {
	init_options: u16, // if set NF will stop after time reaches time_to_live
	time_to_live: u16, // If set NF will stop after pkts TX reach pkt_limit
	pkt_limit: u16,
}

#[derive(Default)]
struct ThreadInfo {
	core: u16, // Instance ID of parent NF or 0
	parent: u16,
	children_cut: rte_atomic16_t,
}

#[derive(Default)]
struct SharedCore {
	// Sleep state (shared mem variable) to track state of NF and trigger wakeups
	// sleep_state = 1 => NF sleeping (waiting on semaphore)
	// sleep_state = 0 => NF running (not waiting on semaphore)
	sleep_state: rte_atomic16_t,
	// nf_mutex: std::sync::RwLock;
}

/// Define a NF structure with all needed info, including:
/// 	thread information, function callbacks, flags, stats and shared core info.
/// This structure is available in the NF when processing packets or executing the callback.
/// nf denotes the lifetime of the nf
#[derive(Default)]
pub struct OnvmNF<'nf> {
	rx_q: Option<&'nf rte_ring>,
	tx_q: Option<&'nf rte_ring>,
	msg_q: Option<&'nf rte_ring>,
	nf_tx_mgr: Option<&'nf QueueMgr<'nf>>,
	instance_id: u16,
	service_id: u16,
	status: u8,
	tag: Option<&'nf str>,
	// FIXME: we need to figure out what msg_data should be
	// Connected to msg_common_rs::OnvmNfMsg
	// void *data;
	thread_info: ThreadInfo,
	flags: Flags,
	function_table: Option<&'nf OnvmFunctionTable>,
	stats: Option<&'nf String>,
	shared_core: Option<&'nf SharedCore>,
}

// impl<'nf> Default for OnvmNF<'nf> {
// 	fn default() -> Self {
// 		Self {
// 			rx_q: unsafe {&(*(ptr::null()))},
// 			tx_q: unsafe { &mem::MaybeUninit::<rte_ring>::uninit().assume_init() },
// 			msg_q: unsafe { &mem::MaybeUninit::<rte_ring>::uninit().assume_init() },
// 			..Default::default()
// 		}
// 	}
// }

/// The config structure to inialize the NF with onvm_mgr
pub struct OnvmNfInitCfg<'nf> {
	instance_id: u16,
	service_id: u16,
	core: u16,
	init_options: u16,
	status: u8,
	tag: Option<&'nf str>,
	// If set NF will stop after time reaches time_to_live
	time_to_live: u16,
	// If set NF will stop after pkts TX reach pkt_limit
	pkt_limit: u16,
}

/// Define a structure to describe a service chain entry
#[derive(Default)]
pub struct OnvmServiceChainEntry {
	destination: u16,
	action: u8,
}

#[derive(Default)]
pub struct OnvmServiceChain {
	sc: [OnvmServiceChainEntry; ONVM_MAX_CHAIN_LENGTH as usize],
	chain_length: u8,
	ref_cnt: u8,
}

pub struct LpmRequest {
	name: String,
	max_num_rules: u32,
	num_tbl8s: u32,
	socket_id: u32,
	status: u32,
}

/// Structure used to initiate a flow tables hash_table from a secondary process, it is enqueued onto the managers message ring
pub struct FtRequest {}

/// define common names for structures shared between server and NF
pub const MP_NF_RXQ_NAME: &str = "MProc_Client_{}_RX";
pub const MP_NF_TXQ_NAME: &str = "MProc_Client_{}_TX";
pub const MP_CLIENT_SEM_NAME: &str = "MProc_Client_{}_SEM";
pub const PKTMBUF_POOL_NAME: &str = "MProc_pktmbuf_pool";
pub const MZ_PORT_INFO: &str = "MProc_port_info";
pub const MZ_CORES_STATUS: &str = "MProc_cores_info";
pub const MZ_NF_INFO: &str = "MProc_nf_init_cfg";
pub const MZ_SERVICES_INFO: &str = "MProc_services_info";
pub const MZ_NF_PER_SERVICE_INFO: &str = "MProc_nf_per_service_info";
pub const MZ_ONVM_CONFIG: &str = "MProc_onvm_config";
pub const MZ_SCP_INFO: &str = "MProc_scp_info";
pub const MZ_FTP_INFO: &str = "MProc_ftp_info";
pub const _MGR_MSG_QUEUE_NAME: &str = "MSG_MSG_QUEUE";
pub const _NF_MSG_QUEUE_NAME: &str = "NF_%u_MSG_QUEUE";
pub const _NF_MEMPOOL_NAME: &str = "NF_INFO_MEMPOOL";
pub const _NF_MSG_POOL_NAME: &str = "NF_MSG_MEMPOOL";

/// interrupt semaphore specific updates
pub const SHMSZ: u8 = 4; // size of shared memory segement (page_size)
pub const KEY_PREFIX: u8 = 123; // prefix len for key

/// common names for NF states
pub const NF_WAITING_FOR_ID: u8 = 0; // First step in startup process, doesn't have ID confirmed by manager yet
pub const NF_STARTING: u8 = 1; // When a NF is in the startup process and already has an id
pub const NF_RUNNING: u8 = 2; // Running normally
pub const NF_PAUSED: u8 = 3; // NF is not receiving packets, but may in the future
pub const NF_STOPPED: u8 = 4; // NF has stopped and in the shutdown process
pub const NF_ID_CONFLICT: u8 = 5; // NF is trying to declare an ID already in use
pub const NF_NO_IDS: u8 = 6; // There are no available IDs for this NF
pub const NF_SERVICE_MAX: u8 = 7; // Service ID has exceeded the maximum amount
pub const NF_SERVICE_COUNT_MAX: u8 = 8; // Maximum amount of NF's per service spawned
pub const NF_NO_CORES: u8 = 9; // There are no cores available or specified core can't be used
pub const NF_NO_DEDICATED_CORES: u8 = 10; // There is no space for a dedicated core
pub const NF_CORE_OUT_OF_RANGE: u8 = 11; // The manually selected core is out of range
pub const NF_CORE_BUSY: u8 = 12; // The manually selected core is busy
pub const NF_WAITING_FOR_LPM: u8 = 13; // NF is waiting for a LPM request to be fulfilled
pub const NF_WAITING_FOR_FT: u8 = 14; // NF is waiting for a flow-table request to be fulfilled
pub const NF_NO_ID: i8 = -1;

pub const NO_FLAGS: u32 = 0;

pub const RSS_SYMMETRIC_KEY: RefCell<[u8; 40]> = RefCell::new([
	0x6d, 0x5a, 0x6d, 0x5a, 0x6d, 0x5a, 0x6d, 0x5a, 0x6d, 0x5a, 0x6d, 0x5a, 0x6d, 0x5a, 0x6d, 0x5a,
	0x6d, 0x5a, 0x6d, 0x5a, 0x6d, 0x5a, 0x6d, 0x5a, 0x6d, 0x5a, 0x6d, 0x5a, 0x6d, 0x5a, 0x6d, 0x5a,
	0x6d, 0x5a, 0x6d, 0x5a, 0x6d, 0x5a, 0x6d, 0x5a,
]);

/// Given the rx queue name template above, get the queue name
fn get_rx_queue_name(id: u8) -> String {
	// FIXME: too many conversions going on
	// can we optimise this?
	let id = format!("{}", id);
	MP_NF_RXQ_NAME.replace("{}", &id)
}

/// Given the tx queue name template above, get the queue name
fn get_tx_queue_name(id: u8) -> String {
	// FIXME: too many conversions going on
	// can we optimise this?
	let id = format!("{}", id);
	MP_NF_TXQ_NAME.replace("{}", &id)
}

/// Given the name template above, get the mgr -> NF msg queue name
fn get_msg_queue_name(id: u8) -> String {
	// FIXME: too many conversions going on
	// can we optimise this?
	let id = format!("{}", id);
	_NF_MSG_QUEUE_NAME.replace("{}", &id)
}

/// Interface checking if a given NF is "valid", meaning if it's running.
fn onvm_nf_is_valid(nf: &OnvmNF) -> bool {
	nf.status == NF_RUNNING
}

// Given the rx queue name template above, get the key of the shared memory
// fn get_rx_shmkey(id: u8) -> key_t

// Given the sem name template above, get the sem name
// fn get_sem_name(id: u8) -> String
// fn whether_wakeup_client(nf: &OnvmNF, nf_wakeup_info: &NfWakeupInfo) -> u8

static RTE_LOGTYPE_APP: u32 = RTE_LOGTYPE_USER1;

/// Updates the ether_addr struct with a fake, safe MAC address
fn onvm_get_fake_macaddr(mac_addr: &EtherAddr) {
	let mut mac_addr_bytes = mac_addr.addr_bytes;
	mac_addr_bytes[0] = 2;
	mac_addr_bytes[1] = 0;
	mac_addr_bytes[2] = 0;
}

/// Tries to fetch the MAC address of the port_id.
/// Returns Result<(), u8>
/// () if port is valid, 1 if port is invalid.
fn onvm_get_macaddr(port_id: u16, mac_addr: &mut EtherAddr) -> Result<(), u8> {
	unsafe {
		if rte_eth_dev_is_valid_port(port_id) == 1 {
			rte_eth_macaddr_get(port_id, mac_addr as *mut _ as *mut rte_ether_addr);
			Ok(())
		} else {
			Err(1)
		}
	}
}
