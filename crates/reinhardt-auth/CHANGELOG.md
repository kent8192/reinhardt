# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0-alpha.15](https://github.com/kent8192/reinhardt-web/compare/reinhardt-auth@v0.1.0-alpha.14...reinhardt-auth@v0.1.0-alpha.15) - 2026-02-23

### Maintenance

- *(license)* migrate from MIT/Apache-2.0 to BSD 3-Clause

## [0.1.0-alpha.14](https://github.com/kent8192/reinhardt-web/compare/reinhardt-auth@v0.1.0-alpha.13...reinhardt-auth@v0.1.0-alpha.14) - 2026-02-23

### Maintenance

- updated the following local packages: reinhardt-query, reinhardt-query, reinhardt-apps, reinhardt-db, reinhardt-db

## [0.1.0-alpha.13](https://github.com/kent8192/reinhardt-web/compare/reinhardt-auth@v0.1.0-alpha.12...reinhardt-auth@v0.1.0-alpha.13) - 2026-02-21

### Fixed

- use logging framework instead of eprintln in authentication
- replace std Mutex with tokio Mutex to prevent async deadlocks
- replace unwrap with safe error handling in JWT claim extraction
- add authentication and authorization enforcement to all endpoints
- add path traversal prevention with input validation

### Security

- use server secret as HMAC key material in session auth hash
- harden XSS, CSRF, auth, and proxy trust
- fix TOTP algorithm, proxy trust, and session cookies
- implement constant-time comparison and argon2 password hashing

### Styling

- apply rustfmt to pre-existing unformatted files
- apply formatting to files introduced by merge from main
- apply rustfmt formatting to workspace files

### Documentation

- add security note on client-side auth state limitations

### Maintenance

- add SAFETY comment to unsafe block in hasher_boundary_value

## [0.1.0-alpha.12](https://github.com/kent8192/reinhardt-web/compare/reinhardt-auth@v0.1.0-alpha.11...reinhardt-auth@v0.1.0-alpha.12) - 2026-02-16

### Maintenance

- updated the following local packages: reinhardt-db, reinhardt-db

## [0.1.0-alpha.11](https://github.com/kent8192/reinhardt-web/compare/reinhardt-auth@v0.1.0-alpha.10...reinhardt-auth@v0.1.0-alpha.11) - 2026-02-15

### Maintenance

- updated the following local packages: reinhardt-query, reinhardt-query, reinhardt-apps, reinhardt-db, reinhardt-db

## [0.1.0-alpha.9](https://github.com/kent8192/reinhardt-web/compare/reinhardt-auth@v0.1.0-alpha.8...reinhardt-auth@v0.1.0-alpha.9) - 2026-02-14

### Maintenance

- updated the following local packages: reinhardt-query, reinhardt-query, reinhardt-apps, reinhardt-db, reinhardt-db

## [0.1.0-alpha.8](https://github.com/kent8192/reinhardt-web/compare/reinhardt-auth@v0.1.0-alpha.7...reinhardt-auth@v0.1.0-alpha.8) - 2026-02-14

### Maintenance

- updated the following local packages: reinhardt-query, reinhardt-query, reinhardt-apps, reinhardt-db, reinhardt-db

## [0.1.0-alpha.6](https://github.com/kent8192/reinhardt-web/compare/reinhardt-auth@v0.1.0-alpha.5...reinhardt-auth@v0.1.0-alpha.6) - 2026-02-12

### Changed

- convert relative paths to absolute paths
- restore single-level super:: paths preserved by convention

## [0.1.0-alpha.5](https://github.com/kent8192/reinhardt-web/compare/reinhardt-auth@v0.1.0-alpha.4...reinhardt-auth@v0.1.0-alpha.5) - 2026-02-10

### Fixed

- *(auth)* remove unused reinhardt-test dev-dependency
- *(release)* revert unpublished crate versions to pre-release state

### Reverted

- undo PR [[#219](https://github.com/kent8192/reinhardt-web/issues/219)](https://github.com/kent8192/reinhardt-web/issues/219) version bumps for unpublished crates

## [0.1.0-alpha.4](https://github.com/kent8192/reinhardt-web/compare/reinhardt-auth@v0.1.0-alpha.3...reinhardt-auth@v0.1.0-alpha.4) - 2026-02-06

### Other

- updated the following local packages: reinhardt-utils, reinhardt-di, reinhardt-apps, reinhardt-db, reinhardt-db

## [0.1.0-alpha.3](https://github.com/kent8192/reinhardt-web/compare/reinhardt-auth@v0.1.0-alpha.2...reinhardt-auth@v0.1.0-alpha.3) - 2026-02-03

### Other

- updated the following local packages: reinhardt-core, reinhardt-http, reinhardt-utils, reinhardt-di, reinhardt-apps, reinhardt-db, reinhardt-db

## [0.1.0-alpha.2](https://github.com/kent8192/reinhardt-web/compare/reinhardt-auth@v0.1.0-alpha.1...reinhardt-auth@v0.1.0-alpha.2) - 2026-02-03

### Other

- add release-plz migration markers to CHANGELOGs
- *(changelog)* remove obsolete [0.1.0] sections
- *(changelog)* add missing 0.1.0-alpha.1 release entries
- *(package)* replace version.workspace with explicit versions
- N/A

### Added
- Work in progress features (not yet released)

### Changed
- N/A

### Deprecated
- N/A

### Removed
- N/A

### Fixed
- N/A

### Security
- N/A


<!-- release-plz-separator -->
<!-- Entries below this line were created before release-plz adoption -->

## [0.1.0-alpha.1] - 2026-01-23

### Added

- Initial crates.io release

