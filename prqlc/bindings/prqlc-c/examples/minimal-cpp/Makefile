PRQL_PROJECT=../../../../..

run: build
	./main.out

build-prql:
	cargo build --package prqlc-c --release

UNAME_S := $(shell uname -s)

LD_FLAGS = -L${PRQL_PROJECT}/target/release \
	${PRQL_PROJECT}/target/release/libprqlc_c.a

ifeq ($(UNAME_S),Darwin)
	LD_FLAGS := $(LD_FLAGS) -framework CoreFoundation
endif

# TODO: would be helpful to allow running with a debug build too.
build: main.cpp build-prql
	g++ main.cpp -o main.out \
		-I${PRQL_PROJECT}/prqlc/bindings/prqlc-c \
		$(LD_FLAGS)

valgrind: build
	valgrind ./main.out
