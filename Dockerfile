# Dockerfile to build the prql development environment

# Build with docker build -t prql .
# Invoke with ????? docker run wgvanity [ string ]

FROM rust:1.63.0-buster
WORKDIR app

# ========= Install essential packages =========
RUN apt-get update; apt install -y cmake pkg-config libssl-dev git gcc build-essential clang libclang-dev 
RUN apt-get install -y python3.7 python3-pip

# ========= Install task =========
RUN sh -c "$(curl --location https://taskfile.dev/install.sh)" -- -d -b /usr/local/bin

# ========= Install the cargo tools =========
RUN cargo install --locked \
        cargo-audit \
        cargo-insta \
        cargo-release \
        default-target \
        mdbook \
        mdbook-admonish \
        mdbook-toc \
        wasm-bindgen-cli \
        wasm-pack

# ========= Install homebrew =========
# https://stackoverflow.com/questions/58292862/how-to-install-homebrew-on-ubuntu-inside-docker-container
RUN useradd -m -s /bin/zsh linuxbrew && \
    usermod -aG sudo linuxbrew &&  \
    mkdir -p /home/linuxbrew/.linuxbrew && \
    chown -R linuxbrew: /home/linuxbrew/.linuxbrew
USER linuxbrew
RUN /bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/master/install.sh)"
USER root
RUN chown -R $CONTAINER_USER: /home/linuxbrew/.linuxbrew
ENV PATH="/home/linuxbrew/.linuxbrew/bin:${PATH}"

# ========= Install the other dev tools =========
RUN python3 -m pip install -U pre-commit					# Install pre-commit
RUN python3 -m pip install maturin							# Install maturin
RUN brew install \
	hugo \
	npm
RUN npm install --location=global \
	prettier \
	prettier-plugin-go-template 					

# ========= Copy the taskfile =========
COPY Taskfile.yml .


# ========= Need to figure out the rest 
# ========= Create a volume so tools can access local directory 
# ========= Create commands to run the playground
# ========= Create command to watch for changes and re-run


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
