# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0-alpha.15](https://github.com/kent8192/reinhardt-web/compare/reinhardt-views@v0.1.0-alpha.14...reinhardt-views@v0.1.0-alpha.15) - 2026-02-23

### Maintenance

- updated the following local packages: reinhardt-query, reinhardt-core, reinhardt-http, reinhardt-di, reinhardt-db, reinhardt-auth, reinhardt-utils, reinhardt-rest

## [0.1.0-alpha.14](https://github.com/kent8192/reinhardt-web/compare/reinhardt-views@v0.1.0-alpha.13...reinhardt-views@v0.1.0-alpha.14) - 2026-02-23

### Maintenance

- updated the following local packages: reinhardt-query, reinhardt-db, reinhardt-auth, reinhardt-rest

## [0.1.0-alpha.13](https://github.com/kent8192/reinhardt-web/compare/reinhardt-views@v0.1.0-alpha.12...reinhardt-views@v0.1.0-alpha.13) - 2026-02-21

### Fixed

- replace Box::leak with Arc to prevent memory leak
- escape user input to prevent XSS

## [0.1.0-alpha.12](https://github.com/kent8192/reinhardt-web/compare/reinhardt-views@v0.1.0-alpha.11...reinhardt-views@v0.1.0-alpha.12) - 2026-02-16

### Maintenance

- updated the following local packages: reinhardt-db, reinhardt-auth, reinhardt-rest

## [0.1.0-alpha.11](https://github.com/kent8192/reinhardt-web/compare/reinhardt-views@v0.1.0-alpha.10...reinhardt-views@v0.1.0-alpha.11) - 2026-02-15

### Maintenance

- updated the following local packages: reinhardt-query, reinhardt-rest, reinhardt-db, reinhardt-auth

## [0.1.0-alpha.10](https://github.com/kent8192/reinhardt-web/compare/reinhardt-views@v0.1.0-alpha.9...reinhardt-views@v0.1.0-alpha.10) - 2026-02-14

### Maintenance

- updated the following local packages: reinhardt-auth, reinhardt-rest

## [0.1.0-alpha.9](https://github.com/kent8192/reinhardt-web/compare/reinhardt-views@v0.1.0-alpha.8...reinhardt-views@v0.1.0-alpha.9) - 2026-02-14

### Maintenance

- updated the following local packages: reinhardt-query, reinhardt-db, reinhardt-auth, reinhardt-rest

## [0.1.0-alpha.8](https://github.com/kent8192/reinhardt-web/compare/reinhardt-views@v0.1.0-alpha.7...reinhardt-views@v0.1.0-alpha.8) - 2026-02-14

### Maintenance

- updated the following local packages: reinhardt-query, reinhardt-db, reinhardt-auth, reinhardt-rest

## [0.1.0-alpha.6](https://github.com/kent8192/reinhardt-web/compare/reinhardt-views@v0.1.0-alpha.5...reinhardt-views@v0.1.0-alpha.6) - 2026-02-12

### Maintenance

- updated the following local packages: reinhardt-core, reinhardt-utils, reinhardt-db, reinhardt-auth, reinhardt-http, reinhardt-di, reinhardt-rest

## [0.1.0-alpha.5](https://github.com/kent8192/reinhardt-web/compare/reinhardt-views@v0.1.0-alpha.4...reinhardt-views@v0.1.0-alpha.5) - 2026-02-10

### Fixed

- *(views)* move tests to integration crate to break circular publish chain
- *(release)* revert unpublished crate versions to pre-release state

### Reverted

- undo PR [[#219](https://github.com/kent8192/reinhardt-web/issues/219)](https://github.com/kent8192/reinhardt-web/issues/219) version bumps for unpublished crates

### Styling

- apply formatting to migrated test files and modified source files

## [0.1.0-alpha.4](https://github.com/kent8192/reinhardt-web/compare/reinhardt-views@v0.1.0-alpha.3...reinhardt-views@v0.1.0-alpha.4) - 2026-02-06

### Other

- updated the following local packages: reinhardt-utils, reinhardt-di, reinhardt-rest, reinhardt-db, reinhardt-db, reinhardt-auth

## [0.1.0-alpha.3](https://github.com/kent8192/reinhardt-web/compare/reinhardt-views@v0.1.0-alpha.2...reinhardt-views@v0.1.0-alpha.3) - 2026-02-03

### Other

- updated the following local packages: reinhardt-core, reinhardt-http, reinhardt-utils, reinhardt-di, reinhardt-db, reinhardt-db, reinhardt-auth, reinhardt-rest

## [0.1.0-alpha.2](https://github.com/kent8192/reinhardt-web/compare/reinhardt-views@v0.1.0-alpha.1...reinhardt-views@v0.1.0-alpha.2) - 2026-02-03

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

