# Some of this is shared with /Dockerfile

FROM mcr.microsoft.com/devcontainers/rust:0.203.9-1-bullseye

RUN apt-get update \
    && apt-get install -y --no-install-recommends \
    cmake \
    && rm -rf /var/lib/apt/lists/* \
    && sh -c "$(curl --location https://taskfile.dev/install.sh)" -- -d -b /usr/local/bin

# ========= Install cargo-tools for non-root user (vscode) =========
COPY Taskfile.yml /tmp/Taskfile.yml
USER vscode
RUN task -t /tmp/Taskfile.yml install-cargo-tools

USER root
