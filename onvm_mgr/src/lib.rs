/*
 * Created on Fri Sep 25 2020:03:38:32
 * Created by Ratnadeep Bhattacharya
 */

// `

pub mod mgr;
pub mod nflib;

use nflib::{common, msg_common};
use mgr::{get_args, init};

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
