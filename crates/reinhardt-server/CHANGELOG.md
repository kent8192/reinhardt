# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.4.0-alpha.7](https://github.com/kent8192/reinhardt-web/compare/reinhardt-server@v0.4.0-alpha.6...reinhardt-server@v0.4.0-alpha.7) - 2026-08-19

### Documentation

- update version references to v0.3.3
- update version references to v0.3.4
- update version references to v0.3.5
- update version references to v0.3.6
- update version references to v0.3.7
- update version references to v0.3.8

### Maintenance

- merge main into develop/0.4.0

## [0.4.0-alpha.6](https://github.com/kent8192/reinhardt-web/compare/reinhardt-server@v0.4.0-alpha.5...reinhardt-server@v0.4.0-alpha.6) - 2026-08-06

### Documentation

- *(release)* restore coherent alpha.3 references

## [0.4.0-alpha.1](https://github.com/kent8192/reinhardt-web/compare/reinhardt-server@v0.3.2...reinhardt-server@v0.4.0-alpha.1) - 2026-07-21

### Fixed

- *(release)* restore develop prerelease lifecycle

### Maintenance

- merge main into develop/0.4.0
- merge develop/0.4.0 into remove-anyhow branch
## [0.3.10](https://github.com/kent8192/reinhardt-web/compare/reinhardt-server@v0.3.9...reinhardt-server@v0.3.10) - 2026-08-22

### Maintenance

- update Cargo.toml dependencies

## [0.3.9](https://github.com/kent8192/reinhardt-web/compare/reinhardt-server@v0.3.8...reinhardt-server@v0.3.9) - 2026-08-21

### Maintenance

- update Cargo.toml dependencies

## [0.3.8](https://github.com/kent8192/reinhardt-web/compare/reinhardt-server@v0.3.7...reinhardt-server@v0.3.8) - 2026-08-16

### Maintenance

- update Cargo.toml dependencies

## [0.3.7](https://github.com/kent8192/reinhardt-web/compare/reinhardt-server@v0.3.6...reinhardt-server@v0.3.7) - 2026-08-12

### Maintenance

- update Cargo.toml dependencies

## [0.3.6](https://github.com/kent8192/reinhardt-web/compare/reinhardt-server@v0.3.5...reinhardt-server@v0.3.6) - 2026-08-04

### Maintenance

- update Cargo.toml dependencies

## [0.3.5](https://github.com/kent8192/reinhardt-web/compare/reinhardt-server@v0.3.4...reinhardt-server@v0.3.5) - 2026-08-02

### Maintenance

- update Cargo.toml dependencies

## [0.3.4](https://github.com/kent8192/reinhardt-web/compare/reinhardt-server@v0.3.3...reinhardt-server@v0.3.4) - 2026-07-30

### Maintenance

- update Cargo.toml dependencies

## [0.3.3](https://github.com/kent8192/reinhardt-web/compare/reinhardt-server@v0.3.2...reinhardt-server@v0.3.3) - 2026-07-28

### Maintenance

- update Cargo.toml dependencies

## [0.3.2](https://github.com/kent8192/reinhardt-web/compare/reinhardt-server@v0.3.1...reinhardt-server@v0.3.2) - 2026-07-14

### Fixed

- *(ci)* allow intentional dependency-version duplicates

## [0.3.0](https://github.com/kent8192/reinhardt-web/compare/reinhardt-server@v0.2.0...reinhardt-server@v0.3.0) - 2026-06-28

Stable release of `reinhardt-server` for the Reinhardt 0.3.0 line. This
crate moves with the coordinated Reinhardt 0.3.0 release train.

### Migration Notes

- Review the root CHANGELOG and `instructions/MIGRATION_0.3.md` before upgrading from 0.2.x.

### Maintenance

- align crate release metadata with the Reinhardt 0.3.0 stable release train.

## [0.2.0](https://github.com/kent8192/reinhardt-web/compare/reinhardt-server@v0.1.3...reinhardt-server@v0.2.0) - 2026-06-11

Stable release of `reinhardt-server` for the Reinhardt 0.2.0 line. This
entry consolidates the 0.2.0 release-candidate series into one
stable release section.

### Migration Notes

- Review the breaking changes listed below before upgrading from 0.1.x.
- See the root CHANGELOG and `instructions/MIGRATION_0.2.md` for cross-crate migration guidance.

### Added

- *(server)* add RateLimitSettings fragment

### Changed

- [**breaking**] align develop/0.2.0 with main, preserving 8 feature crates

### Deprecated

- *(server)* deprecate RateLimitConfig in favor of RateLimitSettings

### Fixed

- *(ci)* recover develop release-plz prerelease

### Performance

- atomize facade dependency feature gates

### Maintenance

- update Cargo.toml dependencies
- add reinhardt-conf and serde deps for rate-limit settings

## [0.1.0](https://github.com/kent8192/reinhardt-web/compare/reinhardt-server@v0.1.0-rc.30...reinhardt-server@v0.1.0) - 2026-05-22

Initial stable release of `reinhardt-server` as part of the
reinhardt-web 0.1.0 release. The hyper-based HTTP server launcher that
wires routers, middleware, and the request-scoped DI container into a
production-ready listener.

For the workspace-wide release narrative, see the [root CHANGELOG](https://github.com/kent8192/reinhardt-web/blob/main/CHANGELOG.md#010---2026-05-22).
Per-prerelease history is in the [Release Discussions](https://github.com/kent8192/reinhardt-web/discussions/categories/release).

### Capabilities at 0.1.0

- **Hyper-based listener** — Drives the assembled router and middleware
  chain over hyper, with sensible defaults and a reduced Tokio feature
  surface; the build profile uses `debug=1` to keep compile times
  manageable without losing essential symbols.
- **Hardened error responses** — All server-level errors flow through
  `SafeErrorResponse` so stack traces and internal details never leak
  to clients.
- **Sliding-window rate limiting** — Built-in rate limiter applies a
  sliding window and evicts stale entries periodically; trusted-proxy
  validation gates `X-Forwarded-For` so request IP attribution stays
  honest behind load balancers.
- **Body-size and decompression guardrails** — Request body size
  limits and decompression-bomb prevention sit in front of the
  middleware chain, so malicious payloads are rejected before they
  reach handlers.
- **Reduced log surface** — WebSocket logging verbosity is dialled
  back so the access log does not exfiltrate payload data.

### Notable Breaking Changes

This crate's API surface stabilized incrementally during the rc
cycle; cross-crate breaking changes are catalogued in the
[Breaking Changes Discussions](https://github.com/kent8192/reinhardt-web/discussions/categories/breaking-changes).

### Migration Notes

This is the first stable release, so there is no prior stable version
to migrate from. See the [root CHANGELOG](https://github.com/kent8192/reinhardt-web/blob/main/CHANGELOG.md#010---2026-05-22)
for the cross-crate migration guide.
