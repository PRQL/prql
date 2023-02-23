FROM mcr.microsoft.com/devcontainers/rust:0.203.7-1
USER vscode
RUN cargo install \
    cargo-audit cargo-insta cargo-release \
    default-target mdbook mdbook-admonish mdbook-toc wasm-bindgen-cli \
    wasm-pack
USER root
