#
# Created on Sun Sep 27 2020:23:23:20
# Created by Ratnadeep Bhattacharya
# RUSTFLAGS="$RUSTFLAGS -A dead_code" cargo chec

CC = cargo

check:
	@$(CC) check

run:
	@$(CC) run -- -l 0-3 -n 2 --proc-type=primary --base-virtaddr=0x7f000000000

rebuild_run: clean
	@(CC) run

debug:
	@$(CC) build

rebuild_debug: clean
	@$(CC) build

release:
	@$(CC) build --release

rebuild_release: clean
	@$(CC) build

test:
	@$(CC) test -- --nocapture --show-output -q

rebuild_test: clean
	@$(CC) build

clean:
	@$(CC) clean
