PRQL_PROJECT=../../../../..

run: build
	./main.out

build-prql:
	cargo build --package prqlc-c --release

UNAME_S := $(shell uname -s)

LD_FLAGS = -L${PRQL_PROJECT}/target/release \
	${PRQL_PROJECT}/target/release/libprqlc_c.a \
	-pthread -ldl -lm

ifeq ($(UNAME_S),Darwin)
	LD_FLAGS := $(LD_FLAGS) -framework CoreFoundation
endif

# TODO: would be helpful to allow running with a debug build too.
build: main.c build-prql
	gcc main.c -o main.out \
		-I${PRQL_PROJECT}/prqlc/bindings/prqlc-c \
		$(LD_FLAGS)

valgrind: build
	valgrind ./main.out
