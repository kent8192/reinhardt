# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0-alpha.18](https://github.com/kent8192/reinhardt-web/compare/reinhardt-rest@v0.1.0-alpha.17...reinhardt-rest@v0.1.0-alpha.18) - 2026-02-23

### Maintenance

- updated the following local packages: reinhardt-query, reinhardt-core, reinhardt-core, reinhardt-http, reinhardt-db, reinhardt-db, reinhardt-auth, reinhardt-auth, reinhardt-utils

## [0.1.0-alpha.17](https://github.com/kent8192/reinhardt-web/compare/reinhardt-rest@v0.1.0-alpha.16...reinhardt-rest@v0.1.0-alpha.17) - 2026-02-23

### Maintenance

- updated the following local packages: reinhardt-query, reinhardt-db, reinhardt-db, reinhardt-auth, reinhardt-auth

## [0.1.0-alpha.16](https://github.com/kent8192/reinhardt-web/compare/reinhardt-rest@v0.1.0-alpha.15...reinhardt-rest@v0.1.0-alpha.16) - 2026-02-21

### Fixed

- propagate parse errors and validate min/max constraints
- cache compiled regex in NamespaceVersioning for performance
- replace expect() with safe get_ident() handling in attribute parsing
- collapse nested if block in serde_attrs to satisfy clippy
- pin CDN versions and add SRI integrity attributes
- add database dialect support for PostgreSQL compatibility
- handle serde attributes and improve validation
- update filter test assertions to expect MySQL-style backtick quoting
- use parameterized queries in SimpleSearchBackend

### Security

- harden XSS, CSRF, auth, and proxy trust

### Styling

- fix pre-existing clippy warnings and apply rustfmt
- apply rustfmt to pre-existing unformatted files
- apply rustfmt after clippy auto-fix
- fix remaining clippy warnings across workspace

## [0.1.0-alpha.15](https://github.com/kent8192/reinhardt-web/compare/reinhardt-rest@v0.1.0-alpha.14...reinhardt-rest@v0.1.0-alpha.15) - 2026-02-16

### Maintenance

- updated the following local packages: reinhardt-db, reinhardt-db, reinhardt-auth, reinhardt-auth

## [0.1.0-alpha.13](https://github.com/kent8192/reinhardt-web/compare/reinhardt-rest@v0.1.0-alpha.12...reinhardt-rest@v0.1.0-alpha.13) - 2026-02-14

### Maintenance

- updated the following local packages: reinhardt-auth, reinhardt-auth

## [0.1.0-alpha.12](https://github.com/kent8192/reinhardt-web/compare/reinhardt-rest@v0.1.0-alpha.11...reinhardt-rest@v0.1.0-alpha.12) - 2026-02-14

### Maintenance

- updated the following local packages: reinhardt-query, reinhardt-db, reinhardt-db, reinhardt-auth, reinhardt-auth

## [0.1.0-alpha.11](https://github.com/kent8192/reinhardt-web/compare/reinhardt-rest@v0.1.0-alpha.10...reinhardt-rest@v0.1.0-alpha.11) - 2026-02-14

### Maintenance

- updated the following local packages: reinhardt-query, reinhardt-db, reinhardt-db, reinhardt-auth, reinhardt-auth

## [0.1.0-alpha.10](https://github.com/kent8192/reinhardt-web/compare/reinhardt-rest@v0.1.0-alpha.9...reinhardt-rest@v0.1.0-alpha.10) - 2026-02-14

### Changed

- *(rest)* remove unused sea-orm dependency

## [0.1.0-alpha.9](https://github.com/kent8192/reinhardt-web/compare/reinhardt-rest@v0.1.0-alpha.8...reinhardt-rest@v0.1.0-alpha.9) - 2026-02-12

### Maintenance

- updated the following local packages: reinhardt-core, reinhardt-core, reinhardt-utils, reinhardt-db, reinhardt-db, reinhardt-auth, reinhardt-auth, reinhardt-http

## [0.1.0-alpha.8](https://github.com/kent8192/reinhardt-web/compare/reinhardt-rest@v0.1.0-alpha.7...reinhardt-rest@v0.1.0-alpha.8) - 2026-02-10

### Fixed

- *(rest)* move tests to integration crate to break circular publish chain
- *(release)* revert unpublished crate versions to pre-release state

### Reverted

- undo PR [[#219](https://github.com/kent8192/reinhardt-web/issues/219)](https://github.com/kent8192/reinhardt-web/issues/219) version bumps for unpublished crates

### Styling

- apply formatting to migrated test files and modified source files

## [0.1.0-alpha.7](https://github.com/kent8192/reinhardt-web/compare/reinhardt-rest@v0.1.0-alpha.6...reinhardt-rest@v0.1.0-alpha.7) - 2026-02-06

### Fixed

- remove reinhardt-urls from doc example to avoid circular dependency
- break circular dependency between reinhardt-openapi-macros and reinhardt-rest
- remove unused dev-dependencies from reinhardt-rest

### Other

- Revert "Merge pull request #202 from kent8192/release-plz-2026-02-06T13-32-57Z"
- release

## [0.1.0-alpha.6](https://github.com/kent8192/reinhardt-web/compare/reinhardt-rest@v0.1.0-alpha.5...reinhardt-rest@v0.1.0-alpha.6) - 2026-02-03

### Other

- updated the following local packages: reinhardt-core, reinhardt-core, reinhardt-pages, reinhardt-http, reinhardt-utils, reinhardt-server, reinhardt-db, reinhardt-db, reinhardt-auth, reinhardt-auth

## [0.1.0-alpha.5](https://github.com/kent8192/reinhardt-web/compare/reinhardt-rest@v0.1.0-alpha.4...reinhardt-rest@v0.1.0-alpha.5) - 2026-02-03

### Other

- merge main into chore/release-plz-migration
- add release-plz migration markers to CHANGELOGs

<!-- release-plz-separator -->
<!-- Entries below this line were created before release-plz adoption -->

## [0.1.0-alpha.4] - 2026-01-30

### Changed

- Version bump for publish workflow correction (no functional changes)

## [0.1.0-alpha.3] - 2026-01-30

### Changed

- Moved `OpenApiRouter` to `reinhardt-openapi` crate to resolve circular dependency
- Re-exported `generate_openapi_schema` from `endpoints` module for backward compatibility

### Removed

- Removed `openapi/router_wrapper.rs` (moved to `reinhardt-openapi` crate)

### Notes

- See [Issue #23](https://github.com/kent8192/reinhardt-web/issues/23) for circular dependency resolution details

## [0.1.0-alpha.2] - 2026-01-23

### Fixed

- Embed branding assets within crate for crates.io compatibility

## [0.1.0-alpha.1] - 2026-01-23

### Added

- Initial release with RESTful API framework with serializers, viewsets, and browsable API interface

