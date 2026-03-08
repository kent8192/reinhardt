# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0](https://github.com/kent8192/reinhardt-web/releases/tag/reinhardt-throttling@v0.1.0) - 2026-03-08

### Changed

- refactor!(throttling): remove unused key and backend fields from bucket structs

### Fixed

- *(throttling)* use per-key bucket state in TokenBucket rate limiter
- *(meta)* fix workspace inheritance and authors metadata
- return Result instead of panicking in TimeRange::new
- add TTL-based eviction to MemoryBackend
- check window expiration in get_count to prevent false denials
- validate refill interval and use wall clock for hour calculation
- use Lua script for atomic INCR/EXPIRE in Redis

### Security

- fix overflow, division-by-zero, and missing input validation
- add cache key validation to prevent injection

### Documentation

- add missing doc comments for public API modules and types

### Testing

- *(throttling)* add test coverage for get_country_code GeoIP path

### Styling

- fix pre-existing clippy warnings and apply rustfmt
- apply rustfmt to pre-existing unformatted files

### Maintenance

- *(testing)* add insta snapshot testing dependency across all crates

### Other

- resolve conflicts with main branch
