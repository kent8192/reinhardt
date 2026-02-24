# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0-alpha.18](https://github.com/kent8192/reinhardt-web/compare/reinhardt-web@v0.1.0-alpha.17...reinhardt-web@v0.1.0-alpha.18) - 2026-02-24

### Added

- *(website)* add favicon generated from logo
- *(website)* add WASM frontend and multiplatform cards to Why Reinhardt section
- *(website)* add 6 additional feature cards to Why Reinhardt section
- *(website)* expand color palette and add web font variables
- *(website)* replace Zola inline syntax highlighting with highlight.js
- *(website)* add sidebar navigation to standalone pages

### Changed

- *(website)* reorder header nav and implement unified weight-based sidebar
- *(website)* move onboarding content into quickstart section
- *(website)* switch docs to weight-based ordering for reference material

### Documentation

- *(website)* add sidebar_weight to tutorial pages
- *(website)* add tutorials index page with card-based navigation
- *(website)* audit and fix errors across docs pages

### Fixed

- *(website)* prevent visited link color from overriding button text
- correct repository URLs from reinhardt-rs to reinhardt-web
- *(website)* add security headers, SRI, FOUC prevention, accessibility, and optimize assets
- *(website)* fix content links, fabricated APIs, and import paths
- *(ci)* add branch flag and preview cleanup to deploy-website workflow
- *(website)* replace cargo run commands with cargo make task equivalents
- *(website)* unify sidebar navigation across quickstart and docs sections
- *(website)* add package = "reinhardt-web" and update version to 0.1.0-alpha.18 in all examples
- *(website)* correct docs.rs links to reinhardt-web crate
- *(website)* update getting-started examples to use decorator and viewset patterns
- *(website)* correct API patterns in serialization and rest quickstart tutorials
- *(website)* restructure viewsets tutorial to use urls.rs pattern
- *(website)* simplify server_fn definitions and use server_fn pattern in form macros
- *(website)* add deepwiki reference to site configuration
- *(website)* center standalone pages like changelog and security
- *(website)* adjust logo size and spacing in navbar and hero section
- *(website)* replace fn main() patterns with cargo make runserver convention across docs
- *(website)* use root-relative paths instead of absolute permalinks in sidebar links

### Maintenance

- *(website)* update license references from MIT/Apache-2.0 to BSD-3-Clause
- *(website)* add Cloudflare Pages deployment workflow
- add terraform patterns to gitignore
- *(infra)* add terraform configuration for cloudflare pages and github secrets
- *(infra)* rename terraform template to conventional .example.tfvars format

### Styling

- *(website)* redesign visual components with modern aesthetics

## [0.1.0-alpha.16](https://github.com/kent8192/reinhardt-web/compare/reinhardt-web@v0.1.0-alpha.15...reinhardt-web@v0.1.0-alpha.16) - 2026-02-21

### Fixed

- add panic prevention and error handling for admin operations

### Documentation

- remove non-existent feature flags from lib.rs documentation

### Maintenance

- add explanatory comments to undocumented #[allow(...)] attributes

## [0.1.0-alpha.15](https://github.com/kent8192/reinhardt-web/compare/reinhardt-web@v0.1.0-alpha.14...reinhardt-web@v0.1.0-alpha.15) - 2026-02-16

### Added

- *(examples)* add settings files for all examples
- *(examples)* add ci.toml and auto-detect CI environment
- *(examples)* add docker-compose.yml for PostgreSQL examples
- *(examples)* add docker-up dependency to runserver for PostgreSQL examples

### Changed

- *(examples)* remove reinhardt-examples references and adopt monorepo-only strategy
- *(examples)* remove stale staging/production settings templates
- *(examples)* simplify settings.rs to consistent pattern
- *(examples)* move docker-compose.yml into each PostgreSQL example

### Documentation

- *(examples)* add quick start instructions to README.md

### Fixed

- *(examples)* update Docker build context and COPY paths for flattened structure
- *(gitignore)* update stale examples/local path to flattened structure
- *(ci)* remove stale example package overrides from release-plz.toml
- *(ci)* remove stale test-common-crates job from test-examples.yml
- *(examples)* restore required default settings values for Settings deserialization

### Maintenance

- *(examples)* remove stale examples/local settings files
- *(examples)* remove stale configuration files from old repository
- *(examples)* remove unused example-common and example-test-macros crates
- *(examples)* update stale remote-examples-test task in Makefile.toml
- *(examples)* remove stale help tasks from all example Makefile.toml
- *(examples)* remove stale availability test referencing deleted example_common crate
- *(examples)* remove stale settings from example base.toml files

### Other

- Squashed 'examples/' changes from 3a2c7662..77534e4c

### Styling

- format twitter example common component

## [0.1.0-alpha.14](https://github.com/kent8192/reinhardt-web/compare/reinhardt-web@v0.1.0-alpha.13...reinhardt-web@v0.1.0-alpha.14) - 2026-02-15

### Fixed

- resolve Test Examples CI failures

### Maintenance

- add setup-protoc step to test-examples workflow
- remove pull_request trigger from test-examples.yml

## [0.1.0-alpha.12](https://github.com/kent8192/reinhardt-web/compare/reinhardt-web@v0.1.0-alpha.11...reinhardt-web@v0.1.0-alpha.12) - 2026-02-14

### Maintenance

- add copilot setup steps workflow

## [0.1.0-alpha.11](https://github.com/kent8192/reinhardt-web/compare/reinhardt-web@v0.1.0-alpha.10...reinhardt-web@v0.1.0-alpha.11) - 2026-02-14

### Changed

- *(query)* replace super::super:: with crate:: absolute paths in query submodules
- *(query)* replace super::super:: with crate:: absolute paths in dcl tests
- *(db)* replace super::super:: with crate:: absolute paths in migrations
- *(rest)* remove unused sea-orm dependency
- *(query)* remove unused backend imports in drop role and drop user tests
- *(db)* fix unused variable assignments in migration operation tests
- *(query)* move DML integration tests to integration test crate

### Fixed

- *(query)* add missing DropBehavior import in revoke statement tests
- *(query)* add Table variant special handling in Iden derive macro
- *(query)* add missing code fence markers in alter_type doc example
- *(ci)* migrate publish check to cargo publish --workspace
- *(query)* add explicit path attributes to DML test module declarations
- *(query)* add Meta::List support to Iden derive macro attribute parsing
- *(query)* read iden attribute from struct-level instead of first field
- *(db)* bind insert values in many-to-many manager instead of discarding
- *(query)* reject whitespace-only names in CreateUser and GrantRole validation
- *(commands)* remove unused reinhardt-i18n dev-dependency
- *(release)* roll back unpublished crate versions after partial release failure

### Maintenance

- increase test partition counts for faster CI execution

### Styling

- *(query)* format Iden derive macro code

## [0.1.0-alpha.10](https://github.com/kent8192/reinhardt-web/compare/reinhardt-web@v0.1.0-alpha.9...reinhardt-web@v0.1.0-alpha.10) - 2026-02-12

### Changed

- convert relative paths to absolute paths
- *(db)* convert relative paths to absolute paths in orm execution
- restore single-level super:: paths preserved by convention

### Fixed

- correct incorrect path conversions in test imports
- *(release)* roll back unpublished crate versions and enable release_always

### Maintenance

- *(todo-check)* add clippy todo lint job to TODO Check workflow

### Reverted

- undo unintended visibility and formatting changes

## [0.1.0-alpha.9](https://github.com/kent8192/reinhardt-web/compare/reinhardt-web@v0.1.0-alpha.8...reinhardt-web@v0.1.0-alpha.9) - 2026-02-11

### Fixed

- *(dentdelion)* correct doctest import path to use prelude module

## [0.1.0-alpha.8](https://github.com/kent8192/reinhardt-web/compare/reinhardt-web@v0.1.0-alpha.7...reinhardt-web@v0.1.0-alpha.8) - 2026-02-10

### Documentation

- update TODO policy with CI enforcement
- rewrite CLAUDE.md TODO check sections in English

### Maintenance

- *(todo-check)* add semgrep rules for TODO/FIXME comment detection
- *(todo-check)* add reusable workflow for unresolved TODO scanning
- integrate TODO check into CI pipeline
- *(todo-check)* switch from semgrep scan to semgrep ci
- *(clippy)* add deny lints for todo/unimplemented/dbg_macro
- *(todo-check)* remove redundant todo macro rule and fix block comment pattern
- *(todo-check)* separate clippy todo lints into dedicated task

## [0.1.0-alpha.7](https://github.com/kent8192/reinhardt-web/compare/reinhardt-web@v0.1.0-alpha.6...reinhardt-web@v0.1.0-alpha.7) - 2026-02-10

### Fixed

- *(db)* remove unused reinhardt-test dev-dependency
- *(auth)* remove unused reinhardt-test dev-dependency
- *(core)* replace reinhardt-test with local poll_until helper
- *(server)* replace reinhardt-test with local poll_until helper
- *(utils)* break circular publish dependency with reinhardt-test
- *(rest)* move tests to integration crate to break circular publish chain
- *(views)* move tests to integration crate to break circular publish chain
- *(di)* move unit tests to integration crate to break circular publish chain
- *(http)* move integration tests to tests crate to break circular publish chain
- *(admin)* move database tests to integration crate to break circular publish chain
- *(utils)* use fully qualified Result type in poll_until helpers
- *(utils)* fix integration test imports and remove private field access
- *(di)* fix compilation errors in migrated unit tests
- *(admin)* fix User model id type to Option<i64> for impl_test_model macro
- *(di)* implement deep clone for InjectionContext request scope
- *(ci)* remove version from reinhardt-test workspace dep to avoid cargo 1.84+ resolution failure
- *(ci)* add gix workaround and manual dispatch support for release-plz
- *(ci)* broaden publish-check skip condition for release-plz fix branches
- *(ci)* use startsWith instead of contains for publish-check skip condition
- *(release)* revert unpublished crate versions to pre-release state

### Maintenance

- *(websockets)* remove manual CHANGELOG entries for release-plz

### Reverted

- undo release PR [[#215](https://github.com/kent8192/reinhardt-web/issues/215)](https://github.com/kent8192/reinhardt-web/issues/215) version bumps
- undo PR [[#219](https://github.com/kent8192/reinhardt-web/issues/219)](https://github.com/kent8192/reinhardt-web/issues/219) version bumps for unpublished crates

### Styling

- apply formatting to migrated test files and modified source files
- apply formatting to di and utils integration tests

## [0.1.0-alpha.6](https://github.com/kent8192/reinhardt-web/compare/reinhardt-web@v0.1.0-alpha.5...reinhardt-web@v0.1.0-alpha.6) - 2026-02-07

### Other

- Merge pull request #129 from kent8192/fix/issue-128-bug-runserver-uses-settingsdefault-instead-of-loading-from-settings-directory

## [0.1.0-alpha.5](https://github.com/kent8192/reinhardt-web/compare/reinhardt-web@v0.1.0-alpha.4...reinhardt-web@v0.1.0-alpha.5) - 2026-02-07

### Fixed

- add version to reinhardt-test workspace dependency for crates.io publishing
- *(utils)* remove unused dev-dependencies to break circular publish chain
- *(ci)* improve publish-check filter for non-publishable crates
- remove reinhardt-urls from doc example to avoid circular dependency
- break circular dependency between reinhardt-openapi-macros and reinhardt-rest
- remove unused dev-dependencies from reinhardt-rest
- remove reinhardt-di self-reference dev-dependency

### Other

- undo unpublished reinhardt-web v0.1.0-alpha.5 version bump and CHANGELOG entry
- release
- Revert "Merge pull request #202 from kent8192/release-plz-2026-02-06T13-32-57Z"
- release
- skip publish-check for release-plz branches
- add secrets inherit to reusable workflows
- install protoc for reinhardt-grpc build
- add publish dry-run check to detect circular dev-dependencies

## [0.1.0-alpha.4](https://github.com/kent8192/reinhardt-web/compare/reinhardt-web@v0.1.0-alpha.3...reinhardt-web@v0.1.0-alpha.4) - 2026-02-03

### Other

- Merge pull request #111 from kent8192/fix/issue-81-bug-reinhardt-pages-wasm-build-fails-due-to-tokiomio-server-side-dependencies

## [0.1.0-alpha.3](https://github.com/kent8192/reinhardt-web/compare/reinhardt-web@v0.1.0-alpha.2...reinhardt-web@v0.1.0-alpha.3) - 2026-02-03

### Fixed

- *(ci)* use GitHub App token for release-plz to trigger CI workflows
- add publish = false to example packages
- *(ci)* add missing example packages to release-plz exclusion list
- *(ci)* use registry-manifest-path to avoid workspace member errors
- *(ci)* run release-plz only on push to main
- *(ci)* use release-plz update for PR validation
- *(ci)* use release --dry-run for PR validation
- *(ci)* remove WASM build artifacts from git tracking
- *(examples)* standardize settings file pattern with .example.toml
- *(ci)* remove proptest regression files from git tracking
- *(ci)* use jlumbroso/free-disk-space for ui-test workflow
- *(ci)* use jlumbroso/free-disk-space for examples tests
- *(ci)* use jlumbroso/free-disk-space for integration tests
- *(ci)* add disk cleanup step to integration test workflows
- *(ci)* increase root-reserve-mb from 4GB to 8GB
- *(ci)* move docker pull after rust setup to avoid disk space issues
- *(ci)* replace docker save/load with pull-only approach
- *(ci)* delete tar files after docker load to save disk space
- *(ci)* split docker image lists by workflow to avoid disk space issues

### Other

- Merge pull request #167 from kent8192/fix/release-plz-cargo-metadata-warn
- *(release-plz)* use prebuilt binary for dry-run testing
- *(release-plz)* add --dry-run flag for debugging
- *(openapi)* bump version to 0.1.0-alpha.2 for release-plz fix
- add docker image caching to avoid rate limits
- Merge pull request #110 from kent8192/fix/issue-83-docs-improve-getting-started-experience-and-ecosystem-documentation
- change release PR branch prefix to release/
- merge main into chore/release-plz-migration
- update release label description for release-plz
- add release-plz migration markers to CHANGELOGs
- remove cargo-workspaces configuration from Cargo.toml
- update CLAUDE.md for release-plz migration
- simplify release commits section for release-plz
- rewrite release process documentation for release-plz
- remove Version Cascade Policy
- remove cargo-workspaces publish workflows
- add release-plz GitHub Actions workflow
- add release-plz configuration

### Sub-Crate Updates

<!-- Add sub-crate updates here following the format:
- `[crate-name]` updated to v[version] ([CHANGELOG](crates/[crate-name]/CHANGELOG.md#[anchor]))
  - Brief summary of key changes
-->

<!-- release-plz-separator -->
<!-- Entries below this line were created before release-plz adoption -->

## [0.1.0-alpha.2] - 2026-01-29

### Changed

- **BREAKING**: Update `static-files` feature to use `reinhardt-utils/staticfiles` (#114)

### Sub-Crate Updates

- `reinhardt-utils` updated to v0.1.0-alpha.4 ([CHANGELOG](crates/reinhardt-utils/CHANGELOG.md#010-alpha4---2026-01-30))
  - Re-release after version correction
- `reinhardt-conf` updated to v0.1.0-alpha.4 ([CHANGELOG](crates/reinhardt-conf/CHANGELOG.md#010-alpha4---2026-01-30))
  - Re-release after version correction
- `reinhardt-pages` updated to v0.1.0-alpha.4 ([CHANGELOG](crates/reinhardt-pages/CHANGELOG.md#010-alpha4---2026-01-30))
  - Re-release after version correction
- `reinhardt-test` updated to v0.1.0-alpha.4 ([CHANGELOG](crates/reinhardt-test/CHANGELOG.md#010-alpha4---2026-01-30))
  - Re-release after version correction
- `reinhardt-commands` updated to v0.1.0-alpha.4 ([CHANGELOG](crates/reinhardt-commands/CHANGELOG.md#010-alpha4---2026-01-30))
  - Version bump for publish workflow correction
- `reinhardt-rest` updated to v0.1.0-alpha.4 ([CHANGELOG](crates/reinhardt-rest/CHANGELOG.md#010-alpha4---2026-01-30))
  - Version bump for publish workflow correction
- `reinhardt-http` updated to v0.1.0-alpha.3 ([CHANGELOG](crates/reinhardt-http/CHANGELOG.md#010-alpha3---2026-01-30))
  - Version bump for publish workflow correction
- `reinhardt-db` updated to v0.1.0-alpha.3 ([CHANGELOG](crates/reinhardt-db/CHANGELOG.md#010-alpha3---2026-01-30))
  - Version bump for publish workflow correction
- `reinhardt-forms` updated to v0.1.0-alpha.3 ([CHANGELOG](crates/reinhardt-forms/CHANGELOG.md#010-alpha3---2026-01-30))
  - Version bump for publish workflow correction
- `reinhardt-pages-macros` updated to v0.1.0-alpha.3 ([CHANGELOG](crates/reinhardt-pages/macros/CHANGELOG.md#010-alpha3---2026-01-30))
  - Version bump for publish workflow correction

## [0.1.0-alpha.1] - 2026-01-23

### Sub-Crate Updates

- `reinhardt-shortcuts` updated to v0.1.0-alpha.2 ([CHANGELOG](crates/reinhardt-shortcuts/CHANGELOG.md#010-alpha2---2026-01-23))
  - Initial release with keyboard shortcut support
- `reinhardt-i18n` updated to v0.1.0-alpha.2 ([CHANGELOG](crates/reinhardt-i18n/CHANGELOG.md#010-alpha2---2026-01-23))
  - Initial release with internationalization support

### Added

- Initial release of the full-stack API framework facade crate
- Feature presets: minimal, standard, full, api-only, graphql-server, websocket-server, cli-tools, test-utils
- Fine-grained feature flags for authentication, database backends, middleware, and more
- WASM target support via conditional compilation
- Re-exports of all Reinhardt sub-crates through a unified API

---

## Sub-Crate CHANGELOGs

For detailed changes in individual sub-crates, refer to their respective CHANGELOG files:

### Core & Foundation
- [reinhardt-core](crates/reinhardt-core/CHANGELOG.md) - Core framework types and traits
- [reinhardt-utils](crates/reinhardt-utils/CHANGELOG.md) - Utility functions and macros
- [reinhardt-conf](crates/reinhardt-conf/CHANGELOG.md) - Configuration management

### Database & ORM
- [reinhardt-db](crates/reinhardt-db/CHANGELOG.md) - Database connection and query building

### Dependency Injection
- [reinhardt-di](crates/reinhardt-di/CHANGELOG.md) - Dependency injection container
- [reinhardt-dentdelion](crates/reinhardt-dentdelion/CHANGELOG.md) - DI macros and utilities

### HTTP & REST
- [reinhardt-http](crates/reinhardt-http/CHANGELOG.md) - HTTP server and request handling
- [reinhardt-rest](crates/reinhardt-rest/CHANGELOG.md) - REST API framework
- [reinhardt-middleware](crates/reinhardt-middleware/CHANGELOG.md) - HTTP middleware
- [reinhardt-server](crates/reinhardt-server/CHANGELOG.md) - Server runtime

### GraphQL & gRPC
- [reinhardt-graphql](crates/reinhardt-graphql/CHANGELOG.md) - GraphQL server implementation
- [reinhardt-graphql-macros](crates/reinhardt-graphql/macros/CHANGELOG.md) - GraphQL procedural macros
- [reinhardt-grpc](crates/reinhardt-grpc/CHANGELOG.md) - gRPC server implementation

### WebSockets & Real-time
- [reinhardt-websockets](crates/reinhardt-websockets/CHANGELOG.md) - WebSocket support

### Authentication & Authorization
- [reinhardt-auth](crates/reinhardt-auth/CHANGELOG.md) - Authentication and authorization

### Views & Forms
- [reinhardt-views](crates/reinhardt-views/CHANGELOG.md) - View rendering and templates
- [reinhardt-forms](crates/reinhardt-forms/CHANGELOG.md) - Form handling and validation

### Routing & Dispatch
- [reinhardt-urls](crates/reinhardt-urls/CHANGELOG.md) - URL routing
- [reinhardt-dispatch](crates/reinhardt-dispatch/CHANGELOG.md) - Request dispatcher
- [reinhardt-commands](crates/reinhardt-commands/CHANGELOG.md) - Command pattern implementation

### Background Tasks & Messaging
- [reinhardt-tasks](crates/reinhardt-tasks/CHANGELOG.md) - Background task queue
- [reinhardt-mail](crates/reinhardt-mail/CHANGELOG.md) - Email sending

### Internationalization & Shortcuts
- [reinhardt-i18n](crates/reinhardt-i18n/CHANGELOG.md) - Internationalization support
- [reinhardt-shortcuts](crates/reinhardt-shortcuts/CHANGELOG.md) - Keyboard shortcuts

### Admin & CLI
- [reinhardt-admin](crates/reinhardt-admin/CHANGELOG.md) - Admin interface
- [reinhardt-admin-cli](crates/reinhardt-admin-cli/CHANGELOG.md) - Admin CLI tools

### Testing
- [reinhardt-test](crates/reinhardt-test/CHANGELOG.md) - Testing utilities and fixtures
