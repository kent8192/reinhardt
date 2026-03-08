# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0](https://github.com/kent8192/reinhardt-web/releases/tag/reinhardt-core@v0.1.0) - 2026-03-08

### Added

- add path sanitization and input validation helpers
- add resource limits configuration types
- add numeric safety utilities for checked arithmetic
- add redirect URL validation utilities
- add anchor link support to is_safe_url
- add enhanced sanitization utilities for XSS prevention

### Changed

- replace glob imports with explicit re-exports in validators prelude
- use dynamic crate path resolution for all dependencies
- replace glob import with explicit rayon trait imports
- convert relative paths to absolute paths
- restore single-level super:: paths preserved by convention

### Fixed

- *(macros)* dereference extractor before validation in pre_validate
- *(macros)* replace skeleton tests with meaningful assertions in pre_validate
- *(core)* add wasm32 platform gate to parallel and jsonschema validator modules
- *(deps)* align dependency versions to workspace definitions
- *(core)* use character count instead of byte length in CharField validation
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
- *(core)* replace reinhardt-test with local poll_until helper

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

### Styling

- fix clippy warnings and formatting in files merged from main
- apply formatting to model_attribute.rs
- replace map_or(false, ...) with is_some_and in model_attribute.rs
- apply formatting to files introduced by merge from main
- apply rustfmt formatting to workspace files
- fix formatting in security module

### Maintenance

- *(license)* migrate from MIT/Apache-2.0 to BSD 3-Clause
