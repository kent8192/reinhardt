FROM myoung34/github-runner:2.322.0

# Install Rust toolchain
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable \
	&& . "$HOME/.cargo/env" \
	&& rustup component add clippy rustfmt

# Install cargo tools (matching AWS Spot runner toolset)
RUN . "$HOME/.cargo/env" \
	&& cargo install cargo-nextest --locked \
	&& cargo install cargo-make --locked

# Install mold linker
RUN apt-get update \
	&& apt-get install -y --no-install-recommends mold \
	&& rm -rf /var/lib/apt/lists/*

# Copy healthcheck script
COPY scripts/healthcheck.sh /usr/local/bin/healthcheck.sh
RUN chmod +x /usr/local/bin/healthcheck.sh

# Ensure cargo is in PATH for all users
ENV PATH="/root/.cargo/bin:${PATH}"
