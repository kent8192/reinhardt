# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0-rc.16](https://github.com/kent8192/reinhardt-web/compare/reinhardt-testkit@v0.1.0-rc.15...reinhardt-testkit@v0.1.0-rc.16) - 2026-04-15

### Added

- *(testkit)* add in-process test transport for APIClient via Handler trait
- migrate UUID generation from v4 to v7 across entire codebase
- *(testkit)* add with_test_di_context() for isolated parallel-safe test DI contexts
- *(testkit)* add builder-based auth testing API

### Deprecated

- update deprecation since version from 0.2.0 to 0.1.0-rc.16

### Fixed

- *(admin)* add missing SingletonScope import and fix formatting
- *(testkit)* use random portion of UUID v7 for unique suffix
- *(testkit)* address review feedback and CI clippy failures
- *(testkit)* update deprecated since to 0.1.0-rc.16 and suppress internal usage warnings
- *(docs)* use backticks for feature-gated types in testkit auth docs

### Maintenance

- upgrade workspace dependencies to latest versions
- *(testkit)* add auth-testing feature with reinhardt-auth and reinhardt-middleware optional deps

### Security

- *(testkit)* remove sensitive values from cookie validation panic messages

### Styling

- *(testkit)* fix rustfmt formatting in di.rs
- *(testkit)* format cookie value assertion in client.rs

### Testing

- *(di)* register test types in global registry for Depends resolution

## [0.1.0-rc.15](https://github.com/kent8192/reinhardt-web/compare/reinhardt-testkit@v0.1.0-rc.14...reinhardt-testkit@v0.1.0-rc.15) - 2026-03-29

### Fixed

- *(testkit)* use multi-thread runtime and install any drivers in fixture tests

### Maintenance

- update rust toolchain to 1.94.1 and set MSRV 1.94.0

### Testing

- *(testkit)* add comprehensive tests for core utility modules
- *(testkit)* add tests for fixture modules (server, testcontainers, resources, validator)

## [0.1.0-rc.14](https://github.com/kent8192/reinhardt-web/compare/reinhardt-testkit@v0.1.0-rc.13...reinhardt-testkit@v0.1.0-rc.14) - 2026-03-24

### Fixed

- *(testkit)* unify PostgreSQL version, add pool close, and cleanup backoff
- *(test)* improve test infrastructure reliability and E2E fixtures
- *(test,testkit)* address Copilot review feedback on test infrastructure
- *(testkit)* initialize ORM global state in postgres_with_migrations_from_dir
- resolve CI failures and remove sea-query dependency

## [0.1.0-rc.12](https://github.com/kent8192/reinhardt-web/compare/reinhardt-testkit@v0.1.0-rc.11...reinhardt-testkit@v0.1.0-rc.12) - 2026-03-18

### Added

- *(testkit)* add postgres_with_migrations_from_dir helper using FilesystemSource

### Deprecated

- *(testkit)* deprecate global_registry-based migration fixtures

### Documentation

- *(macros,testkit)* use backticks for cross-crate intra-doc links

### Styling

- *(testkit)* apply auto-fix formatting to fixtures re-export

## [0.1.0-rc.9](https://github.com/kent8192/reinhardt-web/compare/reinhardt-testkit@v0.1.0-rc.8...reinhardt-testkit@v0.1.0-rc.9) - 2026-03-15

### Styling

- add explanatory comments to remaining #[allow(dead_code)] attributes
