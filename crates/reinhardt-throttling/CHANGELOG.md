# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0-rc.6](https://github.com/kent8192/reinhardt-web/compare/reinhardt-throttling@v0.1.0-rc.5...reinhardt-throttling@v0.1.0-rc.6) - 2026-03-09

### Documentation

- *(throttling)* add missing public API documentation

## [0.1.0-rc.5](https://github.com/kent8192/reinhardt-web/compare/reinhardt-throttling@v0.1.0-rc.4...reinhardt-throttling@v0.1.0-rc.5) - 2026-03-07

### Documentation

- add missing doc comments for public API modules and types

### Other

- resolve conflicts with main branch

### Testing

- *(throttling)* add test coverage for get_country_code GeoIP path

## [0.1.0-rc.2](https://github.com/kent8192/reinhardt-web/compare/reinhardt-throttling@v0.1.0-rc.1...reinhardt-throttling@v0.1.0-rc.2) - 2026-03-04

### Fixed

- *(throttling)* use per-key bucket state in TokenBucket rate limiter
- *(meta)* fix workspace inheritance and authors metadata

### Maintenance

- *(testing)* add insta snapshot testing dependency across all crates

## [0.1.0-rc.1](https://github.com/kent8192/reinhardt-web/compare/reinhardt-throttling@v0.1.0-alpha.2...reinhardt-throttling@v0.1.0-rc.1) - 2026-02-21

### Fixed

- return Result instead of panicking in TimeRange::new
- add TTL-based eviction to MemoryBackend
- check window expiration in get_count to prevent false denials
- validate refill interval and use wall clock for hour calculation
- use Lua script for atomic INCR/EXPIRE in Redis

### Security

- fix overflow, division-by-zero, and missing input validation
- add cache key validation to prevent injection

### Changed

- refactor!(throttling): remove unused key and backend fields from bucket structs

### Styling

- fix pre-existing clippy warnings and apply rustfmt
- apply rustfmt to pre-existing unformatted files

## [0.1.0-alpha.2](https://github.com/kent8192/reinhardt-web/compare/reinhardt-throttling@v0.1.0-alpha.1...reinhardt-throttling@v0.1.0-alpha.2) - 2026-02-03

### Other

- *(package)* replace version.workspace with explicit versions
