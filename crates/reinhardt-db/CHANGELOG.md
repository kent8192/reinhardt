# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0-rc.23](https://github.com/kent8192/reinhardt-web/compare/reinhardt-db@v0.1.0-rc.22...reinhardt-db@v0.1.0-rc.23) - 2026-04-29

### Added

- *(db)* add CustomManager and HasCustomManager traits
- *(db)* expose QuerySet::filters accessor

### Documentation

- *(db)* add runnable doctest and Quick Start to CustomManager
- *(db)* document custom object managers in README

### Fixed

- *(ci)* silence clippy::type_complexity on bulk_update_sql_detailed
- *(db)* emit add/drop constraint operations from autodetector

### Styling

- apply rustfmt formatting from cargo make auto-fix

### Testing

- *(db)* add CustomManager smoke tests
- *(db)* broaden CustomManager SQL parity coverage
- *(db)* construct unit-struct managers directly to satisfy clippy

## [0.1.0-rc.21](https://github.com/kent8192/reinhardt-web/compare/reinhardt-db@v0.1.0-rc.20...reinhardt-db@v0.1.0-rc.21) - 2026-04-23

### Documentation

- add reinhardt-version-sync markers to all crate READMEs

## [0.1.0-rc.20](https://github.com/kent8192/reinhardt-web/compare/reinhardt-db@v0.1.0-rc.19...reinhardt-db@v0.1.0-rc.20) - 2026-04-23

### Documentation

- *(db)* fix ConnectionPool API and import path inaccuracies in README

## [0.1.0-rc.16](https://github.com/kent8192/reinhardt-web/compare/reinhardt-db@v0.1.0-rc.15...reinhardt-db@v0.1.0-rc.16) - 2026-04-20

### Added

- migrate UUID generation from v4 to v7 across entire codebase
- *(db)* add SetAutoIncrementValue and CreateCompositePrimaryKey migration ops
- *(db)* extend Autodetector to emit CreateCompositePrimaryKey and SetAutoIncrementValue
- *(db)* handle composite PK modification, fix auto-increment fallback, add diagnostic

### Changed

- *(orm)* adopt array_windows for type-safe sliding window iteration

### Documentation

- *(db)* correct SetAutoIncrementValue docstring to match setval impl

### Fixed

- *(migrations)* skip no-op migrations for struct-only renames
- *(db)* fail hard on empty CreateCompositePrimaryKey columns
- *(db)* refuse silent Postgres fallback for SetAutoIncrementValue in to_statement

### Other

- resolve conflict with main in admin router tests

### Styling

- apply cargo fmt
- *(db)* apply rustfmt formatting to autodetector
- apply cargo fmt --all

## [0.1.0-rc.15](https://github.com/kent8192/reinhardt-web/compare/reinhardt-db@v0.1.0-rc.14...reinhardt-db@v0.1.0-rc.15) - 2026-03-29

### Documentation

- complete feature flags tables in core and db crates

### Fixed

- *(db)* use if-let pattern instead of is_some/unwrap

### Maintenance

- update rust toolchain to 1.94.1 and set MSRV 1.94.0

### Security

- *(db)* [**breaking**] remove SQL injection vulnerable BatchUpdateBuilder methods

## [0.1.0-rc.14](https://github.com/kent8192/reinhardt-web/compare/reinhardt-db@v0.1.0-rc.13...reinhardt-db@v0.1.0-rc.14) - 2026-03-24

### Fixed

- *(migrations)* handle `.to_string()` in dependency tuple parsing
- *(migrations)* resolve multi-element dependency parsing and deterministic sort
- *(reinhardt-db)* remove unnecessary dereference in pool connection
- *(db)* apply ManuallyDrop to backends_pool PooledConnection Drop
- *(reinhardt-db)* fix dependency collection, table tracking, BFS ordering, and lock pattern in migrations
- *(reinhardt-db)* fix cache value in next_number_cached and optimize squash dedup
- *(db)* escape double quotes in PostgreSQL quote_identifier
- *(db)* quote column names in ON CONFLICT clauses
- *(db)* quote identifiers in BatchInsertBuilder::build_sql
- *(query,db)* address copilot review on SQL injection PR
- *(db)* update test expectations for quoted identifiers and parameterized LIMIT

### Styling

- *(docs)* apply auto-fix formatting and lint corrections

## [0.1.0-rc.13](https://github.com/kent8192/reinhardt-web/compare/reinhardt-db@v0.1.0-rc.12...reinhardt-db@v0.1.0-rc.13) - 2026-03-18

### Fixed

- *(migrations)* skip duplicate operations check for empty-operations migrations

## [0.1.0-rc.12](https://github.com/kent8192/reinhardt-web/compare/reinhardt-db@v0.1.0-rc.11...reinhardt-db@v0.1.0-rc.12) - 2026-03-18

### Added

- *(db)* add migration conflict detection and merge name generation

### Documentation

- *(db)* recommend CARGO_MANIFEST_DIR for workspace-safe migration paths

### Fixed

- *(db)* warn when FilesystemSource root directory does not exist

### Other

- incorporate main branch docs.rs fixes

### Styling

- *(db)* apply auto-fix formatting to filesystem source

## [0.1.0-rc.11](https://github.com/kent8192/reinhardt-web/compare/reinhardt-db@v0.1.0-rc.10...reinhardt-db@v0.1.0-rc.11) - 2026-03-16

### Fixed

- *(reinhardt-db)* fix makemigrations codegen for type mismatch and missing fields

## [0.1.0-rc.9](https://github.com/kent8192/reinhardt-web/compare/reinhardt-db@v0.1.0-rc.8...reinhardt-db@v0.1.0-rc.9) - 2026-03-15

### Changed

- *(db)* centralize schema identifier escaping and document value_expression safety

### Fixed

- *(db)* escape SQL identifiers in extension and schema operations
- *(db)* replace lock/read/write unwrap with poison-safe alternatives
- *(db)* add double-panic prevention and improve poison recovery

### Styling

- apply auto-fix for fmt and clippy

### Testing

- *(db)* add schema name escaping tests

## [0.1.0-rc.5](https://github.com/kent8192/reinhardt-web/compare/reinhardt-db@v0.1.0-rc.4...reinhardt-db@v0.1.0-rc.5) - 2026-03-07

### Added

- add reinhardt-query prelude re-exports to reinhardt-db orm

## [0.1.0-rc.4](https://github.com/kent8192/reinhardt-web/compare/reinhardt-db@v0.1.0-rc.3...reinhardt-db@v0.1.0-rc.4) - 2026-03-05

### Fixed

- *(db)* use extract_string_field in migration AST parser to handle .to_string() pattern

## [0.1.0-rc.2](https://github.com/kent8192/reinhardt-web/compare/reinhardt-db@v0.1.0-rc.1...reinhardt-db@v0.1.0-rc.2) - 2026-03-04

### Fixed

- *(db)* prevent SQL injection in BatchUpdateBuilder and QuerySet filters
- *(db)* preserve backward compatibility for batch_ops API
- *(deps)* align dependency versions to workspace definitions

### Other

- resolve fields.rs conflict with main

### Styling

- *(db)* apply formatter to batch_ops

### Testing

- *(db)* add coverage tests for BigUnsigned overflow clamping

## [0.1.0-rc.1](https://github.com/kent8192/reinhardt-web/compare/reinhardt-db@v0.1.0-alpha.16...reinhardt-db@v0.1.0-rc.1) - 2026-02-24

### Fixed

- *(db)* gate sqlite-dependent tests with feature flag
- *(db)* replace float test values to avoid clippy approx_constant lint

### Testing

- *(db)* add warning log test for .sql file detection

## [0.1.0-alpha.16](https://github.com/kent8192/reinhardt-web/compare/reinhardt-db@v0.1.0-alpha.15...reinhardt-db@v0.1.0-alpha.16) - 2026-02-23

### Maintenance

- *(license)* migrate from MIT/Apache-2.0 to BSD 3-Clause

## [0.1.0-alpha.15](https://github.com/kent8192/reinhardt-web/compare/reinhardt-db@v0.1.0-alpha.14...reinhardt-db@v0.1.0-alpha.15) - 2026-02-23

### Maintenance

- updated the following local packages: reinhardt-query, reinhardt-conf

## [0.1.0-alpha.14](https://github.com/kent8192/reinhardt-web/compare/reinhardt-db@v0.1.0-alpha.13...reinhardt-db@v0.1.0-alpha.14) - 2026-02-21

### Added

- add Repository<T> for type-safe ODM CRUD operations
- implement IndexModel with builder pattern and MongoDB conversion
- add core Document trait for ODM layer
- add ODM-specific error types for validation and operation failures

### Fixed

- add safe numeric conversions with proper error handling
- adapt DatabaseConfig.password usage to SecretString type
- use parameterized queries and escape identifiers to prevent SQL injection
- add BackendError variant and proper error mapping in repository
- make bson an optional dependency
- use bson::error::Error for deserialization

### Security

- document raw SQL injection surface in query builder APIs
- replace panics with error returns and use checked integer conversion
- fix path traversal and credential masking
- fix savepoint name injection in orm transaction module

### Changed

- update references for flattened examples structure
- clean up unused fixtures and fix documentation
- remove unnecessary async_trait from Document trait
- reorganize re-exports for ODM and low-level API separation
- make bson dependency always available for ODM support

### Styling

- fix pre-existing clippy warnings and apply rustfmt
- collapse nested if statements per clippy::collapsible_if
- apply rustfmt formatting to workspace files
- apply code formatting to security fix files
- format code with rustfmt

### Maintenance

- mark implicit TODOs for NoSQL ODM completion
- remove unused ValidationError import

## [0.1.0-alpha.12](https://github.com/kent8192/reinhardt-web/compare/reinhardt-db@v0.1.0-alpha.11...reinhardt-db@v0.1.0-alpha.12) - 2026-02-15

### Maintenance

- updated the following local packages: reinhardt-query, reinhardt-conf

## [0.1.0-alpha.11](https://github.com/kent8192/reinhardt-web/compare/reinhardt-db@v0.1.0-alpha.10...reinhardt-db@v0.1.0-alpha.11) - 2026-02-14

### Maintenance

- updated the following local packages: reinhardt-query, reinhardt-conf

## [0.1.0-alpha.10](https://github.com/kent8192/reinhardt-web/compare/reinhardt-db@v0.1.0-alpha.9...reinhardt-db@v0.1.0-alpha.10) - 2026-02-14

### Maintenance

- updated the following local packages: reinhardt-query, reinhardt-conf

## [0.1.0-alpha.9](https://github.com/kent8192/reinhardt-web/compare/reinhardt-db@v0.1.0-alpha.8...reinhardt-db@v0.1.0-alpha.9) - 2026-02-14

### Changed

- *(db)* replace super::super:: with crate:: absolute paths in migrations
- *(db)* fix unused variable assignments in migration operation tests

### Fixed

- *(db)* bind insert values in many-to-many manager instead of discarding

## [0.1.0-alpha.8](https://github.com/kent8192/reinhardt-web/compare/reinhardt-db@v0.1.0-alpha.7...reinhardt-db@v0.1.0-alpha.8) - 2026-02-12

### Changed

- convert relative paths to absolute paths
- *(db)* convert relative paths to absolute paths in orm execution
- restore single-level super:: paths preserved by convention

### Fixed

- correct incorrect path conversions in test imports

## [0.1.0-alpha.7](https://github.com/kent8192/reinhardt-web/compare/reinhardt-db@v0.1.0-alpha.6...reinhardt-db@v0.1.0-alpha.7) - 2026-02-10

### Fixed

- *(db)* remove unused reinhardt-test dev-dependency
- *(release)* revert unpublished crate versions to pre-release state

### Reverted

- undo PR [[#219](https://github.com/kent8192/reinhardt-web/issues/219)](https://github.com/kent8192/reinhardt-web/issues/219) version bumps for unpublished crates

## [0.1.0-alpha.6](https://github.com/kent8192/reinhardt-web/compare/reinhardt-db@v0.1.0-alpha.5...reinhardt-db@v0.1.0-alpha.6) - 2026-02-06

### Other

- updated the following local packages: reinhardt-di, reinhardt-conf

## [0.1.0-alpha.5](https://github.com/kent8192/reinhardt-web/compare/reinhardt-db@v0.1.0-alpha.4...reinhardt-db@v0.1.0-alpha.5) - 2026-02-03

### Other

- updated the following local packages: reinhardt-core, reinhardt-conf, reinhardt-di

## [0.1.0-alpha.4](https://github.com/kent8192/reinhardt-web/compare/reinhardt-db@v0.1.0-alpha.3...reinhardt-db@v0.1.0-alpha.4) - 2026-02-03

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

## [0.1.0-alpha.3] - 2026-01-30

### Changed

- Version bump for publish workflow correction (no functional changes)

## [0.1.0-alpha.2] - 2026-01-29

### Changed

- Improve CHECK constraints comments in PostgreSQL and MySQL introspectors for clarity
- Update package version from workspace reference to explicit version

## [0.1.0-alpha.1] - 2026-01-23

### Added

- Initial crates.io release

