#
# Created on Sun Sep 27 2020:23:23:20
# Created by Ratnadeep Bhattacharya
# RUSTFLAGS="$RUSTFLAGS -A dead_code" cargo chec

CC = cargo

check:
	@$(CC) check

debug:
	@$(CC) build

release:
	@$(CC) build --release

test:
	@$(CC) run

#exp:
#	@export LOCAL_FFI_PATH=/capsule-ffi
