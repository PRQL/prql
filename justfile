pull-request: fmt prqlc-test

fmt:
    @ echo '--- remove trailing whitespace ---'
    @ rg '\s+$' --files-with-matches --glob '!*.rs' . \
        | xargs -I _ sh -c "echo _ && sd '[\t ]+$' '' _"

    @  echo '--- no-dbg ---'
    @! rg 'dbg!' --glob '*.rs' . --no-heading

    @ echo '--- cargo fmt ---'
    @ cargo fmt --all

    @ echo '--- prettier ---'
    @ prettier --write . \
            --config=.prettierrc.yaml \
            --ignore-path=.prettierignore \
            --ignore-unknown \
            --log-level=warn


prqlc-lint:
    @echo '--- clippy ---'
    @cargo clippy --all --fix --allow-staged


target := 'x86_64-unknown-linux-gnu'

# Test prqlc
prqlc-test:
    @echo "Testing prqlc…"

    cargo clippy --all-targets --target={{ target }} -- -D warnings

    @# Note that `--all-targets` doesn't refer to targets like
    @# `wasm32-unknown-unknown`; it refers to lib / bin / tests etc.
    @#
    @# Autoformatting does not make this clear to read, but this tertiary
    @# expression states to run:
    @# - External DB integration tests — `--features=test-dbs-external`
    @#   on Linux
    @# - No features on Windows
    @# - Internal DB integration tests — `--features=test-dbs` otherwise
    @#
    @# Below, we also add:
    @# - Unreferenced snapshots - `--unreferenced=auto` on Linux
    @#
    @# We'd like to reenable on Windows, ref https://github.com/wangfenjin/duckdb-rs/issues/179.

    cargo test --no-run --locked --target={{ target }}

    cargo insta test --target={{ target }}

prqlc-python-build mode='debug':
    #!/usr/bin/env bash
    if [ '{{mode}}' = 'release' ]; then
        release='--release'
    else
        release=''
    fi

    maturin build $release \
       -o target/python \
       -m prqlc/bindings/python/Cargo.toml

prqlc-python-test:
    #!/usr/bin/env bash
    python -m venv target/venv
    source target/venv/bin/activate
    pip install target/python/prql_python-*.whl
    pip install -r prqlc/bindings/python/requirements.txt
    pytest
