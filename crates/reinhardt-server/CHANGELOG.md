# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0-rc.16](https://github.com/kent8192/reinhardt-web/compare/reinhardt-server@v0.1.0-rc.15...reinhardt-server@v0.1.0-rc.16) - 2026-04-19

### Fixed

- *(admin)* prevent static files from returning Content-Type: application/json
- *(http)* convert errors to responses within middleware chain

### Maintenance

- upgrade workspace dependencies to latest versions
- *(build)* reduce tokio features and enable debug=1 profile for faster compilation

### Styling

- *(server)* apply rustfmt formatting to diagnostic warning

## [0.1.0-rc.15](https://github.com/kent8192/reinhardt-web/compare/reinhardt-server@v0.1.0-rc.14...reinhardt-server@v0.1.0-rc.15) - 2026-03-29

### Maintenance

- update rust toolchain to 1.94.1 and set MSRV 1.94.0

## [0.1.0-rc.14](https://github.com/kent8192/reinhardt-web/compare/reinhardt-server@v0.1.0-rc.13...reinhardt-server@v0.1.0-rc.14) - 2026-03-24

### Documentation

- *(crates)* update version references from 0.1.0-alpha.1 to 0.1.0-rc.13 across all READMEs
- *(readme)* fix documentation discrepancies across crate READMEs
- address Copilot review feedback on crate READMEs

## [0.1.0-rc.12](https://github.com/kent8192/reinhardt-web/compare/reinhardt-server@v0.1.0-rc.11...reinhardt-server@v0.1.0-rc.12) - 2026-03-18

### Security

- *(server)* route error handler through SafeErrorResponse

## [0.1.0-rc.9](https://github.com/kent8192/reinhardt-web/compare/reinhardt-server@v0.1.0-rc.8...reinhardt-server@v0.1.0-rc.9) - 2026-03-15

### Styling

- add explanatory comments to remaining #[allow(dead_code)] attributes

## [0.1.0-rc.2](https://github.com/kent8192/reinhardt-web/compare/reinhardt-server@v0.1.0-rc.1...reinhardt-server@v0.1.0-rc.2) - 2026-03-04

### Maintenance

- *(testing)* add insta snapshot testing dependency across all crates

## [0.1.0-rc.1](https://github.com/kent8192/reinhardt-web/compare/reinhardt-server@v0.1.0-alpha.8...reinhardt-server@v0.1.0-rc.1) - 2026-02-23

### Maintenance

- *(license)* migrate from MIT/Apache-2.0 to BSD 3-Clause

## [0.1.0-alpha.8](https://github.com/kent8192/reinhardt-web/compare/reinhardt-server@v0.1.0-alpha.7...reinhardt-server@v0.1.0-alpha.8) - 2026-02-21

### Fixed

- implement sliding window rate limiting and document HTTP/2 middleware gap

### Security

- reduce WebSocket log verbosity to prevent data exposure
- add periodic eviction of stale rate limit entries
- add request body size limits and decompression bomb prevention
- add trusted proxy validation for X-Forwarded-For

### Styling

- apply rustfmt to pre-existing unformatted files

## [0.1.0-alpha.7](https://github.com/kent8192/reinhardt-web/compare/reinhardt-server@v0.1.0-alpha.6...reinhardt-server@v0.1.0-alpha.7) - 2026-02-14

### Maintenance

- updated the following local packages: reinhardt-di

## [0.1.0-alpha.6](https://github.com/kent8192/reinhardt-web/compare/reinhardt-server@v0.1.0-alpha.5...reinhardt-server@v0.1.0-alpha.6) - 2026-02-12

### Maintenance

- updated the following local packages: reinhardt-core, reinhardt-http, reinhardt-di

## [0.1.0-alpha.5](https://github.com/kent8192/reinhardt-web/compare/reinhardt-server@v0.1.0-alpha.4...reinhardt-server@v0.1.0-alpha.5) - 2026-02-09

### Fixed

- *(server)* replace reinhardt-test with local poll_until helper

### Reverted

- undo PR [[#219](https://github.com/kent8192/reinhardt-web/issues/219)](https://github.com/kent8192/reinhardt-web/issues/219) version bumps for unpublished crates

## [0.1.0-alpha.4](https://github.com/kent8192/reinhardt-web/compare/reinhardt-server@v0.1.0-alpha.3...reinhardt-server@v0.1.0-alpha.4) - 2026-02-06

### Other

- updated the following local packages: reinhardt-di

## [0.1.0-alpha.3](https://github.com/kent8192/reinhardt-web/compare/reinhardt-server@v0.1.0-alpha.2...reinhardt-server@v0.1.0-alpha.3) - 2026-02-03

### Other

- updated the following local packages: reinhardt-core, reinhardt-http, reinhardt-di

## [0.1.0-alpha.2](https://github.com/kent8192/reinhardt-web/compare/reinhardt-server@v0.1.0-alpha.1...reinhardt-server@v0.1.0-alpha.2) - 2026-02-03

### Other

- add release-plz migration markers to CHANGELOGs
- *(changelog)* remove obsolete [0.1.0] sections
- *(changelog)* add missing 0.1.0-alpha.1 release entries
- *(package)* replace version.workspace with explicit versions

### Breaking Changes
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

