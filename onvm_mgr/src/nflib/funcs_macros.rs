/*
 * Created on Tue Sep 29 2020:18:08:59
 * Created by Ratnadeep Bhattacharya
 */

/* functions and macros used throughout */
use super::structs::EtherAddr;
// DPDK functions
use capsule_ffi::{rte_eth_dev_is_valid_port, rte_eth_macaddr_get};
// DPDK Structures
use capsule_ffi::rte_ether_addr;

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

/// Updates the ether_addr struct with a fake, safe MAC address
fn onvm_get_fake_macaddr(mac_addr: &EtherAddr) {
	let mut mac_addr_bytes = mac_addr.get_mac();
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
