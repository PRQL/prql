#!/bin/sh

# TODO: use a task file for these build scripts

cargo build -p prql-lib --release

mkdir -p lib
cp ../../target/release/libprql_lib.* ../prql-lib/libprql_lib.h lib
