PRQL_PROJECT=../../../../..

run: build
	./zig-out/bin/minimal-zig

build-prql:
	cargo build --package prqlc-c --release

build:
	mkdir -p c/
	cp ${PRQL_PROJECT}/prqlc/bindings/prqlc-c/libprqlc_c.h c/
	cp ${PRQL_PROJECT}/target/release/libprqlc_c.* c/
	zig build

test: build
	zig build test
