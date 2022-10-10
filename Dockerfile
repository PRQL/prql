# Dockerfile to build the prql development environment

# Build with docker build -t prql .
# Invoke with docker run -it -v $(pwd)/:/app -p 3000:3000 prql
#   To run playground:
#			- cd playground
# 		- npm start
#		To build website:
#		To build book:
#		To ... (what else?)

FROM rust:1.63.0-slim-buster

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

# ========= Set up workdir & copy the taskfile =========
WORKDIR /app
COPY Taskfile.yml .

# ========= Install cargo-tools =========
RUN task install-cargo-tools

# ========= Install Node 16.x =========
RUN curl -sL https://deb.nodesource.com/setup_16.x | bash -
RUN apt install -y nodejs	

# ========= Install remaining development tools using task =========
RUN task install-pre-commit
RUN task install-npm-dependencies
RUN task install-precommit-install-hooks

# ========= Install hugo =========
RUN curl -L https://github.com/gohugoio/hugo/releases/download/v0.91.2/hugo_0.91.2_Linux-64bit.deb -o hugo.deb \
	&& apt install ./hugo.deb

ENTRYPOINT ["/bin/bash"]