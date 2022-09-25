# Dockerfile to build the prql development environment

# Build with docker build -t prql .
# Invoke with ????? docker run wgvanity [ string ]

FROM rust:1.63.0-buster
WORKDIR app

# ========= Install essential apt packages =========
RUN apt-get update; apt install -y \
	cmake \
	pkg-config \
	libssl-dev \
	git \
	gcc \
	build-essential \
	clang \
	libclang-dev \
	python3.7 \
	python3-pip
RUN rm -rf /var/lib/apt/lists/*

# ========= Install task =========
RUN sh -c "$(curl --location https://taskfile.dev/install.sh)" -- -d -b /usr/local/bin

# ========= Copy the taskfile =========
COPY Taskfile.yml .

# ========= Install homebrew =========
RUN task install-brew
ENV PATH="/home/linuxbrew/.linuxbrew/bin:${PATH}"

# ========= Install the cargo tools =========
RUN task install-cargo-tools

# ========= Install the other dev tools =========
RUN task install-pre-commit
RUN task install-brew-dependencies
RUN task install-npm-dependencies
RUN task install-precommit-install-hooks

# ========= Need to figure out the rest 
# ========= Create a volume so tools can access local directory 
# ========= Create commands to run the playground
# ========= Create command to watch for changes and re-run


# FROM chef AS planner
# RUN cargo setup-dev

COPY . .

ENTRYPOINT ["cargo test"]

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

# ENTRYPOINT ["/usr/local/bin/wireguard-vanity-address"]
# CMD ["Rich"] # default is "Rich"; supply your own string as a parameter
