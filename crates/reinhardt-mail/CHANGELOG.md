# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.3.7](https://github.com/kent8192/reinhardt-web/compare/reinhardt-mail@v0.3.6...reinhardt-mail@v0.3.7) - 2026-08-12

### Testing

- *(mail)* cover message values and delivery
- *(mail)* cover backend selection and file delivery
- *(mail)* cover role helper failure policies
- *(mail)* strengthen coverage assertions
- *(mail)* move cross-crate coverage
- *(mail)* strengthen backend and failure coverage
- *(mail)* align backend coverage with public APIs
- *(mail)* close review coverage gaps

## [0.3.0](https://github.com/kent8192/reinhardt-web/compare/reinhardt-mail@v0.2.2...reinhardt-mail@v0.3.0) - 2026-06-28

Stable release of `reinhardt-mail` for the Reinhardt 0.3.0 line. This
entry consolidates the 0.3.0 release-candidate series into one
stable release section.

### Migration Notes

- Review the root CHANGELOG and `instructions/MIGRATION_0.3.md` before upgrading from 0.2.x.

### Maintenance

- merge main into develop/0.3.0

## [0.2.2](https://github.com/kent8192/reinhardt-web/compare/reinhardt-mail@v0.2.1...reinhardt-mail@v0.2.2) - 2026-06-25

### Documentation

- update version references to v0.2.1

## [0.2.0](https://github.com/kent8192/reinhardt-web/compare/reinhardt-mail@v0.1.3...reinhardt-mail@v0.2.0) - 2026-06-11

Stable release of `reinhardt-mail` for the Reinhardt 0.2.0 line. This
entry consolidates the 0.2.0 release-candidate series into one
stable release section.

### Migration Notes

- Review the breaking changes listed below before upgrading from 0.1.x.
- See the root CHANGELOG and `instructions/MIGRATION_0.2.md` for cross-crate migration guidance.

### Breaking Changes

- *(conf)* [**breaking**] remove legacy advanced settings types

### Changed

- [**breaking**] align develop/0.2.0 with main, preserving 8 feature crates

### Deprecated

- bridge SmtpConfig to the EmailSettings fragment
- shield smtp_integration test from SmtpConfig deprecation

### Fixed

- *(mail)* accept settings email fragments
- *(conf)* [**breaking**] remove legacy advanced settings types
- *(ci)* recover develop release-plz prerelease
- *(ci)* unblock release docs and form tests

### Documentation

- *(mail,conf)* fix unresolved intra-doc links to settings fragments

### Maintenance

- update Cargo.toml dependencies

## [0.1.0](https://github.com/kent8192/reinhardt-web/compare/reinhardt-mail@v0.1.0-rc.30...reinhardt-mail@v0.1.0) - 2026-05-22

Initial stable release of `reinhardt-mail` as part of the reinhardt-web
0.1.0 release. Provides an async email-sending API with pluggable
backends (SMTP, file, console, in-memory) and a `ProjectSettings`-driven
configuration surface.

For the workspace-wide release narrative, see the [root CHANGELOG](https://github.com/kent8192/reinhardt-web/blob/main/CHANGELOG.md#010---2026-05-22).
Per-prerelease history is in the [Release Discussions](https://github.com/kent8192/reinhardt-web/discussions/categories/release).

### Capabilities at 0.1.0

- **Async backend trait** — A backend-agnostic `EmailBackend` trait
  with built-in SMTP (lettre), filesystem, console, and locmem
  implementations. Backends are selected via configuration and share
  a common `send_messages()` surface.
- **Hardened SMTP transport** — TLS hostname verification is enforced
  by default, credentials are zeroized after use, and the connection
  pool exposes semaphore-based concurrency limits with validated
  configuration parameters.
- **Header and address validation** — Addresses are parsed with
  RFC 2822 / IDNA-aware validators, header names are rejected when
  they violate RFC 2822, and length limits guard against
  header-injection and resource-exhaustion vectors.
- **Multipart and attachment support** — Messages can carry plain
  and HTML alternatives plus typed attachments via `mime` /
  `mime_guess`; dev backends render attachments faithfully so
  fixtures match production output.
- **Settings-driven configuration** — Backend selection, SMTP
  endpoints, and pool sizing flow through `reinhardt-conf` settings
  with config errors surfaced even when `fail_silently` is enabled.

### Notable Breaking Changes

`reinhardt-mail` did not introduce its own framework-wide breaking
changes in 0.1.0. Workspace-level breaking changes are tracked at the
[Breaking Changes Discussions](https://github.com/kent8192/reinhardt-web/discussions/categories/breaking-changes)
and summarized in the [root CHANGELOG](https://github.com/kent8192/reinhardt-web/blob/main/CHANGELOG.md#010---2026-05-22).

### Migration Notes

See the workspace-level [Migration Guide](https://github.com/kent8192/reinhardt-web/blob/main/CHANGELOG.md#010---2026-05-22)
for the full upgrade flow. Crate-specific notes:

- The `lettre` dependency floor is `0.11.22` to clear RUSTSEC-2026-0141.
  Pin downstream backends accordingly if you re-export them.
- Configuration is now read through `ProjectSettings` rather than
  ad-hoc `env::var()` calls; see [#4295](https://github.com/kent8192/reinhardt-web/discussions/4295).
