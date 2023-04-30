# Dockerfile to build the prql development environment.
# See https://prql-lang.org/book/contributing/developing-with-docker.html for
# more details.

# Some of this is shared with /.devcontainer/base-image/Dockerfile.

# Build with:
#
#   cd <top-level-PRQL-directory>
#   docker build -t prql .
#
# Invoke with:
#
#   cd <top-level-PRQL-directory>
#   docker run -it -v $(pwd)/:/src -p 3000:3000 prql
#
# You'll see a root@xxxxxxxxx:/app/# prompt
# Enter the relevant command
# Ctrl-c to exit that task
# Ctrl-d to close down the Docker machine

FROM rust:1.65.0-slim-buster
# Surprising this isn't already in the rust image
ENV PATH="/root/.cargo/bin:${PATH}"

# ========= Install essential apt packages =========
RUN apt-get -yq update \
  && apt install -y \
  cmake \
  pkg-config \
  libssl-dev \
  git \
  gcc \
  build-essential \
  clang \
  libclang-dev \
  python3.7 \
  python3-pip \
  curl \
  gnupg \
  ca-certificates \
  && rm -rf /var/lib/apt/lists/*

# ========= Install task =========
RUN sh -c "$(curl --location https://taskfile.dev/install.sh)" -- -d -b /usr/local/bin

# ========= Install cargo-tools =========
COPY Taskfile.yml /tmp/Taskfile.yml
RUN task -t /tmp/Taskfile.yml install-cargo-tools

# ========= Set up workdir =========
WORKDIR /src

# TODO: currently this doesn't support doing things like running the playground,
# since we don't install hugo & node. Default `apt` doesn't install up-to-date
# versions, and I couldn't get `brew` working on an ARM machine. But that would
# be a welcome addition in the future.

# TODO: we could consider building the dependencies here, to take advantage of
# Docker's caching. It's possible but not completely trivial:
# https://stackoverflow.com/a/60590697/3064736
