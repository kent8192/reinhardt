# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0](https://github.com/kent8192/reinhardt-web/releases/tag/reinhardt-server@v0.1.0) - 2026-03-08

### Fixed

- implement sliding window rate limiting and document HTTP/2 middleware gap
- *(server)* replace reinhardt-test with local poll_until helper

### Security

- reduce WebSocket log verbosity to prevent data exposure
- add periodic eviction of stale rate limit entries
- add request body size limits and decompression bomb prevention
- add trusted proxy validation for X-Forwarded-For

### Styling

- apply rustfmt to pre-existing unformatted files

### Maintenance

- *(testing)* add insta snapshot testing dependency across all crates
- *(license)* migrate from MIT/Apache-2.0 to BSD 3-Clause
