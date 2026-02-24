# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0-alpha.13](https://github.com/kent8192/reinhardt-web/compare/reinhardt-dispatch@v0.1.0-alpha.12...reinhardt-dispatch@v0.1.0-alpha.13) - 2026-02-23

### Maintenance

- updated the following local packages: reinhardt-core, reinhardt-http, reinhardt-urls, reinhardt-middleware, reinhardt-views

## [0.1.0-alpha.12](https://github.com/kent8192/reinhardt-web/compare/reinhardt-dispatch@v0.1.0-alpha.11...reinhardt-dispatch@v0.1.0-alpha.12) - 2026-02-23

### Maintenance

- updated the following local packages: reinhardt-middleware, reinhardt-views, reinhardt-urls

## [0.1.0-alpha.11](https://github.com/kent8192/reinhardt-web/compare/reinhardt-dispatch@v0.1.0-alpha.10...reinhardt-dispatch@v0.1.0-alpha.11) - 2026-02-21

### Fixed

- fix dead code, default handler, and lost request context
- log signal send errors instead of silently discarding
- replace lock unwrap with poison error recovery

### Security

- add configurable middleware chain depth limit
- add content-type and nosniff headers to error responses
- prevent information disclosure in exception handler

### Styling

- apply rustfmt to pre-existing unformatted files
- apply rustfmt to pre-existing formatting violations in 16 files

## [0.1.0-alpha.10](https://github.com/kent8192/reinhardt-web/compare/reinhardt-dispatch@v0.1.0-alpha.9...reinhardt-dispatch@v0.1.0-alpha.10) - 2026-02-16

### Maintenance

- updated the following local packages: reinhardt-middleware, reinhardt-views, reinhardt-urls

## [0.1.0-alpha.9](https://github.com/kent8192/reinhardt-web/compare/reinhardt-dispatch@v0.1.0-alpha.8...reinhardt-dispatch@v0.1.0-alpha.9) - 2026-02-15

### Maintenance

- updated the following local packages: reinhardt-middleware, reinhardt-views, reinhardt-urls

## [0.1.0-alpha.8](https://github.com/kent8192/reinhardt-web/compare/reinhardt-dispatch@v0.1.0-alpha.7...reinhardt-dispatch@v0.1.0-alpha.8) - 2026-02-14

### Maintenance

- updated the following local packages: reinhardt-middleware, reinhardt-views, reinhardt-urls

## [0.1.0-alpha.7](https://github.com/kent8192/reinhardt-web/compare/reinhardt-dispatch@v0.1.0-alpha.6...reinhardt-dispatch@v0.1.0-alpha.7) - 2026-02-14

### Maintenance

- updated the following local packages: reinhardt-middleware, reinhardt-views, reinhardt-urls

## [0.1.0-alpha.6](https://github.com/kent8192/reinhardt-web/compare/reinhardt-dispatch@v0.1.0-alpha.5...reinhardt-dispatch@v0.1.0-alpha.6) - 2026-02-14

### Maintenance

- updated the following local packages: reinhardt-middleware, reinhardt-views, reinhardt-urls

## [0.1.0-alpha.5](https://github.com/kent8192/reinhardt-web/compare/reinhardt-dispatch@v0.1.0-alpha.4...reinhardt-dispatch@v0.1.0-alpha.5) - 2026-02-10

### Maintenance

- updated the following local packages: reinhardt-views, reinhardt-middleware, reinhardt-urls

## [0.1.0-alpha.4](https://github.com/kent8192/reinhardt-web/compare/reinhardt-dispatch@v0.1.0-alpha.3...reinhardt-dispatch@v0.1.0-alpha.4) - 2026-02-06

### Other

- updated the following local packages: reinhardt-middleware, reinhardt-views, reinhardt-urls

## [0.1.0-alpha.3](https://github.com/kent8192/reinhardt-web/compare/reinhardt-dispatch@v0.1.0-alpha.2...reinhardt-dispatch@v0.1.0-alpha.3) - 2026-02-03

### Other

- updated the following local packages: reinhardt-core, reinhardt-http, reinhardt-middleware, reinhardt-views, reinhardt-urls

## [0.1.0-alpha.2](https://github.com/kent8192/reinhardt-web/compare/reinhardt-dispatch@v0.1.0-alpha.1...reinhardt-dispatch@v0.1.0-alpha.2) - 2026-02-03

### Other

- add release-plz migration markers to CHANGELOGs
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

