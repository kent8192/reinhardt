# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0-rc.16](https://github.com/kent8192/reinhardt-web/compare/reinhardt-middleware@v0.1.0-rc.15...reinhardt-middleware@v0.1.0-rc.16) - 2026-04-13

### Added

- *(middleware)* add JwtAuthMiddleware for stateless token-based auth
- *(middleware)* add RemoteUserMiddleware and PersistentRemoteUserMiddleware
- *(middleware)* add LoginRequiredMiddleware for unauthenticated redirect
- *(middleware)* wire module declarations, re-exports, and feature flags
- *(session)* add AsyncSessionBackend trait for pluggable async session storage ([[#3369](https://github.com/kent8192/reinhardt-web/issues/3369)](https://github.com/kent8192/reinhardt-web/issues/3369))
- *(middleware)* add RedisSessionBackend behind session-redis feature
- *(middleware)* add CookieSessionAuthMiddleware for cookie-based session auth
- *(middleware)* add OriginGuardMiddleware for CSRF protection
- migrate UUID generation from v4 to v7 across entire codebase

### Changed

- *(middleware)* address Copilot review on [[#3413](https://github.com/kent8192/reinhardt-web/issues/3413)](https://github.com/kent8192/reinhardt-web/issues/3413)

### Documentation

- *(middleware)* fix intra-doc link errors for feature-gated types

### Fixed

- *(http)* convert errors to responses within middleware chain
- *(middleware)* ensure security headers on error responses from inner handlers
- *(middleware)* convert errors to responses in security-critical middleware
- *(middleware)* convert errors to responses in functional middleware
- *(middleware)* resolve docs.rs build and feature gate issues
- *(auth)* add is_staff and is_superuser fields to JWT Claims
- *(middleware)* extract is_staff from JWT claims instead of hardcoding false

### Security

- *(middleware)* reject empty user_id as unauthenticated
- keep UUID v4 for security-sensitive tokens

### Styling

- fix clippy warnings and format issues
- *(middleware)* apply cargo make auto-fix formatting

### Testing

- *(middleware)* add integration tests for error response headers

## [0.1.0-rc.15](https://github.com/kent8192/reinhardt-web/compare/reinhardt-middleware@v0.1.0-rc.14...reinhardt-middleware@v0.1.0-rc.15) - 2026-03-29

### Added

- *(middleware)* add exempt_paths to CspConfig for path-based CSP bypass

### Documentation

- fix stale doc comments in middleware, admin, apps, and core crates

### Fixed

- suppress deprecated User trait warnings in downstream crates

### Other

- resolve conflict with main (CSRF cookie tests)

## [0.1.0-rc.14](https://github.com/kent8192/reinhardt-web/compare/reinhardt-middleware@v0.1.0-rc.13...reinhardt-middleware@v0.1.0-rc.14) - 2026-03-24

### Fixed

- *(middleware)* inject session ID into request extensions before handler
- address copilot review feedback for session-id-inject
- address Copilot review comments on security documentation and validation
- *(middleware)* respect handler-set CSP headers in CspMiddleware

### Security

- harden header trust and authorization checks

## [0.1.0-rc.12](https://github.com/kent8192/reinhardt-web/compare/reinhardt-middleware@v0.1.0-rc.11...reinhardt-middleware@v0.1.0-rc.12) - 2026-03-18

### Security

- *(middleware)* add Vary header when wildcard origins combined with credentials

## [0.1.0-rc.9](https://github.com/kent8192/reinhardt-web/compare/reinhardt-middleware@v0.1.0-rc.8...reinhardt-middleware@v0.1.0-rc.9) - 2026-03-15

### Fixed

- *(middleware)* replace parse().unwrap() with safe alternatives for panic prevention
- *(middleware)* correct header handling on parse failures

## [0.1.0-rc.5](https://github.com/kent8192/reinhardt-web/compare/reinhardt-middleware@v0.1.0-rc.4...reinhardt-middleware@v0.1.0-rc.5) - 2026-03-07

### Fixed

- remove develop/0.2.0 content accidentally merged via PR [[#1918](https://github.com/kent8192/reinhardt-web/issues/1918)](https://github.com/kent8192/reinhardt-web/issues/1918)

## [0.1.0-rc.3](https://github.com/kent8192/reinhardt-web/compare/reinhardt-middleware@v0.1.0-rc.2...reinhardt-middleware@v0.1.0-rc.3) - 2026-03-05

### Fixed

- *(release)* use path-only dev-dep for reinhardt-test in cyclic crates

## [0.1.0-rc.2](https://github.com/kent8192/reinhardt-web/compare/reinhardt-middleware@v0.1.0-rc.1...reinhardt-middleware@v0.1.0-rc.2) - 2026-03-04

### Fixed

- *(middleware)* validate host header against allowed hosts in HTTPS redirect
- *(middleware)* add missing import in HttpsRedirectMiddleware doc test
- *(meta)* fix workspace inheritance and authors metadata

### Maintenance

- *(testing)* add insta snapshot testing dependency across all crates

### Other

- resolve conflict with main (criterion version)

## [0.1.0-rc.1](https://github.com/kent8192/reinhardt-web/compare/reinhardt-middleware@v0.1.0-alpha.15...reinhardt-middleware@v0.1.0-rc.1) - 2026-02-24

### Maintenance

- updated the following local packages: reinhardt-auth

## [0.1.0-alpha.15](https://github.com/kent8192/reinhardt-web/compare/reinhardt-middleware@v0.1.0-alpha.14...reinhardt-middleware@v0.1.0-alpha.15) - 2026-02-23

### Maintenance

- updated the following local packages: reinhardt-core, reinhardt-http, reinhardt-conf, reinhardt-di, reinhardt-auth, reinhardt-mail

## [0.1.0-alpha.14](https://github.com/kent8192/reinhardt-web/compare/reinhardt-middleware@v0.1.0-alpha.13...reinhardt-middleware@v0.1.0-alpha.14) - 2026-02-23

### Maintenance

- updated the following local packages: reinhardt-conf, reinhardt-auth, reinhardt-mail

## [0.1.0-alpha.13](https://github.com/kent8192/reinhardt-web/compare/reinhardt-middleware@v0.1.0-alpha.12...reinhardt-middleware@v0.1.0-alpha.13) - 2026-02-21

### Added

- add security middleware components (Refs #292)

### Fixed

- apply permission checks uniformly to all HTTP methods
- remove map_err on non-Result OpenApiRouter::wrap return value
- resolve clippy collapsible_if warnings after merge with main
- remove duplicate rand dependency entry
- resolve post-merge build errors from main integration

### Security

- harden session cookie and add X-Frame-Options header
- add lazy eviction for in-memory session store
- add stale bucket eviction to rate limit store cleanup
- add sliding window to circuit breaker statistics
- fix CSP header sanitization and CSRF panic
- harden XSS, CSRF, auth, and proxy trust
- validate CORS origin against request per Fetch Standard
- add trusted proxy validation for X-Forwarded-For
- replace regex XSS sanitization with proper escaping
- use cryptographic random for CSRF fallback secret
- replace predictable CSP nonce with cryptographic random

### Styling

- fix import order in security_middleware
- apply rustfmt after clippy auto-fix
- fix remaining clippy warnings across workspace
- apply rustfmt formatting to workspace files

## [0.1.0-alpha.12](https://github.com/kent8192/reinhardt-web/compare/reinhardt-middleware@v0.1.0-alpha.11...reinhardt-middleware@v0.1.0-alpha.12) - 2026-02-16

### Maintenance

- updated the following local packages: reinhardt-auth

## [0.1.0-alpha.11](https://github.com/kent8192/reinhardt-web/compare/reinhardt-middleware@v0.1.0-alpha.10...reinhardt-middleware@v0.1.0-alpha.11) - 2026-02-15

### Maintenance

- updated the following local packages: reinhardt-conf, reinhardt-auth, reinhardt-mail

## [0.1.0-alpha.10](https://github.com/kent8192/reinhardt-web/compare/reinhardt-middleware@v0.1.0-alpha.9...reinhardt-middleware@v0.1.0-alpha.10) - 2026-02-14

### Maintenance

- updated the following local packages: reinhardt-auth

## [0.1.0-alpha.9](https://github.com/kent8192/reinhardt-web/compare/reinhardt-middleware@v0.1.0-alpha.8...reinhardt-middleware@v0.1.0-alpha.9) - 2026-02-14

### Maintenance

- updated the following local packages: reinhardt-conf, reinhardt-auth, reinhardt-mail

## [0.1.0-alpha.8](https://github.com/kent8192/reinhardt-web/compare/reinhardt-middleware@v0.1.0-alpha.7...reinhardt-middleware@v0.1.0-alpha.8) - 2026-02-14

### Maintenance

- updated the following local packages: reinhardt-conf, reinhardt-auth, reinhardt-mail

## [0.1.0-alpha.7](https://github.com/kent8192/reinhardt-web/compare/reinhardt-middleware@v0.1.0-alpha.6...reinhardt-middleware@v0.1.0-alpha.7) - 2026-02-14

### Maintenance

- updated the following local packages: reinhardt-conf, reinhardt-di, reinhardt-auth, reinhardt-mail

## [0.1.0-alpha.6](https://github.com/kent8192/reinhardt-web/compare/reinhardt-middleware@v0.1.0-alpha.5...reinhardt-middleware@v0.1.0-alpha.6) - 2026-02-12

### Maintenance

- updated the following local packages: reinhardt-core, reinhardt-conf, reinhardt-auth, reinhardt-http, reinhardt-di, reinhardt-mail

## [0.1.0-alpha.5](https://github.com/kent8192/reinhardt-web/compare/reinhardt-middleware@v0.1.0-alpha.4...reinhardt-middleware@v0.1.0-alpha.5) - 2026-02-10

### Maintenance

- updated the following local packages: reinhardt-auth

## [0.1.0-alpha.4](https://github.com/kent8192/reinhardt-web/compare/reinhardt-middleware@v0.1.0-alpha.3...reinhardt-middleware@v0.1.0-alpha.4) - 2026-02-06

### Other

- updated the following local packages: reinhardt-di, reinhardt-conf, reinhardt-auth, reinhardt-mail

## [0.1.0-alpha.3](https://github.com/kent8192/reinhardt-web/compare/reinhardt-middleware@v0.1.0-alpha.2...reinhardt-middleware@v0.1.0-alpha.3) - 2026-02-03

### Other

- updated the following local packages: reinhardt-core, reinhardt-http, reinhardt-conf, reinhardt-di, reinhardt-auth, reinhardt-mail

## [0.1.0-alpha.2](https://github.com/kent8192/reinhardt-web/compare/reinhardt-middleware@v0.1.0-alpha.1...reinhardt-middleware@v0.1.0-alpha.2) - 2026-02-03

### Fixed

- *(ci)* remove proptest regression files from git tracking

### Other

- add release-plz migration markers to CHANGELOGs
- *(changelog)* remove obsolete [0.1.0] sections
- *(changelog)* add missing 0.1.0-alpha.1 release entries
- *(package)* replace version.workspace with explicit versions

### Breaking Changes
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

