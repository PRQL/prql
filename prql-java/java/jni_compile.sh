#!/bin/bash
set -e

echo "start cross compilations"

cd ..

echo "compile target=x86_64-unknown-linux-gnu"
rustup target add x86_64-unknown-linux-gnu
cross build --release --target=x86_64-unknown-linux-gnu
cp -f target/x86_64-unknown-linux-gnu/release/libprql_java.so java-api/src/main/resources/libprql_java-linux64.so

echo "compile target=aarch64-unknown-linux-gnu"
rustup target add aarch64-unknown-linux-gnu
cross build --release --target=aarch64-unknown-linux-gnu
cp -f target/aarch64-unknown-linux-gnu/release/libprql_java.so java-api/src/main/resources/libprql_java-linux-aarch64.so
