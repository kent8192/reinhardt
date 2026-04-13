# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0-rc.16](https://github.com/kent8192/reinhardt-web/compare/reinhardt-di@v0.1.0-rc.15...reinhardt-di@v0.1.0-rc.16) - 2026-04-13

### Added

- *(di)* add DiError::Authorization variant for 403 responses
- *(di)* [**breaking**] unify #[inject] parameter type from Arc<T> to Depends<T>
- *(di)* auto-derive Clone in #[injectable] macro
- *(di)* add qualified_type_names map to DependencyRegistry
- *(di)* add is_framework_type() with 40 unit tests
- *(di)* register qualified type names from macros
- *(di)* add FrameworkTypeOverride validation to RegistryValidator
- *(di)* [**breaking**] deprecate Injected<T> in favor of Depends<T> and remove auto-Clone

### Changed

- *(di)* remove dead use_cache branch and avoid Arc rewrap in Depends
- *(di)* remove unnecessary Clone bound from Depends<T> and Injected<T>
- *(di)* address Copilot review feedback on orphan rule validation

### Documentation

- *(di)* document attribute ordering requirement and add compile-fail tests

### Fixed

- *(di)* add Authentication variant to DiError for proper 401 responses
- *(di)* add #[non_exhaustive] to DiError enum
- *(auth)* use DiError::Authentication for unauthenticated user errors
- *(di)* resolve `#[inject]` type mismatch in `#[injectable_factory]` macro
- *(di)* wrap injectable_factory body in cycle detection scope
- update integration tests and docs for Depends<T> unification
- *(di)* generate `Injectable` impl in `#[injectable_factory]` for `Depends<T>` support
- *(di)* add scope fallback in resolve for pre-seeded types
- *(di)* register Injectable types in global registry for Depends resolution
- *(di)* resolve clippy warnings in tests, benchmarks, and override_registry
- *(di)* fall back to Injectable::inject() when type is not in registry
- *(di)* remove Injectable bound from Depends::resolve() for factory types
- *(di)* use resolve_from_registry() in injectable_factory macro only
- *(docs)* add Injectable impl to module-level doc example in depends.rs

### Other

- resolve conflict with main in di.rs ui module registration
- resolve conflict in registration.rs with main

### Styling

- *(di)* apply auto-fix formatting
- *(di)* apply rustfmt formatting
- apply rustfmt formatting fixes
- apply rustfmt to clippy-fixed files

### Testing

- *(di)* add edge-case tests for injectable auto-derive Clone
- *(di)* register test types in global registry for Depends resolution
- *(di)* add type_name format regression tests

## [0.1.0-rc.15](https://github.com/kent8192/reinhardt-web/compare/reinhardt-di@v0.1.0-rc.14...reinhardt-di@v0.1.0-rc.15) - 2026-03-29

### Fixed

- *(admin)* add deferred DI registration to bridge route-server scope gap
- *(di)* register HTTP request in request_scope during fork_for_request

### Maintenance

- update rust toolchain to 1.94.1 and set MSRV 1.94.0

## [0.1.0-rc.14](https://github.com/kent8192/reinhardt-web/compare/reinhardt-di@v0.1.0-rc.13...reinhardt-di@v0.1.0-rc.14) - 2026-03-24

### Added

- *(reinhardt-di)* add protocol-agnostic `fork()` method to `InjectionContext`

### Changed

- *(reinhardt-di)* extract fork_inner helper for InjectionContext fork methods

### Documentation

- fix outdated references in SECURITY.md, CONTRIBUTING.md, and documentation standards
- *(readme)* fix documentation discrepancies across crate READMEs

### Fixed

- *(reinhardt-pages,reinhardt-di)* add Content-Type negotiation for server_fn and Json<T> extractor
- *(reinhardt-di)* address Copilot review on Content-Type handling

## [0.1.0-rc.13](https://github.com/kent8192/reinhardt-web/compare/reinhardt-di@v0.1.0-rc.12...reinhardt-di@v0.1.0-rc.13) - 2026-03-18

### Fixed

- *(di)* set HTTP request on per-request InjectionContext in use_inject macro

## [0.1.0-rc.12](https://github.com/kent8192/reinhardt-web/compare/reinhardt-di@v0.1.0-rc.11...reinhardt-di@v0.1.0-rc.12) - 2026-03-18

### Added

- *(di)* add Option<T> blanket Injectable impl for optional injection

## [0.1.0-rc.9](https://github.com/kent8192/reinhardt-web/compare/reinhardt-di@v0.1.0-rc.8...reinhardt-di@v0.1.0-rc.9) - 2026-03-15

### Styling

- add explanatory comments to remaining #[allow(dead_code)] attributes

## [0.1.0-rc.5](https://github.com/kent8192/reinhardt-web/compare/reinhardt-di@v0.1.0-rc.4...reinhardt-di@v0.1.0-rc.5) - 2026-03-07

### Documentation

- add missing doc comments for public API modules and types

## [0.1.0-rc.2](https://github.com/kent8192/reinhardt-web/compare/reinhardt-di@v0.1.0-rc.1...reinhardt-di@v0.1.0-rc.2) - 2026-03-04

### Maintenance

- *(testing)* add insta snapshot testing dependency across all crates

## [0.1.0-rc.1](https://github.com/kent8192/reinhardt-web/compare/reinhardt-di@v0.1.0-alpha.8...reinhardt-di@v0.1.0-rc.1) - 2026-02-23

### Maintenance

- *(license)* migrate from MIT/Apache-2.0 to BSD 3-Clause

## [0.1.0-alpha.8](https://github.com/kent8192/reinhardt-web/compare/reinhardt-di@v0.1.0-alpha.7...reinhardt-di@v0.1.0-alpha.8) - 2026-02-21

### Fixed

- add reset_global_registry to enable test isolation
- return error for unregistered types instead of defaulting to Singleton
- remove undeclared tracing dependency from injectable macro output
- prevent Arc::try_unwrap panic and DependencyStream element consumption
- handle RwLock poisoning gracefully in scope and override registry

### Security

- improve generated name hygiene, crate path diagnostics, and type path validation
- reject unknown macro arguments and unsupported scope attribute
- add regex pattern length limit to prevent ReDoS attacks
- fix non-deterministic path tuple extraction order
- add body size limits to parameter extractors
- remove info leak and validate factory code generation
- migrate cycle detection to task_local and remove sampling

### Changed

- extract shared parse_cookies into cookie_util module

### Styling

- apply workspace-wide formatting fixes

### Testing

- add DependencyStream::is_empty non-destructive regression tests for #453

### Maintenance

- remove sea-query and sea-schema from workspace dependencies

## [0.1.0-alpha.6](https://github.com/kent8192/reinhardt-web/compare/reinhardt-di@v0.1.0-alpha.5...reinhardt-di@v0.1.0-alpha.6) - 2026-02-12

### Maintenance

- updated the following local packages: reinhardt-core, reinhardt-core, reinhardt-http

## [0.1.0-alpha.5](https://github.com/kent8192/reinhardt-web/compare/reinhardt-di@v0.1.0-alpha.4...reinhardt-di@v0.1.0-alpha.5) - 2026-02-09

### Fixed

- *(di)* move unit tests to integration crate to break circular publish chain
- *(di)* implement deep clone for InjectionContext request scope

### Reverted

- undo PR [[#219](https://github.com/kent8192/reinhardt-web/issues/219)](https://github.com/kent8192/reinhardt-web/issues/219) version bumps for unpublished crates

## [0.1.0-alpha.4](https://github.com/kent8192/reinhardt-web/compare/reinhardt-di@v0.1.0-alpha.3...reinhardt-di@v0.1.0-alpha.4) - 2026-02-06

### Fixed

- remove reinhardt-di self-reference dev-dependency

### Other

- Revert "Merge pull request #202 from kent8192/release-plz-2026-02-06T13-32-57Z"
- release

## [0.1.0-alpha.3](https://github.com/kent8192/reinhardt-web/compare/reinhardt-di@v0.1.0-alpha.2...reinhardt-di@v0.1.0-alpha.3) - 2026-02-03

### Other

- updated the following local packages: reinhardt-core, reinhardt-core, reinhardt-http

## [0.1.0-alpha.2](https://github.com/kent8192/reinhardt-web/compare/reinhardt-di@v0.1.0-alpha.1...reinhardt-di@v0.1.0-alpha.2) - 2026-02-03

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

