# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0](https://github.com/kent8192/reinhardt-web/releases/tag/reinhardt-web@v0.1.0) - 2026-03-08

### Added

- Initial release of the full-stack API framework facade crate
- Feature presets: minimal, standard, full, api-only, graphql-server, websocket-server, cli-tools, test-utils
- Fine-grained feature flags for authentication, database backends, middleware, and more
- WASM target support via conditional compilation
- Re-exports of all Reinhardt sub-crates through a unified API
- *(examples)* introduce Injected<T> usage in di-showcase
- *(website)* add favicon generated from logo
- *(website)* add WASM frontend and multiplatform cards to Why Reinhardt section
- *(website)* add 6 additional feature cards to Why Reinhardt section
- *(website)* expand color palette and add web font variables
- *(website)* replace Zola inline syntax highlighting with highlight.js
- *(website)* add sidebar navigation to standalone pages
- *(examples)* add settings files for all examples
- *(examples)* add ci.toml and auto-detect CI environment
- *(examples)* add docker-compose.yml for PostgreSQL examples
- *(examples)* add docker-up dependency to runserver for PostgreSQL examples

### Changed

- *(ci)* remove redundant flags from cargo check task
- *(website)* reorder header nav and implement unified weight-based sidebar
- *(website)* move onboarding content into quickstart section
- *(website)* switch docs to weight-based ordering for reference material
- *(examples)* remove reinhardt-examples references and adopt monorepo-only strategy
- *(examples)* remove stale staging/production settings templates
- *(examples)* simplify settings.rs to consistent pattern
- *(examples)* move docker-compose.yml into each PostgreSQL example
- *(query)* replace super::super:: with crate:: absolute paths in query submodules
- *(query)* replace super::super:: with crate:: absolute paths in dcl tests
- *(db)* replace super::super:: with crate:: absolute paths in migrations
- *(rest)* remove unused sea-orm dependency
- *(query)* remove unused backend imports in drop role and drop user tests
- *(db)* fix unused variable assignments in migration operation tests
- *(query)* move DML integration tests to integration test crate
- convert relative paths to absolute paths
- *(db)* convert relative paths to absolute paths in orm execution
- restore single-level super:: paths preserved by convention

### Fixed

- *(ci)* enforce semver-check during RC phase instead of skipping
- *(ci)* remove non-existent paths from CODEOWNERS
- *(macros)* replace skeleton tests with meaningful assertions in pre_validate
- *(examples)* add force-link for library crate in di-showcase manage.rs
- *(ci)* prevent UI Tests from running when Phase 1 checks fail
- *(ci)* add missing validator dependency to reinhardt-test-support
- *(core)* add wasm32 platform gate to parallel and jsonschema validator modules
- *(commands)* correct project template compilation errors
- *(commands)* correct app template compilation errors
- *(ci)* change runner selection to opt-in for self-hosted runners
- *(ci)* add 5-minute grace period for JIT runner scale-down
- *(ci)* increase JIT runner minimum running time to 15 minutes
- *(ci)* use ubuntu user for runner userdata and run_as configuration
- *(ci)* add Ubuntu userdata template to replace Amazon Linux default
- *(ci)* remove nounset flag from userdata to fix unbound variable error
- *(ci)* use correct root device name for Ubuntu AMI (/dev/sda1)
- *(ci)* install protoc v28 instead of system v3.12 for proto3 optional
- *(ci)* add unzip to userdata package list for protoc installation
- *(ci)* use .cargo/config.toml instead of RUSTFLAGS for mold in coverage
- *(ci)* remove mold linker from coverage jobs to fix profraw generation
- *(ci)* enable job_retry to prevent ephemeral runner scaling deadlock
- *(ci)* add missing rust setup and gh cli for self-hosted runners
- *(middleware)* validate host header against allowed hosts in HTTPS redirect
- *(middleware)* add missing import in HttpsRedirectMiddleware doc test
- *(auth)* use deterministic UUID for RemoteUserAuthentication
- *(urls)* convert path-type parameters to matchit catch-all syntax in RadixTree mode
- *(test)* update rand 0.9 API usage in csrf integration tests
- *(ci)* handle cargo metadata failure and jq errors in detect-affected-packages.sh
- *(ci)* use git log to detect changed files in PR branches that contain main
- *(ci)* use origin/HEAD_REF instead of HEAD to detect changed files in PRs
- *(ci)* resolve permanent cache miss in setup-rust action
- *(ci)* remove shell quoting bug in nextest filter expression passing
- *(website)* set cloudflare pages production branch to main before deploy
- *(website)* add workflow_dispatch trigger for manual deployment
- *(website)* add DNS records for custom domain resolution
- *(infra)* add import blocks for existing Cloudflare resources
- *(db)* gate sqlite-dependent tests with feature flag
- *(db)* replace float test values to avoid clippy approx_constant lint
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
- add panic prevention and error handling for admin operations
- *(examples)* update Docker build context and COPY paths for flattened structure
- *(gitignore)* update stale examples/local path to flattened structure
- *(ci)* remove stale example package overrides from release-plz.toml
- *(ci)* remove stale test-common-crates job from test-examples.yml
- *(examples)* restore required default settings values for Settings deserialization
- resolve Test Examples CI failures
- *(query)* add missing DropBehavior import in revoke statement tests
- *(query)* add Table variant special handling in Iden derive macro
- *(query)* add missing code fence markers in alter_type doc example
- *(query)* add explicit path attributes to DML test module declarations
- *(query)* add Meta::List support to Iden derive macro attribute parsing
- *(query)* read iden attribute from struct-level instead of first field
- *(db)* bind insert values in many-to-many manager instead of discarding
- *(query)* reject whitespace-only names in CreateUser and GrantRole validation
- *(commands)* remove unused reinhardt-i18n dev-dependency
- *(dentdelion)* correct doctest import path to use prelude module
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
- *(examples)* standardize settings file pattern with .example.toml

### Documentation

- *(stability)* relax SP-1 API freeze and add SP-6 non-breaking addition review
- *(claude)* add SP-6 non-breaking addition policy to quick reference
- *(pr)* add three-dot diff rule for PR verification (RP-5)
- *(pr)* replace Japanese text with English in RP-5
- *(website)* update admin customization tutorial to use separate admin struct pattern
- add agent-detected bug verification policy (SC-2a, IL-3)
- *(rest)* align REST tutorial docs with actual API
- *(basis)* align basis tutorial docs with actual API
- align cookbook and quickstart docs with actual API
- add official website link to Quick Navigation
- update internal documentation links to official website URLs
- remove repository-hosted documentation migrated to reinhardt-web.dev
- *(website)* add sidebar_weight to tutorial pages
- *(website)* add tutorials index page with card-based navigation
- *(website)* audit and fix errors across docs pages
- remove non-existent feature flags from lib.rs documentation
- *(examples)* add quick start instructions to README.md
- update TODO policy with CI enforcement
- rewrite CLAUDE.md TODO check sections in English

### Testing

- *(db)* add field mapping and migrations integration tests
- *(db)* add warning log test for .sql file detection

### Styling

- *(urls)* apply project formatting to pattern module
- *(website)* redesign visual components with modern aesthetics
- format twitter example common component
- *(query)* format Iden derive macro code
- apply formatting to migrated test files and modified source files
- apply formatting to di and utils integration tests

### Maintenance

- *(labels)* add rc-addition label for SP-6 non-breaking additions
- *(semver)* update comments to reflect SP-1 relaxation policy
- *(serena)* clean up project.yml formatting
- *(template)* add self-hosted runner checkbox to PR template
- require PR checkbox opt-in for self-hosted runner selection
- migrate remaining workflows to support self-hosted runners
- phase test jobs to prevent spot vCPU quota exhaustion
- skip CI for out-of-date PR branches
- add branch status check to test-examples workflow
- add agent-suspect and stable-migration labels to labels.yml
- add RC stability timer monitoring workflow
- *(semver)* auto-detect breaking changes from commit messages
- increase semver-check timeout from 30 to 45 minutes
- add Tachyon Inc. copyright notices
- remove out-of-date branch skip from CI workflows
- add run-examples output to detect-affected-packages workflow
- fix BASE_REF fallback in detect-examples step
- skip examples-test when no examples changes on non-release PRs
- skip test-examples matrix when no examples changes on non-release PRs
- switch detect-affected-packages from git log to git diff
- use GitHub PR Files API to detect changed files in PR context
- add pull-requests: read permission to CI workflow
- fail explicitly on gh api errors instead of silently swallowing them
- *(website)* update license references from MIT/Apache-2.0 to BSD-3-Clause
- *(website)* add Cloudflare Pages deployment workflow
- add terraform patterns to gitignore
- *(infra)* add terraform configuration for cloudflare pages and github secrets
- *(infra)* rename terraform template to conventional .example.tfvars format
- add explanatory comments to undocumented #[allow(...)] attributes
- *(examples)* remove stale examples/local settings files
- *(examples)* remove stale configuration files from old repository
- *(examples)* remove unused example-common and example-test-macros crates
- *(examples)* update stale remote-examples-test task in Makefile.toml
- *(examples)* remove stale help tasks from all example Makefile.toml
- *(examples)* remove stale availability test referencing deleted example_common crate
- *(examples)* remove stale settings from example base.toml files
- add setup-protoc step to test-examples workflow
- remove pull_request trigger from test-examples.yml
- add copilot setup steps workflow
- increase test partition counts for faster CI execution
- *(todo-check)* add clippy todo lint job to TODO Check workflow
- *(todo-check)* add semgrep rules for TODO/FIXME comment detection
- *(todo-check)* add reusable workflow for unresolved TODO scanning
- integrate TODO check into CI pipeline
- *(todo-check)* switch from semgrep scan to semgrep ci
- *(clippy)* add deny lints for todo/unimplemented/dbg_macro
- *(todo-check)* remove redundant todo macro rule and fix block comment pattern
- *(todo-check)* separate clippy todo lints into dedicated task

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
