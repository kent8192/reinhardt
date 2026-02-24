# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0-alpha.16](https://github.com/kent8192/reinhardt-web/compare/reinhardt-websockets@v0.1.0-alpha.15...reinhardt-websockets@v0.1.0-alpha.16) - 2026-02-24

### Maintenance

- updated the following local packages: reinhardt-pages

## [0.1.0-alpha.15](https://github.com/kent8192/reinhardt-web/compare/reinhardt-websockets@v0.1.0-alpha.14...reinhardt-websockets@v0.1.0-alpha.15) - 2026-02-23

### Maintenance

- updated the following local packages: reinhardt-di, reinhardt-auth, reinhardt-pages

## [0.1.0-alpha.14](https://github.com/kent8192/reinhardt-web/compare/reinhardt-websockets@v0.1.0-alpha.13...reinhardt-websockets@v0.1.0-alpha.14) - 2026-02-23

### Maintenance

- updated the following local packages: reinhardt-auth, reinhardt-pages

## [0.1.0-alpha.13](https://github.com/kent8192/reinhardt-web/compare/reinhardt-websockets@v0.1.0-alpha.12...reinhardt-websockets@v0.1.0-alpha.13) - 2026-02-21

### Added

- add default rate limiting for websocket connections

### Fixed

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

### Styling

- apply formatting to files introduced by merge from main
- apply rustfmt to pre-existing formatting violations in 16 files
- apply rustfmt after clippy auto-fix
- fix remaining clippy warnings across workspace
- apply rustfmt formatting to workspace files
- apply rustfmt formatting to 146 files
- apply rstest convention to new tests
- fix rustfmt formatting in connection.rs

## [0.1.0-alpha.12](https://github.com/kent8192/reinhardt-web/compare/reinhardt-websockets@v0.1.0-alpha.11...reinhardt-websockets@v0.1.0-alpha.12) - 2026-02-16

### Maintenance

- updated the following local packages: reinhardt-auth, reinhardt-pages

## [0.1.0-alpha.11](https://github.com/kent8192/reinhardt-web/compare/reinhardt-websockets@v0.1.0-alpha.10...reinhardt-websockets@v0.1.0-alpha.11) - 2026-02-15

### Maintenance

- updated the following local packages: reinhardt-auth, reinhardt-pages

## [0.1.0-alpha.10](https://github.com/kent8192/reinhardt-web/compare/reinhardt-websockets@v0.1.0-alpha.9...reinhardt-websockets@v0.1.0-alpha.10) - 2026-02-14

### Maintenance

- updated the following local packages: reinhardt-auth, reinhardt-pages

## [0.1.0-alpha.9](https://github.com/kent8192/reinhardt-web/compare/reinhardt-websockets@v0.1.0-alpha.8...reinhardt-websockets@v0.1.0-alpha.9) - 2026-02-14

### Maintenance

- updated the following local packages: reinhardt-auth, reinhardt-pages

## [0.1.0-alpha.8](https://github.com/kent8192/reinhardt-web/compare/reinhardt-websockets@v0.1.0-alpha.7...reinhardt-websockets@v0.1.0-alpha.8) - 2026-02-14

### Maintenance

- updated the following local packages: reinhardt-auth, reinhardt-pages

## [0.1.0-alpha.7](https://github.com/kent8192/reinhardt-web/compare/reinhardt-websockets@v0.1.0-alpha.6...reinhardt-websockets@v0.1.0-alpha.7) - 2026-02-14

### Maintenance

- updated the following local packages: reinhardt-di, reinhardt-auth, reinhardt-pages

## [0.1.0-alpha.6](https://github.com/kent8192/reinhardt-web/compare/reinhardt-websockets@v0.1.0-alpha.5...reinhardt-websockets@v0.1.0-alpha.6) - 2026-02-12

### Maintenance

- updated the following local packages: reinhardt-auth, reinhardt-pages, reinhardt-di

## [0.1.0-alpha.5](https://github.com/kent8192/reinhardt-web/compare/reinhardt-websockets@v0.1.0-alpha.4...reinhardt-websockets@v0.1.0-alpha.5) - 2026-02-10

### Fixed

- *(release)* revert unpublished crate versions to pre-release state

### Maintenance

- *(websockets)* remove manual CHANGELOG entries for release-plz

### Reverted

- undo release PR [[#215](https://github.com/kent8192/reinhardt-web/issues/215)](https://github.com/kent8192/reinhardt-web/issues/215) version bumps
- undo PR [[#219](https://github.com/kent8192/reinhardt-web/issues/219)](https://github.com/kent8192/reinhardt-web/issues/219) version bumps for unpublished crates

## [0.1.0-alpha.4](https://github.com/kent8192/reinhardt-web/compare/reinhardt-websockets@v0.1.0-alpha.3...reinhardt-websockets@v0.1.0-alpha.4) - 2026-02-06

### Other

- updated the following local packages: reinhardt-di, reinhardt-pages

## [0.1.0-alpha.3](https://github.com/kent8192/reinhardt-web/compare/reinhardt-websockets@v0.1.0-alpha.2...reinhardt-websockets@v0.1.0-alpha.3) - 2026-02-03

### Other

- updated the following local packages: reinhardt-pages, reinhardt-di

## [0.1.0-alpha.2](https://github.com/kent8192/reinhardt-web/compare/reinhardt-websockets@v0.1.0-alpha.1...reinhardt-websockets@v0.1.0-alpha.2) - 2026-02-03

### Other

- add release-plz migration markers to CHANGELOGs
- *(changelog)* remove obsolete [0.1.0] sections
- *(changelog)* add missing 0.1.0-alpha.1 release entries
- *(package)* replace version.workspace with explicit versions
- N/A

### Added
- Work in progress features (not yet released)

### Changed
- N/A

### Deprecated
- N/A

### Removed
- N/A

### Fixed
- N/A

### Security
- N/A


<!-- release-plz-separator -->
<!-- Entries below this line were created before release-plz adoption -->

## [0.1.0-alpha.1] - 2026-01-23

### Added

- Initial crates.io release

