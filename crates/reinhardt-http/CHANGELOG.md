# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0-alpha.9](https://github.com/kent8192/reinhardt-web/compare/reinhardt-http@v0.1.0-alpha.8...reinhardt-http@v0.1.0-alpha.9) - 2026-02-23

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

