FROM mcr.microsoft.com/devcontainers/rust:0.203.8-1-bullseye

RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        cmake \
    && rm -rf /var/lib/apt/lists/* \
    && sh -c "$(curl --location https://taskfile.dev/install.sh)" -- -d -b /usr/local/bin

# ========= Install cargo-tools for non-root user (vscode) =========
COPY Taskfile.cargo-tools.yml /tmp/Taskfile.cargo-tools.yml
WORKDIR /tmp
USER vscode
RUN task -t Taskfile.cargo-tools.yml install

USER root
