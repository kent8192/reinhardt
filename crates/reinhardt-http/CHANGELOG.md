# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0](https://github.com/kent8192/reinhardt-web/releases/tag/reinhardt-http@v0.1.0) - 2026-03-08

### Added

- `extract_bearer_token()` - Extract Bearer token from Authorization header
- `get_header()` - Get specific header value
- `get_client_ip()` - Get client IP from X-Forwarded-For/X-Real-IP/remote_addr
- `validate_content_type()` - Validate Content-Type header
- `query_as<T>()` - Type-safe query parameter deserialization

### Fixed

- *(http)* use char_indices for UTF-8 safe truncation in truncate_for_log
- *(meta)* fix workspace inheritance and authors metadata
- add session timeout for chunked uploads
- fix streaming parser, cookie parsing, and request builder
- recover from poisoned mutex instead of panicking
- prevent panics from lock poisoning, query parsing, and input validation
- add path traversal prevention with input validation
- *(http)* move integration tests to tests crate to break circular publish chain

### Security

- use cryptographically random filenames for uploads
- add safe error response builder to prevent info leakage
- harden XSS, CSRF, auth, and proxy trust
- prevent path traversal in file upload handling

### Documentation

- add missing doc comments for public API modules and types
- add security note on client-side auth state limitations

### Styling

- fix pre-existing clippy warnings and apply rustfmt
- apply rustfmt to pre-existing unformatted files
- collapse nested if statements per clippy::collapsible_if
- apply code formatting to security fix files

### Maintenance

- *(testing)* add insta snapshot testing dependency across all crates
- *(license)* migrate from MIT/Apache-2.0 to BSD 3-Clause

### Notes

- Methods migrated from reinhardt-micro crate for better API ergonomics
