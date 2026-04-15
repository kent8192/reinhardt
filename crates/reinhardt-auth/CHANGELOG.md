# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0-rc.16](https://github.com/kent8192/reinhardt-web/compare/reinhardt-auth@v0.1.0-rc.15...reinhardt-auth@v0.1.0-rc.16) - 2026-04-15

### Added

- *(auth)* add SuperuserInit trait and SuperuserCreator registry
- *(auth)* auto-register SuperuserCreator via inventory for #[user(full = true)] + #[model] types
- *(auth)* add Guard<P>, Public, All, Any, Not runtime types
- *(auth)* add guard!() proc macro with winnow parser
- migrate UUID generation from v4 to v7 across entire codebase

### Documentation

- *(auth)* add deprecation notice to standalone createsuperuser binary

### Fixed

- *(docs)* resolve broken intra-doc links and incorrect test assertion
- *(auth)* resolve clippy needless_borrow in Guard Injectable
- *(middleware)* convert errors to responses in security-critical middleware
- *(auth)* use DiError::Authentication for unauthenticated user errors
- *(auth)* add is_staff and is_superuser fields to JWT Claims

### Maintenance

- upgrade workspace dependencies to latest versions

### Other

- resolve conflict with main in createsuperuser error message

### Security

- keep UUID v4 for security-sensitive tokens

## [0.1.0-rc.15](https://github.com/kent8192/reinhardt-web/compare/reinhardt-auth@v0.1.0-rc.14...reinhardt-auth@v0.1.0-rc.15) - 2026-03-29

### Added

- *(auth)* add AuthIdentity trait as User trait replacement
- *(auth)* add AuthPermission database model
- *(auth)* extend Group struct with #[model] support
- *(auth)* integrate GroupManager with PermissionsMixin

### Deprecated

- *(auth)* mark User trait and DefaultUser as deprecated

### Fixed

- fix!(auth): add JwtError enum and reject expired tokens by default
- *(macros)* apply formatting and clippy fixes to #[user] macro
- *(auth)* reword deprecated note to fix rustdoc intra-doc link error
- *(auth)* require jwt/token feature for DatabaseTokenStorage re-export
- *(auth)* use get_singleton instead of resolve for DatabaseConnection in DI

### Styling

- apply rustfmt to user_macro_integration tests

### Testing

- *(auth)* add comprehensive edge case and custom field tests for #[user] macro
- *(auth)* add GroupManager integration and user macro tests

## [0.1.0-rc.14](https://github.com/kent8192/reinhardt-web/compare/reinhardt-auth@v0.1.0-rc.13...reinhardt-auth@v0.1.0-rc.14) - 2026-03-24

### Fixed

- *(auth)* invalidate old session on login to prevent session fixation
- address copilot review feedback for session fixation
- address Copilot review comments on security documentation and validation

### Security

- harden header trust and authorization checks

## [0.1.0-rc.12](https://github.com/kent8192/reinhardt-web/compare/reinhardt-auth@v0.1.0-rc.11...reinhardt-auth@v0.1.0-rc.12) - 2026-03-18

### Added

- *(auth)* add AuthInfo lightweight auth extractor
- *(auth)* add AuthUser<U> extractor with tuple struct destructuring
- *(auth)* add validate_auth_extractors startup DI validation

### Changed

- *(auth)* update re-exports and suppress deprecation warnings

### Documentation

- *(auth)* use backticks instead of intra-doc links for cross-crate types

### Fixed

- *(auth)* add warning log when DatabaseConnection is missing for CurrentUser injection
- *(auth)* remove Uuid::nil() fallback on user_id parse failure

## [0.1.0-rc.9](https://github.com/kent8192/reinhardt-web/compare/reinhardt-auth@v0.1.0-rc.8...reinhardt-auth@v0.1.0-rc.9) - 2026-03-15

### Documentation

- *(auth)* fix private intra-doc link in get_user_info

### Fixed

- *(auth)* harden session cookie, token comparison, and permission checks

### Performance

- *(auth)* use SHA-256 digest index for O(1) token lookup

### Security

- *(auth)* enforce HTTPS for OAuth2/OIDC endpoint URLs
- *(auth)* sanitize URL error messages and improve loopback detection

### Styling

- fix formatting in url_validation.rs

### Testing

- *(auth)* add tests for permission and composite auth error handling

## [0.1.0-rc.7](https://github.com/kent8192/reinhardt-web/compare/reinhardt-auth@v0.1.0-rc.6...reinhardt-auth@v0.1.0-rc.7) - 2026-03-11

### Fixed

- *(auth)* add missing #[rstest] annotations to 57 existing tests

### Styling

- apply format fixes

### Testing

- *(auth)* add CacheSessionBackend direct CRUD tests
- *(auth)* add JwtSessionBackend extended edge case tests
- *(auth)* add LoginHandler/LogoutHandler edge case tests
- *(auth)* add SocialAccountStorage extended coverage
- *(auth)* add UserMapper extended coverage
- *(auth)* add session replication gap tests
- *(auth)* add session migration functional tests
- *(auth)* add tenant isolation gap tests
- *(auth)* add session rotation gap tests
- *(auth)* add session cleanup functional tests
- *(auth)* add permissions edge case tests (object, IP, time-based)
- *(auth)* add serialization empty data roundtrip tests
- *(auth)* add repository trait unit tests

## [0.1.0-rc.5](https://github.com/kent8192/reinhardt-web/compare/reinhardt-auth@v0.1.0-rc.4...reinhardt-auth@v0.1.0-rc.5) - 2026-03-07

### Fixed

- *(auth)* remove invalid sync poison recovery test for tokio RwLock
- *(auth)* remove async poison recovery test for tokio RwLock
- *(auth)* move HMAC validation to config init and improve test coverage
- remove develop/0.2.0 content accidentally merged via PR [[#1918](https://github.com/kent8192/reinhardt-web/issues/1918)](https://github.com/kent8192/reinhardt-web/issues/1918)

### Styling

- *(auth)* fix trailing newline in token_storage tests

## [0.1.0-rc.4](https://github.com/kent8192/reinhardt-web/compare/reinhardt-auth@v0.1.0-rc.3...reinhardt-auth@v0.1.0-rc.4) - 2026-03-05

### Fixed

- forward redis-backend and middleware features to sub-crates

## [0.1.0-rc.2](https://github.com/kent8192/reinhardt-web/compare/reinhardt-auth@v0.1.0-rc.1...reinhardt-auth@v0.1.0-rc.2) - 2026-03-04

### Fixed

- *(auth)* validate client_id matches authorization code in OAuth2 exchange
- *(meta)* fix workspace inheritance and authors metadata
- *(test)* update rand 0.9 API usage in auth integration tests

### Other

- resolve conflict with main (criterion version)

## [0.1.0-rc.1](https://github.com/kent8192/reinhardt-web/compare/reinhardt-auth@v0.1.0-alpha.15...reinhardt-auth@v0.1.0-rc.1) - 2026-02-24

### Maintenance

- updated the following local packages: reinhardt-db, reinhardt-db

## [0.1.0-alpha.15](https://github.com/kent8192/reinhardt-web/compare/reinhardt-auth@v0.1.0-alpha.14...reinhardt-auth@v0.1.0-alpha.15) - 2026-02-23

### Maintenance

- *(license)* migrate from MIT/Apache-2.0 to BSD 3-Clause

## [0.1.0-alpha.14](https://github.com/kent8192/reinhardt-web/compare/reinhardt-auth@v0.1.0-alpha.13...reinhardt-auth@v0.1.0-alpha.14) - 2026-02-23

### Maintenance

- updated the following local packages: reinhardt-query, reinhardt-query, reinhardt-apps, reinhardt-db, reinhardt-db

## [0.1.0-alpha.13](https://github.com/kent8192/reinhardt-web/compare/reinhardt-auth@v0.1.0-alpha.12...reinhardt-auth@v0.1.0-alpha.13) - 2026-02-21

### Fixed

- use logging framework instead of eprintln in authentication
- replace std Mutex with tokio Mutex to prevent async deadlocks
- replace unwrap with safe error handling in JWT claim extraction
- add authentication and authorization enforcement to all endpoints
- add path traversal prevention with input validation

### Security

- use server secret as HMAC key material in session auth hash
- harden XSS, CSRF, auth, and proxy trust
- fix TOTP algorithm, proxy trust, and session cookies
- implement constant-time comparison and argon2 password hashing

### Styling

- apply rustfmt to pre-existing unformatted files
- apply formatting to files introduced by merge from main
- apply rustfmt formatting to workspace files

### Documentation

- add security note on client-side auth state limitations

### Maintenance

- add SAFETY comment to unsafe block in hasher_boundary_value

## [0.1.0-alpha.12](https://github.com/kent8192/reinhardt-web/compare/reinhardt-auth@v0.1.0-alpha.11...reinhardt-auth@v0.1.0-alpha.12) - 2026-02-16

### Maintenance

- updated the following local packages: reinhardt-db, reinhardt-db

## [0.1.0-alpha.11](https://github.com/kent8192/reinhardt-web/compare/reinhardt-auth@v0.1.0-alpha.10...reinhardt-auth@v0.1.0-alpha.11) - 2026-02-15

### Maintenance

- updated the following local packages: reinhardt-query, reinhardt-query, reinhardt-apps, reinhardt-db, reinhardt-db

## [0.1.0-alpha.9](https://github.com/kent8192/reinhardt-web/compare/reinhardt-auth@v0.1.0-alpha.8...reinhardt-auth@v0.1.0-alpha.9) - 2026-02-14

### Maintenance

- updated the following local packages: reinhardt-query, reinhardt-query, reinhardt-apps, reinhardt-db, reinhardt-db

## [0.1.0-alpha.8](https://github.com/kent8192/reinhardt-web/compare/reinhardt-auth@v0.1.0-alpha.7...reinhardt-auth@v0.1.0-alpha.8) - 2026-02-14

### Maintenance

- updated the following local packages: reinhardt-query, reinhardt-query, reinhardt-apps, reinhardt-db, reinhardt-db

## [0.1.0-alpha.6](https://github.com/kent8192/reinhardt-web/compare/reinhardt-auth@v0.1.0-alpha.5...reinhardt-auth@v0.1.0-alpha.6) - 2026-02-12

### Changed

- convert relative paths to absolute paths
- restore single-level super:: paths preserved by convention

## [0.1.0-alpha.5](https://github.com/kent8192/reinhardt-web/compare/reinhardt-auth@v0.1.0-alpha.4...reinhardt-auth@v0.1.0-alpha.5) - 2026-02-10

### Fixed

- *(auth)* remove unused reinhardt-test dev-dependency
- *(release)* revert unpublished crate versions to pre-release state

### Reverted

- undo PR [[#219](https://github.com/kent8192/reinhardt-web/issues/219)](https://github.com/kent8192/reinhardt-web/issues/219) version bumps for unpublished crates

## [0.1.0-alpha.4](https://github.com/kent8192/reinhardt-web/compare/reinhardt-auth@v0.1.0-alpha.3...reinhardt-auth@v0.1.0-alpha.4) - 2026-02-06

### Other

- updated the following local packages: reinhardt-utils, reinhardt-di, reinhardt-apps, reinhardt-db, reinhardt-db

## [0.1.0-alpha.3](https://github.com/kent8192/reinhardt-web/compare/reinhardt-auth@v0.1.0-alpha.2...reinhardt-auth@v0.1.0-alpha.3) - 2026-02-03

### Other

- updated the following local packages: reinhardt-core, reinhardt-http, reinhardt-utils, reinhardt-di, reinhardt-apps, reinhardt-db, reinhardt-db

## [0.1.0-alpha.2](https://github.com/kent8192/reinhardt-web/compare/reinhardt-auth@v0.1.0-alpha.1...reinhardt-auth@v0.1.0-alpha.2) - 2026-02-03

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

