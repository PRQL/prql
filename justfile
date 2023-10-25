choose:
    just --choose

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
            --log-level=warn &


prqlc-lint:
    @echo '--- clippy ---'
    @cargo clippy --all --fix --allow-staged


# Test prqlc
packages := '--package=prqlc-ast --package=prqlc-parser --package=prql-compiler --package=prqlc'
prqlc-test-fast:
    cargo clippy --all-targets {{packages}} -- -D warnings

    # cargo insta test, but allowing multiple --package arguments
    INSTA_FORCE_PASS=1 cargo test --locked {{packages}}

    cargo insta review

prqlc-test:
    cargo clippy --all-targets {{packages}} -- -D warnings

    INSTA_FORCE_PASS=1 cargo test --locked

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
