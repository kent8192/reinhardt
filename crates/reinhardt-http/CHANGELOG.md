# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.4.0-alpha.7](https://github.com/kent8192/reinhardt-web/compare/reinhardt-http@v0.4.0-alpha.6...reinhardt-http@v0.4.0-alpha.7) - 2026-08-19

### Documentation

- update version references to v0.3.3
- update version references to v0.3.4
- update version references to v0.3.5
- update version references to v0.3.6
- update version references to v0.3.7
- update version references to v0.3.8

### Fixed

- *(auth)* deny bare legacy extension authentication

### Maintenance

- merge main into develop/0.4.0

## [0.4.0-alpha.6](https://github.com/kent8192/reinhardt-web/compare/reinhardt-http@v0.4.0-alpha.5...reinhardt-http@v0.4.0-alpha.6) - 2026-08-06

### Documentation

- *(release)* restore coherent alpha.3 references

## [0.4.0-alpha.1](https://github.com/kent8192/reinhardt-web/compare/reinhardt-http@v0.3.2...reinhardt-http@v0.4.0-alpha.1) - 2026-07-21

### Fixed

- restore merged runtime integrations
- *(release)* restore develop prerelease lifecycle

### Maintenance

- merge main into develop/0.4.0
- merge develop/0.4.0 into remove-anyhow branch
## [0.3.9](https://github.com/kent8192/reinhardt-web/compare/reinhardt-http@v0.3.8...reinhardt-http@v0.3.9) - 2026-08-21

### Documentation

- *(security)* define request surface boundaries
- *(security)* qualify caller-enforced boundaries
- *(security)* document remaining trust boundaries

### Fixed

- *(security)* qualify boundary control ownership

## [0.3.3](https://github.com/kent8192/reinhardt-web/compare/reinhardt-http@v0.3.2...reinhardt-http@v0.3.3) - 2026-07-28

### Fixed

- *(auth)* deny bare legacy extension authentication

## [0.3.2](https://github.com/kent8192/reinhardt-web/compare/reinhardt-http@v0.3.1...reinhardt-http@v0.3.2) - 2026-07-14

### Fixed

- *(auth)* complete active identity propagation
- *(auth)* preserve active identity compatibility

## [0.3.0](https://github.com/kent8192/reinhardt-web/compare/reinhardt-http@v0.2.0...reinhardt-http@v0.3.0) - 2026-06-28

Stable release of `reinhardt-http` for the Reinhardt 0.3.0 line. This
entry consolidates the 0.3.0 release-candidate series into one
stable release section.

### Migration Notes

- Review the root CHANGELOG and `instructions/MIGRATION_0.3.md` before upgrading from 0.2.x.

### Fixed

- *(http)* preserve PathParams iterator type

### Performance

- *(http)* reduce request allocation overhead
- *(http)* reduce router allocation hot path

## [0.2.0](https://github.com/kent8192/reinhardt-web/compare/reinhardt-http@v0.1.3...reinhardt-http@v0.2.0) - 2026-06-11

Stable release of `reinhardt-http` for the Reinhardt 0.2.0 line. This
entry consolidates the 0.2.0 release-candidate series into one
stable release section.

### Migration Notes

- Review the breaking changes listed below before upgrading from 0.1.x.
- See the root CHANGELOG and `instructions/MIGRATION_0.2.md` for cross-crate migration guidance.

### Changed

- [**breaking**] align develop/0.2.0 with main, preserving 8 feature crates

### Fixed

- *(ci)* recover develop release-plz prerelease

### Maintenance

- update Cargo.toml dependencies

## [0.1.0](https://github.com/kent8192/reinhardt-web/compare/reinhardt-http@v0.1.0-rc.30...reinhardt-http@v0.1.0) - 2026-05-22

Initial stable release of `reinhardt-http` as part of the reinhardt-web
0.1.0 release. Provides the foundational HTTP request and response
abstractions, the middleware-chain plumbing, and the sanitization /
input-validation helpers used by every higher layer in the framework.

For the workspace-wide release narrative, see the [root CHANGELOG](https://github.com/kent8192/reinhardt-web/blob/main/CHANGELOG.md#010---2026-05-22).
Per-prerelease history is in the [Release Discussions](https://github.com/kent8192/reinhardt-web/discussions/categories/release).

### Capabilities at 0.1.0

- **Typed `Request` / `Response` builders** — Owning request and
  response types with ergonomic header accessors (`append_header` for
  multi-value `Set-Cookie`, `with_header_if_absent` /
  `try_with_header_if_absent`), `query_as<T>()` for type-safe query
  deserialization, and `extract_bearer_token()` / `get_client_ip()` /
  `validate_content_type()` helpers migrated from `reinhardt-micro` for
  a single API surface.
- **Middleware chain with DI contribution** — `Middleware` trait
  exposes a `di_registrations` hook plus type-erased DI APIs so that
  middleware (sessions, auth, etc.) can register its own services into
  the request-scoped container, and `ExcludeMiddleware` provides
  declarative route exclusion. Errors raised inside the chain are
  converted to responses uniformly rather than panicking.
- **Sanitization and input-validation suite** — `validate_html_attr_name`,
  `is_safe_url` (with anchor-link support), path-traversal prevention,
  XSS escaping, `SafeErrorResponse` to prevent information leakage, and
  resource-limit configuration covering body size and upload paths.
- **Defensive runtime primitives** — Poison-recovery patterns replace
  every `Mutex::lock().unwrap()`, `char_indices`-based truncation
  preserves UTF-8 in log messages, and chunked uploads carry session
  timeouts. Uploaded files use cryptographically random filenames.
- **Cross-cutting integration points** — Per-request `InjectionContext`
  is installed on the HTTP request so `use_inject` resolves the active
  context; `AuthState::from_extensions()` discovers auth state
  directly from request extensions for downstream middleware.

### Notable Breaking Changes

This crate's public API was stabilized incrementally across the alpha
and rc lifecycle; the workspace-wide breaking changes are catalogued
in the [root CHANGELOG](https://github.com/kent8192/reinhardt-web/blob/main/CHANGELOG.md#010---2026-05-22)
and in the [Breaking Changes Discussions](https://github.com/kent8192/reinhardt-web/discussions/categories/breaking-changes).

### Migration Notes

This is the first stable release, so there is no prior stable version
to migrate from. See the [root CHANGELOG](https://github.com/kent8192/reinhardt-web/blob/main/CHANGELOG.md#010---2026-05-22)
for the cross-crate migration guide.
