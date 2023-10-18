PRQL_PROJECT=../../../../..

run: build
	./zig-out/bin/minimal-zig

build-prql:
	cargo build --package prqlc-clib --release

build:
	mkdir --package c/
	cp ${PRQL_PROJECT}/prqlc/bindings/clib/libprqlc.h c/
	cp ${PRQL_PROJECT}/target/release/libprqlc.so c/
	zig build

test: build
	zig test
