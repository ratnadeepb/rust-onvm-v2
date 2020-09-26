/*
 * Created on Fri Sep 25 2020:00:24:57
 * Created by Ratnadeep Bhattacharya
 */

use crate::nflib::{common, msg_common};
// structures
use capsule_ffi::{rte_mbuf, rte_ring};
use std::mem;
use std::sync::Arc;

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

//TODO:
// extern struct rte_ring *incoming_msg_queue;

