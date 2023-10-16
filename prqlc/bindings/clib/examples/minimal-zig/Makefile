PRQL_PROJECT=../../../../..

run: build
	./zig-out/bin/minimal-zig

build-prql:
	cargo build -p prqlc-clib --release

build:
	mkdir -p c/
	cp ${PRQL_PROJECT}/prqlc/bindings/clib/libprqlc.h c/
	cp ${PRQL_PROJECT}/target/release/libprqlc.so c/
	zig build
