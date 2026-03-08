# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0](https://github.com/kent8192/reinhardt-web/releases/tag/reinhardt-auth@v0.1.0) - 2026-03-08

### Changed

- convert relative paths to absolute paths
- restore single-level super:: paths preserved by convention

### Fixed

- *(auth)* remove invalid sync poison recovery test for tokio RwLock
- *(auth)* remove async poison recovery test for tokio RwLock
- *(auth)* move HMAC validation to config init and improve test coverage
- remove develop/0.2.0 content accidentally merged via PR [[#1918](https://github.com/kent8192/reinhardt-web/issues/1918)](https://github.com/kent8192/reinhardt-web/issues/1918)
- forward redis-backend and middleware features to sub-crates
- *(auth)* validate client_id matches authorization code in OAuth2 exchange
- *(meta)* fix workspace inheritance and authors metadata
- *(test)* update rand 0.9 API usage in auth integration tests
- use logging framework instead of eprintln in authentication
- replace std Mutex with tokio Mutex to prevent async deadlocks
- replace unwrap with safe error handling in JWT claim extraction
- add authentication and authorization enforcement to all endpoints
- add path traversal prevention with input validation
- *(auth)* remove unused reinhardt-test dev-dependency

### Security

- use server secret as HMAC key material in session auth hash
- harden XSS, CSRF, auth, and proxy trust
- fix TOTP algorithm, proxy trust, and session cookies
- implement constant-time comparison and argon2 password hashing

### Documentation

- add security note on client-side auth state limitations

### Styling

- *(auth)* fix trailing newline in token_storage tests
- apply rustfmt to pre-existing unformatted files
- apply formatting to files introduced by merge from main
- apply rustfmt formatting to workspace files

### Maintenance

- *(license)* migrate from MIT/Apache-2.0 to BSD 3-Clause
- add SAFETY comment to unsafe block in hasher_boundary_value

### Other

- resolve conflict with main (criterion version)
