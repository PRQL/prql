# Dockerfile to build the prql development environment


# Build with docker build -t prql .
# Invoke with docker run wgvanity [ string ]

# FROM lukemathwalker/cargo-chef:latest-rust-1.56.0 AS chef
FROM rust:1.63.0-buster
WORKDIR app

RUN apt-get update; apt install -y cmake pkg-config libssl-dev git gcc build-essential clang libclang-dev 
RUN apt-get install -y python3.7 python3-pip
# Install task
RUN sh -c "$(curl --location https://taskfile.dev/install.sh)" -- -d -b /usr/local/bin

# copy the task file
COPY Taskfile.yml .

# Install the Cargo-based tools (takes a long time)
RUN task setup-cargo-tools

# install homebrew
# https://stackoverflow.com/questions/58292862/how-to-install-homebrew-on-ubuntu-inside-docker-container
RUN useradd -m -s /bin/zsh linuxbrew && \
    usermod -aG sudo linuxbrew &&  \
    mkdir -p /home/linuxbrew/.linuxbrew && \
    chown -R linuxbrew: /home/linuxbrew/.linuxbrew
USER linuxbrew
RUN /bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/master/install.sh)"
USER root
RUN chown -R $CONTAINER_USER: /home/linuxbrew/.linuxbrew

RUN task setup-other-devtools

# FROM chef AS planner
# RUN cargo setup-dev

COPY . .

# FROM chef AS builder 
# COPY --from=planner /app/recipe.json recipe.json
# Build dependencies - this is the caching Docker layer!
# RUN cargo chef cook --release --recipe-path recipe.json
# Build application
# COPY . .
# RUN cargo build --release --bin wireguard-vanity-address

# We do not need the Rust toolchain to run the binary!
# FROM debian:buster-slim AS runtime
# WORKDIR app
# COPY --from=builder /app/target/release/wireguard-vanity-address /usr/local/bin

ENTRYPOINT ["/usr/local/bin/wireguard-vanity-address"]
CMD ["Rich"] # default is "Rich"; supply your own string as a parameter
