#!/bin/bash

# We set prefix-key to the version from Cargo.toml for Swatinem/rust-cache@v2
# since the caches seem to accumulate cruft over time;
# ref https://github.com/PRQL/prql/pull/2407

version=$(cargo metadata --format-version=1 --no-deps | jq --raw-output '.packages[] | select(.name == "prql-compiler") | .version')
# Setting `2` on 2023-09-07 because of bloated cache can revert on next version
echo "version=${version}2" >>"$GITHUB_ENV"
