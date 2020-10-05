/*
 * Created on Fri Oct 02 2020:16:09:45
 * Created by Ratnadeep Bhattacharya
 */

// use super::structs;
use super::{constants, funcs_macros, structs};
use crate::error_handling::exit_on_failure;
use crate::mgr::global;
use exitfailure::ExitFailure;
use num_cpus;

pub fn onvm_threading_get_core(
	core_value: u16,
	flags: u8,
	core_status: structs::CoreStatus,
	global_state: &global::GlobalState,
) -> Result<(), ExitFailure> {
	let max_cores;
	let best_core;
	let pref_core_id = core_value;
	let min_nf_count = 0;

	max_cores = num_cpus::get();

	// REVIEW: Since users can't set preferred core, is this needed?
	/* Check status of preffered core */
	if funcs_macros::onvm_check_bit(flags, constants::MANUAL_CORE_ASSIGNMENT_BIT) {
		/* If manual core assignment and core is out of bounds */
		if pref_core_id < 0
			|| pref_core_id > max_cores
			|| global_state.cores[pref_core_id as usize].enabled
		{
			return Ok(exit_on_failure(
				"NF Core out of range".into(),
				"In the onvm_threading_get_core function",
			)?);
		}

		/* If used as a dedicated core already */
		if global_state.cores[pref_core_id as usize].is_dedicated_core != 0 {
			return Ok(exit_on_failure(
				"NF Core busy".into(),
				"In the onvm_threading_get_core function",
			)?);
		}

		/* If dedicated core requested ensure no NFs are running on that core */
		if !funcs_macros::onvm_check_bit(flags, constants::SHARE_CORE_BIT) {
			if global_state.cores[pref_core_id as usize].nf_count == 0 {
				global_state.cores[pref_core_id as usize].is_dedicated_core = 1;
			} else {
				return Ok(exit_on_failure(
					"No dedicated cores".into(),
					"In the onvm_threading_get_core function",
				)?);
			}
		}

		global_state.cores[pref_core_id as usize] += 1;
		return Ok(());
	}

	/* Find the most optimal core, least NFs running */
	for i in 0..max_cores as usize {
		if global_state.cores[i].enabled && global_state.cores[i].is_dedicated_core == 0 {
			// if 
		}
	}
}
