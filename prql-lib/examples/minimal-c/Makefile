PRQL_PROJECT=../../..

build-prql:
	cargo build -p prql-lib --release

build: main.c build-prql
	gcc main.c -o main.out \
		-I${PRQL_PROJECT}/prql-lib \
		-L${PRQL_PROJECT}/target/release \
		-l:libprql_lib.a \
		-pthread -ldl -lm

run: build
	./main.out

valgrind: build
	valgrind ./main.out
