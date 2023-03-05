FROM mcr.microsoft.com/devcontainers/rust:0.203.7-1

RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        cmake \
    && rm -rf /var/lib/apt/lists/* \
    && sh -c "$(curl --location https://taskfile.dev/install.sh)" -- -d -b /usr/local/bin

# ========= Install cargo-tools for non-root user (vscode) =========
COPY Taskfile.yml /tmp/Taskfile.yml
WORKDIR /tmp
USER vscode
RUN task install-cargo-tools

USER root
