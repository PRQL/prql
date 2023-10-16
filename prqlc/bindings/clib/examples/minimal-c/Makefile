PRQL_PROJECT=../../../..

build-prql:
	cargo build -p prql-lib --release

# TODO: would be helpful to allow running with a debug build too.
build: main.c build-prql
	gcc main.c -o main.out \
		-I${PRQL_PROJECT}/prqlc/bindings/clib \
		-L${PRQL_PROJECT}/target/release \
		${PRQL_PROJECT}/target/release/libprqlc.a \
		-pthread -ldl -lm

run: build
	./main.out

valgrind: build
	valgrind ./main.out
