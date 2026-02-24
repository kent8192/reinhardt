# Dockerfile for examples-github-issues API testing (GraphQL + JWT)
# Build context: workspace root (../..)

FROM rust:1.91.1-slim AS builder

WORKDIR /app

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    curl \
    && rm -rf /var/lib/apt/lists/*

# Copy entire workspace for building (workspace dependencies require this)
COPY . .

# Build only the target example binary
RUN cargo build --release --package examples-github-issues --bin examples-github-issues

# Runtime stage
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    ca-certificates \
    curl \
    && rm -rf /var/lib/apt/lists/*

# Copy the built binary
COPY --from=builder /app/target/release/examples-github-issues /usr/local/bin/manage

# Copy database file if exists (for SQLite examples)
COPY --from=builder /app/examples/examples-github-issues/db.sqlite3 /app/db.sqlite3

# Copy settings files
COPY --from=builder /app/examples/examples-github-issues/settings /app/settings

WORKDIR /app

EXPOSE 8000

# Run the server
CMD ["manage", "runserver", "0.0.0.0:8000"]
