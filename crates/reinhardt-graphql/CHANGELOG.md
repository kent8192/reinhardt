# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0-alpha.7](https://github.com/kent8192/reinhardt-web/compare/reinhardt-graphql@v0.1.0-alpha.6...reinhardt-graphql@v0.1.0-alpha.7) - 2026-02-24

### Fixed

- *(release)* roll back unpublished crate versions after partial release failure

### Maintenance

- *(license)* migrate from MIT/Apache-2.0 to BSD 3-Clause

## [0.1.0-alpha.6](https://github.com/kent8192/reinhardt-web/compare/reinhardt-graphql@v0.1.0-alpha.5...reinhardt-graphql@v0.1.0-alpha.6) - 2026-02-21

### Fixed

- emit errors on crate resolution failure instead of silent fallback
- replace unwrap with safe error handling in context lookups
- merge main branch QueryLimits changes with backpressure features
- emit compile error for invalid skip_if expressions
- propagate stream errors to GraphQL clients instead of dropping
- replace expect() with proper error handling in subscription macro (#814)
- roll back unpublished crate versions after partial release failure
- roll back unpublished crate versions and enable release_always

### Security

- add input validation and resource limits
- improve subscription error handling
- add query complexity limits and access control
- add backpressure to subscription channels

### Styling

- fix remaining clippy warnings across workspace

## [0.1.0-alpha.5](https://github.com/kent8192/reinhardt-web/compare/reinhardt-graphql@v0.1.0-alpha.4...reinhardt-graphql@v0.1.0-alpha.5) - 2026-02-10

### Maintenance

- updated the following local packages: reinhardt-grpc

## [0.1.0-alpha.4](https://github.com/kent8192/reinhardt-web/compare/reinhardt-graphql@v0.1.0-alpha.3...reinhardt-graphql@v0.1.0-alpha.4) - 2026-02-06

### Other

- updated the following local packages: reinhardt-di, reinhardt-grpc

## [0.1.0-alpha.3](https://github.com/kent8192/reinhardt-web/compare/reinhardt-graphql@v0.1.0-alpha.2...reinhardt-graphql@v0.1.0-alpha.3) - 2026-02-03

### Other

- updated the following local packages: reinhardt-di, reinhardt-test, reinhardt-grpc

## [0.1.0-alpha.2](https://github.com/kent8192/reinhardt-web/compare/reinhardt-graphql@v0.1.0-alpha.1...reinhardt-graphql@v0.1.0-alpha.2) - 2026-02-03

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

