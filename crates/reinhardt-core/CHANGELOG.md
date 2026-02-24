# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0-alpha.8](https://github.com/kent8192/reinhardt-web/compare/reinhardt-core@v0.1.0-alpha.7...reinhardt-core@v0.1.0-alpha.8) - 2026-02-23

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

