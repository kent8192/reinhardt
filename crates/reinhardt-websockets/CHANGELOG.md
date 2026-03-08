# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0](https://github.com/kent8192/reinhardt-web/releases/tag/reinhardt-websockets@v0.1.0) - 2026-03-08

### Added

- add default rate limiting for websocket connections

### Fixed

- *(websockets)* release connection slot on disconnect in RateLimitMiddleware
- *(websockets)* add non_exhaustive to ConnectionContext
- *(websockets)* release lock before send in Room::send_to
- apply middleware to upgrade and add graceful shutdown
- fix missing match arms in connection state machine
- add match arms for BinaryPayload, HeartbeatTimeout, SlowConsumer
- add error handling for connection, room, and consumer operations
- resolve clippy warnings across workspace
- implement auto-reconnect with exponential backoff
- add connection timeout for WebSocket (#508)
- handle partial failure in room broadcast (#511)

### Security

- add authentication support for Redis channel layer
- add compression negotiation limits with size-bounded decompression
- add configurable ping/pong keepalive intervals
- sanitize error messages to prevent internal state leakage
- fix concurrency races, overflow, and resource exhaustion vulnerabilities
- enable default message size limits
- add origin header validation

### Documentation

- add missing doc comments for public API modules and types

### Styling

- apply formatting to files introduced by merge from main
- apply rustfmt to pre-existing formatting violations in 16 files
- apply rustfmt after clippy auto-fix
- fix remaining clippy warnings across workspace
- apply rustfmt formatting to workspace files
- apply rustfmt formatting to 146 files
- apply rstest convention to new tests
- fix rustfmt formatting in connection.rs

### Maintenance

- *(testing)* add insta snapshot testing dependency across all crates

### Other

- resolve conflicts with origin/main
