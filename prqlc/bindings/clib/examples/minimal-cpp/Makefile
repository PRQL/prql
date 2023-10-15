PRQL_PROJECT=../../../..

build-prql:
	cargo build -p prql-lib --release

# TODO: would be helpful to allow running with a debug build too.
build: main.cpp build-prql
	g++ main.cpp -o main.out \
		-I${PRQL_PROJECT}/prqlc/bindings/prql-lib \
		-L${PRQL_PROJECT}/target/release \
		${PRQL_PROJECT}/target/release/libprql_lib.a

run: build
	./main.out

valgrind: build
	valgrind ./main.out
