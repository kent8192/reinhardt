# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0](https://github.com/kent8192/reinhardt-web/releases/tag/reinhardt-di@v0.1.0) - 2026-03-08

### Changed

- extract shared parse_cookies into cookie_util module

### Fixed

- add reset_global_registry to enable test isolation
- return error for unregistered types instead of defaulting to Singleton
- remove undeclared tracing dependency from injectable macro output
- prevent Arc::try_unwrap panic and DependencyStream element consumption
- handle RwLock poisoning gracefully in scope and override registry
- *(di)* move unit tests to integration crate to break circular publish chain
- *(di)* implement deep clone for InjectionContext request scope

### Security

- improve generated name hygiene, crate path diagnostics, and type path validation
- reject unknown macro arguments and unsupported scope attribute
- add regex pattern length limit to prevent ReDoS attacks
- fix non-deterministic path tuple extraction order
- add body size limits to parameter extractors
- remove info leak and validate factory code generation
- migrate cycle detection to task_local and remove sampling

### Documentation

- add missing doc comments for public API modules and types

### Testing

- add DependencyStream::is_empty non-destructive regression tests for #453

### Styling

- apply workspace-wide formatting fixes

### Maintenance

- *(testing)* add insta snapshot testing dependency across all crates
- *(license)* migrate from MIT/Apache-2.0 to BSD 3-Clause
- remove sea-query and sea-schema from workspace dependencies
