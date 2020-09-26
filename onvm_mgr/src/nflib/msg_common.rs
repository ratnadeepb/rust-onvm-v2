/*
 * Created on Sat Sep 26 2020:18:55:24
 * Created by Ratnadeep Bhattacharya
 */

const MSG_NOOP: u8 = 0;
const MSG_STOP: u8 = 1;
const MSG_NF_STARTING: u8 = 2;
const MSG_NF_STOPPING: u8 = 3;
const MSG_NF_READY: u8 = 4;
const MSG_SCALE: u8 = 5;
const MSG_FROM_NF: u8 = 6;
const MSG_REQUEST_LPM_REGION: u8 = 7;
const MSG_CHANGE_CORE: u8 = 8;
const MSG_REQUEST_FT: u8 = 9;

pub struct OnvmNfMsg {
	msg_type: u8, // Constant saying what type of message is
	// FIXME: we need to figure out what msg_data should be
	msg_data: String, // These should be rte_malloc'd so they're stored in hugepages
}
