PRQL_PROJECT=../../../../..

run: build
	./main.out

build-prql:
	cargo build --package prqlc-c --release

# TODO: would be helpful to allow running with a debug build too.
build: main.c build-prql
	gcc main.c -o main.out \
		-I${PRQL_PROJECT}/prqlc/bindings/prqlc-c \
		-L${PRQL_PROJECT}/target/release \
		${PRQL_PROJECT}/target/release/libprqlc_lib.a \
		-pthread -ldl -lm

valgrind: build
	valgrind ./main.out
