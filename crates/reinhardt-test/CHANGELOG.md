# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0-rc.16](https://github.com/kent8192/reinhardt-web/compare/reinhardt-test@v0.1.0-rc.15...reinhardt-test@v0.1.0-rc.16) - 2026-04-13

### Added

- add CDP-based E2E browser testing infrastructure and WASM SPA fixes
- *(test)* add MSW-style network interception module with core components

### Changed

- replace target_arch = "wasm32" with target_family/target_os best practice

### Deprecated

- *(test)* mark MockFetch and mock_server_fn as deprecated in favor of MSW

### Fixed

- *(test)* gate native-only dependencies for wasm32 target compilation
- *(auth)* add is_staff and is_superuser fields to JWT Claims
- *(ci)* apply rustfmt and fix collapsible_if clippy lint after main merge
- *(ci)* add #[allow(deprecated)] to re-exports and tests using deprecated mock APIs
- *(test)* address Copilot review feedback on MSW module
- *(ci)* resolve clippy errors in MSW module for native builds
- *(ci)* use backticks instead of intra-doc links for feature-gated types
- *(testkit)* remove auth-testing feature, make auth unconditional on native targets

### Testing

- *(test)* add WASM integration tests, rstest fixtures, and fix URL matching

## [0.1.0-rc.15](https://github.com/kent8192/reinhardt-web/compare/reinhardt-test@v0.1.0-rc.14...reinhardt-test@v0.1.0-rc.15) - 2026-03-29

### Maintenance

- update rust toolchain to 1.94.1 and set MSRV 1.94.0

### Testing

- *(test)* add fixture output validation tests for auth and admin_panel
- *(test)* add WASM module tests and convert existing tests to rstest

## [0.1.0-rc.14](https://github.com/kent8192/reinhardt-web/compare/reinhardt-test@v0.1.0-rc.13...reinhardt-test@v0.1.0-rc.14) - 2026-03-24

### Added

- *(test)* add fantoccini E2E browser testing utility fixtures

### Fixed

- address copilot review feedback and merge main
- *(test)* wrap env var calls in unsafe blocks for Rust 2024 edition
- *(test)* improve test infrastructure reliability and E2E fixtures
- *(test,testkit)* address Copilot review feedback on test infrastructure
- resolve CI failures and remove sea-query dependency

### Styling

- apply rustfmt formatting fixes

## [0.1.0-rc.5](https://github.com/kent8192/reinhardt-web/compare/reinhardt-test@v0.1.0-rc.4...reinhardt-test@v0.1.0-rc.5) - 2026-03-07

### Changed

- *(reinhardt-test)* delegate to reinhardt-testkit with re-exports

### Other

- resolve conflict with main branch version bump to rc.4

## [0.1.0-rc.2](https://github.com/kent8192/reinhardt-web/compare/reinhardt-test@v0.1.0-rc.1...reinhardt-test@v0.1.0-rc.2) - 2026-03-04

### Fixed

- *(deps)* update reinhardt-test outdated deps
- *(deps)* convert Vec to Bytes for tungstenite message types

## [0.1.0-rc.1](https://github.com/kent8192/reinhardt-web/compare/reinhardt-test@v0.1.0-alpha.20...reinhardt-test@v0.1.0-rc.1) - 2026-02-24

### Maintenance

- updated the following local packages: reinhardt-db, reinhardt-auth, reinhardt-rest, reinhardt-views, reinhardt-urls, reinhardt-pages, reinhardt-admin, reinhardt-websockets

## [0.1.0-alpha.20](https://github.com/kent8192/reinhardt-web/compare/reinhardt-test@v0.1.0-alpha.19...reinhardt-test@v0.1.0-alpha.20) - 2026-02-24

### Maintenance

- updated the following local packages: reinhardt-pages, reinhardt-admin, reinhardt-websockets

## [0.1.0-alpha.19](https://github.com/kent8192/reinhardt-web/compare/reinhardt-test@v0.1.0-alpha.18...reinhardt-test@v0.1.0-alpha.19) - 2026-02-23

### Maintenance

- *(license)* migrate from MIT/Apache-2.0 to BSD 3-Clause

## [0.1.0-alpha.18](https://github.com/kent8192/reinhardt-web/compare/reinhardt-test@v0.1.0-alpha.17...reinhardt-test@v0.1.0-alpha.18) - 2026-02-23

### Maintenance

- updated the following local packages: reinhardt-query, reinhardt-conf, reinhardt-db, reinhardt-auth, reinhardt-rest, reinhardt-views, reinhardt-urls, reinhardt-pages, reinhardt-admin, reinhardt-websockets

## [0.1.0-alpha.17](https://github.com/kent8192/reinhardt-web/compare/reinhardt-test@v0.1.0-alpha.16...reinhardt-test@v0.1.0-alpha.17) - 2026-02-21

### Added

- standardize PostgreSQL version to 17

### Fixed

- fix TOCTOU port binding and missing sqlx pool workaround
- replace unwrap with descriptive expect in WASM helpers and containers
- add panic prevention and error handling for admin operations
- use configured credentials in RabbitMQ connection_url (#859)
- implement actual delay in DelayedHandler (#861)
- add URL encoding to prevent injection in query parameters
- migrate SQL utilities to SeaQuery for SQL injection prevention
- use escape_css_selector from reinhardt-core in WASM helpers
- use escape_html_content from reinhardt-core in DebugToolbar
- delegate has_permission to TestUser for wildcard support
- sync session user state when permissions change
- use String instead of Box::leak for ModelSchemaInfo
- store WASM closures in future struct instead of forget()
- use per-fixture tracking and UUIDs in DCL fixtures
- set env var before runtime in shared_postgres fixture
- extend container lifetime in redis_cluster_client fixture (#869)
- return Result from RequestBuilder::header instead of panicking
- panic with descriptive message on serialization failure in MockHttpRequest
- execute callbacks in MockTimers::run_due_callbacks and document MutationTracker limitations
- replace `mem::zeroed()` with `Option<C>` to eliminate UB in `into_inner()`

### Security

- fix path traversal in temp_file_url and cookie header injection

### Changed

- deduplicate request() by delegating to request_with_extra_headers()

### Styling

- fix clippy warnings and formatting in files merged from main

### Documentation

- add SAFETY comments to unsafe Send/Sync implementations

## [0.1.0-alpha.16](https://github.com/kent8192/reinhardt-web/compare/reinhardt-test@v0.1.0-alpha.15...reinhardt-test@v0.1.0-alpha.16) - 2026-02-16

### Maintenance

- updated the following local packages: reinhardt-db, reinhardt-auth, reinhardt-rest, reinhardt-views, reinhardt-urls, reinhardt-pages, reinhardt-admin, reinhardt-websockets

## [0.1.0-alpha.15](https://github.com/kent8192/reinhardt-web/compare/reinhardt-test@v0.1.0-alpha.14...reinhardt-test@v0.1.0-alpha.15) - 2026-02-15

### Maintenance

- updated the following local packages: reinhardt-query, reinhardt-rest, reinhardt-conf, reinhardt-db, reinhardt-auth, reinhardt-views, reinhardt-urls, reinhardt-pages, reinhardt-admin, reinhardt-websockets

## [0.1.0-alpha.14](https://github.com/kent8192/reinhardt-web/compare/reinhardt-test@v0.1.0-alpha.13...reinhardt-test@v0.1.0-alpha.14) - 2026-02-14

### Maintenance

- updated the following local packages: reinhardt-auth, reinhardt-rest, reinhardt-views, reinhardt-urls, reinhardt-pages, reinhardt-admin, reinhardt-websockets

## [0.1.0-alpha.13](https://github.com/kent8192/reinhardt-web/compare/reinhardt-test@v0.1.0-alpha.12...reinhardt-test@v0.1.0-alpha.13) - 2026-02-14

### Maintenance

- updated the following local packages: reinhardt-query, reinhardt-conf, reinhardt-db, reinhardt-auth, reinhardt-rest, reinhardt-views, reinhardt-urls, reinhardt-pages, reinhardt-admin, reinhardt-websockets

## [0.1.0-alpha.12](https://github.com/kent8192/reinhardt-web/compare/reinhardt-test@v0.1.0-alpha.11...reinhardt-test@v0.1.0-alpha.12) - 2026-02-14

### Maintenance

- updated the following local packages: reinhardt-query, reinhardt-conf, reinhardt-db, reinhardt-auth, reinhardt-rest, reinhardt-views, reinhardt-urls, reinhardt-pages, reinhardt-admin, reinhardt-websockets

## [0.1.0-alpha.10](https://github.com/kent8192/reinhardt-web/compare/reinhardt-test@v0.1.0-alpha.9...reinhardt-test@v0.1.0-alpha.10) - 2026-02-12

### Maintenance

- updated the following local packages: reinhardt-core, reinhardt-utils, reinhardt-conf, reinhardt-db, reinhardt-auth, reinhardt-urls, reinhardt-pages, reinhardt-admin, reinhardt-http, reinhardt-di, reinhardt-server, reinhardt-rest, reinhardt-views, reinhardt-websockets

## [0.1.0-alpha.9](https://github.com/kent8192/reinhardt-web/compare/reinhardt-test@v0.1.0-alpha.8...reinhardt-test@v0.1.0-alpha.9) - 2026-02-10

### Maintenance

- updated the following local packages: reinhardt-admin

## [0.1.0-alpha.8](https://github.com/kent8192/reinhardt-web/compare/reinhardt-test@v0.1.0-alpha.7...reinhardt-test@v0.1.0-alpha.8) - 2026-02-10

### Maintenance

- updated the following local packages: reinhardt-db, reinhardt-auth, reinhardt-rest, reinhardt-views, reinhardt-admin, reinhardt-websockets, reinhardt-urls, reinhardt-pages

## [0.1.0-alpha.7](https://github.com/kent8192/reinhardt-web/compare/reinhardt-test@v0.1.0-alpha.6...reinhardt-test@v0.1.0-alpha.7) - 2026-02-06

### Other

- updated the following local packages: reinhardt-utils, reinhardt-di, reinhardt-rest, reinhardt-conf, reinhardt-server, reinhardt-db, reinhardt-auth, reinhardt-views, reinhardt-urls, reinhardt-pages, reinhardt-admin, reinhardt-websockets

## [0.1.0-alpha.6](https://github.com/kent8192/reinhardt-web/compare/reinhardt-test@v0.1.0-alpha.5...reinhardt-test@v0.1.0-alpha.6) - 2026-02-03

### Other

- updated the following local packages: reinhardt-core, reinhardt-pages, reinhardt-http, reinhardt-utils, reinhardt-conf, reinhardt-di, reinhardt-server, reinhardt-db, reinhardt-auth, reinhardt-rest, reinhardt-views, reinhardt-urls, reinhardt-admin, reinhardt-websockets

## [0.1.0-alpha.5](https://github.com/kent8192/reinhardt-web/compare/reinhardt-test@v0.1.0-alpha.4...reinhardt-test@v0.1.0-alpha.5) - 2026-02-03

### Other

- merge main into chore/release-plz-migration
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

## [0.1.0-alpha.4] - 2026-01-30

### Changed

- Re-release of 0.1.0-alpha.3 content after version correction
- Rename feature `static` to `staticfiles` following `reinhardt-utils` module rename (#114)
- Update imports for `reinhardt_utils::staticfiles` module rename


## [0.1.0-alpha.3] - 2026-01-29 [YANKED]

**Note:** This version was yanked due to version skipping in the main crate (`reinhardt-web`). Use the latest available version instead.

### Changed

- Rename feature `static` to `staticfiles` following `reinhardt-utils` module rename (#114)
- Update imports for `reinhardt_utils::staticfiles` module rename


## [0.1.0-alpha.1] - 2026-01-23

### Added

- Initial crates.io release

