/*
 * Created on Tue Sep 29 2020:18:08:59
 * Created by Ratnadeep Bhattacharya
 */

/* functions and macros used throughout */
use super::structs::EtherAddr;
// DPDK functions
use capsule_ffi::{rte_eth_dev_is_valid_port, rte_eth_macaddr_get};
// DPDK Structures
use super::constants;
use super::structs;
use bit_field::BitField;
use capsule_ffi::rte_ether_addr;
use capsule_ffi::rte_mbuf;

#[macro_export]
macro_rules! get_rx_queue_name {
	($n: tt) => {
		format!("MProc_Client_{}_RX", $n)
	};
}

#[macro_export]
macro_rules! get_tx_queue_name {
	($n: tt) => {
		format!("MProc_Client_{}_TX", $n)
	};
}

#[macro_export]
macro_rules! get_msg_queue_name {
	($n: tt) => {
		format!("NF_{}_MSG_QUEUE", $n)
	};
}

#[inline]
pub fn onvm_check_bit(flags: &mut u8, n: usize) -> bool {
	flags.get_bit(n)
}

#[inline]
pub fn onvm_set_bit(flags: &mut u8, n: usize) {
	flags.set_bit(n, true);
}

#[inline]
pub fn onvm_clear_bit(flags: &mut u8, n: usize) {
	flags.set_bit(n, false);
}

#[inline]
pub fn onvm_get_pkt_name(pkt: &rte_mbuf) -> Option<&structs::OnvmPktMeta> {
	unsafe {
		let p = pkt.__bindgen_anon_5.udata64 as *const u64;
		if !p.is_null() {
			Some(&(*(p as *const structs::OnvmPktMeta)))
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

/// Updates the ether_addr struct with a fake, safe MAC address
pub fn onvm_get_fake_macaddr(mac_addr: &EtherAddr) {
	let mut mac_addr_bytes = mac_addr.get_mac();
	mac_addr_bytes[0] = 2;
	mac_addr_bytes[1] = 0;
	mac_addr_bytes[2] = 0;
}

/// Tries to fetch the MAC address of the port_id.
/// Returns Result<(), u8>
/// () if port is valid, 1 if port is invalid.
pub fn onvm_get_macaddr(port_id: u16, mac_addr: &mut EtherAddr) -> Result<(), u8> {
	unsafe {
		if rte_eth_dev_is_valid_port(port_id) == 1 {
			rte_eth_macaddr_get(port_id, mac_addr as *mut _ as *mut rte_ether_addr);
			Ok(())
		} else {
			Err(1)
		}
	}
}

pub fn onvm_nf_is_valid(nf: &structs::OnvmNF) -> bool {
	*nf.status.borrow_mut() == constants::NF_RUNNING
}
