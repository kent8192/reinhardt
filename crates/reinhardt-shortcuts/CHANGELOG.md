# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0-rc.16](https://github.com/kent8192/reinhardt-web/compare/reinhardt-shortcuts@v0.1.0-rc.15...reinhardt-shortcuts@v0.1.0-rc.16) - 2026-04-19

### Changed

- deduplicate utility functions across crates

### Fixed

- *(ci)* route escape_html through reinhardt_core in shortcuts

## [0.1.0-rc.15](https://github.com/kent8192/reinhardt-web/compare/reinhardt-shortcuts@v0.1.0-rc.14...reinhardt-shortcuts@v0.1.0-rc.15) - 2026-03-29

### Changed

- *(shortcuts)* remove hardcoded postgres from database feature

## [0.1.0-rc.14](https://github.com/kent8192/reinhardt-web/compare/reinhardt-shortcuts@v0.1.0-rc.13...reinhardt-shortcuts@v0.1.0-rc.14) - 2026-03-24

### Documentation

- *(crates)* update version references from 0.1.0-alpha.1 to 0.1.0-rc.13 across all READMEs
- *(readme)* fix documentation discrepancies across crate READMEs

## [0.1.0-rc.5](https://github.com/kent8192/reinhardt-web/compare/reinhardt-shortcuts@v0.1.0-rc.4...reinhardt-shortcuts@v0.1.0-rc.5) - 2026-03-07

### Documentation

- add missing doc comments for public API modules and types

## [0.1.0-rc.3](https://github.com/kent8192/reinhardt-web/compare/reinhardt-shortcuts@v0.1.0-rc.2...reinhardt-shortcuts@v0.1.0-rc.3) - 2026-03-05

### Fixed

- *(release)* move reinhardt-test to optional dep in non-cyclic crates

## [0.1.0-rc.2](https://github.com/kent8192/reinhardt-web/compare/reinhardt-shortcuts@v0.1.0-rc.1...reinhardt-shortcuts@v0.1.0-rc.2) - 2026-03-04

### Maintenance

- *(testing)* add insta snapshot testing dependency across all crates

## [0.1.0-rc.1](https://github.com/kent8192/reinhardt-web/compare/reinhardt-shortcuts@v0.1.0-alpha.14...reinhardt-shortcuts@v0.1.0-rc.1) - 2026-02-24

### Maintenance

- updated the following local packages: reinhardt-db, reinhardt-views, reinhardt-urls

## [0.1.0-alpha.14](https://github.com/kent8192/reinhardt-web/compare/reinhardt-shortcuts@v0.1.0-alpha.13...reinhardt-shortcuts@v0.1.0-alpha.14) - 2026-02-24

### Fixed

- *(release)* roll back unpublished crate versions after partial release failure

### Maintenance

- *(license)* migrate from MIT/Apache-2.0 to BSD 3-Clause

## [0.1.0-alpha.13](https://github.com/kent8192/reinhardt-web/compare/reinhardt-shortcuts@v0.1.0-alpha.12...reinhardt-shortcuts@v0.1.0-alpha.13) - 2026-02-23

### Maintenance

- updated the following local packages: reinhardt-db, reinhardt-views, reinhardt-urls

## [0.1.0-alpha.12](https://github.com/kent8192/reinhardt-web/compare/reinhardt-shortcuts@v0.1.0-alpha.11...reinhardt-shortcuts@v0.1.0-alpha.12) - 2026-02-21

### Fixed

- use HeaderValue::from_static for hardcoded header values
- fix data integrity in render_to_string and sanitize 404 errors
- prevent database error message leakage in HTTP response
- prevent URL validation bypass via From trait (#726)

### Security

- add XSS safety documentation and input sanitization for render_html
- prevent open redirect attacks

### Changed

- add configurable capacity limit to TemplateContext
- add security headers helper function

### Styling

- apply formatting to files introduced by merge from main

## [0.1.0-alpha.11](https://github.com/kent8192/reinhardt-web/compare/reinhardt-shortcuts@v0.1.0-alpha.10...reinhardt-shortcuts@v0.1.0-alpha.11) - 2026-02-16

### Maintenance

- updated the following local packages: reinhardt-db, reinhardt-views, reinhardt-urls

## [0.1.0-alpha.10](https://github.com/kent8192/reinhardt-web/compare/reinhardt-shortcuts@v0.1.0-alpha.9...reinhardt-shortcuts@v0.1.0-alpha.10) - 2026-02-15

### Maintenance

- updated the following local packages: reinhardt-db, reinhardt-views, reinhardt-urls

## [0.1.0-alpha.9](https://github.com/kent8192/reinhardt-web/compare/reinhardt-shortcuts@v0.1.0-alpha.8...reinhardt-shortcuts@v0.1.0-alpha.9) - 2026-02-14

### Maintenance

- updated the following local packages: reinhardt-views, reinhardt-urls

## [0.1.0-alpha.8](https://github.com/kent8192/reinhardt-web/compare/reinhardt-shortcuts@v0.1.0-alpha.7...reinhardt-shortcuts@v0.1.0-alpha.8) - 2026-02-14

### Maintenance

- updated the following local packages: reinhardt-db, reinhardt-views, reinhardt-urls

## [0.1.0-alpha.7](https://github.com/kent8192/reinhardt-web/compare/reinhardt-shortcuts@v0.1.0-alpha.6...reinhardt-shortcuts@v0.1.0-alpha.7) - 2026-02-14

### Maintenance

- updated the following local packages: reinhardt-db, reinhardt-views, reinhardt-urls

## [0.1.0-alpha.6](https://github.com/kent8192/reinhardt-web/compare/reinhardt-shortcuts@v0.1.0-alpha.5...reinhardt-shortcuts@v0.1.0-alpha.6) - 2026-02-10

### Maintenance

- updated the following local packages: reinhardt-db, reinhardt-views, reinhardt-urls

## [0.1.0-alpha.5](https://github.com/kent8192/reinhardt-web/compare/reinhardt-shortcuts@v0.1.0-alpha.4...reinhardt-shortcuts@v0.1.0-alpha.5) - 2026-02-06

### Other

- updated the following local packages: reinhardt-db, reinhardt-views, reinhardt-urls

## [0.1.0-alpha.4](https://github.com/kent8192/reinhardt-web/compare/reinhardt-shortcuts@v0.1.0-alpha.3...reinhardt-shortcuts@v0.1.0-alpha.4) - 2026-02-03

### Other

- updated the following local packages: reinhardt-core, reinhardt-http, reinhardt-db, reinhardt-views, reinhardt-urls, reinhardt-test

## [0.1.0-alpha.3](https://github.com/kent8192/reinhardt-web/compare/reinhardt-shortcuts@v0.1.0-alpha.2...reinhardt-shortcuts@v0.1.0-alpha.3) - 2026-02-03

### Other

- add release-plz migration markers to CHANGELOGs
- *(changelog)* remove obsolete [0.1.0] sections

<!-- release-plz-separator -->
<!-- Entries below this line were created before release-plz adoption -->

## [0.1.0-alpha.2] - 2026-01-23

### Changed

- Re-publish with correct repository URL (reinhardt-web)

## [0.1.0-alpha.1] - 2026-01-23 [YANKED]

### Added

- Initial crates.io release

