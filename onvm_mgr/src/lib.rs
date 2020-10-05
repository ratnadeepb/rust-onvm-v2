/*
 * Created on Fri Sep 25 2020:03:38:32
 * Created by Ratnadeep Bhattacharya
 */

// `

pub mod error_handling;
pub mod mgr;
pub mod nflib;

// #[allow(unused_imports)] // remove when code stabilises
// use mgr::get_args;
use std::ffi::{c_void, CString};
use std::mem;
use std::os::raw::{c_char, c_int};
// use std::sync::Arc;
use std::{thread, time};

// DPDK functions
use capsule_ffi::{_rte_get_timer_hz, _rte_get_tsc_cycles, _rte_lcore_id, rte_log};
// DPDK constants
use capsule_ffi::{RTE_LOGTYPE_USER1, RTE_LOG_ERR, RTE_LOG_INFO};

// use nflib::{common, msg_common};

const MAX_SHUTDOWN_ITERS: u8 = 10;

#[derive(Default)]
pub struct MgrState {
    pub global_stats_sleep_time: u8, // also used to run the main thread of onvm
    pub global_verbosity_level: u8,
    pub global_pkt_limit: u8,
    pub global_time_to_live: u8,
}

/// Stats thread periodically prints per-port and per-NF stats.
pub fn master_thread_main(global_state: &mgr::global::GlobalNFState) {
    // pub fn master_thread_main() {
    // True as long as the main thread loop should keep running
    let mut main_keep_running = 1;
    // We'll want to shut down the TX/RX threads second so that we don't
    // race the stats display to be able to print, so keep this varable separate
    let mut worker_keep_running = 1;
    let mut thread_state: MgrState = Default::default();
    thread_state.global_stats_sleep_time = 1;

    let i: usize;
    let shutdown_iter_count: u8;
    let sleeptime = thread_state.global_stats_sleep_time;
    let verbosity_level = thread_state.global_verbosity_level;
    let time_to_live = thread_state.global_time_to_live;
    let pkt_limit = thread_state.global_pkt_limit;
    let start_time = unsafe { _rte_get_tsc_cycles() };
    let mut total_rx_pkts: u32;

    let f = format!("Core {}: Running master thread\n", unsafe {
        _rte_lcore_id()
    });
    unsafe {
        rte_log(
            RTE_LOG_INFO,
            RTE_LOGTYPE_USER1,
            &f[..] as *const _ as *const i8,
        )
    };

    let f = format!("Stats verbosity level = {}\n", verbosity_level);
    unsafe {
        rte_log(
            RTE_LOG_INFO,
            RTE_LOGTYPE_USER1,
            &f[..] as *const _ as *const i8,
        )
    };

    if time_to_live > 0 {
        let f = format!(
            "Manager time to live = {}\n",
            thread_state.global_time_to_live
        );
        unsafe {
            rte_log(
                RTE_LOG_INFO,
                RTE_LOGTYPE_USER1,
                &f[..] as *const _ as *const i8,
            )
        };
    }

    if pkt_limit > 0 {
        let f = format!("Manager packet limit = {}\n", thread_state.global_pkt_limit);
        unsafe {
            rte_log(
                RTE_LOG_INFO,
                RTE_LOGTYPE_USER1,
                &f[..] as *const _ as *const i8,
            )
        };
    }
    /* Initial pause so above printf is seen */
    thread::sleep(time::Duration::from_secs(5));

    /* Loop forever: sleep always returns 0 or <= param */
    let sleeptime = time::Duration::from_secs(sleeptime as u64);
    // REVIEW: This is a polling while loop. Can we convert this to an event based async loop?
    while main_keep_running > 0 {
        // let now = time::Instant::now();
        thread::sleep(sleeptime);
        mgr::net_funcs::onvm_nf_check_status(global_state);

        // NOTE: The next if loop is more wordy than required since Rust needs type annotations to perform the math ops
        let cur: u64 = unsafe { _rte_get_tsc_cycles() };
        let t: u64 = (cur - start_time) * nflib::constants::TIME_TTL_MULTIPLIER as u64;
        let t1: u64 = unsafe { _rte_get_timer_hz() };
        if time_to_live > 0 && t / t1 >= time_to_live as u64 {
            println!("Time to live exceeded, shutting down");
            main_keep_running = 0;
        }

        if pkt_limit > 0 {
            total_rx_pkts = 0;
            for i in 0..*global_state.ports.num_ports.borrow() as usize {
                total_rx_pkts += global_state.ports.clone().rx_stats.rx
                    [global_state.ports.id.borrow()[i as usize] as usize]
                    as u32;
            }
            let lim: u32 = pkt_limit as u32 * nflib::constants::PKT_TTL_MULTIPLIER;
            if total_rx_pkts >= lim {
                println!("Packet limit exceeded, shutting down");
                main_keep_running = 0;
            }
        }
    } // end of while loop

    // REVIEW: How to convert this?
    // #ifdef RTE_LIBRTE_PDUMP
    //         rte_pdump_uninit();
    // #endif
    unsafe {
        let f = format!("Core {}: Initiating shutdown sequence\n", _rte_lcore_id());
        rte_log(
            RTE_LOG_INFO,
            RTE_LOGTYPE_USER1,
            &f[..] as *const _ as *const i8,
        );
    }
    /* Stop all RX and TX threads */
    worker_keep_running = 0;

    /* Tell all NFs to stop */
    for i in 0..nflib::constants::MAX_NFS as usize {
        let status = unsafe { *(*(*global_state.nfs[i].clone())).status.borrow() };
        // let status = unsafe { *(*global_state.nfs.clone()[i]).status.borrow() };
        // let status = unsafe { (**global_state.nfs.clone()[i].borrow()).status };
        if status == nflib::constants::NF_RUNNING as u16 {
            continue;
        }
        unsafe {
            let f = format!(
                "Core {}: Notifying NF {} to shut down\n",
                _rte_lcore_id(),
                i
            );
            rte_log(
                RTE_LOG_INFO,
                RTE_LOGTYPE_USER1,
                &f[..] as *const _ as *const i8,
            );
        }
        let ret = unsafe {
            // let a = Arc::get_mut(&mut global_state.nfs.clone()[i]);
            let a = *global_state.nfs[i].clone();
            mgr::net_funcs::onvm_nf_send_msg(
                i as u16,
                nflib::structs::OnvmNFMsg::NfStopping(a),
                global_state.clone(),
            )
        };

        /* If in shared core mode NFs might be sleeping */
        // REVIEW: Shared cores not implemented yet
        // if (ONVM_NF_SHARE_CORES && rte_atomic16_read(nf_wakeup_infos[i].shm_server) == 1) {
        //                 nf_wakeup_infos[i].num_wakeups++;
        //                 rte_atomic16_set(nf_wakeup_infos[i].shm_server, 0);
        //                 sem_post(nf_wakeup_infos[i].mutex);
        //         }
    } // NFs stop for loop

    /* Wait to process all exits */
    for _ in 0..MAX_SHUTDOWN_ITERS as usize {
        mgr::net_funcs::onvm_nf_check_status(global_state);
        unsafe {
            let f = &format!(
                "Core {}: Waiting for {} NFs to exit\n",
                _rte_lcore_id(),
                *global_state.num_nfs.borrow()
            )[..] as *const _ as *const i8;
            rte_log(RTE_LOG_ERR, RTE_LOGTYPE_USER1, f);
        }
        thread::sleep(sleeptime);
    }

    if *global_state.num_nfs.borrow() > 0 {
        unsafe {
            let f = &format!(
                "Core {}: Up to {} NFs may still be running and must be killed manually\n",
                _rte_lcore_id(),
                *global_state.num_nfs.borrow()
            )[..] as *const _ as *const i8;
            rte_log(RTE_LOG_ERR, RTE_LOGTYPE_USER1, f);
        }
    }

    /* Clean up the shared memory */
    // TODO:
    // if (ONVM_NF_SHARE_CORES) {
    //             for (i = 0; i < MAX_NFS; i++) {
    //                     sem_close(nf_wakeup_infos[i].mutex);
    //                     sem_unlink(nf_wakeup_infos[i].sem_name);
    //             }
    //     }
    unsafe {
        let f =
            &format!("Core {}: Master thread done\n", _rte_lcore_id())[..] as *const _ as *const i8;
        rte_log(RTE_LOG_ERR, RTE_LOGTYPE_USER1, f);
    }
}

/*
 * Function to receive packets from the NIC
 * and distribute them to the default service
 */
fn rx_thread_main() {}
fn tx_thread_main() {}

fn handle_signal(sig: i32) {}

// fn wakeup_client(nf_wakeup_info: nflib::structs::nf_wakeup_info) {}

fn wakeup_thread_main() {}

pub fn main_run(args: Vec<String>) {
    // let args = std::env::args();
    // let mut _v: Vec<*mut c_char> = args
    //     .map(|arg| CString::new(&arg[..]).unwrap().into_raw())
    //     .collect();
    // let argc = (_v.len() + 1) as c_int;
    // let argv = _v.as_mut_ptr();
    // mem::forget(_v);
    let cur_lcore: u32;
    let rx_lcores: u32;
    let tx_lcores: u32;
    let wakeup_lcores: u32;

    /* initialise the system */
    mgr::init::init(args);
}

#[cfg(test)]
mod tests {
    use crate::mgr;
    use std::ffi::CString;
    use std::mem;
    use std::os::raw::{c_char, c_int};
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
    #[test]
    fn onvm_run_init() {
        // FIXME: How to get environment arguments in a tes?
        println!("Starting test init"); // DEBUG
                                        // let mut args = vec!["-p", "1"];
        let args = std::env::args();
        println!("Environment args: {:?}", &args);
        let mut _v: Vec<*mut c_char> = args
            .map(|arg| CString::new(&arg[..]).unwrap().into_raw())
            .collect();
        let argc = (_v.len() + 1) as c_int;
        let argv = _v.as_mut_ptr();
        mem::forget(_v);
        unsafe {
            println!(
                "Calling init with argc: {} and argv: {:?}",
                &argc,
                &*(*argv)
            )
        }; // DEBUG
        let _ = mgr::init::init(argc, argv);
    }
}
