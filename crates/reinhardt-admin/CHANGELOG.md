# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0](https://github.com/kent8192/reinhardt-web/releases/tag/reinhardt-admin@v0.1.0) - 2026-03-08

### Added

- Initial release
- Admin panel functionality (via `reinhardt-panel`)
- CLI tool functionality (via `reinhardt-cli`)

### Changed

- convert relative paths to absolute paths
- clean up type naming, document intentional patterns

### Fixed

- *(admin)* replace unwrap with error propagation in insert values call
- *(deps)* align dependency versions to workspace definitions
- detect and report duplicate model registration
- sort columns for deterministic INSERT order
- apply clippy and fmt fixes to database module
- add panic prevention and error handling for admin operations
- pin native-tls to =0.2.14 to fix build failure
- add resource limits to prevent DoS in reinhardt-admin (#622, #623, #625, #626)
- fix raw SQL and info leakage in reinhardt-admin (#628, #630)
- add authentication and authorization enforcement to all endpoints
- use parameterized queries and escape identifiers to prevent SQL injection
- add input validation for mutation endpoints
- *(admin)* move database tests to integration crate to break circular publish chain

### Security

- add audit logging for all CRUD operations
- add CSP headers, CSRF token generation, and XSS prevention
- add input validation, file size limits, and TOCTOU mitigations
- harden XSS, CSRF, auth, and proxy trust
- change default ModelAdmin permissions to deny
- use parameterized queries and escape LIKE patterns

### Testing

- add regression test for LIKE wildcard injection fix

### Styling

- apply workspace-wide formatting fixes
- apply rustfmt to pre-existing formatting violations in 16 files
- apply code formatting to security fix files

### Maintenance

- *(license)* migrate from MIT/Apache-2.0 to BSD 3-Clause
- *(clippy)* add deny lints for todo/unimplemented/dbg_macro
- fix contradictory unimplemented!() messages in export handler
- fix misleading table_name() default implementation doc
