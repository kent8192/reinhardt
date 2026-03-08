# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0](https://github.com/kent8192/reinhardt-web/releases/tag/reinhardt-test@v0.1.0) - 2026-03-08

### Added

- standardize PostgreSQL version to 17

### Changed

- *(reinhardt-test)* delegate to reinhardt-testkit with re-exports
- deduplicate request() by delegating to request_with_extra_headers()
- Rename feature `static` to `staticfiles` following `reinhardt-utils` module rename (#114)
- Update imports for `reinhardt_utils::staticfiles` module rename
- Rename feature `static` to `staticfiles` following `reinhardt-utils` module rename (#114)
- Update imports for `reinhardt_utils::staticfiles` module rename

### Fixed

- *(deps)* update reinhardt-test outdated deps
- *(deps)* convert Vec to Bytes for tungstenite message types
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

### Documentation

- add SAFETY comments to unsafe Send/Sync implementations

### Styling

- fix clippy warnings and formatting in files merged from main

### Maintenance

- *(license)* migrate from MIT/Apache-2.0 to BSD 3-Clause

### Other

- resolve conflict with main branch version bump to rc.4
