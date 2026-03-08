# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0](https://github.com/kent8192/reinhardt-web/releases/tag/reinhardt-mail@v0.1.0) - 2026-03-08

### Fixed

- *(release)* use path-only dev-dep for reinhardt-test in cyclic crates
- document semaphore-based pool concurrency and add stress test
- validate header names against RFC 2822
- propagate config errors even when fail_silently is enabled
- add attachment rendering in dev backends and fix arbitrary header injection
- pin native-tls to =0.2.14 to fix build failure
- fix email validation and field access control (#512, #515, #517)
- enable proper TLS hostname verification in SMTP backend
- prevent email header injection via address validation

### Security

- add email length validation and credential zeroization
- fix HTML escaping, rate limiting, and validation

### Performance

- avoid unnecessary email body clone

### Documentation

- add missing doc comments for public API modules and types

### Styling

- apply rustfmt to pre-existing unformatted files
- collapse nested if statements per clippy::collapsible_if
- apply rustfmt formatting to workspace files
- apply code formatting to security fix files

### Maintenance

- *(testing)* add insta snapshot testing dependency across all crates
- *(workspace)* remove unpublished reinhardt-settings-cli and fix stale references
- add explanatory comments to undocumented #[allow(...)] attributes

### Other

- resolve conflicts with origin/main
