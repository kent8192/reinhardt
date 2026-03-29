# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0-rc.15](https://github.com/kent8192/reinhardt-web/compare/reinhardt-urls@v0.1.0-rc.14...reinhardt-urls@v0.1.0-rc.15) - 2026-03-29

### Fixed

- *(admin)* add deferred DI registration to bridge route-server scope gap
- *(di)* apply deferred DI registrations to existing singleton scope
- *(di)* register DatabaseConnection in user-provided DI context

### Maintenance

- update rust toolchain to 1.94.1 and set MSRV 1.94.0

## [0.1.0-rc.14](https://github.com/kent8192/reinhardt-web/compare/reinhardt-urls@v0.1.0-rc.13...reinhardt-urls@v0.1.0-rc.14) - 2026-03-24

### Added

- *(urls)* add exclude() builder method for middleware route exclusion

### Changed

- *(urls)* address Copilot review feedback

### Documentation

- *(crates)* update version references from 0.1.0-alpha.1 to 0.1.0-rc.13 across all READMEs

### Fixed

- *(reinhardt-urls)* normalize path slashes to prevent double-slash in URL joining
- *(reinhardt-urls)* restrict join_path visibility to pub(crate)

### Other

- resolve conflict with main in middleware.rs

### Styling

- *(urls)* apply formatting to server_router.rs

## [0.1.0-rc.12](https://github.com/kent8192/reinhardt-web/compare/reinhardt-urls@v0.1.0-rc.11...reinhardt-urls@v0.1.0-rc.12) - 2026-03-18

### Added

- *(urls)* add middleware registry for type discovery

### Fixed

- *(commands)* address Copilot review feedback on introspect command

## [0.1.0-rc.10](https://github.com/kent8192/reinhardt-web/compare/reinhardt-urls@v0.1.0-rc.9...reinhardt-urls@v0.1.0-rc.10) - 2026-03-15

### Fixed

- *(macros)* remove feature-dependent code generation from #[routes] macro
- *(urls)* restore semver-compatible new() and add __macro_new()

## [0.1.0-rc.9](https://github.com/kent8192/reinhardt-web/compare/reinhardt-urls@v0.1.0-rc.8...reinhardt-urls@v0.1.0-rc.9) - 2026-03-15

### Fixed

- *(urls)* replace lock/read/write().unwrap() with safe alternatives for panic prevention
- *(urls)* resolve race condition and add poison logging
- *(urls)* avoid holding RwLock guard across await point

## [0.1.0-rc.8](https://github.com/kent8192/reinhardt-web/compare/reinhardt-urls@v0.1.0-rc.7...reinhardt-urls@v0.1.0-rc.8) - 2026-03-12

### Documentation

- document client-router feature requirement for UnifiedRouter in prelude

## [0.1.0-rc.7](https://github.com/kent8192/reinhardt-web/compare/reinhardt-urls@v0.1.0-rc.6...reinhardt-urls@v0.1.0-rc.7) - 2026-03-11

### Added

- *(urls)* add WASM-compatible UnifiedRouter variant with ServerRouterStub

### Fixed

- *(urls)* suppress dead_code warning for WASM-only `merge` method

## [0.1.0-rc.5](https://github.com/kent8192/reinhardt-web/compare/reinhardt-urls@v0.1.0-rc.4...reinhardt-urls@v0.1.0-rc.5) - 2026-03-07

### Fixed

- *(urls)* accept case-insensitive UUIDs per RFC 4122
- *(urls)* correct UUID converter test expectations for case-insensitive validation

### Other

- resolve conflict with main in labels.yml

## [0.1.0-rc.2](https://github.com/kent8192/reinhardt-web/compare/reinhardt-urls@v0.1.0-rc.1...reinhardt-urls@v0.1.0-rc.2) - 2026-03-04

### Fixed

- *(urls)* convert path-type parameters to matchit catch-all syntax in RadixTree mode

### Styling

- *(urls)* apply project formatting to pattern module

### Testing

- *(urls)* add coverage tests for LazyLoaded clone-based get and get_if_loaded

## [0.1.0-rc.1](https://github.com/kent8192/reinhardt-web/compare/reinhardt-urls@v0.1.0-alpha.15...reinhardt-urls@v0.1.0-rc.1) - 2026-02-24

### Maintenance

- updated the following local packages: reinhardt-db, reinhardt-middleware, reinhardt-views

## [0.1.0-alpha.15](https://github.com/kent8192/reinhardt-web/compare/reinhardt-urls@v0.1.0-alpha.14...reinhardt-urls@v0.1.0-alpha.15) - 2026-02-23

### Maintenance

- *(license)* migrate from MIT/Apache-2.0 to BSD 3-Clause

## [0.1.0-alpha.14](https://github.com/kent8192/reinhardt-web/compare/reinhardt-urls@v0.1.0-alpha.13...reinhardt-urls@v0.1.0-alpha.14) - 2026-02-23

### Maintenance

- updated the following local packages: reinhardt-db, reinhardt-middleware, reinhardt-views

## [0.1.0-alpha.13](https://github.com/kent8192/reinhardt-web/compare/reinhardt-urls@v0.1.0-alpha.12...reinhardt-urls@v0.1.0-alpha.13) - 2026-02-21

### Fixed

- add memory-bounded eviction to LRU route cache
- bound LRU heap growth via periodic compaction
- prevent double substitution in UrlPattern::build_url
- handle lock poisoning and improve error handling in router and URL resolution
- replace Box::leak with Arc to prevent memory leak
- add path traversal prevention with input validation

### Security

- add compile-time validation for paths, SQL, and crate references
- fix path validation for ambiguous params and wildcards
- add input validation for route paths and SQL expressions
- add ReDoS prevention and input validation
- prevent path traversal and parameter injection

### Changed

- remove incorrect dead_code annotations from proxy fields

### Styling

- apply rustfmt to pre-existing unformatted files
- replace never-looping for with if-let per clippy::never_loop
- apply rustfmt formatting to workspace files
- apply code formatting to security fix files

### Documentation

- document wildcard pattern cross-segment matching behavior

## [0.1.0-alpha.12](https://github.com/kent8192/reinhardt-web/compare/reinhardt-urls@v0.1.0-alpha.11...reinhardt-urls@v0.1.0-alpha.12) - 2026-02-16

### Maintenance

- updated the following local packages: reinhardt-db, reinhardt-middleware, reinhardt-views

## [0.1.0-alpha.11](https://github.com/kent8192/reinhardt-web/compare/reinhardt-urls@v0.1.0-alpha.10...reinhardt-urls@v0.1.0-alpha.11) - 2026-02-15

### Maintenance

- updated the following local packages: reinhardt-db, reinhardt-middleware, reinhardt-views

## [0.1.0-alpha.10](https://github.com/kent8192/reinhardt-web/compare/reinhardt-urls@v0.1.0-alpha.9...reinhardt-urls@v0.1.0-alpha.10) - 2026-02-14

### Maintenance

- updated the following local packages: reinhardt-middleware, reinhardt-views

## [0.1.0-alpha.9](https://github.com/kent8192/reinhardt-web/compare/reinhardt-urls@v0.1.0-alpha.8...reinhardt-urls@v0.1.0-alpha.9) - 2026-02-14

### Maintenance

- updated the following local packages: reinhardt-db, reinhardt-middleware, reinhardt-views

## [0.1.0-alpha.8](https://github.com/kent8192/reinhardt-web/compare/reinhardt-urls@v0.1.0-alpha.7...reinhardt-urls@v0.1.0-alpha.8) - 2026-02-14

### Maintenance

- updated the following local packages: reinhardt-db, reinhardt-middleware, reinhardt-views

## [0.1.0-alpha.7](https://github.com/kent8192/reinhardt-web/compare/reinhardt-urls@v0.1.0-alpha.6...reinhardt-urls@v0.1.0-alpha.7) - 2026-02-14

### Maintenance

- updated the following local packages: reinhardt-di, reinhardt-db, reinhardt-views, reinhardt-middleware

## [0.1.0-alpha.6](https://github.com/kent8192/reinhardt-web/compare/reinhardt-urls@v0.1.0-alpha.5...reinhardt-urls@v0.1.0-alpha.6) - 2026-02-12

### Changed

- convert relative paths to absolute paths
- restore single-level super:: paths preserved by convention

### Fixed

- correct incorrect path conversions in test imports

## [0.1.0-alpha.5](https://github.com/kent8192/reinhardt-web/compare/reinhardt-urls@v0.1.0-alpha.4...reinhardt-urls@v0.1.0-alpha.5) - 2026-02-10

### Maintenance

- updated the following local packages: reinhardt-db, reinhardt-views, reinhardt-middleware

## [0.1.0-alpha.4](https://github.com/kent8192/reinhardt-web/compare/reinhardt-urls@v0.1.0-alpha.3...reinhardt-urls@v0.1.0-alpha.4) - 2026-02-06

### Other

- updated the following local packages: reinhardt-di, reinhardt-db, reinhardt-middleware, reinhardt-views

## [0.1.0-alpha.3](https://github.com/kent8192/reinhardt-web/compare/reinhardt-urls@v0.1.0-alpha.2...reinhardt-urls@v0.1.0-alpha.3) - 2026-02-03

### Other

- updated the following local packages: reinhardt-core, reinhardt-http, reinhardt-di, reinhardt-db, reinhardt-middleware, reinhardt-views

## [0.1.0-alpha.2](https://github.com/kent8192/reinhardt-web/compare/reinhardt-urls@v0.1.0-alpha.1...reinhardt-urls@v0.1.0-alpha.2) - 2026-02-03

### Other

- add release-plz migration markers to CHANGELOGs
- *(changelog)* remove obsolete [0.1.0] sections
- *(changelog)* add missing 0.1.0-alpha.1 release entries
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

