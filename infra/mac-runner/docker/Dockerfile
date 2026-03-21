FROM myoung34/github-runner:2.333.0-ubuntu-jammy

# Install build dependencies (pkg-config + libssl-dev required for openssl-sys crate)
RUN apt-get update \
	&& apt-get install -y --no-install-recommends pkg-config libssl-dev \
	&& rm -rf /var/lib/apt/lists/*

# Install Rust toolchain
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable \
	&& . "$HOME/.cargo/env" \
	&& rustup component add clippy rustfmt

# Install cargo-nextest via pre-built binary (avoids aws-lc-sys GCC bug in Docker)
RUN curl -LsSf https://get.nexte.st/latest/linux-arm | tar zxf - -C /usr/local/bin

# Install cargo-make from source
RUN . "$HOME/.cargo/env" \
	&& cargo install cargo-make --locked

# Install mold linker (repo version is too old, use GitHub release)
RUN MOLD_VERSION=2.35.1 \
	&& curl -LsSf "https://github.com/rui314/mold/releases/download/v${MOLD_VERSION}/mold-${MOLD_VERSION}-aarch64-linux.tar.gz" \
	| tar xzf - --strip-components=1 -C /usr/local

# Copy healthcheck script
COPY scripts/healthcheck.sh /usr/local/bin/healthcheck.sh
RUN chmod +x /usr/local/bin/healthcheck.sh

# Copy cleanup scripts for workspace management
COPY scripts/pre-start-cleanup.sh /usr/local/bin/pre-start-cleanup.sh
COPY scripts/entrypoint-wrapper.sh /usr/local/bin/entrypoint-wrapper.sh
RUN chmod +x /usr/local/bin/pre-start-cleanup.sh /usr/local/bin/entrypoint-wrapper.sh

# Ensure cargo is in PATH for all users
ENV PATH="/root/.cargo/bin:${PATH}"

ENTRYPOINT ["/usr/local/bin/entrypoint-wrapper.sh"]
