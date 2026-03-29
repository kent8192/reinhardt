# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

<!-- release-plz-separator -->
<!-- Entries below this line were created before release-plz adoption -->

## [0.1.0-rc.15](https://github.com/kent8192/reinhardt-web/compare/reinhardt-admin@v0.1.0-rc.14...reinhardt-admin@v0.1.0-rc.15) - 2026-03-29

### Changed

- *(admin)* migrate CurrentUser to AuthUser in server functions

### Fixed

- *(admin)* preserve query string in popstate navigation handler
- *(admin)* migrate remaining CurrentUser to AuthUser and update example
- *(admin)* replace CRLF before individual char replacement in TSV export
- *(admin)* update require_model_permission and callers to use AdminUser trait
- *(admin)* update test helpers to use AdminUser trait after merge
- *(admin)* update integration tests to match new admin API signatures
- *(di)* apply deferred DI registrations to existing singleton scope
- *(admin)* add serde helper for Vec<String> ORM deserialization
- *(admin)* accept any #[user] type for admin authentication via type-erased loader

### Other

- resolve conflict with main in delete.rs
- resolve conflicts with main in features.rs
- resolve conflict with main in delete.rs
- resolve conflict with main (AdminUser + admin_routes_with_di)
- resolve conflict with main in admin test module
- resolve conflicts with main branch

### Styling

- apply rustfmt formatting

## [0.1.0-rc.14](https://github.com/kent8192/reinhardt-web/compare/reinhardt-admin@v0.1.0-rc.13...reinhardt-admin@v0.1.0-rc.14) - 2026-03-24

### Added

- *(admin)* serve admin SPA HTML shell from admin_routes()

### Documentation

- *(crates)* update version references from 0.1.0-alpha.1 to 0.1.0-rc.13 across all READMEs

### Fixed

- *(reinhardt-admin)* register admin server functions in admin_routes()
- *(reinhardt-admin)* address Copilot review on router docs and test assertions
- *(admin)* register AdminSite in global DI with correct TypeId
- *(admin)* include admin SPA placeholder assets and fix static dir path
- *(admin)* apply CSP security headers to admin SPA HTML response
- *(admin)* add admin_static_routes() to serve embedded static assets

### Performance

- *(admin)* use zero-copy Bytes::from_static for embedded static assets

## [0.1.0-rc.12](https://github.com/kent8192/reinhardt-web/compare/reinhardt-admin@v0.1.0-rc.11...reinhardt-admin@v0.1.0-rc.12) - 2026-03-18

### Changed

- *(auth)* update re-exports and suppress deprecation warnings

## [0.1.0-rc.2](https://github.com/kent8192/reinhardt-web/compare/reinhardt-admin@v0.1.0-rc.1...reinhardt-admin@v0.1.0-rc.2) - 2026-03-04

### Fixed

- *(admin)* replace unwrap with error propagation in insert values call
- *(deps)* align dependency versions to workspace definitions

## [0.1.0-rc.1](https://github.com/kent8192/reinhardt-web/compare/reinhardt-admin@v0.1.0-alpha.17...reinhardt-admin@v0.1.0-rc.1) - 2026-02-24

### Maintenance

- updated the following local packages: reinhardt-db, reinhardt-db, reinhardt-auth, reinhardt-urls, reinhardt-pages

## [0.1.0-alpha.17](https://github.com/kent8192/reinhardt-web/compare/reinhardt-admin@v0.1.0-alpha.16...reinhardt-admin@v0.1.0-alpha.17) - 2026-02-24

### Maintenance

- updated the following local packages: reinhardt-pages

## [0.1.0-alpha.16](https://github.com/kent8192/reinhardt-web/compare/reinhardt-admin@v0.1.0-alpha.15...reinhardt-admin@v0.1.0-alpha.16) - 2026-02-23

### Maintenance

- *(license)* migrate from MIT/Apache-2.0 to BSD 3-Clause

## [0.1.0-alpha.15](https://github.com/kent8192/reinhardt-web/compare/reinhardt-admin@v0.1.0-alpha.14...reinhardt-admin@v0.1.0-alpha.15) - 2026-02-23

### Maintenance

- updated the following local packages: reinhardt-query, reinhardt-apps, reinhardt-db, reinhardt-db, reinhardt-auth, reinhardt-urls, reinhardt-pages

## [0.1.0-alpha.14](https://github.com/kent8192/reinhardt-web/compare/reinhardt-admin@v0.1.0-alpha.13...reinhardt-admin@v0.1.0-alpha.14) - 2026-02-21

### Fixed

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

### Security

- add audit logging for all CRUD operations
- add CSP headers, CSRF token generation, and XSS prevention
- add input validation, file size limits, and TOCTOU mitigations
- harden XSS, CSRF, auth, and proxy trust
- change default ModelAdmin permissions to deny
- use parameterized queries and escape LIKE patterns

### Changed

- clean up type naming, document intentional patterns

### Styling

- apply workspace-wide formatting fixes
- apply rustfmt to pre-existing formatting violations in 16 files
- apply code formatting to security fix files

### Testing

- add regression test for LIKE wildcard injection fix

### Maintenance

- fix contradictory unimplemented!() messages in export handler
- fix misleading table_name() default implementation doc

## [0.1.0-alpha.13](https://github.com/kent8192/reinhardt-web/compare/reinhardt-admin@v0.1.0-alpha.12...reinhardt-admin@v0.1.0-alpha.13) - 2026-02-16

### Maintenance

- updated the following local packages: reinhardt-db, reinhardt-db, reinhardt-auth, reinhardt-urls, reinhardt-pages

## [0.1.0-alpha.12](https://github.com/kent8192/reinhardt-web/compare/reinhardt-admin@v0.1.0-alpha.11...reinhardt-admin@v0.1.0-alpha.12) - 2026-02-15

### Maintenance

- updated the following local packages: reinhardt-query, reinhardt-apps, reinhardt-db, reinhardt-db, reinhardt-auth, reinhardt-urls, reinhardt-pages

## [0.1.0-alpha.11](https://github.com/kent8192/reinhardt-web/compare/reinhardt-admin@v0.1.0-alpha.10...reinhardt-admin@v0.1.0-alpha.11) - 2026-02-14

### Maintenance

- updated the following local packages: reinhardt-auth, reinhardt-urls, reinhardt-pages

## [0.1.0-alpha.10](https://github.com/kent8192/reinhardt-web/compare/reinhardt-admin@v0.1.0-alpha.9...reinhardt-admin@v0.1.0-alpha.10) - 2026-02-14

### Maintenance

- updated the following local packages: reinhardt-query, reinhardt-apps, reinhardt-db, reinhardt-db, reinhardt-auth, reinhardt-urls, reinhardt-pages

## [0.1.0-alpha.9](https://github.com/kent8192/reinhardt-web/compare/reinhardt-admin@v0.1.0-alpha.8...reinhardt-admin@v0.1.0-alpha.9) - 2026-02-14

### Maintenance

- updated the following local packages: reinhardt-query, reinhardt-apps, reinhardt-db, reinhardt-db, reinhardt-auth, reinhardt-urls, reinhardt-pages

## [0.1.0-alpha.7](https://github.com/kent8192/reinhardt-web/compare/reinhardt-admin@v0.1.0-alpha.6...reinhardt-admin@v0.1.0-alpha.7) - 2026-02-12

### Changed

- convert relative paths to absolute paths

## [0.1.0-alpha.6](https://github.com/kent8192/reinhardt-web/compare/reinhardt-admin@v0.1.0-alpha.5...reinhardt-admin@v0.1.0-alpha.6) - 2026-02-10

### Maintenance

- *(clippy)* add deny lints for todo/unimplemented/dbg_macro

## [0.1.0-alpha.5](https://github.com/kent8192/reinhardt-web/compare/reinhardt-admin@v0.1.0-alpha.4...reinhardt-admin@v0.1.0-alpha.5) - 2026-02-10

### Fixed

- *(admin)* move database tests to integration crate to break circular publish chain
- *(release)* revert unpublished crate versions to pre-release state

### Reverted

- undo PR [[#219](https://github.com/kent8192/reinhardt-web/issues/219)](https://github.com/kent8192/reinhardt-web/issues/219) version bumps for unpublished crates

## [0.1.0-alpha.4](https://github.com/kent8192/reinhardt-web/compare/reinhardt-admin@v0.1.0-alpha.3...reinhardt-admin@v0.1.0-alpha.4) - 2026-02-06

### Other

- updated the following local packages: reinhardt-utils, reinhardt-di, reinhardt-apps, reinhardt-db, reinhardt-db, reinhardt-auth, reinhardt-urls, reinhardt-pages

## [0.1.0-alpha.3](https://github.com/kent8192/reinhardt-web/compare/reinhardt-admin@v0.1.0-alpha.2...reinhardt-admin@v0.1.0-alpha.3) - 2026-02-03

### Other

- updated the following local packages: reinhardt-core, reinhardt-pages, reinhardt-http, reinhardt-utils, reinhardt-di, reinhardt-apps, reinhardt-db, reinhardt-db, reinhardt-auth, reinhardt-urls

## [0.1.0-alpha.2](https://github.com/kent8192/reinhardt-web/compare/reinhardt-admin@v0.1.0-alpha.1...reinhardt-admin@v0.1.0-alpha.2) - 2026-02-03

### Other

- add release-plz migration markers to CHANGELOGs
- *(changelog)* add missing 0.1.0-alpha.1 release entries
- *(package)* replace version.workspace with explicit versions

## [0.1.0-alpha.1] - 2026-01-23

### Added
- Initial release
- Admin panel functionality (via `reinhardt-panel`)
- CLI tool functionality (via `reinhardt-cli`)
