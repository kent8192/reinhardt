# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0-rc.16](https://github.com/kent8192/reinhardt-web/compare/reinhardt-graphql@v0.1.0-rc.15...reinhardt-graphql@v0.1.0-rc.16) - 2026-04-18

### Added

- migrate UUID generation from v4 to v7 across entire codebase
- *(di)* [**breaking**] deprecate Injected<T> in favor of Depends<T> and remove auto-Clone

### Fixed

- *(di)* fall back to Injectable::inject() when type is not in registry
- *(di)* remove Injectable bound from Depends::resolve() for factory types

## [0.1.0-rc.15](https://github.com/kent8192/reinhardt-web/compare/reinhardt-graphql@v0.1.0-rc.14...reinhardt-graphql@v0.1.0-rc.15) - 2026-03-29

### Maintenance

- update rust toolchain to 1.94.1 and set MSRV 1.94.0

## [0.1.0-rc.14](https://github.com/kent8192/reinhardt-web/compare/reinhardt-graphql@v0.1.0-rc.13...reinhardt-graphql@v0.1.0-rc.14) - 2026-03-24

### Changed

- *(graphql)* extract exceeds_max_chars helper with short-circuit and add multi-byte tests

### Fixed

- *(graphql)* use character count instead of byte length for name validation
- resolve merge conflict keeping both escape tracking and char count tests
- *(reinhardt-graphql)* fork DI context per-request in GraphQL handler macros
- *(reinhardt-graphql)* evaluate field count immediately during character processing
- *(reinhardt-graphql)* handle inline fragment type conditions and block strings in field counter

## [0.1.0-rc.10](https://github.com/kent8192/reinhardt-web/compare/reinhardt-graphql@v0.1.0-rc.9...reinhardt-graphql@v0.1.0-rc.10) - 2026-03-15

### Added

- *(graphql)* re-export async_graphql base types through reinhardt facade

## [0.1.0-rc.9](https://github.com/kent8192/reinhardt-web/compare/reinhardt-graphql@v0.1.0-rc.8...reinhardt-graphql@v0.1.0-rc.9) - 2026-03-15

### Fixed

- *(graphql)* replace rwlock unwrap with poison-recovery pattern
- *(graphql)* centralize poison recovery with logging helpers

## [0.1.0-rc.5](https://github.com/kent8192/reinhardt-web/compare/reinhardt-graphql@v0.1.0-rc.4...reinhardt-graphql@v0.1.0-rc.5) - 2026-03-07

### Documentation

- add missing doc comments for public API modules and types

## [0.1.0-rc.3](https://github.com/kent8192/reinhardt-web/compare/reinhardt-graphql@v0.1.0-rc.2...reinhardt-graphql@v0.1.0-rc.3) - 2026-03-05

### Fixed

- *(release)* move reinhardt-test to optional dep in non-cyclic crates

## [0.1.0-rc.2](https://github.com/kent8192/reinhardt-web/compare/reinhardt-graphql@v0.1.0-rc.1...reinhardt-graphql@v0.1.0-rc.2) - 2026-03-04

### Maintenance

- *(testing)* add insta snapshot testing dependency across all crates

## [0.1.0-rc.1](https://github.com/kent8192/reinhardt-web/compare/reinhardt-graphql@v0.1.0-alpha.6...reinhardt-graphql@v0.1.0-rc.1) - 2026-02-24

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

