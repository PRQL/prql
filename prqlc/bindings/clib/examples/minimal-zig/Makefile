PRQL_PROJECT=../../../../..

run: build
	./zig-out/bin/minimal-zig

build-prql:
	cargo build --package prqlc-clib --release

build:
	mkdir -p c/
	cp ${PRQL_PROJECT}/prqlc/bindings/clib/libprqlc_lib.h c/
	cp ${PRQL_PROJECT}/target/release/libprqlc_lib.so c/
	zig build

test: build
	zig build test
