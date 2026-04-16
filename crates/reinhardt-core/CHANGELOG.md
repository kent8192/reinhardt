# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0-rc.16](https://github.com/kent8192/reinhardt-web/compare/reinhardt-core@v0.1.0-rc.15...reinhardt-core@v0.1.0-rc.16) - 2026-04-16

### Added

- *(auth)* add SuperuserInit trait and SuperuserCreator registry
- *(auth)* auto-register SuperuserCreator via inventory for #[user(full = true)] + #[model] types
- *(core)* add AuthProtection enum and EndpointMetadata extension
- *(core)* detect auth parameters in route macros for metadata
- *(core)* set task-local resolve context in request dispatch macros
- *(urls)* [**breaking**] support async functions in #[routes] macro
- *(commands)* add RunserverHook for concurrent service startup and pre-listen validation
- *(urls)* add compile-time type-safe URL resolution via extension traits
- migrate UUID generation from v4 to v7 across entire codebase
- *(di)* [**breaking**] deprecate Injected<T> in favor of Depends<T> and remove auto-Clone
- *(macros)* [**breaking**] extend #[url_patterns] for viewset/mount, add #[viewset] macro, deprecate named variants
- *(macros)* accept typed identifier instead of string literal in `#[url_patterns]`

### Changed

- *(di)* remove unnecessary Clone bound from Depends<T> and Injected<T>

### Fixed

- *(macros)* delegate DI injection errors to From<DiError> instead of hardcoding 500
- *(pages)* cfg-gate @event handler compilation to wasm32 only
- resolve merge conflicts with main and fix CI failures
- *(core)* unify flush_updates to actually execute pending effects
- *(di)* [**breaking**] make DependencyRegistration const-compatible for Rust 2024 edition
- *(di)* address Copilot review feedback on const-compatible DependencyRegistration
- *(commands)* address Copilot review feedback on RunserverHook
- *(urls)* address Copilot review on URL resolver macro generation
- *(query,core)* replace approx_constant test values to avoid clippy deny
- *(core)* resolve clippy warnings in reactive, security, and exception modules
- *(macros)* remove url-resolver feature flag, gate on platform instead
- *(macros)* use resolve_from_registry() in #[routes] for factory type compatibility
- *(macros)* integrate standalone mode with namespaced URL resolvers
- *(macros)* resolve merge conflict with main
- *(macros)* move #[viewset] usage to module-level free function
- *(macros)* address review — error propagation, WASM skip, ident validation
- *(macros)* resolve url_patterns path resolution and nested endpoint detection
- *(macros)* use tt repetition instead of path fragment in __for_each_url_resolver
- *(macros)* address Copilot review — use tt+ and normalize test assertions
- *(macros)* use tt+ pattern instead of path in for_each_url_resolver macro
- *(urls)* add UrlResolver trait import to generated url_prelude methods

### Styling

- *(core)* apply rustfmt formatting
- apply auto-fix formatting corrections
- apply rustfmt formatting fixes
- apply rustfmt to clippy-fixed files
- *(macros)* apply rustfmt to url_patterns tests

## [0.1.0-rc.15](https://github.com/kent8192/reinhardt-web/compare/reinhardt-core@v0.1.0-rc.14...reinhardt-core@v0.1.0-rc.15) - 2026-03-29

### Added

- *(orm)* add Vec/Value/HashMap support to field_type_to_metadata_string
- *(macros)* inject ManyToMany relationships in #[user] + #[model]
- *(orm)* add #[field(skip = true)] attribute for non-DB fields

### Fixed

- *(admin)* generate table_name() and permission methods in admin macro
- *(macros)* allow too_many_arguments on generated Model::new function
- *(core)* add feature gates to conditionally compiled modules

### Maintenance

- update rust toolchain to 1.94.1 and set MSRV 1.94.0

## [0.1.0-rc.14](https://github.com/kent8192/reinhardt-web/compare/reinhardt-core@v0.1.0-rc.13...reinhardt-core@v0.1.0-rc.14) - 2026-03-24

### Added

- *(macros)* extend nom parser for { field: policy } override blocks
- *(macros)* add #[setting()] attribute parsing and field_policies() generation
- *(macros)* add composition override blocks and ComposedSettings generation

### Changed

- *(macros)* require explicit CoreSettings in #[settings] macro
- *(macros)* generate HasSettings<F> in both settings macros

### Fixed

- *(reinhardt-core)* fork DI context per-request in route and action macros
- *(reinhardt-core)* fix DLQ lock ordering, send_async filtering, overflow, and counter bugs in signals
- *(reinhardt-core)* pre-allocate results vector and fix profiler average calculation
- *(core)* prevent panics from user-controlled pagination and string inputs
- *(core)* address Copilot review feedback on panic-safety PR
- *(macros)* add missing CoreSettings and HasCoreSettings imports for explicit settings
- *(settings)* address Copilot review feedback for field policy system
- *(settings)* use section-nested keys in #[settings] macro validation and deserialization

### Other

- resolve conflict with main (BUILTIN_FRAGMENTS + resolve helpers)
- resolve conflict with main (implicit inference tests from PR [[#2860](https://github.com/kent8192/reinhardt-web/issues/2860)](https://github.com/kent8192/reinhardt-web/issues/2860))

### Styling

- reformat long lines in effect and burst modules
- apply formatting fixes for field policy changes

### Testing

- *(settings)* update tests for explicit CoreSettings requirement

## [0.1.0-rc.12](https://github.com/kent8192/reinhardt-web/compare/reinhardt-core@v0.1.0-rc.11...reinhardt-core@v0.1.0-rc.12) - 2026-03-18

### Added

- *(rest)* add operation-level OpenAPI route attributes

### Documentation

- *(macros)* use backtick for FilesystemSource in collect_migrations doc

### Fixed

- *(rest)* address Copilot review feedback on OpenAPI annotations

## [0.1.0-rc.9](https://github.com/kent8192/reinhardt-web/compare/reinhardt-core@v0.1.0-rc.8...reinhardt-core@v0.1.0-rc.9) - 2026-03-15

### Added

- *(core)* add validate_html_attr_name for attribute key validation

### Fixed

- *(core,pages)* escape script tag content and HTML attributes to prevent XSS
- *(pages)* validate attr keys, fix SSR lang escaping, enhance script escape docs
- *(core)* correct misleading CSRF token rotation comment
- *(core)* replace lock().unwrap() with safe alternatives for panic prevention
- *(core)* centralize message mutex poison recovery with logging

## [0.1.0-rc.5](https://github.com/kent8192/reinhardt-web/compare/reinhardt-core@v0.1.0-rc.4...reinhardt-core@v0.1.0-rc.5) - 2026-03-07

### Fixed

- *(macros)* dereference extractor before validation in pre_validate
- *(macros)* replace skeleton tests with meaningful assertions in pre_validate

## [0.1.0-rc.4](https://github.com/kent8192/reinhardt-web/compare/reinhardt-core@v0.1.0-rc.3...reinhardt-core@v0.1.0-rc.4) - 2026-03-05

### Fixed

- *(core)* add wasm32 platform gate to parallel and jsonschema validator modules

## [0.1.0-rc.2](https://github.com/kent8192/reinhardt-web/compare/reinhardt-core@v0.1.0-rc.1...reinhardt-core@v0.1.0-rc.2) - 2026-03-04

### Fixed

- *(deps)* align dependency versions to workspace definitions
- *(core)* use character count instead of byte length in CharField validation

## [0.1.0-rc.1](https://github.com/kent8192/reinhardt-web/compare/reinhardt-core@v0.1.0-alpha.7...reinhardt-core@v0.1.0-rc.1) - 2026-02-23

### Maintenance

- *(license)* migrate from MIT/Apache-2.0 to BSD 3-Clause

## [0.1.0-alpha.7](https://github.com/kent8192/reinhardt-web/compare/reinhardt-core@v0.1.0-alpha.6...reinhardt-core@v0.1.0-alpha.7) - 2026-02-21

### Added

- add path sanitization and input validation helpers
- add resource limits configuration types
- add numeric safety utilities for checked arithmetic
- add redirect URL validation utilities
- add anchor link support to is_safe_url
- add enhanced sanitization utilities for XSS prevention

### Fixed

- fix DOT format output vulnerable to content injection
- log signal send errors instead of silently discarding
- emit errors instead of silently ignoring invalid macro arguments
- fix Request type path and remove tracing from use_inject generated code
- replace unwrap() with proper syn::Error propagation in proc macros
- prevent arithmetic underflow in cursor pagination encoder
- use exact MIME type matching in ContentTypeValidator
- replace Box::leak with Arc to prevent memory leak
- emit error when permission function lacks Request (#775)
- use push instead of push_str for single char in escape_css_selector

### Security

- add default size limits to multipart parser
- replace eprintln with tracing to prevent type info leakage
- fix fragile CSRF token format parsing
- add input validation for route paths and SQL expressions
- fix signal handler deadlock by releasing lock before callback execution
- fix input validation and resource limits across form fields
- remove info leak and validate factory code generation
- use HMAC-SHA256 for cursor integrity validation
- fix CSP header sanitization and CSRF panic
- add request body size limits and decompression bomb prevention

### Changed

- replace glob imports with explicit re-exports in validators prelude
- use dynamic crate path resolution for all dependencies
- replace glob import with explicit rayon trait imports

### Styling

- fix clippy warnings and formatting in files merged from main
- apply formatting to model_attribute.rs
- replace map_or(false, ...) with is_some_and in model_attribute.rs
- apply formatting to files introduced by merge from main
- apply rustfmt formatting to workspace files
- fix formatting in security module

## [0.1.0-alpha.5](https://github.com/kent8192/reinhardt-web/compare/reinhardt-core@v0.1.0-alpha.4...reinhardt-core@v0.1.0-alpha.5) - 2026-02-12

### Changed

- convert relative paths to absolute paths
- restore single-level super:: paths preserved by convention

## [0.1.0-alpha.4](https://github.com/kent8192/reinhardt-web/compare/reinhardt-core@v0.1.0-alpha.3...reinhardt-core@v0.1.0-alpha.4) - 2026-02-08

### Fixed

- *(core)* replace reinhardt-test with local poll_until helper

## [0.1.0-alpha.3](https://github.com/kent8192/reinhardt-web/compare/reinhardt-core@v0.1.0-alpha.2...reinhardt-core@v0.1.0-alpha.3) - 2026-02-03

### Other

- Merge pull request #111 from kent8192/fix/issue-81-bug-reinhardt-pages-wasm-build-fails-due-to-tokiomio-server-side-dependencies

## [0.1.0-alpha.2](https://github.com/kent8192/reinhardt-web/compare/reinhardt-core@v0.1.0-alpha.1...reinhardt-core@v0.1.0-alpha.2) - 2026-02-03

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

