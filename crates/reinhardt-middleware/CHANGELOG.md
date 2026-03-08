# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0](https://github.com/kent8192/reinhardt-web/releases/tag/reinhardt-middleware@v0.1.0) - 2026-03-08

### Added

- add security middleware components (Refs #292)

### Fixed

- remove develop/0.2.0 content accidentally merged via PR [[#1918](https://github.com/kent8192/reinhardt-web/issues/1918)](https://github.com/kent8192/reinhardt-web/issues/1918)
- *(release)* use path-only dev-dep for reinhardt-test in cyclic crates
- *(middleware)* validate host header against allowed hosts in HTTPS redirect
- *(middleware)* add missing import in HttpsRedirectMiddleware doc test
- *(meta)* fix workspace inheritance and authors metadata
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

### Maintenance

- *(testing)* add insta snapshot testing dependency across all crates

### Other

- resolve conflict with main (criterion version)
