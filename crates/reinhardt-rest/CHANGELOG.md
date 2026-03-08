# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0](https://github.com/kent8192/reinhardt-web/releases/tag/reinhardt-rest@v0.1.0) - 2026-03-08

### Added

- Initial release with RESTful API framework with serializers, viewsets, and browsable API interface

### Changed

- *(rest)* remove unused sea-orm dependency
- Moved `OpenApiRouter` to `reinhardt-openapi` crate to resolve circular dependency
- Re-exported `generate_openapi_schema` from `endpoints` module for backward compatibility

### Removed

- Removed `openapi/router_wrapper.rs` (moved to `reinhardt-openapi` crate)

### Fixed

- *(rest)* use workspace redis dependency instead of pinned rc version
- *(meta)* fix workspace inheritance and authors metadata
- propagate parse errors and validate min/max constraints
- cache compiled regex in NamespaceVersioning for performance
- replace expect() with safe get_ident() handling in attribute parsing
- collapse nested if block in serde_attrs to satisfy clippy
- pin CDN versions and add SRI integrity attributes
- add database dialect support for PostgreSQL compatibility
- handle serde attributes and improve validation
- update filter test assertions to expect MySQL-style backtick quoting
- use parameterized queries in SimpleSearchBackend
- *(rest)* move tests to integration crate to break circular publish chain
- Embed branding assets within crate for crates.io compatibility

### Security

- harden XSS, CSRF, auth, and proxy trust

### Styling

- fix pre-existing clippy warnings and apply rustfmt
- apply rustfmt to pre-existing unformatted files
- apply rustfmt after clippy auto-fix
- fix remaining clippy warnings across workspace
- apply formatting to migrated test files and modified source files

### Maintenance

- *(testing)* add insta snapshot testing dependency across all crates

### Notes

- See [Issue #23](https://github.com/kent8192/reinhardt-web/issues/23) for circular dependency resolution details
