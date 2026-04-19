# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0-rc.15](https://github.com/kent8192/reinhardt-web/compare/reinhardt-macros@v0.1.0-rc.14...reinhardt-macros@v0.1.0-rc.15) - 2026-03-29

### Added

- *(orm)* add Vec/Value/HashMap support to field_type_to_metadata_string
- *(macros)* inject ManyToMany relationships in #[user] + #[model]
- *(orm)* add #[field(skip = true)] attribute for non-DB fields

### Fixed

- *(admin)* generate table_name() and permission methods in admin macro
- *(macros)* allow too_many_arguments on generated Model::new function

### Maintenance

- update rust toolchain to 1.94.1 and set MSRV 1.94.0

## [0.1.0-rc.14](https://github.com/kent8192/reinhardt-web/compare/reinhardt-macros@v0.1.0-rc.13...reinhardt-macros@v0.1.0-rc.14) - 2026-03-24

### Added

- *(macros)* add nom v8.0.0 parser for settings composition syntax
- *(macros)* implement #[settings] attribute macro (fragment + composition)
- *(macros)* extend nom parser for { field: policy } override blocks
- *(macros)* add #[setting()] attribute parsing and field_policies() generation
- *(macros)* add composition override blocks and ComposedSettings generation

### Changed

- *(macros)* require explicit CoreSettings in #[settings] macro
- *(macros)* generate HasSettings<F> in both settings macros

### Documentation

- *(crates)* update version references from 0.1.0-alpha.1 to 0.1.0-rc.13 across all READMEs

### Fixed

- *(reinhardt-core)* fork DI context per-request in route and action macros
- suppress deprecated Settings warnings and fix unreachable pub visibility
- *(macros)* add missing CoreSettings and HasCoreSettings imports for explicit settings
- *(settings)* address Copilot review feedback for field policy system
- *(settings)* use section-nested keys in #[settings] macro validation and deserialization

### Other

- resolve conflict with main (BUILTIN_FRAGMENTS + resolve helpers)
- resolve conflict with main (implicit inference tests from PR [[#2860](https://github.com/kent8192/reinhardt-web/issues/2860)](https://github.com/kent8192/reinhardt-web/issues/2860))

### Styling

- apply rustfmt formatting
- apply formatting fixes for field policy changes

### Testing

- *(macros)* add trybuild fail tests for #[settings] proc macro
- *(settings)* update tests for explicit CoreSettings requirement

## [0.1.0-rc.13](https://github.com/kent8192/reinhardt-web/compare/reinhardt-macros@v0.1.0-rc.12...reinhardt-macros@v0.1.0-rc.13) - 2026-03-18

### Fixed

- *(di)* set HTTP request on per-request InjectionContext in use_inject macro

## [0.1.0-rc.12](https://github.com/kent8192/reinhardt-web/compare/reinhardt-macros@v0.1.0-rc.11...reinhardt-macros@v0.1.0-rc.12) - 2026-03-18

### Added

- *(core)* auto-detect #[inject] without requiring use_inject = true
- *(rest)* add operation-level OpenAPI route attributes

### Deprecated

- *(core)* deprecate collect_migrations! macro in favor of FilesystemSource

### Documentation

- *(macros)* use backtick for FilesystemSource in collect_migrations doc

## [0.1.0-rc.10](https://github.com/kent8192/reinhardt-web/compare/reinhardt-macros@v0.1.0-rc.9...reinhardt-macros@v0.1.0-rc.10) - 2026-03-15

### Added

- *(macros)* add range(min, max) support to #[derive(Validate)]

### Fixed

- *(macros)* remove feature-dependent code generation from #[routes] macro
- *(urls)* restore semver-compatible new() and add __macro_new()

### Testing

- *(macros)* add integration and UI tests for validate range attribute

## [0.1.0-rc.9](https://github.com/kent8192/reinhardt-web/compare/reinhardt-macros@v0.1.0-rc.8...reinhardt-macros@v0.1.0-rc.9) - 2026-03-15

### Added

- feat!(macros): add #[derive(Validate)] proc macro for field-level validation

### Changed

- refactor!(macros): replace external validator crate in pre_validate codegen

## [0.1.0-rc.5](https://github.com/kent8192/reinhardt-web/compare/reinhardt-macros@v0.1.0-rc.4...reinhardt-macros@v0.1.0-rc.5) - 2026-03-07

### Fixed

- *(macros)* dereference extractor before validation in pre_validate
- *(macros)* replace skeleton tests with meaningful assertions in pre_validate

## [0.1.0-rc.2](https://github.com/kent8192/reinhardt-web/compare/reinhardt-macros@v0.1.0-rc.1...reinhardt-macros@v0.1.0-rc.2) - 2026-03-04

### Fixed

- *(macros)* add auto_increment param to field registration
- *(macros)* infer not_null from Rust Option type in field registration
- *(macros)* map DateTime to TimestampTz for timezone-aware columns

## [0.1.0-rc.1](https://github.com/kent8192/reinhardt-web/compare/reinhardt-macros@v0.1.0-alpha.3...reinhardt-macros@v0.1.0-rc.1) - 2026-02-23

### Fixed

- *(release)* advance version to skip yanked alpha.3 and restore publish capability for dependents

## [0.1.0-alpha.3] - 2026-02-21 [YANKED]

This release was yanked shortly after publication. Use v0.1.0-alpha.4 instead.

## [0.1.0-alpha.2](https://github.com/kent8192/reinhardt-web/compare/reinhardt-macros@v0.1.0-alpha.1...reinhardt-macros@v0.1.0-alpha.2) - 2026-02-03

### Other

- *(package)* replace version.workspace with explicit versions
