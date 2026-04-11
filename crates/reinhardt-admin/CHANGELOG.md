# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

<!-- release-plz-separator -->
<!-- Entries below this line were created before release-plz adoption -->

## [0.1.0-rc.16](https://github.com/kent8192/reinhardt-web/compare/reinhardt-admin@v0.1.0-rc.15...reinhardt-admin@v0.1.0-rc.16) - 2026-04-11

### Added

- *(admin)* add login server function with JWT authentication
- *(admin)* add login page, auth gate, and 401 redirect for WASM SPA
- *(admin)* add UnoCSS runtime and Google Fonts CDN for admin panel styling
- *(admin)* add reinhardt-conf and tracing dependencies
- *(admin)* add AdminSettings type definitions with defaults
- *(admin)* expose settings module from lib.rs
- *(admin)* implement SettingsFragment for AdminSettings
- *(admin)* add CSP and security header validation warnings
- *(admin)* add from_str parsing for FrameOptions and ReferrerPolicy
- *(admin)* add SecurityHeaders conversion from AdminSettings
- *(admin)* route admin_spa_handler through AdminSettings
- *(admin)* wire admin_spa_handler to use configurable AdminSettings
- *(admin)* integrate UnoCSS generation into collectstatic
- *(admin)* add UnoCSS runtime as CDN-downloaded vendor asset
- add CDP-based E2E browser testing infrastructure and WASM SPA fixes
- *(di)* [**breaking**] unify #[inject] parameter type from Arc<T> to Depends<T>

### Changed

- *(admin)* [**breaking**] mark AdminRoute as non_exhaustive and reorder Login variant
- *(admin)* migrate page components to page! macro and Tailwind classes
- *(admin)* update workaround comments for page! @event closure capture limitation
- *(admin)* add issue refs to workaround comments and autocomplete to form inputs
- replace login form page! HTML with form! macro
- migrate login form to form! macro with server_fn
- remove CSRF workaround now that [[#3337](https://github.com/kent8192/reinhardt-web/issues/3337)](https://github.com/kent8192/reinhardt-web/issues/3337) is fixed
- *(admin)* [**breaking**] consolidate admin route builders into admin_routes_with_di
- *(admin)* [**breaking**] remove AdminRouter struct and deprecated AdminSite methods

### Documentation

- *(admin)* fix broken intra-doc link to CspMiddleware

### Fixed

- *(admin)* use path_params instead of full URI in static file handler
- *(admin)* call WASM init() in SPA HTML for web target output
- *(admin)* support HEAD requests for static file handler
- *(admin)* remove broken presetWind() global function call for UnoCSS v66+
- *(admin)* initialize UnoCSS runtime with v66+ API for preset-wind
- *(admin)* correct catch-all route count assertion for GET + HEAD
- *(admin)* replace CDN references with local vendor paths
- *(admin)* correct UnoCSS runtime CDN URL and font filename references
- *(admin)* rename from_str to parse_or_default to satisfy clippy should_implement_trait
- *(merge)* resolve conflicts with main adopting FromStr trait implementation
- *(di)* add Authentication variant to DiError for proper 401 responses
- *(admin)* make WASM JS reference test dynamic based on build state
- *(admin)* use explicit headers and string conversion for CSV/TSV export
- *(admin)* correct static route assertion to match catch-all pattern
- *(admin)* use write_record for CSV/TSV export to support map-based records
- *(admin)* handle UUID primary keys in create RETURNING clause
- *(admin)* ensure vendor assets are available during development
- *(auth)* add is_staff and is_superuser fields to JWT Claims
- *(admin)* embed staff status in JWT token during admin login
- *(pages)* migrate WASM HTTP client from gloo-net to reqwest
- *(pages)* replace gloo-net with reqwest and fix server_fn JSON deserialization
- *(pages)* inline @event closure capture to fix move semantics
- *(admin)* prefix page! event params with underscore to suppress non-WASM warnings
- remove vendor fonts from git tracking to resolve release-plz conflict
- resolve merge conflicts with main and fix CI failures
- resolve merge conflicts with main, migrate login to form! macro
- *(admin)* switch WASM SPA to mount() rendering with scheduler init
- *(admin)* make AdminSite registry lookups case-insensitive
- align E2E tests with upstream WASM SPA fixes from PR [[#3350](https://github.com/kent8192/reinhardt-web/issues/3350)](https://github.com/kent8192/reinhardt-web/issues/3350)/[[#3351](https://github.com/kent8192/reinhardt-web/issues/3351)](https://github.com/kent8192/reinhardt-web/issues/3351)/[[#3352](https://github.com/kent8192/reinhardt-web/issues/3352)](https://github.com/kent8192/reinhardt-web/issues/3352)
- gate WASM-only imports with #[cfg(client)] to suppress unused warnings
- *(admin)* suppress deprecated warnings in AdminRouter impl and fix formatting
- *(admin)* add missing SingletonScope import and fix formatting
- *(http)* use newtype wrappers for bool extension values to prevent TypeId collision
- update integration tests and docs for Depends<T> unification
- *(di)* register Injectable types in global registry for Depends resolution

### Other

- resolve conflict with main (deduplicate tests)
- resolve conflict with main in security tests
- Fix CSP blocking admin WASM SPA init script
- Change AuthState user_id from i64 to String for UUID support
- Auto-inject timestamps for auto_now/auto_now_add fields in admin CRUD
- Fix admin CRUD type coercion for timestamptz and uuid columns

### Styling

- *(admin)* add Open Props and Animate.css CDN, refactor style.css with design tokens
- *(admin)* add Animate.css entrance animations to page components
- *(admin)* apply rustfmt to test assertion in router module
- *(admin)* fix formatting in settings, security, and router
- apply rustfmt formatting fixes
- apply rustfmt formatting via cargo make auto-fix
- apply rustfmt formatting
- apply rustfmt to site.rs
- *(admin)* apply cargo make auto-fix formatting

## [0.1.0-rc.15](https://github.com/kent8192/reinhardt-web/compare/reinhardt-admin@v0.1.0-rc.14...reinhardt-admin@v0.1.0-rc.15) - 2026-03-29

### Changed

- *(admin)* migrate CurrentUser to AuthUser in server functions

### Fixed

- *(admin)* preserve query string in popstate navigation handler
- *(admin)* migrate remaining CurrentUser to AuthUser and update example
- *(admin)* replace CRLF before individual char replacement in TSV export
- *(admin)* update require_model_permission and callers to use AdminUser trait
- *(admin)* update test helpers to use AdminUser trait after merge
- *(admin)* update integration tests to match new admin API signatures
- *(di)* apply deferred DI registrations to existing singleton scope
- *(admin)* add serde helper for Vec<String> ORM deserialization
- *(admin)* accept any #[user] type for admin authentication via type-erased loader

### Other

- resolve conflict with main in delete.rs
- resolve conflicts with main in features.rs
- resolve conflict with main in delete.rs
- resolve conflict with main (AdminUser + admin_routes_with_di)
- resolve conflict with main in admin test module
- resolve conflicts with main branch

### Styling

- apply rustfmt formatting

## [0.1.0-rc.14](https://github.com/kent8192/reinhardt-web/compare/reinhardt-admin@v0.1.0-rc.13...reinhardt-admin@v0.1.0-rc.14) - 2026-03-24

### Added

- *(admin)* serve admin SPA HTML shell from admin_routes()

### Documentation

- *(crates)* update version references from 0.1.0-alpha.1 to 0.1.0-rc.13 across all READMEs

### Fixed

- *(reinhardt-admin)* register admin server functions in admin_routes()
- *(reinhardt-admin)* address Copilot review on router docs and test assertions
- *(admin)* register AdminSite in global DI with correct TypeId
- *(admin)* include admin SPA placeholder assets and fix static dir path
- *(admin)* apply CSP security headers to admin SPA HTML response
- *(admin)* add admin_static_routes() to serve embedded static assets

### Performance

- *(admin)* use zero-copy Bytes::from_static for embedded static assets

## [0.1.0-rc.12](https://github.com/kent8192/reinhardt-web/compare/reinhardt-admin@v0.1.0-rc.11...reinhardt-admin@v0.1.0-rc.12) - 2026-03-18

### Changed

- *(auth)* update re-exports and suppress deprecation warnings

## [0.1.0-rc.2](https://github.com/kent8192/reinhardt-web/compare/reinhardt-admin@v0.1.0-rc.1...reinhardt-admin@v0.1.0-rc.2) - 2026-03-04

### Fixed

- *(admin)* replace unwrap with error propagation in insert values call
- *(deps)* align dependency versions to workspace definitions

## [0.1.0-rc.1](https://github.com/kent8192/reinhardt-web/compare/reinhardt-admin@v0.1.0-alpha.17...reinhardt-admin@v0.1.0-rc.1) - 2026-02-24

### Maintenance

- updated the following local packages: reinhardt-db, reinhardt-db, reinhardt-auth, reinhardt-urls, reinhardt-pages

## [0.1.0-alpha.17](https://github.com/kent8192/reinhardt-web/compare/reinhardt-admin@v0.1.0-alpha.16...reinhardt-admin@v0.1.0-alpha.17) - 2026-02-24

### Maintenance

- updated the following local packages: reinhardt-pages

## [0.1.0-alpha.16](https://github.com/kent8192/reinhardt-web/compare/reinhardt-admin@v0.1.0-alpha.15...reinhardt-admin@v0.1.0-alpha.16) - 2026-02-23

### Maintenance

- *(license)* migrate from MIT/Apache-2.0 to BSD 3-Clause

## [0.1.0-alpha.15](https://github.com/kent8192/reinhardt-web/compare/reinhardt-admin@v0.1.0-alpha.14...reinhardt-admin@v0.1.0-alpha.15) - 2026-02-23

### Maintenance

- updated the following local packages: reinhardt-query, reinhardt-apps, reinhardt-db, reinhardt-db, reinhardt-auth, reinhardt-urls, reinhardt-pages

## [0.1.0-alpha.14](https://github.com/kent8192/reinhardt-web/compare/reinhardt-admin@v0.1.0-alpha.13...reinhardt-admin@v0.1.0-alpha.14) - 2026-02-21

### Fixed

- detect and report duplicate model registration
- sort columns for deterministic INSERT order
- apply clippy and fmt fixes to database module
- add panic prevention and error handling for admin operations
- pin native-tls to =0.2.14 to fix build failure
- add resource limits to prevent DoS in reinhardt-admin (#622, #623, #625, #626)
- fix raw SQL and info leakage in reinhardt-admin (#628, #630)
- add authentication and authorization enforcement to all endpoints
- use parameterized queries and escape identifiers to prevent SQL injection
- add input validation for mutation endpoints

### Security

- add audit logging for all CRUD operations
- add CSP headers, CSRF token generation, and XSS prevention
- add input validation, file size limits, and TOCTOU mitigations
- harden XSS, CSRF, auth, and proxy trust
- change default ModelAdmin permissions to deny
- use parameterized queries and escape LIKE patterns

### Changed

- clean up type naming, document intentional patterns

### Styling

- apply workspace-wide formatting fixes
- apply rustfmt to pre-existing formatting violations in 16 files
- apply code formatting to security fix files

### Testing

- add regression test for LIKE wildcard injection fix

### Maintenance

- fix contradictory unimplemented!() messages in export handler
- fix misleading table_name() default implementation doc

## [0.1.0-alpha.13](https://github.com/kent8192/reinhardt-web/compare/reinhardt-admin@v0.1.0-alpha.12...reinhardt-admin@v0.1.0-alpha.13) - 2026-02-16

### Maintenance

- updated the following local packages: reinhardt-db, reinhardt-db, reinhardt-auth, reinhardt-urls, reinhardt-pages

## [0.1.0-alpha.12](https://github.com/kent8192/reinhardt-web/compare/reinhardt-admin@v0.1.0-alpha.11...reinhardt-admin@v0.1.0-alpha.12) - 2026-02-15

### Maintenance

- updated the following local packages: reinhardt-query, reinhardt-apps, reinhardt-db, reinhardt-db, reinhardt-auth, reinhardt-urls, reinhardt-pages

## [0.1.0-alpha.11](https://github.com/kent8192/reinhardt-web/compare/reinhardt-admin@v0.1.0-alpha.10...reinhardt-admin@v0.1.0-alpha.11) - 2026-02-14

### Maintenance

- updated the following local packages: reinhardt-auth, reinhardt-urls, reinhardt-pages

## [0.1.0-alpha.10](https://github.com/kent8192/reinhardt-web/compare/reinhardt-admin@v0.1.0-alpha.9...reinhardt-admin@v0.1.0-alpha.10) - 2026-02-14

### Maintenance

- updated the following local packages: reinhardt-query, reinhardt-apps, reinhardt-db, reinhardt-db, reinhardt-auth, reinhardt-urls, reinhardt-pages

## [0.1.0-alpha.9](https://github.com/kent8192/reinhardt-web/compare/reinhardt-admin@v0.1.0-alpha.8...reinhardt-admin@v0.1.0-alpha.9) - 2026-02-14

### Maintenance

- updated the following local packages: reinhardt-query, reinhardt-apps, reinhardt-db, reinhardt-db, reinhardt-auth, reinhardt-urls, reinhardt-pages

## [0.1.0-alpha.7](https://github.com/kent8192/reinhardt-web/compare/reinhardt-admin@v0.1.0-alpha.6...reinhardt-admin@v0.1.0-alpha.7) - 2026-02-12

### Changed

- convert relative paths to absolute paths

## [0.1.0-alpha.6](https://github.com/kent8192/reinhardt-web/compare/reinhardt-admin@v0.1.0-alpha.5...reinhardt-admin@v0.1.0-alpha.6) - 2026-02-10

### Maintenance

- *(clippy)* add deny lints for todo/unimplemented/dbg_macro

## [0.1.0-alpha.5](https://github.com/kent8192/reinhardt-web/compare/reinhardt-admin@v0.1.0-alpha.4...reinhardt-admin@v0.1.0-alpha.5) - 2026-02-10

### Fixed

- *(admin)* move database tests to integration crate to break circular publish chain
- *(release)* revert unpublished crate versions to pre-release state

### Reverted

- undo PR [[#219](https://github.com/kent8192/reinhardt-web/issues/219)](https://github.com/kent8192/reinhardt-web/issues/219) version bumps for unpublished crates

## [0.1.0-alpha.4](https://github.com/kent8192/reinhardt-web/compare/reinhardt-admin@v0.1.0-alpha.3...reinhardt-admin@v0.1.0-alpha.4) - 2026-02-06

### Other

- updated the following local packages: reinhardt-utils, reinhardt-di, reinhardt-apps, reinhardt-db, reinhardt-db, reinhardt-auth, reinhardt-urls, reinhardt-pages

## [0.1.0-alpha.3](https://github.com/kent8192/reinhardt-web/compare/reinhardt-admin@v0.1.0-alpha.2...reinhardt-admin@v0.1.0-alpha.3) - 2026-02-03

### Other

- updated the following local packages: reinhardt-core, reinhardt-pages, reinhardt-http, reinhardt-utils, reinhardt-di, reinhardt-apps, reinhardt-db, reinhardt-db, reinhardt-auth, reinhardt-urls

## [0.1.0-alpha.2](https://github.com/kent8192/reinhardt-web/compare/reinhardt-admin@v0.1.0-alpha.1...reinhardt-admin@v0.1.0-alpha.2) - 2026-02-03

### Other

- add release-plz migration markers to CHANGELOGs
- *(changelog)* add missing 0.1.0-alpha.1 release entries
- *(package)* replace version.workspace with explicit versions

## [0.1.0-alpha.1] - 2026-01-23

### Added
- Initial release
- Admin panel functionality (via `reinhardt-panel`)
- CLI tool functionality (via `reinhardt-cli`)
