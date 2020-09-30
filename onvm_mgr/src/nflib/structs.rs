/*
 * Created on Tue Sep 29 2020:13:18:34
 * Created by Ratnadeep Bhattacharya
 */

use super::constants::*;
use crate::error_handling::exit_on_failure;
use exitfailure::ExitFailure;
use std::sync::Weak;
// Functions
use capsule_ffi::{rte_eth_dev_is_valid_port, rte_eth_macaddr_get};
// Structures
use capsule_ffi::{rte_atomic16_t, rte_ether_addr, rte_mbuf, rte_ring};
// Constants
use capsule_ffi::{RTE_LOGTYPE_USER1, RTE_MAX_ETHPORTS};

// contains all structs for use in nflib

/// Message passing
pub struct OnvmNfMsg {
	msg_type: u8, // Constant saying what type of message is
	// FIXME: we need to figure out what msg_data should be
	msg_data: String, // These should be rte_malloc'd so they're stored in hugepages
}

pub enum OnvmAction {
	DROP, // drop packet
	NEXT, // to whatever the next action is configured
	TONF, // // send to the NF specified in the argument field, if on the same host
	OUT,  // send the packet out the NIC port set in the argument field
}

pub struct OnvmPktMeta {
	action: OnvmAction, // Action to be performed
	destination: u16,   // where to go next
	src: u16,           // who processed the packet last
	chain_index: u8,    // index of the current step in the service chain
	flags: u8, // bits for custom NF data. Use with caution to prevent collisions from different NFs
}

/// Local buffers to put packets in, used to send packets in bursts to the NFs or to the NIC
/// This buffer takes ownership of the packets
#[derive(Default)]
pub struct PacketBuf {
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

	pub fn len(&self) -> usize {
		self.buffer.len()
	}
}

/// Packets may be transported by a tx thread or by an NF. This data structure encapsulates data specific to tx threads.
// pub struct TxThreadInfo<'nf> {
pub struct TxThreadInfo {
	first_nf: u8,
	last_nf: u8,
	// the tx thread should know where the packets buffer is, so as to be able to fetch the packets
	// port_tx_bufs: Option<&'nf PacketBuf>,
	// REVIEW: A better way might be to move the ownership of the packets to the tx thread
	port_tx_bufs: Weak<PacketBuf>,
}

impl TxThreadInfo {
	fn new(first_nf: u8, last_nf: u8, port_tx_bufs: &PacketBuf) -> Self {
		Self {
			first_nf,
			last_nf,
			port_tx_bufs: unsafe { Weak::from_raw(port_tx_bufs) },
		}
	}

	/* add a packet to packet buffer through the tx thread
	that can fail if the weak pointer is null or the packet buffer itself is null.
	neither situation should be handled here
	the program can do little else but exit
	*/
	// REVIEW: Ideally packets shouldn't be added to packet buffer through tx thread
	pub fn add_mbuf(&mut self, pkt: rte_mbuf) -> Result<(), ExitFailure> {
		// try to upgrade the weak reference to an arc
		// this increments the Arc count for the inner value - the PacketBuf - preventing it from being dropped
		match &mut self.port_tx_bufs.upgrade() {
			// get a mutable
			Some(tx_buf) => match std::sync::Arc::get_mut(tx_buf) {
				Some(buf) => {
					if buf.len() < PACKET_READ_SIZE {
						buf.add_mbuf(pkt);
					} // otherwise packet is silently dropped
					Ok(())
				}
				// the local buffer points to null
				None => Ok(exit_on_failure(
					"Packet buffer does not exist",
					"Packet Buffer pointer in tx thread info points to null",
				)?),
			},
			// the original buffer is null
			None => Ok(exit_on_failure(
				"Packet buffer was not found",
				"Packet Buffer pointer in tx thread info is null",
			)?),
		}
	}
}

pub enum QmgrType {
	NF,
	MGR,
}

type MgrTypeT = QmgrType;

pub enum Qmgr {
	Mgr(TxThreadInfo),
	NF(PacketBuf),
}

/// Generic data struct that tx threads and nfs both use. Allows pkt functions to be shared
/// The queue manager takes ownership of the packet buffer or the tx thread
pub struct QueueMgr {
	id: u8,
	mgr_type: MgrTypeT,
	buf: Qmgr,
	nf_rx_buf: PacketBuf,
}

impl QueueMgr {
	#[inline]
	fn get_self(id: u8, mgr_type: MgrTypeT, buf: Qmgr, nf_rx_buf: PacketBuf) -> Self {
		Self {
			id,
			mgr_type,
			buf,
			nf_rx_buf: nf_rx_buf,
		}
	}

	pub fn new(id: u8, mgr_type: MgrTypeT, buf: Qmgr, nf_rx_buf: PacketBuf) -> Option<Self> {
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
pub struct EtherAddr {
	addr_bytes: [u8; 6],
}

impl EtherAddr {
	pub fn new(addr_bytes: [u8; 6]) -> Self {
		Self { addr_bytes }
	}

	pub fn from_rte_ether_addr(&self, addr: rte_ether_addr) -> Self {
		Self::new(addr.addr_bytes)
	}

	pub fn get_mac(&self) -> [u8; 6] {
		self.addr_bytes
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

impl OnvmConfiguration {
	pub fn set_flag(&mut self, share: u8) {
		self.flags.onvm_nf_share_cores = share;
	}
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

/// The NF local context will own the NF struct
pub struct OnvmNfLocalCtx {
	nf: Option<OnvmNF>,
	nf_init_finished: rte_atomic16_t,
	keep_running: rte_atomic16_t,
	nf_stopped: rte_atomic16_t,
}

#[derive(Default)]
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
pub struct OnvmNF {
	// REVIEW: we might want to change *mut rte_ring to something less risky!
	rx_q: Option<*mut rte_ring>,
	tx_q: Option<*mut rte_ring>,
	msg_q: Option<*mut rte_ring>,
	nf_tx_mgr: Option<QueueMgr>,
	instance_id: u16,
	service_id: u16,
	status: u8,
	tag: String,
	// FIXME: we need to figure out what msg_data should be
	// Connected to msg_common_rs::OnvmNfMsg
	// void *data;
	thread_info: ThreadInfo,
	flags: Flags,
	function_table: OnvmFunctionTable,
	// stats: Option<&'nf String>,
	shared_core: SharedCore,
}

/// The config structure to inialize the NF with onvm_mgr
#[derive(Default)]
pub struct OnvmNfInitCfg {
	instance_id: u16,
	service_id: u16,
	core: u16,
	init_options: u16,
	status: u8,
	tag: Option<String>,
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
