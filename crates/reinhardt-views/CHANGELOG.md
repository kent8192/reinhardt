# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0-rc.16](https://github.com/kent8192/reinhardt-web/compare/reinhardt-views@v0.1.0-rc.15...reinhardt-views@v0.1.0-rc.16) - 2026-04-19

### Added

- *(di)* [**breaking**] deprecate Injected<T> in favor of Depends<T> and remove auto-Clone

### Styling

- apply rustfmt and page! macro formatting

### Testing

- *(views)* add response body verification to viewset routing tests

## [0.1.0-rc.15](https://github.com/kent8192/reinhardt-web/compare/reinhardt-views@v0.1.0-rc.14...reinhardt-views@v0.1.0-rc.15) - 2026-03-29

### Fixed

- suppress deprecated User trait warnings in downstream crates

### Maintenance

- update rust toolchain to 1.94.1 and set MSRV 1.94.0

## [0.1.0-rc.14](https://github.com/kent8192/reinhardt-web/compare/reinhardt-views@v0.1.0-rc.13...reinhardt-views@v0.1.0-rc.14) - 2026-03-24

### Changed

- *(views,middleware)* extract PATCH merge helper and use typed SET_COOKIE header

### Fixed

- *(views)* use actual total count for pagination instead of page length
- use saturating arithmetic for pagination overflow safety
- *(views)* determine partial update from HTTP method, not config field
- *(views)* return 405 instead of 400 for unsupported HTTP methods
- *(views)* reject non-object PATCH body with 400 Bad Request

## [0.1.0-rc.10](https://github.com/kent8192/reinhardt-web/compare/reinhardt-views@v0.1.0-rc.9...reinhardt-views@v0.1.0-rc.10) - 2026-03-15

### Documentation

- update version references in crate READMEs to 0.1.0-rc.9

## [0.1.0-rc.9](https://github.com/kent8192/reinhardt-web/compare/reinhardt-views@v0.1.0-rc.8...reinhardt-views@v0.1.0-rc.9) - 2026-03-15

### Fixed

- *(views)* replace unwrap with safe alternatives for panic prevention
- *(views)* replace panics with error handling and add poison logging

## [0.1.0-rc.5](https://github.com/kent8192/reinhardt-web/compare/reinhardt-views@v0.1.0-rc.4...reinhardt-views@v0.1.0-rc.5) - 2026-03-07

### Fixed

- merge main and resolve CI issues

## [0.1.0-rc.2](https://github.com/kent8192/reinhardt-web/compare/reinhardt-views@v0.1.0-rc.1...reinhardt-views@v0.1.0-rc.2) - 2026-03-04

### Fixed

- *(views)* replace std RwLock with parking_lot to prevent poisoning panics
- *(views)* add RefUnwindSafe impl for ViewSetHandler
- *(views)* remove unsafe keyword from RefUnwindSafe impl

### Maintenance

- *(testing)* add insta snapshot testing dependency across all crates

## [0.1.0-rc.1](https://github.com/kent8192/reinhardt-web/compare/reinhardt-views@v0.1.0-alpha.15...reinhardt-views@v0.1.0-rc.1) - 2026-02-24

### Maintenance

- updated the following local packages: reinhardt-db, reinhardt-auth, reinhardt-rest

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

