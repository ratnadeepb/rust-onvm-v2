/*
 * Created on Tue Sep 29 2020:13:58:22
 * Created by Ratnadeep Bhattacharya
 */

use failure;

// Show exit error
pub fn exit_on_failure(msg: &'static str, context: &str) -> Result<(), failure::Error> {
	let err = failure::err_msg(msg);
	Ok(Err(err.context(context.to_string()))?)
}
