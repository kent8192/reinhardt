# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0-rc.16](https://github.com/kent8192/reinhardt-web/compare/reinhardt-web@v0.1.0-rc.15...reinhardt-web@v0.1.0-rc.16) - 2026-04-19

### Added

- *(pages)* add JWT token management and auth header injection for WASM SPA
- *(admin)* add login server function with JWT authentication
- *(admin)* add login page, auth gate, and 401 redirect for WASM SPA
- *(auth)* add SuperuserInit trait and SuperuserCreator registry
- *(commands)* add createsuperuser as built-in management command
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
- *(conf)* add OpenApiSettings fragment
- add SubmitButton support to form! macro fields
- add CDP-based E2E browser testing infrastructure and WASM SPA fixes
- *(urls)* implement Debug for UnifiedRouter and ServerRouter
- *(di)* [**breaking**] unify #[inject] parameter type from Arc<T> to Depends<T>
- *(urls)* [**breaking**] support async functions in #[routes] macro
- *(commands)* add RunserverHook for concurrent service startup and pre-listen validation
- add release data collection script for announcement automation
- add Discussion posting script for release announcements
- add release announcement automation to release-plz workflow
- *(test)* add MSW-style network interception module with core components
- *(pages)* add MockableServerFn trait and macro generation under msw feature
- *(core)* add use_endpoint! macro for multi-file view resolver re-export
- *(macros)* add #[export_endpoints] attribute for multi-file view modules

### Changed

- *(admin)* [**breaking**] mark AdminRoute as non_exhaustive and reorder Login variant
- *(db)* remove redundant migration naming in autodetector
- *(conf)* use #[settings(fragment = true)] macro for OpenApiSettings
- *(http)* remove dead should_stop_chain check in CCH
- *(admin)* update workaround comments for page! @event closure capture limitation
- *(admin)* add issue refs to workaround comments and autocomplete to form inputs
- *(urls)* extract AsyncRouterFactoryFn type alias to reduce type complexity
- *(di)* remove dead use_cache branch and avoid Arc rewrap in Depends
- *(di)* remove unnecessary Clone bound from Depends<T> and Injected<T>
- *(examples)* remove non-pages examples and revert to #[routes]
- *(examples)* rename cfg aliases from server/client to native/wasm
- *(examples)* use build_composed and move defaults to TOML
- *(examples-twitter)* use framework AuthState and AuthUser<U> extractor
- *(examples)* register all apps, add #[app_config], simplify manage.rs

### Deprecated

- *(rest)* deprecate OpenApiConfig since 0.1.0-rc.16
- *(test)* mark MockFetch and mock_server_fn as deprecated in favor of MSW

### Documentation

- *(admin)* fix broken intra-doc link to CspMiddleware
- *(auth)* add deprecation notice to standalone createsuperuser binary
- *(conf)* fix composable settings TOML structure and add serde defaults
- *(conf)* fix unresolved SettingsFragment link in openapi module doc
- *(rest)* fix broken intra-doc link to OpenApiSettings
- *(middleware)* fix intra-doc link errors for feature-gated types
- *(http)* address Copilot review on [[#3417](https://github.com/kent8192/reinhardt-web/issues/3417)](https://github.com/kent8192/reinhardt-web/issues/3417)
- *(.devin)* update wiki.json to reflect breaking changes since rc.15
- *(.devin)* fix inaccuracies flagged by Copilot review
- *(.devin)* remove define_views! references from wiki.json
- *(.devin)* abstract view module organization descriptions in wiki.json
- *(.devin)* abstract overly specific descriptions in wiki.json
- *(.devin)* remove Migration Guide rc.15→rc.16 page from wiki.json
- *(.devin)* fix version reference, DI wording, and grammar in wiki.json

### Fixed

- *(ci)* stop unattended-upgrades before apt-get to prevent dpkg lock
- *(pages)* add web-sys Storage feature for sessionStorage access
- *(admin)* use path_params instead of full URI in static file handler
- *(admin)* call WASM init() in SPA HTML for web target output
- *(admin)* support HEAD requests for static file handler
- *(admin)* remove broken presetWind() global function call for UnoCSS v66+
- *(admin)* initialize UnoCSS runtime with v66+ API for preset-wind
- *(docs)* resolve broken intra-doc links and incorrect test assertion
- *(conf)* remove #[serde(flatten)] from SecuritySettings and fix TOML scoping
- *(conf)* add missing newline at end of pages template base.example.toml
- *(admin)* replace CDN references with local vendor paths
- *(settings)* update tests to use nested security keys after [[#3176](https://github.com/kent8192/reinhardt-web/issues/3176)](https://github.com/kent8192/reinhardt-web/issues/3176) de-flatten
- *(ci)* guard mutation-test if condition against undefined inputs context
- *(commands)* derive is_initial from migration number instead of hardcoding
- *(db)* generate AlterColumn, CreateIndex, and DropIndex operations from schema diff
- *(db)* generate CreateIndex for indexes on newly created tables
- *(db)* detect and generate constraint changes in SchemaDiff
- *(urls)* route framework-level 404/405 responses through middleware chain
- *(rest)* suppress deprecation warning on OpenApiConfig re-export
- *(middleware)* convert errors to responses in security-critical middleware
- *(middleware)* convert errors to responses in functional middleware
- *(middleware)* convert errors to responses in cross-crate middleware
- *(di)* add Authentication variant to DiError for proper 401 responses
- *(admin)* make WASM JS reference test dynamic based on build state
- *(test)* use recursive file count in collectstatic tests
- *(test)* update middleware tests for ErrorToResponseHandler behavior
- *(test)* skip WASM artifact tests when WASM is not built
- *(test)* update migration E2E tests for implemented operations
- *(test)* update versioning test for ErrorToResponseHandler
- *(admin)* use explicit headers and string conversion for CSV/TSV export
- *(di)* add #[non_exhaustive] to DiError enum
- *(examples)* replace deprecated SecurityConfig with SecuritySettings
- *(admin)* correct static route assertion to match catch-all pattern
- *(test)* skip hidden files in collectstatic test helper
- *(pages)* preserve HTTP status codes for DI auth errors in server_fn
- *(admin)* use write_record for CSV/TSV export to support map-based records
- *(admin)* handle UUID primary keys in create RETURNING clause
- *(commands)* exit with error when --with-pages WASM build fails
- *(admin)* ensure vendor assets are available during development
- *(auth)* add is_staff and is_superuser fields to JWT Claims
- *(admin)* embed staff status in JWT token during admin login
- *(tests)* fix makemigrations and admin create test regressions
- *(pages)* inline @event closure capture to fix move semantics
- *(admin)* prefix page! event params with underscore to suppress non-WASM warnings
- auto-pass CSRF token as server_fn argument in form! macro
- suppress unused_variables warnings in form! macro codegen
- *(infra)* add ed25519 SSH key type for Packer AMI builds
- resolve merge conflicts with main and fix CI failures
- resolve merge conflicts with main, migrate login to form! macro
- *(admin)* switch WASM SPA to mount() rendering with scheduler init
- *(admin)* make AdminSite registry lookups case-insensitive
- align E2E tests with upstream WASM SPA fixes from PR [[#3350](https://github.com/kent8192/reinhardt-web/issues/3350)](https://github.com/kent8192/reinhardt-web/issues/3350)/[[#3351](https://github.com/kent8192/reinhardt-web/issues/3351)](https://github.com/kent8192/reinhardt-web/issues/3351)/[[#3352](https://github.com/kent8192/reinhardt-web/issues/3352)](https://github.com/kent8192/reinhardt-web/issues/3352)
- gate WASM-only imports with #[cfg(client)] to suppress unused warnings
- *(urls)* normalize leading slash after prefix stripping in resolve()
- update integration tests and docs for Depends<T> unification
- *(di)* wrap RESOLVE_CTX.scope() inner block return type as DiResult<T>
- resolve CI failures in format check and cargo check (tests)
- *(commands)* address Copilot review feedback on RunserverHook
- *(di)* add scope fallback in resolve for pre-seeded types
- *(di)* register Injectable types in global registry for Depends resolution
- use temp files instead of shell args to avoid ARG_MAX overflow
- *(ci)* fail announcement job when data collection fails instead of silent success
- address Copilot review feedback
- *(query,core)* replace approx_constant test values to avoid clippy deny
- *(pages-macros)* resolve clippy len_zero and bool_assert_comparison warnings
- *(query)* resolve clippy warnings in tests
- *(grpc)* resolve clippy warnings in tests
- *(di)* resolve clippy warnings in tests, benchmarks, and override_registry
- *(core)* resolve clippy warnings in reactive, security, and exception modules
- *(throttling)* resolve bool_assert_comparison in burst tests
- *(utils)* resolve clippy warnings in staticfiles middleware
- *(query)* move impl blocks before test modules in backend files
- *(query)* use as_str() to avoid ambiguous to_string() with Iden trait
- *(ci)* add #[allow(deprecated)] to re-exports and tests using deprecated mock APIs
- *(test)* address Copilot review feedback on MSW module
- *(admin)* suppress unexpected check-cfg warning for msw feature
- *(ci)* resolve clippy errors in MSW module for native builds
- *(ci)* use backticks instead of intra-doc links for feature-gated types
- *(testkit)* remove auth-testing feature, make auth unconditional on native targets
- *(ci)* replace gh discussion CLI with GraphQL createDiscussion mutation
- *(macros)* add standalone mode to #[routes] for projects without installed_apps!
- *(docs)* escape intra-doc link in TestUser deprecated note
- *(examples)* include orphaned config/apps.rs in module tree
- *(examples)* use routes(standalone) and apply cargo fmt
- *(macros)* move #[viewset] usage to module-level free function
- *(macros)* address review — error propagation, WASM skip, ident validation
- *(examples)* use routes(standalone) for examples without url_prelude
- *(macros)* remove useless Into::into conversion on syn::Ident
- *(macros)* use tt repetition instead of path fragment in __for_each_url_resolver
- *(macros)* address Copilot review — use tt+ and normalize test assertions
- *(ci)* add AdminAuthenticatedUser argument to get_dashboard test calls
- *(urls)* add UrlResolver trait import to generated url_prelude methods
- *(macros)* suppress unexpected_cfgs for client-router in #[routes] macro
- *(testkit)* align rabbitmq image tag to pre-pulled 3-management-alpine
- *(auth)* address Copilot review on OAuth/OIDC mock tests
- *(ci)* add checkout step before local composite action in announce workflow

### Maintenance

- upgrade workspace dependencies to latest versions
- add .wtp.yml to .gitignore
- add announcements directory and Claude CLI prompt for release announcements
- add missing docker images to CI cache lists
- pre-pull mongo, localstack, and chromedp headless-shell images
- *(auth)* add wiremock/rsa/rand_core dev-dependencies for oauth mocks
- *(auth)* remove stale placeholder comments in oauth/oidc modules
- merge main into feature/issue-3670-url-patterns-typed-app-label

### Other

- resolve conflict with main (deduplicate tests)
- resolve conflict with main in security tests
- Fix CSP blocking admin WASM SPA init script
- Change AuthState user_id from i64 to String for UUID support
- Auto-inject timestamps for auto_now/auto_now_add fields in admin CRUD
- Detect workspace target_dir via cargo metadata in WASM builder
- Fix admin CRUD type coercion for timestamptz and uuid columns
- Validate project directory before running makemigrations
- resolve conflict in registration.rs with main

### Security

- *(admin)* enforce is_active DB check in get_dashboard

### Styling

- *(admin)* apply rustfmt to test assertion in router module
- *(examples)* apply rustfmt import ordering and formatting fixes
- *(admin)* fix formatting in settings, security, and router
- fix formatting in OpenApiSettings files
- apply rustfmt formatting fixes
- *(examples)* fix import ordering in twitter middleware
- apply cargo make auto-fix (clippy + fmt)
- apply rustfmt formatting via cargo make auto-fix
- apply rustfmt formatting
- apply rustfmt to site.rs
- apply cargo make auto-fix formatting
- apply rustfmt formatting fixes
- apply rustfmt to clippy-fixed files
- *(examples)* apply reinhardt-admin fmt to github-issues and twitter
- *(examples)* apply reinhardt-admin fmt to tutorial-basis and twitter
- *(examples)* apply reinhardt-admin fmt formatting
- *(core)* fix formatting in reinhardt-core macros and lib
- apply cargo fmt

### Testing

- *(migrations)* add E2E tests for descriptive migration naming
- *(migrations)* expand coverage for all operation name fragments and edge cases
- *(migrations)* restore AlterColumn and CreateIndex E2E tests
- *(urls)* add integration tests for router-level 404 middleware
- *(integration)* add OpenApiSettings composition tests
- add SubmitButton rendering regression tests
- *(di)* update trybuild .stderr files for current compiler output
- *(di)* register test types in global registry for Depends resolution
- *(di)* update trybuild .stderr for Depends<T> trait bound change
- *(test)* add WASM integration tests, rstest fixtures, and fix URL matching
- *(auth)* add oauth/oidc mock integration tests

### Breaking Changes

- **`#[url_patterns]` requires typed arguments** ([#3670](https://github.com/kent8192/reinhardt-web/issues/3670))
  - Old: `#[url_patterns]`, `#[url_patterns("auth")]`, `#[url_patterns(auth)]`, or `#[url_patterns(client = true, app = "auth")]`
  - New: `#[url_patterns(InstalledApp::auth, mode = server|client|unified)]`
  - Function-name convention: `url_patterns()` → `server_url_patterns()` / `client_url_patterns()` / `unified_url_patterns()`
  - Projects must invoke `installed_apps!` (scaffold now generates `src/config/apps.rs` automatically; `reinhardt startapp` appends new entries there)

### Added

- **`AppLabel::path(&self) -> &'static str`** method on the existing `AppLabel` trait ([#3670](https://github.com/kent8192/reinhardt-web/issues/3670)) — a default implementation returns `Self::LABEL`, so existing unit-struct implementors (`AuthApp`, `ContentTypesApp`, `SessionsApp`) continue to work unchanged. Enum-style implementors (like the `InstalledApp` enum generated by `installed_apps!`) override `path()` to dispatch on `self`.
- **`mode = unified` for `#[url_patterns]`** ([#3670](https://github.com/kent8192/reinhardt-web/issues/3670)) — scans `.server(|s| ...)` and `.client(|c| ...)` closures on a `UnifiedRouter` and emits both `url_resolvers` and `client_url_resolvers` modules.
- **`reinhardt startapp` appends new apps to `src/config/apps.rs`** ([#3670](https://github.com/kent8192/reinhardt-web/issues/3670)) — idempotent on repeat invocations; silently skips when the file is absent (older project structure).
- `ValidatorScope` and `ClientTrigger` enums in `reinhardt-manouche::core::form_node`. Validator rules now carry a `scope` field that is propagated through the typed AST to codegen, where `.validate()` is filtered to server-scoped rules (`Both`, `Server`, `ServerAndClient`). (#3654)

### Fixed

- **`UnifiedRouter::with_namespace` now propagates to the inner `ClientRouter`** ([#3726](https://github.com/kent8192/reinhardt-web/issues/3726)) — previously only the server-side namespace was set, so client-side named routes inside a `UnifiedRouter` were not prefixed with `"<namespace>:"`.

### Deprecated

- `form!` macro: `client_validators: { ... }` is now rejected with a customized compile error that directs users to the unified `validators:` block with `#[client(on = submit|input|blur)]` annotations. The feature had no code generation path in any released version; no runtime behavior is lost. The identifier will be removed from the parser entirely in 0.2.0. See `docs/migration/0.2.0-client-validators-removal.md`. (#3654)

### Migration

1. Add `pub mod apps;` to `src/config.rs` and create `src/config/apps.rs`.
2. Populate `src/config/apps.rs` with `installed_apps! { <label>: "<label>", ... }`.
3. In each app's `urls.rs`:
   - Add `use crate::config::apps::InstalledApp;`
   - Change the attribute to `#[url_patterns(InstalledApp::<label>, mode = server)]` (or `client` / `unified`)
   - Rename `url_patterns` → `server_url_patterns` / `client_url_patterns` / `unified_url_patterns`
4. `reinhardt startapp` now handles step 2 for you on new apps.

## [0.1.0-rc.15](https://github.com/kent8192/reinhardt-web/compare/reinhardt-web@v0.1.0-rc.14...reinhardt-web@v0.1.0-rc.15) - 2026-03-29

### Added

- *(staticfiles)* add WasmEntry struct and auto-inject config fields
- *(staticfiles)* implement WASM entry point detection
- *(staticfiles)* implement WASM script injection into HTML
- *(staticfiles)* wire WASM auto-injection into SPA fallback
- *(examples)* remove manual WASM scripts for auto-injection
- *(orm)* add Vec/Value/HashMap support to field_type_to_metadata_string
- *(facade)* add hidden reinhardt_auth module re-export
- *(auth)* add AuthPermission database model
- *(auth)* extend Group struct with #[model] support
- *(macros)* inject ManyToMany relationships in #[user] + #[model]
- *(auth)* integrate GroupManager with PermissionsMixin
- *(orm)* add #[field(skip = true)] attribute for non-DB fields

### Changed

- *(admin)* migrate CurrentUser to AuthUser in server functions

### Documentation

- *(tutorials)* fix incorrect API references in tutorial code examples
- fix documentation-implementation inconsistencies across website and codebase
- *(website)* propagate [[#3060](https://github.com/kent8192/reinhardt-web/issues/3060)](https://github.com/kent8192/reinhardt-web/issues/3060) changes to tutorials and reference docs

### Fixed

- *(query)* preserve single quotes in MySQL user identifier parsing
- fix!(auth): add JwtError enum and reject expired tokens by default
- *(staticfiles)* address security and spec compliance review issues
- *(staticfiles)* add empty wasm_entry check and fix log levels
- *(admin)* add admin_static_routes to reinhardt re-exports
- *(admin)* preserve query string in popstate navigation handler
- *(admin)* migrate remaining CurrentUser to AuthUser and update example
- *(admin)* replace CRLF before individual char replacement in TSV export
- *(testkit)* use multi-thread runtime and install any drivers in fixture tests
- *(admin)* update integration tests to match new admin API signatures
- *(test)* update admin create tests to match fixed implementation
- *(di)* register HTTP request in request_scope during fork_for_request
- *(admin)* add serde helper for Vec<String> ORM deserialization
- *(admin)* accept any #[user] type for admin authentication via type-erased loader
- *(test)* update make_auth_user to return AdminAuthenticatedUser

### Maintenance

- *(staticfiles)* fix formatting and dead code warning

### Other

- resolve conflict with main in delete.rs
- resolve conflicts with main in features.rs
- resolve conflict with main in delete.rs
- resolve conflict with main in tweet admin config
- resolve conflicts with main branch

### Testing

- *(testkit)* add comprehensive tests for core utility modules
- *(testkit)* add tests for fixture modules (server, testcontainers, resources, validator)
- *(test)* add fixture output validation tests for auth and admin_panel
- *(test)* add WASM module tests and convert existing tests to rstest
- *(admin)* add update_record integration tests
- *(admin)* add E2E integration tests for DI resolution pipeline
- *(admin)* enable 12 ignored E2E tests for DI pipeline and CSRF
- *(auth)* add GroupManager integration and user macro tests

## [0.1.0-rc.14](https://github.com/kent8192/reinhardt-web/compare/reinhardt-web@v0.1.0-rc.13...reinhardt-web@v0.1.0-rc.14) - 2026-03-24

### Added

- *(infra)* add custom runner Docker image with Rust toolchain
- *(infra)* add Terraform GitHub config for MAC_RUNNER_ENABLED variable
- *(infra)* add Terraform configuration for Mac local runner
- *(ci)* extend determine-runner with Mac local runner priority
- *(reinhardt-di)* add protocol-agnostic `fork()` method to `InjectionContext`
- *(conf)* define SettingsFragment trait for composable settings
- *(macros)* add nom v8.0.0 parser for settings composition syntax
- *(conf)* define SecuritySettings fragment
- *(macros)* implement #[settings] attribute macro (fragment + composition)
- *(conf)* extract built-in fragments from AdvancedSettings
- *(conf)* define Django-compat fragments (I18n, Template, Contact)
- *(conf)* define CoreSettings fragment with nested SecuritySettings
- *(conf)* re-export fragment types and Has* traits from crate root

### Changed

- *(infra)* rename cancel-runner to hotpath-runner
- *(ci)* replace custom setup-protoc with arduino/setup-protoc@v3
- *(conf)* deprecate AdvancedSettings in favor of fragment system
- *(commands)* update project templates to use ProjectSettings
- *(conf)* deprecate Settings, add HasCoreSettings bridge via serde(flatten)
- *(examples)* migrate all examples from Settings to ProjectSettings
- *(macros)* require explicit CoreSettings in #[settings] macro

### Documentation

- update CRATE_STRUCTURE.md with accurate crate count and descriptions
- fix broken links, typos, and outdated info in root docs
- *(infra)* add Mac local runner README with setup instructions
- correct git_release_enable and semver_check values in release process documentation
- clarify GitHub Releases are enabled only for reinhardt-web
- *(website)* update settings documentation for composable settings system
- *(settings)* update docs and examples for explicit CoreSettings

### Fixed

- *(migrations)* resolve multi-element dependency parsing and deterministic sort
- *(rest)* correct module path in versioning macro
- *(ci)* use atomic dpkg lock timeout and add missing environment key
- *(testkit)* unify PostgreSQL version, add pool close, and cleanup backoff
- *(pages)* protect textarea, style, and script from minification
- *(db-macros)* emit compile error for unknown field attributes
- *(examples)* use /api/ mount point for URL consistency
- *(ci)* add actor check to workflow_dispatch Mac runner gate
- *(infra)* use pre-built binaries for nextest and mold in Dockerfile
- *(infra)* use canonical repo URL (reinhardt-web) for runner registration
- *(infra)* update runner base image to v2.333.0 (v2.322.0 deprecated)
- *(infra)* add OpenSSL dev packages to Mac runner Docker image
- *(ci)* pass explicit config path to tfprovidercheck
- resolve merge conflict keeping both escape tracking and char count tests
- *(ci)* add RUSTSEC-2026-0049 to security audit ignore list
- *(ci)* install protoc from GitHub Releases and fix DinD TLS hostname
- *(ci)* address protoc setup review feedback
- *(infra)* upgrade runner base image to Ubuntu Jammy for GLIBC 2.35
- *(reinhardt-db)* remove unnecessary dereference in pool connection
- *(reinhardt-core)* fork DI context per-request in route and action macros
- *(reinhardt-pages)* fork DI context per-request in server function macros
- *(reinhardt-grpc)* fork DI context per-request in gRPC handler macros
- *(reinhardt-graphql)* fork DI context per-request in GraphQL handler macros
- *(reinhardt-pages,reinhardt-di)* add Content-Type negotiation for server_fn and Json<T> extractor
- *(reinhardt-di)* address Copilot review on Content-Type handling
- *(middleware)* update Settings field access for CoreSettings restructuring
- *(tests)* update integration tests for CoreSettings restructuring
- suppress deprecated Settings warnings and fix unreachable pub visibility
- address Copilot review feedback
- *(macros)* add missing CoreSettings and HasCoreSettings imports for explicit settings
- *(examples)* remove unused CoreSettings/HasCoreSettings imports
- resolve fmt-check and docs-rs-check CI failures
- *(ci)* add RUSTSEC-2026-0066 to security audit ignore list
- *(conf)* import HasCoreSettings trait in Settings test module
- *(test)* configure trusted proxies in rate limit integration tests
- *(test)* update search filter test expectations for LIKE ESCAPE clause
- *(test)* account for auto-discovered app static files in collectstatic test
- *(query)* trim quotes from user part in parse_user_host
- *(settings)* address Copilot review feedback for field policy system
- *(settings)* use empty named-field struct instead of unit struct in test
- *(ci)* use compact jq output for partitions-json in GITHUB_OUTPUT
- *(ci)* avoid --exclude with -p flags in intra-crate integration tests
- *(test)* account for auto-discovered app static files in backward compatibility test

### Maintenance

- add workflow to auto-delete release-plz branches on PR close
- remove accidentally committed tfplan binary

### Other

- resolve conflict with main after explicit CoreSettings refactor
- resolve conflict with main (BUILTIN_FRAGMENTS + resolve helpers)
- resolve conflict with main (implicit inference tests from PR [[#2860](https://github.com/kent8192/reinhardt-web/issues/2860)](https://github.com/kent8192/reinhardt-web/issues/2860))
- resolve conflict with main (collectstatic test assertions + trusted proxy setup)
- resolve conflict with main in middleware.rs
- resolve conflict with main in workflow

### Styling

- *(infra)* apply terraform fmt to all .tf files
- *(i18n)* format method chain in po_parser tests
- reformat long lines in effect and burst modules
- apply rustfmt formatting
- apply rustfmt formatting fixes to examples and integration tests
- apply rustfmt formatting fixes to examples
- apply page! macro DSL formatting via fmt-all
- fix page! macro DSL formatting in relationship components

### Testing

- *(conf)* add comprehensive composable settings tests (12 categories, 120+ scenarios)
- *(macros)* add trybuild fail tests for #[settings] proc macro
- *(integration)* add use case tests for composable settings cross-crate interactions
- *(integration)* add macro pass tests for #[settings] proc macro
- *(integration)* verify SettingsBuilder flat keys map correctly via serde flatten
- *(settings)* update tests for explicit CoreSettings requirement

## [0.1.0-rc.13](https://github.com/kent8192/reinhardt-web/compare/reinhardt-web@v0.1.0-rc.12...reinhardt-web@v0.1.0-rc.13) - 2026-03-18

### Added

- *(commands)* extend InfraSignals with gRPC, storage, mail, session, graphql, admin, i18n detection

### Fixed

- *(commands)* align mail and session detection with workspace feature names

## [0.1.0-rc.12](https://github.com/kent8192/reinhardt-web/compare/reinhardt-web@v0.1.0-rc.11...reinhardt-web@v0.1.0-rc.12) - 2026-03-18

### Added

- *(testkit)* add postgres_with_migrations_from_dir helper using FilesystemSource
- *(di)* add Option<T> blanket Injectable impl for optional injection
- *(core)* auto-detect #[inject] without requiring use_inject = true
- *(auth)* add AuthInfo lightweight auth extractor
- *(auth)* add AuthUser<U> extractor with tuple struct destructuring
- *(auth)* add validate_auth_extractors startup DI validation

### Changed

- *(auth)* update re-exports and suppress deprecation warnings

### Deprecated

- *(testkit)* deprecate global_registry-based migration fixtures
- *(core)* deprecate collect_migrations! macro in favor of FilesystemSource
- *(conf)* mark Settings.installed_apps and related methods as deprecated

### Documentation

- add draft PR conversion protection policy
- add ergonomic auth extractors design spec
- *(auth)* use backticks instead of intra-doc links for cross-crate types
- *(macros)* use backtick for FilesystemSource in collect_migrations doc
- *(macros,testkit)* use backticks for cross-crate intra-doc links

### Fixed

- *(auth)* remove Uuid::nil() fallback on user_id parse failure

### Other

- incorporate main branch docs.rs fixes

### Styling

- *(testkit)* apply auto-fix formatting to fixtures re-export

## [0.1.0-rc.11](https://github.com/kent8192/reinhardt-web/compare/reinhardt-web@v0.1.0-rc.10...reinhardt-web@v0.1.0-rc.11) - 2026-03-16

### Fixed

- *(examples)* add missing feature flags for examples CI
- *(examples)* add missing feature flags for github-issues and rest-api examples

### Other

- resolve conflict with main for examples-database-integration

## [0.1.0-rc.10](https://github.com/kent8192/reinhardt-web/compare/reinhardt-web@v0.1.0-rc.9...reinhardt-web@v0.1.0-rc.10) - 2026-03-15

### Changed

- *(examples)* use reinhardt re-exports for serde and async_trait

### Documentation

- *(readme)* update version references to 0.1.0-rc.9
- *(examples)* update version references in CLAUDE.md to 0.1.0-rc.9
- *(website)* update reinhardt_version to 0.1.0-rc.9
- *(readme)* fix dispatch crate label and add missing components
- *(examples)* add new module re-exports to available re-exports
- update version references in crate READMEs to 0.1.0-rc.9
- *(instructions)* update outdated version references to 0.1.0-rc.9

### Fixed

- *(commands)* propagate openapi-router feature to reinhardt-commands
- *(commands)* gate docs banner on openapi-router feature

### Maintenance

- *(examples)* update workspace dependency to 0.1.0-rc.9

### Styling

- *(examples)* apply import order formatting for rc.9 compatibility

## [0.1.0-rc.9](https://github.com/kent8192/reinhardt-web/compare/reinhardt-web@v0.1.0-rc.8...reinhardt-web@v0.1.0-rc.9) - 2026-03-15

### Added

- *(infra)* add repository Terraform module for GitHub settings
- *(ci)* add infrastructure label and labeler mapping
- *(ci)* guard terraform-plan against fork PRs and add repository module
- *(ci)* add tfprovidercheck provider allowlist
- *(ci)* add semgrep terraform security rules
- *(ci)* add terraform-validate-fork workflow (Stage 1)
- *(ci)* add terraform-plan-privileged workflow (Stage 2)
- *(ci)* add terraform-apply workflow for post-merge automation
- expose reinhardt-query as reinhardt::query via database feature
- expose graphql, i18n, mail modules from reinhardt-web facade
- add grpc, dispatch, deeplink module re-exports with feature flags

### Changed

- *(ci)* migrate housekeeping runner to cancel-runner

### Documentation

- *(readme)* fix outdated versions and incorrect install command
- *(readme)* restructure sections for better first impression
- *(readme)* improve copywriting for broader audience appeal
- *(readme)* improve quick navigation and table readability
- *(auth)* fix private intra-doc link in get_user_info

### Fixed

- *(throttling)* use per-key state in leaky bucket throttle
- *(throttling)* use lazy initialization for per-key bucket state
- *(throttling)* prevent capacity overflow and add per-key isolation tests
- *(ci)* correct workflow configuration issues
- *(ci)* revert create-github-app-token to v2
- *(ci)* change auto-label-pr to pull_request_target for fork PR support
- *(ci)* address review findings in terraform security workflows
- *(ci)* add missing TF_VAR mappings for terraform plan workflows
- *(ci)* use secrets instead of vars for TF_GITHUB_OWNER and TF_GITHUB_REPOSITORY
- *(infra)* install aws cli in housekeeping runner userdata
- *(infra)* remove user_data from ignore_changes in housekeeping runner
- *(infra)* enable unattended-upgrades on housekeeping runner
- *(infra)* upgrade housekeeping runner from t4g.nano to t4g.micro
- *(ci)* add missing organizations_account_email to terraform plan workflows
- *(ci)* download lambda zip files before terraform plan
- *(ci)* download lambda zip files before terraform apply plan step
- *(ci)* downgrade upload-artifact from v7 to v4 in mutation-test workflow

### Maintenance

- *(ci)* exclude test code from CodeQL analysis
- add infra ownership to CODEOWNERS
- add docs/superpowers to .gitignore

### Other

- resolve conflicts with main in README.md
- resolve conflicts with main branch
- incorporate CI fix from fix/housekeeping-runner-instance-type

## [0.1.0-rc.8](https://github.com/kent8192/reinhardt-web/compare/reinhardt-web@v0.1.0-rc.7...reinhardt-web@v0.1.0-rc.8) - 2026-03-12

### Fixed

- *(ci)* prevent guard workflow cancellations from non-migration label events
- collapse nested if statements in start_commands to fix clippy lint
- *(commands)* update startapp test assertions for Rust 2024 module paths

### Maintenance

- update serena project.yml with new config options

## [0.1.0-rc.7](https://github.com/kent8192/reinhardt-web/compare/reinhardt-web@v0.1.0-rc.6...reinhardt-web@v0.1.0-rc.7) - 2026-03-11

### Documentation

- *(website)* update basis tutorial models to match examples
- *(website)* update basis tutorial server functions and components
- *(website)* update basis tutorial forms, testing, and static files
- *(website)* update rest tutorials to match examples

### Fixed

- *(urls)* suppress dead_code warning for WASM-only `merge` method
- *(prelude)* add feature gate for `UnifiedRouter` re-export
- *(ci)* add --no-tests=warn to ui-test nextest run
- *(ci)* handle nextest exit code 4 in coverage workflows
- *(ci)* move msrv-test to selectively-skippable in ci-success gate

### Maintenance

- *(nextest)* add --no-tests=warn to prevent empty partition failures

### Styling

- apply format fixes to src/lib.rs
- *(examples)* format examples-twitter common.rs
- *(examples)* format examples-twitter relationship components

## [0.1.0-rc.5](https://github.com/kent8192/reinhardt-web/compare/reinhardt-web@v0.1.0-rc.4...reinhardt-web@v0.1.0-rc.5) - 2026-03-07

### Added

- *(examples)* introduce Injected<T> usage in di-showcase

### Documentation

- *(stability)* relax SP-1 API freeze and add SP-6 non-breaking addition review
- *(claude)* add SP-6 non-breaking addition policy to quick reference
- *(pr)* add three-dot diff rule for PR verification (RP-5)
- *(pr)* replace Japanese text with English in RP-5

### Fixed

- *(ci)* enforce semver-check during RC phase instead of skipping
- *(ci)* remove non-existent paths from CODEOWNERS
- *(macros)* replace skeleton tests with meaningful assertions in pre_validate
- *(examples)* add force-link for library crate in di-showcase manage.rs
- *(ci)* prevent UI Tests from running when Phase 1 checks fail
- *(ci)* add missing validator dependency to reinhardt-test-support

### Maintenance

- *(labels)* add rc-addition label for SP-6 non-breaking additions
- *(semver)* update comments to reflect SP-1 relaxation policy
- *(serena)* clean up project.yml formatting
- *(template)* add self-hosted runner checkbox to PR template
- require PR checkbox opt-in for self-hosted runner selection

### Testing

- *(db)* add field mapping and migrations integration tests

## [0.1.0-rc.4](https://github.com/kent8192/reinhardt-web/compare/reinhardt-web@v0.1.0-rc.3...reinhardt-web@v0.1.0-rc.4) - 2026-03-05

### Documentation

- *(website)* update admin customization tutorial to use separate admin struct pattern

### Fixed

- *(core)* add wasm32 platform gate to parallel and jsonschema validator modules

## [0.1.0-rc.3](https://github.com/kent8192/reinhardt-web/compare/reinhardt-web@v0.1.0-rc.2...reinhardt-web@v0.1.0-rc.3) - 2026-03-04

### Fixed

- *(commands)* correct project template compilation errors
- *(commands)* correct app template compilation errors

## [0.1.0-rc.2](https://github.com/kent8192/reinhardt-web/compare/reinhardt-web@v0.1.0-rc.1...reinhardt-web@v0.1.0-rc.2) - 2026-03-04

### Changed

- *(ci)* remove redundant flags from cargo check task

### Documentation

- add agent-detected bug verification policy (SC-2a, IL-3)
- *(rest)* align REST tutorial docs with actual API
- *(basis)* align basis tutorial docs with actual API
- align cookbook and quickstart docs with actual API

### Fixed

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
- *(ci)* allow publish-check to be skipped on release-plz branches
- *(ci)* handle cargo metadata failure and jq errors in detect-affected-packages.sh
- *(ci)* use git log to detect changed files in PR branches that contain main
- *(ci)* use origin/HEAD_REF instead of HEAD to detect changed files in PRs
- *(ci)* resolve permanent cache miss in setup-rust action
- *(ci)* remove shell quoting bug in nextest filter expression passing

### Maintenance

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

### Other

- resolve fields.rs conflict with main

### Styling

- *(urls)* apply project formatting to pattern module

## [0.1.0-alpha.19](https://github.com/kent8192/reinhardt-web/compare/reinhardt-web@v0.1.0-alpha.18...reinhardt-web@v0.1.0-alpha.19) - 2026-02-24

### Documentation

- add official website link to Quick Navigation
- update internal documentation links to official website URLs
- remove repository-hosted documentation migrated to reinhardt-web.dev

### Fixed

- *(website)* set cloudflare pages production branch to main before deploy
- *(website)* add workflow_dispatch trigger for manual deployment
- *(website)* add DNS records for custom domain resolution
- *(infra)* add import blocks for existing Cloudflare resources
- *(db)* gate sqlite-dependent tests with feature flag
- *(db)* replace float test values to avoid clippy approx_constant lint

### Testing

- *(db)* add warning log test for .sql file detection

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
