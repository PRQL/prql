# We set prefix-key to the version from Cargo.toml for Swatinem/rust-cache@v2
# since the caches seem to accumulate cruft over time;
# ref https://github.com/PRQL/prql/pull/2407

grep '^version =' Cargo.toml | tr -d ' ' >> $GITHUB_ENV
