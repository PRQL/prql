# syntax=docker/dockerfile:1.4

FROM mcr.microsoft.com/devcontainers/rust:1-1-bullseye

# ========= Install cargo-tools for non-root user (vscode) =========
USER vscode

ARG cargo_crates

RUN <<"EOF"
curl -L --proto '=https' --tlsv1.2 -sSf https://raw.githubusercontent.com/cargo-bins/cargo-binstall/main/install-from-binstall-release.sh | bash
cargo binstall -y --locked ${cargo_crates}
EOF

USER root
