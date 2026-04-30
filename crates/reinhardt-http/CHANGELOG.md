# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0-rc.24](https://github.com/kent8192/reinhardt-web/compare/reinhardt-http@v0.1.0-rc.23...reinhardt-http@v0.1.0-rc.24) - 2026-04-30

### Changed

- *(di,urls,http)* preserve path parameter insertion order through pipeline

## [0.1.0-rc.21](https://github.com/kent8192/reinhardt-web/compare/reinhardt-http@v0.1.0-rc.20...reinhardt-http@v0.1.0-rc.21) - 2026-04-23

### Documentation

- add reinhardt-version-sync markers to all crate READMEs

## [0.1.0-rc.20](https://github.com/kent8192/reinhardt-web/compare/reinhardt-http@v0.1.0-rc.19...reinhardt-http@v0.1.0-rc.20) - 2026-04-23

### Documentation

- *(http)* fix type name and API inaccuracies across HTTP crate READMEs

## [0.1.0-rc.16](https://github.com/kent8192/reinhardt-web/compare/reinhardt-http@v0.1.0-rc.15...reinhardt-http@v0.1.0-rc.16) - 2026-04-20

### Changed

- *(http)* remove dead should_stop_chain check in CCH

### Documentation

- *(http)* update ResponseCookies docs to reflect shared Extensions mechanism
- *(http)* address Copilot review on [[#3417](https://github.com/kent8192/reinhardt-web/issues/3417)](https://github.com/kent8192/reinhardt-web/issues/3417)

### Fixed

- *(http)* convert errors to responses within middleware chain
- *(middleware)* convert errors to responses in cross-crate middleware

## [0.1.0-rc.15](https://github.com/kent8192/reinhardt-web/compare/reinhardt-http@v0.1.0-rc.14...reinhardt-http@v0.1.0-rc.15) - 2026-03-29

### Added

- *(http)* add append_header for multi-value headers like Set-Cookie

### Fixed

- *(admin)* validate CSRF token against cookie and fix auth order in create

## [0.1.0-rc.14](https://github.com/kent8192/reinhardt-web/compare/reinhardt-http@v0.1.0-rc.13...reinhardt-http@v0.1.0-rc.14) - 2026-03-24

### Added

- *(http)* add with_header_if_absent and try_with_header_if_absent to Response
- *(http)* add ExcludeMiddleware for declarative route exclusion

### Changed

- *(urls)* address Copilot review feedback

### Documentation

- *(crates)* update version references from 0.1.0-alpha.1 to 0.1.0-rc.13 across all READMEs

### Fixed

- address Copilot review comments on security documentation and validation
- resolve CI failures and remove sea-query dependency

### Security

- harden header trust and authorization checks

## [0.1.0-rc.13](https://github.com/kent8192/reinhardt-web/compare/reinhardt-http@v0.1.0-rc.12...reinhardt-http@v0.1.0-rc.13) - 2026-03-18

### Fixed

- *(di)* set HTTP request on per-request InjectionContext in use_inject macro

## [0.1.0-rc.12](https://github.com/kent8192/reinhardt-web/compare/reinhardt-http@v0.1.0-rc.11...reinhardt-http@v0.1.0-rc.12) - 2026-03-18

### Fixed

- *(http)* make AuthState::from_extensions() find AuthState object directly
- *(http)* add Error::Http and Error::Serialization to safe client error detail

## [0.1.0-rc.9](https://github.com/kent8192/reinhardt-web/compare/reinhardt-http@v0.1.0-rc.8...reinhardt-http@v0.1.0-rc.9) - 2026-03-15

### Fixed

- *(http)* replace lock().unwrap() with poison-recovery pattern

## [0.1.0-rc.5](https://github.com/kent8192/reinhardt-web/compare/reinhardt-http@v0.1.0-rc.4...reinhardt-http@v0.1.0-rc.5) - 2026-03-07

### Documentation

- add missing doc comments for public API modules and types

## [0.1.0-rc.2](https://github.com/kent8192/reinhardt-web/compare/reinhardt-http@v0.1.0-rc.1...reinhardt-http@v0.1.0-rc.2) - 2026-03-04

### Fixed

- *(http)* use char_indices for UTF-8 safe truncation in truncate_for_log
- *(meta)* fix workspace inheritance and authors metadata

### Maintenance

- *(testing)* add insta snapshot testing dependency across all crates

## [0.1.0-rc.1](https://github.com/kent8192/reinhardt-web/compare/reinhardt-http@v0.1.0-alpha.8...reinhardt-http@v0.1.0-rc.1) - 2026-02-23

### Maintenance

- *(license)* migrate from MIT/Apache-2.0 to BSD 3-Clause

## [0.1.0-alpha.8](https://github.com/kent8192/reinhardt-web/compare/reinhardt-http@v0.1.0-alpha.7...reinhardt-http@v0.1.0-alpha.8) - 2026-02-21

### Fixed

- add session timeout for chunked uploads
- fix streaming parser, cookie parsing, and request builder
- recover from poisoned mutex instead of panicking
- prevent panics from lock poisoning, query parsing, and input validation
- add path traversal prevention with input validation

### Security

- use cryptographically random filenames for uploads
- add safe error response builder to prevent info leakage
- harden XSS, CSRF, auth, and proxy trust
- prevent path traversal in file upload handling

### Styling

- fix pre-existing clippy warnings and apply rustfmt
- apply rustfmt to pre-existing unformatted files
- collapse nested if statements per clippy::collapsible_if
- apply code formatting to security fix files

### Documentation

- add security note on client-side auth state limitations

## [0.1.0-alpha.7](https://github.com/kent8192/reinhardt-web/compare/reinhardt-http@v0.1.0-alpha.6...reinhardt-http@v0.1.0-alpha.7) - 2026-02-12

### Maintenance

- updated the following local packages: reinhardt-core

## [0.1.0-alpha.6](https://github.com/kent8192/reinhardt-web/compare/reinhardt-http@v0.1.0-alpha.5...reinhardt-http@v0.1.0-alpha.6) - 2026-02-08

### Fixed

- *(http)* move integration tests to tests crate to break circular publish chain

## [0.1.0-alpha.5](https://github.com/kent8192/reinhardt-web/compare/reinhardt-http@v0.1.0-alpha.4...reinhardt-http@v0.1.0-alpha.5) - 2026-02-03

### Other

- updated the following local packages: reinhardt-core

## [0.1.0-alpha.4](https://github.com/kent8192/reinhardt-web/compare/reinhardt-http@v0.1.0-alpha.3...reinhardt-http@v0.1.0-alpha.4) - 2026-02-03

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

### Added

- `extract_bearer_token()` - Extract Bearer token from Authorization header
- `get_header()` - Get specific header value
- `get_client_ip()` - Get client IP from X-Forwarded-For/X-Real-IP/remote_addr
- `validate_content_type()` - Validate Content-Type header
- `query_as<T>()` - Type-safe query parameter deserialization

### Notes

- Methods migrated from reinhardt-micro crate for better API ergonomics

## [0.1.0-alpha.1] - 2026-01-23

### Added

- Initial crates.io release

