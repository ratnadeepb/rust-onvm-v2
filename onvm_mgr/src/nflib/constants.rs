/*
 * Created on Tue Sep 29 2020:13:00:18
 * Created by Ratnadeep Bhattacharya
 */

use capsule_ffi::RTE_LOGTYPE_USER1;
use std::cell::RefCell;
/* All the constants in the nflib submodule */

/// message passing between mgr and nfs
pub const MSG_NOOP: u8 = 0;
pub const MSG_STOP: u8 = 1;
pub const MSG_NF_STARTING: u8 = 2;
pub const MSG_NF_STOPPING: u8 = 3;
pub const MSG_NF_READY: u8 = 4;
pub const MSG_SCALE: u8 = 5;
pub const MSG_FROM_NF: u8 = 6;
pub const MSG_REQUEST_LPM_REGION: u8 = 7;
pub const MSG_CHANGE_CORE: u8 = 8;
pub const MSG_REQUEST_FT: u8 = 9;

/// common to all nf features
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

// Measured in millions of packets
const PKT_TTL_MULTIPLIER: u32 = 1000000;

// Measured in seconds
const TIME_TTL_MULTIPLIER: u8 = 1;

// For NF termination handling
const NF_TERM_WAIT_TIME: u8 = 1;
const NF_TERM_INIT_ITER_TIMES: u8 = 3;
const NF_TERM_STOP_ITER_TIMES: u8 = 10;

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

static RTE_LOGTYPE_APP: u32 = RTE_LOGTYPE_USER1;

/// define common names for structures shared between server and NF
pub const MP_NF_RXQ_NAME: RefCell<&str> = RefCell::new(""); // to be populated by get_msg_queue_name macro
pub const MP_NF_TXQ_NAME: RefCell<&str> = RefCell::new(""); // to be populated by get_msg_queue_name macro
pub const MP_CLIENT_SEM_NAME: &str = "MProc_Client_{}_SEM"; // REVIEW: some macro to come here
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
pub const _NF_MSG_QUEUE_NAME: RefCell<&str> = RefCell::new(""); // to be populated by get_msg_queue_name macro
pub const _NF_MEMPOOL_NAME: &str = "NF_INFO_MEMPOOL";
pub const _NF_MSG_POOL_NAME: &str = "NF_MSG_MEMPOOL";
