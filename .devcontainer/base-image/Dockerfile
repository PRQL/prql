# Some of this is shared with /Dockerfile

FROM mcr.microsoft.com/devcontainers/rust:0.203.10-1-bullseye

RUN apt-get update \
    && apt-get install -y --no-install-recommends \
    cmake \
    && rm -rf /var/lib/apt/lists/* \
    && sh -c "$(curl --location https://taskfile.dev/install.sh)" -- -d -b /usr/local/bin

# ========= Install cargo-tools for non-root user (vscode) =========
COPY Taskfile.yml /tmp/Taskfile.yml
USER vscode

# TODO: Trying this rather than the standard; will integrate with the Taskfile before merging.
RUN cargo install binstall
RUN cargo binstall --locked bacon cargo-audit cargo-insta cargo-release default-target mdbook mdbook-admonish mdbook-footnote mdbook-toc wasm-bindgen-cli wasm-pack


RUN task -t /tmp/Taskfile.yml install-cargo-tools

USER root
