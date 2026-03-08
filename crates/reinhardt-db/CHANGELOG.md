# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0](https://github.com/kent8192/reinhardt-web/releases/tag/reinhardt-db@v0.1.0) - 2026-03-08

### Added

- add reinhardt-query prelude re-exports to reinhardt-db orm
- add Repository<T> for type-safe ODM CRUD operations
- implement IndexModel with builder pattern and MongoDB conversion
- add core Document trait for ODM layer
- add ODM-specific error types for validation and operation failures

### Changed

- update references for flattened examples structure
- clean up unused fixtures and fix documentation
- remove unnecessary async_trait from Document trait
- reorganize re-exports for ODM and low-level API separation
- make bson dependency always available for ODM support
- *(db)* replace super::super:: with crate:: absolute paths in migrations
- *(db)* fix unused variable assignments in migration operation tests
- convert relative paths to absolute paths
- *(db)* convert relative paths to absolute paths in orm execution
- restore single-level super:: paths preserved by convention
- Improve CHECK constraints comments in PostgreSQL and MySQL introspectors for clarity

### Fixed

- *(db)* use extract_string_field in migration AST parser to handle .to_string() pattern
- *(db)* prevent SQL injection in BatchUpdateBuilder and QuerySet filters
- *(db)* preserve backward compatibility for batch_ops API
- *(deps)* align dependency versions to workspace definitions
- *(db)* gate sqlite-dependent tests with feature flag
- *(db)* replace float test values to avoid clippy approx_constant lint
- add safe numeric conversions with proper error handling
- adapt DatabaseConfig.password usage to SecretString type
- use parameterized queries and escape identifiers to prevent SQL injection
- add BackendError variant and proper error mapping in repository
- make bson an optional dependency
- use bson::error::Error for deserialization
- *(db)* bind insert values in many-to-many manager instead of discarding
- *(db)* remove unused reinhardt-test dev-dependency

### Security

- document raw SQL injection surface in query builder APIs
- replace panics with error returns and use checked integer conversion
- fix path traversal and credential masking
- fix savepoint name injection in orm transaction module

### Testing

- *(db)* add coverage tests for BigUnsigned overflow clamping
- *(db)* add warning log test for .sql file detection

### Styling

- *(db)* apply formatter to batch_ops
- fix pre-existing clippy warnings and apply rustfmt
- collapse nested if statements per clippy::collapsible_if
- apply rustfmt formatting to workspace files
- apply code formatting to security fix files
- format code with rustfmt

### Maintenance

- *(license)* migrate from MIT/Apache-2.0 to BSD 3-Clause
- mark implicit TODOs for NoSQL ODM completion
- remove unused ValidationError import
