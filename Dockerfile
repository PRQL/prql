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
# docker run -it -v $(pwd)/:/src -p 3000:3000 prql
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

# A previous version of this installed packaged from `apt` — that's fine in
# principle, but it didn't have recent enough versions.
# ========= Install brew =========
RUN /bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"
ENV PATH="/home/linuxbrew/.linuxbrew/bin:${PATH}"

# ========= Install task =========
RUN brew install go-task/tap/go-task

# ========= Set up workdir & copy the taskfile =========
WORKDIR /src
COPY Taskfile.yml .

# ========= Run our standard dev setup tasks =========
RUN task setup-dev

# TODO: we could consider building the dependencies here, to take advantage of
# Docker's caching. It's possible but not completely trivial:
# https://stackoverflow.com/a/60590697/3064736
