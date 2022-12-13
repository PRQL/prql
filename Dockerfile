# Dockerfile to build the prql development environment
# For more details, see the USING_DOCKER.md file

# Build with:
#
# cd <top-level-PRQL-directory>
# docker build -t prql .
#
# Invoke with:
#
# cd <top-level-PRQL-directory>
# docker run -it -v $(pwd)/:/app -p 3000:3000 prql
# You'll see a root@xxxxxxxxx:/app/# prompt
# Enter the commands for the various tasks
# Ctl-C to exit that task
# Enter 'exit' to close down the Docker machine

# See USING_DOCKER.md for instructions for various tasks

FROM rust:1.64.0-slim-buster
# Surprising this isn't aready in the rust image
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

# ========= Install Node 16.x =========
RUN curl -sL https://deb.nodesource.com/setup_16.x | bash -
RUN apt install -y nodejs

# ========= Install hugo =========
RUN apt install -y hugo

# ========= Set up workdir & copy the taskfile =========
WORKDIR /src
COPY Taskfile.yml .

# ========= Install cargo-tools =========
RUN task install-cargo-tools

# ========= Install remaining development tools using task =========
RUN task install-npm-dependencies

# TODO: we could consider building the dependencies here, to take advantage of
# Docker's. It's possible but not completely trivial:
# https://stackoverflow.com/a/60590697/3064736

ENTRYPOINT ["/bin/bash"]
