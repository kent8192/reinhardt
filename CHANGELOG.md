# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.3.7](https://github.com/kent8192/reinhardt-web/compare/reinhardt-web@v0.3.6...reinhardt-web@v0.3.7) - 2026-08-07

### Documentation

- add release announcement(s)
- *(ci)* explain intra-crate coverage reports
- *(ci)* clean focused coverage artifacts

### Fixed

- *(ci)* align intra-crate coverage report target
- *(ci)* keep intra-crate coverage upload fail-closed

### Maintenance

- *(ci)* merge main into coverage repair branch

### Testing

- *(ci)* reject empty intra-crate coverage
- *(storages)* raise coverage above 75%
- *(storages)* isolate cross-crate coverage tests
- *(storages)* strengthen integration error assertions
- *(storages)* assert mapped HTTP error details

## [0.3.6](https://github.com/kent8192/reinhardt-web/compare/reinhardt-web@v0.3.5...reinhardt-web@v0.3.6) - 2026-08-04

### Fixed

- *(orm)* preserve numeric derived primary key bindings
- *(orm)* preserve manual numeric primary key bindings
- *(orm)* retain primary key newtype compatibility
- *(ci)* report workspace coverage to Codecov
- *(ci)* select every package for coverage reports
- *(ci)* align base64 dependency policy skip

### Maintenance

- merge main into primary key binding fix

### Security

- *(orm)* preserve exact custom primary key bindings

## [0.3.5](https://github.com/kent8192/reinhardt-web/compare/reinhardt-web@v0.3.4...reinhardt-web@v0.3.5) - 2026-08-02

### Documentation

- add release announcement(s)

### Fixed

- *(ci)* restore Codecov LCOV uploads
- *(ci)* enable sccache for Rust builds
- *(grpc)* restore facade-only handler DI
- *(grpc)* support package-only facade dependencies
- *(grpc)* preserve explicit facade aliases

## [0.3.4](https://github.com/kent8192/reinhardt-web/compare/reinhardt-web@v0.3.3...reinhardt-web@v0.3.4) - 2026-07-30

### Documentation

- add release announcement(s)

### Fixed

- *(ci)* stream auto-fix additions from staged blobs
- *(orm)* preserve typed keys in manager deletion

## [0.3.3](https://github.com/kent8192/reinhardt-web/compare/reinhardt-web@v0.3.2...reinhardt-web@v0.3.3) - 2026-07-28

### Fixed

- *(ci)* allow auto-fix on non-fast-forward-only branches
- *(ci)* paginate auto-fix branch-rule checks
- *(ci)* allow syn major-version duplicates
- *(orm)* reject oversized queryset bind values
- *(orm)* keep UUID-looking filters as text
- *(orm)* bind typed temporal filter values
- *(orm)* retain UUID filter bindings
- *(admin)* support typed filter values
- *(ci)* allow base64 0.23 duplicate on main
- *(ci)* inherit base64 duplicate policy

### Maintenance

- remove standalone SemVer Check workflow

## [0.3.2](https://github.com/kent8192/reinhardt-web/compare/reinhardt-web@v0.3.1...reinhardt-web@v0.3.2) - 2026-07-14

### Fixed

- *(pages)* contain nested reactive DOM ownership
- *(auth)* reject generic OIDC endpoint redirects
- *(tooling)* remove tracked Serena project configuration
- *(commands)* remove redundant rustdoc link target
- *(ci)* remove tracked ignored release input
- *(ci)* allow intentional dependency-version duplicates
- *(ci)* allow compatible duplicate dependency patches
- *(tests)* use fallback service for SPA fixtures
- *(ci)* tolerate spin patch updates

### Other

- resolve main conflicts in nested reactive ownership

## [0.3.1](https://github.com/kent8192/reinhardt-web/compare/reinhardt-web@v0.3.0...reinhardt-web@v0.3.1) - 2026-07-04

### Added

- add frontend framework benchmark suite

### Fixed

- *(benchmarks)* correct frontend benchmark measurements
- *(ci)* pin standalone examples time resolution
- *(ci)* unblock quick-xml security audit
- *(ci)* adapt XML parser to quick-xml 0.41
- *(formatter)* normalize admin feature page closures
- *(ci)* format basis tutorial page closures

## [0.3.0](https://github.com/kent8192/reinhardt-web/compare/reinhardt-web@v0.2.0...reinhardt-web@v0.3.0) - 2026-06-28

Stable 0.3.0 is the first stable release of the Reinhardt 0.3 line. It
promotes the 0.3.0 release-candidate train into one upgrade story:
retire the deprecated 0.2 compatibility APIs, make Pages, Manouche,
and server-function contracts explicit, stabilize ORM and migration
metadata, and finish the release documentation required for stable
publication.

This release is not a patch-style rollup. It is a migration release for
applications that stayed on 0.2.x while the 0.3 APIs stabilized. The
section below is a curated stable summary of the prerelease train and
final release-polish changes.

### Upgrade Impact

| Area | What changes for application maintainers |
|---|---|
| Public API cleanup | Remove deprecated 0.2 compatibility extractors, Pages resource constructors, and effect-event hooks. |
| Pages and Manouche | Update page bodies, hooks, client routes, server-function endpoint resolution, and WASM-only helper usage to the final 0.3 contracts. |
| ORM and migrations | Review relation-shaped model metadata, conditional partial updates, renamed-field detection, database defaults, and replayed foreign-key constraints. |
| Scaffolding and tutorials | Regenerate Pages projects with the 0.3 scaffold defaults, SQLite tutorial setup, target-neutral modules, and updated quickstart feature sets. |
| Routing, HTTP, and admin | Adopt the refined cookie extractor, PathParams, request allocation, admin list-count, and route helper behavior stabilized during the RC train. |
| Tooling and release flow | Use the Rust 1.96 toolchain, updated formatter/tree-sitter release grouping, public API doc coverage, and hardened release-plz dry-run checks. |

### Release Highlights

- **The 0.2 compatibility surface is removed.** Projects should migrate
  to `CurrentUser`, `use_resource(fetcher, deps)`, `use_callback`, and
  explicit signal reads in effects before upgrading.
- **Pages and WASM contracts are final for the 0.3 line.** Client page
  helpers, server-function endpoint resolution, scaffolded route gates,
  and tutorial layouts now match the stable target model.
- **ORM and migration metadata are stricter.** Relation-shaped model info,
  conditional partial updates, renamed-field handling, database defaults,
  CockroachDB serialization, and replayed foreign-key constraints are all
  represented in the stable release train.
- **Runtime hot paths were tightened.** Request allocation, router
  allocation, database pool acquisition, migration graph planning, and
  admin list-count queries received targeted performance work.
- **Release readiness was hardened.** Public API TODO audits, docs, CI
  cache behavior, formatter output, release-plz classification, and
  tutorial verification were aligned for the stable publication path.

### Breaking Changes

- **Deprecated 0.2 compatibility APIs are removed.** This includes the
  `AuthUser<U>` extractor, deprecated Pages resource constructors, and
  effect-event hooks. Use `CurrentUser<U>`, `use_resource(fetcher, deps)`,
  `use_callback`, and explicit untracked signal reads instead.
- **Pages code should target the final 0.3 hook and route contracts.**
  Regenerate scaffolded Pages apps and review client route gates, server
  function endpoint resolution, and WASM-only helper usage.
- **ORM and migration diffs may become more explicit.** Relation-shaped
  model metadata, renamed fields, database defaults, and replayed foreign
  keys are now represented in the stable migration surface.

### Added

- *(website)* add technical notes domain
- *(macros)* make user macro inert on wasm
- *(orm)* add QuerySet conditional partial updates
- feat!(macros): expose relation-shaped model info fields
- *(formatter)* add semantic page grammar nodes
- *(formatter)* rustfmt page expression islands

### Changed

- *(pages)* derive fetch credentials default
- *(scaffolding)* align Pages app layout
- *(admin)* use keyed depends for server injections

### Removed

- **BREAKING**: Removed the 0.2 compatibility extractor `AuthUser<U>`.
  Use `CurrentUser<U>` for full authenticated-user extraction.
- **BREAKING**: Removed the deprecated Pages resource constructors
  `create_resource` and `create_resource_with_deps`. Use
  `use_resource(fetcher, deps)`.
- **BREAKING**: Removed the deprecated Pages hooks `use_effect_event` and
  `use_effect_event_with`. Use `use_callback` / `use_callback_with` or read
  latest signal values with `.get_untracked()` inside effects.

### Fixed

- *(db)* replay foreign key constraints and defaults
- *(bench)* avoid unused server error bindings
- *(db)* apply alter column defaults in migrations
- *(commands)* align pages tutorial scaffold
- *(commands)* address scaffold review feedback
- *(commands)* include auth tutorial scaffold features
- *(migrations)* handle field changes on renamed models
- *(migrations)* address autodetector review gaps
- *(macros)* stop propagating serde skip to Info relation fields
- *(scaffolding)* default pages projects to sqlite
- *(ci)* quote release-plz dry-run regex
- *(ci)* satisfy server router clippy lint
- *(commands)* repair pages quickstart scaffold defaults
- *(infra)* import website DNS records
- *(ci)* restrict wasm cache seed to main pushes
- *(views)* wire declared viewset serializers
- *(tests)* align viewset serializer fixtures
- *(views)* align viewset serializer examples
- *(db)* suppress replayed migration drift
- *(ci)* pin brotli allocator dependency
- *(examples)* align polls info fixtures with relations
- *(formatter)* handle reviewed page rustfmt islands
- add native stubs for client page functions
- *(pages)* export client_page from prelude
- *(di)* honor cache false for keyed wrappers
- *(ci)* satisfy pages clippy lint
- *(pages)* re-export use_callback_with from reactive::hooks

### Documentation

- add release announcement(s)
- *(release)* expose 0.3 migration guide
- *(pages)* document WASM server fn resolver
- *(tutorial)* fix SQLite poll seed snippet
- *(tutorial)* align 0.3.0 tutorial scaffolding
- *(tutorial)* address 0.3.0 review feedback
- *(tutorial)* gate client route modules
- *(tutorial)* remove native client route specs
- *(tutorial)* align pages scaffolding route gates
- *(formatter)* document page rustfmt islands

### Testing

- *(migrations)* tighten autodetector assertions
- *(examples)* guard polls question FK migration type
- *(pages)* isolate static resolver state

### Styling

- *(examples)* format tutorial basis pages
- format di exports

### Maintenance

- merge main into develop/0.3.0
- merge develop 0.3.0 into build-time perf branch
- merge latest develop 0.3.0 into build-time perf branch
- *(release)* group tree-sitter crates
- merge develop/0.3.0 into pr 5346

### Migration Guide

- Follow [`instructions/MIGRATION_0.3.md`](instructions/MIGRATION_0.3.md) for
  the 0.2 to 0.3 compatibility-removal checklist.

## [0.2.0](https://github.com/kent8192/reinhardt-web/compare/reinhardt-web@v0.1.3...reinhardt-web@v0.2.0) - 2026-06-11

Stable 0.2.0 is the first release of the Reinhardt 0.2 line. It
promotes the 0.2.0 release-candidate train into one upgrade
story: remove the 0.1.x compatibility layer, adopt the final Manouche
v2 page/form model, move application configuration to typed settings
fragments, and make routing, testing, and local development more
explicit.

This release is not a patch-style rollup. It is a migration release for
applications that stayed on 0.1.x while the 0.2 APIs stabilized. The
section below is a curated stable summary of the prerelease train and
final release-polish changes.

### Upgrade Impact

| Area | What changes for application maintainers |
|---|---|
| Pages and forms | Update `page!` bodies, hooks, `use_resource`, `use_form`, dynamic fields, and component invocation to the Manouche v2 contract. |
| Routing | Replace generated typed URL resolver surfaces with explicit named reverse lookups and app-local URL helpers. |
| Auth and DI | Move to application-owned `#[user]` models, `CurrentUser<U>`, `Depends<T>`, and final auth identity traits. |
| Settings | Replace ad-hoc `XxxConfig` / legacy settings APIs with composed `#[settings(fragment = true)]` structures. |
| ORM and migrations | Update filter calls, model builders, generated Info DTO usage, and migration review expectations. |
| Test support | Move server-function and auth tests to MSW-backed mocks, fluent auth helpers, and directory-backed migration fixtures. |

### Release Highlights

- **Manouche v2 becomes the stable page/form model.** `page!` now wraps
  expressions and control flow reactively, hooks use explicit dependency
  tuples, component invocation uses the brace syntax, `use_resource`
  replaces the split resource-hook surface, and `use_form` is driven by
  form definitions.
- **Routing is intentionally more explicit.** The old generated
  `ResolvedUrls` / resolver trait surface is removed in favor of
  fully-qualified route names, `reverse(...)`, and small app-local
  wrapper functions. Client route helpers collapse to arity-inferred
  `route_path`.
- **Settings fragments are the configuration contract.** Auth, tasks,
  server, gRPC, deeplink, websockets, middleware, mail, templates, and
  embedded settings nodes now compose through typed fragments. Secret
  fields accept environment and file-backed source maps.
- **ORM and migration APIs are stricter but easier to compose.**
  Query filters take one filter expression, Django-style lookup helpers
  and composite combinators are available, generated model builders can
  override macro-managed fields where needed, and reverse migration SQL
  can emit multiple backend-specific statements.
- **The browser and WASM testing surface is more realistic.** WASM
  server-function tests resolve endpoints against the browser document
  URL, the MSW harness matches reqwest's WASM backend behavior, SPA link
  rerendering and dynamic radio choices are fixed, and admin browser CRUD
  is wired through the tutorial app.
- **The local development loop is materially faster.** Hot reload now
  chooses the rebuild target from the changed files, static page edits
  can hot-patch without a full rebuild, browsers reload only after a
  successful rebuild, and build-loop benchmarks track cold, server,
  Pages WASM, and HMR paths.
- **Project scaffolding is closer to real projects.** `startproject`
  supports interactive Reinhardt version and feature selection,
  `reinhardt-admin configure` can update facade dependency settings, and
  `manage infra` can provision local PostgreSQL and Redis containers
  while keeping `.reinhardt/local-infra.json` out of generated projects.

### Breaking Changes

- **URL routing**: typed URL helper generation from `#[routes]`,
  `ResolvedUrls`, `url_prelude`, `UrlResolverUnprefixed`, flat route
  accessor traits, and numbered client route helpers are removed. Use
  explicit reverse lookups such as `reverse("server:app:name", params)`
  and app-local wrappers.
- **Pages and forms**: `page!` now wraps dynamic expressions reactively,
  rejects implicit outer captures, removes bare-identifier shorthand,
  and expects explicit dependency tuples for React-style hooks.
  `create_resource*` is superseded by `use_resource(fetcher, deps)`,
  and `use_form` is routed through form definitions.
- **Dependency injection**: `Injected<T>` and `OptionalInjected<T>` are
  removed in favor of `Depends<T>` and `Option<Depends<T>>`.
- **Authentication**: old `User`, `SimpleUser`, `AnonymousUser`,
  `DefaultUser`, and compatibility extractor shapes are removed. Use
  application-owned `#[user]` models, `AuthIdentity`, `BaseUser` /
  `FullUser`, `PermissionsMixin`, and `CurrentUser<U>`.
- **Configuration**: legacy `Settings`, `AdvancedSettings`,
  `JsonFileSource`, `auto_source`, and mutable interpolation APIs are
  removed in favor of composed settings structs and `TomlFileSource`.
- **Database/query**: filter APIs take a single filter expression,
  `SeaRc<T>` is replaced by `SharedRc<T>`, and reverse migration SQL may
  return multiple statements.
- **Testing**: old fetch/server-function mocks, built-in `TestUser`,
  `force_authenticate`, and global-registry migration fixtures are
  replaced by MSW-backed mocks, test-local users, fluent auth helpers,
  and directory-backed migration fixtures.
- **Storage**: `StorageError` is non-exhaustive; downstream matches need
  wildcard arms.

### Documentation

- add release announcement(s)
- align RC website docs.rs links
- align CLI install version examples
- *(build)* update wasm loop measurements
- *(storages)* update test documentation to reflect wiremock replacement
- *(admin)* remove broken DefaultUser intra-doc links
- *(pages)* document spawn compat shim module
- *(pages)* make wasm spawn_task example testable (ignore -> no_run)
- mandate RAII pattern for resource management
- *(wiki)* distribute Obsidian pages across categories and raise capture frequency
- *(wiki)* sync CLAUDE.md/AGENTS.md Obsidian section with OW-7 policy
- *(reinhardt-db)* fix QuerySet doctests for single-argument filter() API
- *(reinhardt-db)* qualify Filter path in with_db doctests
- *(mail,conf)* fix unresolved intra-doc links to settings fragments
- *(deeplink)* document #![allow(deprecated)] allowances
- *(tasks)* note that create_queue_from_settings does not retain settings
- *(tasks)* correct tracking issue reference to [[#5068](https://github.com/kent8192/reinhardt-web/issues/5068)](https://github.com/kent8192/reinhardt-web/issues/5068)

### Styling

- apply formatter fixes across workspace
- format files from merge resolution
- apply rustfmt to non-DSL files on develop/0.2.0
- *(pages)* reorder form component imports to satisfy rustfmt

### Other

- resolve conflicts with develop/0.2.0

### Migration Guide

Follow [`instructions/MIGRATION_0.2.md`](instructions/MIGRATION_0.2.md)
as the canonical 0.1.x to 0.2.0 checklist. The safest order is: update
dependencies, remove deprecated API references, update ORM/query calls,
migrate touched config to settings fragments, regenerate and review
database migrations, then run the verification commands in the guide.

### Added

- Django-like ORM lookup helpers and composite filter combinators.
- Settings fragments and settings-first constructors across auth, tasks,
  server, gRPC, deeplink, websockets, middleware, mail, and templates.
- Manouche v2 component syntax, typed form field generics, `use_resource`,
  hook dependency tracking, and server-function metadata available across
  targets.
- Storage backends and test coverage for local, S3-compatible, GCS, and
  Azure-style storage flows.
- Interactive admin dependency configuration and refreshed project
  templates for 0.2.0 projects.
- Generated model-info companion types are exported for WASM targets so
  tutorial and admin-style flows can share the same model metadata.
- *(commands)* add interactive dependency configuration
- *(orm)* add Django-like lookup helpers
- *(orm)* support composite filter combinators
- *(storages)* [**breaking**] add #[non_exhaustive] to StorageError
- *(tasks)* add settings fragments and settings-first constructors
- *(server)* add RateLimitSettings fragment
- *(grpc)* add GrpcServerSettings fragment for the grpc_server section
- *(deeplink)* add DeeplinkSettings fragment
- *(websockets)* add settings fragments for connection, reconnection, origin, rate limit, and redis
- *(auth)* add settings fragments for session, jwt, token rotation
- *(middleware)* bridge CorsConfig to CorsSettings fragment
- feat!(forms): route use_form through form definitions

### Changed

- URL routing now prefers explicit route names and reverse lookup over
  generated typed resolver surfaces.
- Auth extraction standardizes on `CurrentUser<U>` while keeping
  `AuthUser<U>` as a deprecated 0.2 compatibility wrapper.
- `Model::new()` and generated model builders align with the final 0.2
  model-construction contract.
- Formatter responsibilities are split out of the admin CLI and routed
  through the published `reinhardt-formatter` crate.
- Tutorial route contracts are aggregated or inlined where appropriate so
  the example apps match the final route and page-template structure.
- *(auth)* make CurrentUser canonical extractor
- *(pages)* unify spawn into platform/, expose spawn_task from prelude

### Deprecated

- Compatibility wrappers that still exist for the 0.2 cycle are retained
  only as migration aids and are documented for removal in a later train.
- Legacy config structs are deprecated where settings fragments provide
  the final contract.
- *(tasks)* deprecate config structs in favor of settings fragments
- *(conf)* deprecate TemplateConfig in favor of TemplateSettings fragment
- *(server)* deprecate RateLimitConfig in favor of RateLimitSettings
- *(grpc)* deprecate GrpcServerConfig in favor of GrpcServerSettings
- *(deeplink)* deprecate DeeplinkConfig in favor of DeeplinkSettings
- *(websockets)* deprecate ad-hoc XxxConfig structs in favor of settings fragments
- *(auth)* deprecate SessionConfig, JwtConfig, TokenRotationConfig
- bridge SmtpConfig to the EmailSettings fragment
- shield smtp_integration test from SmtpConfig deprecation

### Fixed

- WASM and feature-boundary failures in pages, auth, urls, test support,
  and release fixtures.
- Form runtime parity, dynamic radio choices, SPA link rerendering, SSR
  hydration IDs, and reactive mount borrow handling.
- Admin formatter wiring, migration fixture preservation, and project
  template dependency wiring.
- Migration generation, model companion derives, query expectations, and
  backend-specific SQL behavior.
- Empty-form focus handling no longer emits unreachable focus paths.
- Project templates now include pages app reverse helpers, collectstatic
  no-input defaults, tutorial-safe model placeholders, and sqlite database
  ignores.
- Release-branch CI is stabilized against the aws-runtime event-stream
  signer issue and broken upstream transitive releases.
- *(website)* treat PR previews as preview channel
- *(website)* point stable selector to dev channel
- *(ci)* install node before website deploy
- *(ci)* repair admin dependency config checks
- *(build)* address CodeRabbit review feedback
- *(commands)* adapt hot reload tests for develop
- *(build)* port Codex review follow-ups
- *(build)* port strict hot patch regression assertion
- *(ci)* tolerate develop semver and wasm gate noise
- *(orm)* address lookup review edge cases
- *(db)* align LIKE escape SQL expectations
- document wasm router stubs
- *(staticfiles)* inject wasm loader for directory index
- *(staticfiles)* preserve raw index in non-spa mode
- *(staticfiles)* inject wasm loader for directory index without spa mode
- *(conf)* support secret source maps
- *(conf)* escape secret source test paths
- *(ci)* stop masking release-plz 422 failures
- *(pages)* enable security feature for WASM builds
- address CodeRabbit review comments
- address remaining CodeRabbit comments
- address Copilot review comments
- address follow-up CodeRabbit comments
- *(ci)* recover develop release-plz prerelease
- *(auth)* [**breaking**] migrate internal consumers from removed User/SimpleUser types
- *(auth)* migrate integration tests from removed auth types
- *(auth)* replace non-existent BackendError with DatabaseError in tests
- *(auth)* address CodeRabbit review feedback
- *(auth)* replace InternalUser in UserManager public API with ManagedUser
- *(auth,urls,pages)* remove stale references and fix latent clippy lints
- *(urls)* migrate reverse tests from removed panicking helpers to try_ variants
- *(pages)* restore brace-form component invocation tests
- *(templates)* restore breaking change dropdowns to issue templates
- *(ci)* resolve all pre-existing compilation failures on develop/0.2.0
- *(tests)* remove duplicate TestUser definition in mfa_integration
- *(admin-cli)* restore Topiary formatter wiring from main
- *(admin-cli)* run rustfmt on DSL-skipped files in fmt command
- *(admin-cli)* pass ignore-all files through rustfmt in fmt command
- *(macros)* propagate serde derives to Info companion struct via model_config
- *(ci)* update WASM consumer fixture for URL routing simplification
- *(macros)* remove unused has_derive_trait from model_derive
- *(ci)* guard WASM-unused exports and restrict compat visibility
- *(di)* collapse nested if-let into let-chain
- *(urls)* update tests for page() and reverse() API changes
- *(macros)* suppress missing_docs on generated Info companion types
- *(storages)* replace LocalStack with wiremock mock S3 server
- *(storages)* address CodeRabbit review feedback
- *(ci)* update test snapshots and assertions for v0.2.0 breaking changes
- *(pages)* remove redundant #[builder(default)] from Option field
- *(ci)* gate develop release-plz publish on release PR merges
- *(admin-cli)* revert rustfmt-damaged migrate_v2 fixtures
- *(admin-cli)* update migrate_v2 expected fixtures to match prettyplease output
- *(pages)* add missing and regenerate stale trybuild .stderr files
- *(test)* regenerate manager_wrong_model trybuild stderr
- *(pages)* remove component_missing_required_prop compile-fail test
- *(pages)* correct component_missing_required_prop compile-fail test
- *(pages)* use brace-form Card {} inside page! for required-prop test
- *(admin-cli)* preserve migrate_v2 fixtures during fmt-all
- *(pages)* document #[allow(dead_code)] on CardProps::item in compile-fail test
- *(core)* dispose Memo only on last clone drop
- *(pages)* make SSR hydration IDs render-scoped
- *(examples-twitter)* import serde directly in WASM-reachable pagination
- *(examples-twitter)* align client SPA with develop/0.2.0 page!/form! API
- *(pages)* keep deprecated reinhardt_pages::spawn re-export shim
- *(storages)* escape #[settings] in deprecation notes for rustdoc
- *(storages)* gate gcs/azure integration tests behind their features
- *(web)* restore #[cfg(native)] gating on the misc export module
- shield downstream consumers of newly deprecated config types
- complete downstream shielding for deprecated config re-exports
- *(deeplink)* derive Default for DeeplinkSettings
- *(testkit)* shield server fixtures from deprecated RateLimitConfig
- split formatter from admin cli
- route fmt cargo-make tasks to formatter
- *(release)* publish reinhardt-formatter
- repair release examples tests
- *(examples)* resolve release candidate locally
- *(examples)* update UnoCSS shells
- *(commands)* update pages template CDN
- *(commands)* align wasm bindgen template
- *(pages)* avoid reentrant reactive mount borrow
- *(pages)* rerender SPA links after cleanup
- *(pages)* render dynamic radio choices
- *(examples)* render basis tutorial vote choices
- *(examples)* restore basis poll choice layout
- *(mail)* accept settings email fragments
- *(conf)* [**breaking**] remove legacy advanced settings types
- *(conf)* emit fragment self settings impls
- *(forms)* address bot review feedback
- *(examples)* render basis tutorial poll vote choices after loading question detail data.
- *(test)* expose the MSW testing facade on WASM builds when the `msw` feature is enabled.

### Security

- Storage integration tests moved away from LocalStack-only assumptions
  and now use deterministic mock servers where appropriate.
- Auth permission tests no longer depend on minute-precision wall-clock
  boundaries.
- URL, redirect, CSRF, and HTML-safety primitives from the 0.1 line remain
  part of the stable security surface.

### Performance

- Hot reload skips unrelated rebuilds, reuses pages wasm artifacts when
  stale checks allow it, and notifies browsers after rebuilds.
- Generated page attributes are batched, unused runtime parser dependencies
  are pruned, and non-browser wasm modules are feature-gated out.
- Build-loop, pages wasm, server-loop, hot reload, and cold workspace
  measurements informed the final dev-profile defaults.
- *(build)* add build-loop benchmark harness
- *(build)* tune dev profile for incremental builds
- *(commands)* skip unrelated hot reload rebuilds
- *(commands)* notify browsers after hot reload rebuilds
- *(build)* keep measured dev profile defaults
- *(commands)* use staleness check for pages wasm reuse
- *(build)* measure pages wasm and server loops
- *(pages)* batch generated page attributes
- *(build)* measure hot reload target selection
- *(pages)* prune unused runtime parser deps
- *(pages)* trim wasm dependency graph
- *(pages)* gate non-browser wasm modules
- *(pages)* hot patch static page edits
- *(build)* tune dev profile for hot reload
- *(build)* measure cold workspace build

### Maintenance

- Release-plz handling for develop trains, branch naming, publish checks,
  stale generated release branches, and release announcements was hardened.
- Public API documentation coverage, docs.rs links, website channel routing,
  and release website deployment were aligned for stable publication.
- Release announcements and tutorial documentation were synchronized with
  route-contract, settings-fragment, typed-form, generated model-info, and
  pages-template guidance.
- Examples and generated templates were refreshed against the local 0.2.0
  workspace instead of published RC assumptions.
- Temporary vendored AWS runtime overrides were removed once the release
  train no longer needed them.
- trigger website deploy workflow changes
- *(commands)* ignore local infra state in templates
- remove examples-twitter from examples test workflow
- create release announcement PRs for develop trains
- run announcement posts for develop merges
- cancel stale reusable workflow runs
- forward merge main v0.1.1 changes into develop 0.2.0
- include all main v0.1.1 PR changes
- forward merge main v0.1.2 changes into develop 0.2.0
- *(examples)* WASM-build the example library to catch client SPA drift
- *(tasks)* add reinhardt-conf and reinhardt-core dependencies for settings
- add reinhardt-conf and serde deps for rate-limit settings
- *(grpc)* add reinhardt-conf, reinhardt-core, serde deps for settings fragment
- *(deeplink)* add reinhardt-conf dependency for settings fragments
- *(websockets)* depend on reinhardt-conf for settings fragments
- *(auth)* add reinhardt-conf dependency for settings fragments
- regenerate example migrations
- *(ci)* merge develop into release docs fix

### Testing

- Release CI expectations, WASM fixtures, trybuild output, migration
  boundaries, HMR reload behavior, and auth clock-boundary tests were
  refreshed for the stable line.
- Tutorial basis CI now exercises the polling tests against the fixed
  runtime expectations.
- *(commands)* verify hmr reload after rebuild
- *(ci)* refresh release CI expectations
- *(auth)* remove time-based permission clock flake
- *(pages)* address CodeRabbit review on hydration tests
- *(pages)* replace skeleton spawn_task test with behavior assertion
- *(forms)* align form runtime UI fixtures

## [0.1.2](https://github.com/kent8192/reinhardt-web/compare/reinhardt-web@v0.1.1...reinhardt-web@v0.1.2) - 2026-05-25

### Documentation

- add release announcement(s)

### Fixed

- *(ci)* add reinhardt-testkit-macros to release-plz version group

## [0.1.1](https://github.com/kent8192/reinhardt-web/compare/reinhardt-web@v0.1.0...reinhardt-web@v0.1.1) - 2026-05-24

### Added

- *(admin-cli)* scaffold migrate-manouche-v2 subcommand
- *(admin-cli)* codemod pipeline scaffolding (walker + rule trait + review fixes)
- *(admin)* add form! macro DSL formatting support

### Fixed

- *(templates)* add server_only, no_ws_resolvers to RESTful project url template
- *(admin)* preserve blank lines inside page! macro DSL when formatting
- *(admin)* preserve comments and blank lines in codemod rewriting
- *(admin)* use text-based item search for codemod formatting preservation
- *(admin-cli)* address CodeRabbit review on form! detection, char/lifetime scan, temp file, and codemod rules
- *(admin-cli)* use unique temp filename in target directory for atomic rename
- *(admin-cli)* ensure temp file cleanup runs on rename failure
- *(admin)* clean up temp file when std::fs::write fails in write_developer_file
- *(admin-cli)* revert version to 0.1.0
- *(ci)* strengthen release-plz publish gate with branch name and PR label verification
- *(ci)* quote if: conditions to prevent YAML tag parsing of !startsWith
- *(ci)* exclude merge-commit trigger from release-plz release gate
- *(ci)* quote if: conditions containing # to prevent YAML comment parsing
- *(admin-cli)* resolve formatting issues in fmt-all output
- *(admin-cli)* remove invalid callbacks wrapper from formatter
- *(admin-cli)* skip rustfmt for closures containing page!/form! macros
- *(admin-cli)* use rustfmt directly for closures with page!/form! macros
- *(reinhardt-admin-cli)* add page! macro protection in closure and handler expression formatting
- *(reinhardt-admin-cli)* emit form! DSL syntax for wrapper, icon, icon_position fields
- *(reinhardt-admin-cli)* add form! token preprocessing to convert internal AST to DSL
- *(reinhardt-admin)* strip trailing commas from Icon attrs before merging children
- *(reinhardt-admin-cli)* fix off-by-one bounds check in parse_wrapper_inner and parse_icon_inner
- *(ci)* resolve Rust 1.94 clippy failures

### Styling

- *(admin)* fix indentation in write_developer_file write-error handler
- *(admin-cli)* apply rustfmt to migrate_v2 codemod tests
- apply fmt-all with updated formatter
- apply fmt-all to convert page!/form! blocks to DSL syntax
## [0.1.0](https://github.com/kent8192/reinhardt-web/compare/reinhardt-web@v0.1.0-rc.30...reinhardt-web@v0.1.0) - 2026-05-22

First stable release of `reinhardt-web`, after 19 alpha and 30 rc
prereleases dating back to 2026-01-23. The entry below is a curated
feature-level summary; per-prerelease commit history is available in
the [Release-category Discussions](https://github.com/kent8192/reinhardt-web/discussions/categories/release).

### Highlights

- **Type-safe, convention-driven framework**: reinhardt-web brings
  Django-like ergonomics to Rust with fully typed URL routing,
  dependency injection, and form validation that fail at compile time,
  not runtime.
- **Full-stack WASM SPA support**: build reactive client UIs with
  Server Functions that auto-serialize across the network boundary,
  complete with CSRF protection and pluggable JWT / session
  authentication.
- **Pragmatic admin interface**: the built-in admin panel supports
  role-based permissions, type-safe query filters, and integrated ORM
  operations without separate admin declarations — just `#[model]` and
  opt in.
- **Async-first, runtime-agnostic**: every public API is async by
  default and integrates cleanly with Tokio, with TestContainers-backed
  integration tests for PostgreSQL, MySQL, SQLite, and CockroachDB.

### Breaking Changes

The list below names every breaking change introduced across the
alpha / rc lifecycle. Each entry links to its announcement in the
[Breaking Changes Discussions](https://github.com/kent8192/reinhardt-web/discussions/categories/breaking-changes);
follow the **Migration Guide** below for the most disruptive moves.

- **Dependency injection unifies on `Depends<T>`** ([#3628](https://github.com/kent8192/reinhardt-web/discussions/3628)) — `#[inject]` no longer accepts `Arc<T>` directly; use `Depends<T>` so the framework can cache resolution, detect cycles, and surface DI metadata.
- **`Injected<T>` deprecated** ([#3631](https://github.com/kent8192/reinhardt-web/discussions/3631)) — replaced by `Depends<T>`; the auto-`Clone` bound is removed.
- **`#[url_patterns]` becomes typed** ([#3770](https://github.com/kent8192/reinhardt-web/discussions/3770)) — accepts `InstalledApp::*` identifiers with `mode = server|client|unified`; pattern functions are renamed accordingly.
- **`#[viewset]` macro and route mounting** ([#4476](https://github.com/kent8192/reinhardt-web/discussions/4476)) — per-app `server_fn` and client UI now live under `apps/<app>/`; `commands/templates/...` no longer carries handler code.
- **URL resolver directory layout** ([#3918](https://github.com/kent8192/reinhardt-web/discussions/3918)) — `ws_urls.rs` and friends move under `src/apps/<app>/urls/`.
- **`AdminUser` trait signature** ([#3615](https://github.com/kent8192/reinhardt-web/discussions/3615)) — `ModelAdmin` permission methods accept `&dyn AdminUser` instead of `&(dyn Any + Send + Sync)`.
- **`admin_routes_with_di()`** ([#3626](https://github.com/kent8192/reinhardt-web/discussions/3626)) — the new entry point applies middleware-contributed DI registrations; legacy `admin_routes()` is removed.
- **`AdminRoute` is `#[non_exhaustive]`** — match arms must include a default fallback; the `Login` variant moved position.
- **OAuth2 `exchange_code` redirect URI** ([#3609](https://github.com/kent8192/reinhardt-web/discussions/3609)) — fourth argument is the callback URL.
- **Typed TOML interpolation** ([#4241](https://github.com/kent8192/reinhardt-web/discussions/4241), [#4229](https://github.com/kent8192/reinhardt-web/discussions/4229)) — `${VAR}` placeholders in TOML coerce to the destination type; opt out with `SettingsBuilder::with_typed_coercion(false)`.
- **`ProjectSettings` replaces `env::var`** in commands/db ([#4295](https://github.com/kent8192/reinhardt-web/discussions/4295)) and `VersioningSettings` in rest ([#4294](https://github.com/kent8192/reinhardt-web/discussions/4294)).
- **`ClientLauncher::on_navigate`** ([#4117](https://github.com/kent8192/reinhardt-web/discussions/4117)) — SPA navigation routes through `Router::on_navigate` instead of relaunching the client per navigation.
- **`client_router::history` dedup** ([#4219](https://github.com/kent8192/reinhardt-web/discussions/4219)) — the duplicate `history` module under `client_router` is removed; consume `pages::router::history` instead.
- **`#[routes]` is async-capable** — route handler signatures may be `async fn`; sync handler ABIs remain supported but no longer represent the canonical shape.
- **`define_views!` → `flatten_imports!`** ([#3783](https://github.com/kent8192/reinhardt-web/discussions/3783)) — multi-file view modules use the renamed declarative macro; `#[export_endpoints]` is removed.
- **`JsonFileSource` / `auto_source` deprecated** ([#4120](https://github.com/kent8192/reinhardt-web/discussions/4120)) — prefer `TomlFileSource` (default interpolation enabled).
- **`#[user(...)]` requires explicit `LABEL`** on `AppLabel` implementors, and emits a `BaseUserManager` impl in 0.1.0.
- **`unique_together` propagates into `ModelMetadata`** ([#4027](https://github.com/kent8192/reinhardt-web/discussions/4027)) — autodetector consumes it; existing migrations regenerate.
- **Admin form widget HTML elements** ([#3771](https://github.com/kent8192/reinhardt-web/discussions/3771)) — `TextArea`, `Select`, and `MultiSelect` render as their semantic HTML elements rather than `<input>`.
- **`manouche` IR / IRVisitor removed** ([#3900](https://github.com/kent8192/reinhardt-web/discussions/3900)) — internal codegen path no longer exposes the IR layer.
- **`examples-rest` layout** ([#4476](https://github.com/kent8192/reinhardt-web/discussions/4476)) — `standalone` flag and `USE_VIEWSET` toggle dropped; `urls/` restructured for `ResolvedUrls`; Bruno collection split per ViewSet.
- **`infra/repository` reviewer lookup** ([#4152](https://github.com/kent8192/reinhardt-web/discussions/4152)) — Terraform module replaces the `data "github_user"` lookup with a numeric user-ID variable.

### Migration Guide

The 0.1.0 stable release consolidates the alpha line and the historical rc
snapshots. For migration purposes, users on any rc version should follow the
guide below as if migrating from pre-release code. Notable breaking changes
accumulated during stabilization are summarized below; the complete list is in
the **Breaking Changes** section above.

- **Typed URL routing** ([#3770](https://github.com/kent8192/reinhardt-web/discussions/3770)):
  replace string-based `#[url_patterns]` with
  `#[url_patterns(InstalledApp::app_name, mode = server|client|unified)]`
  and rename functions to `server_url_patterns()` /
  `client_url_patterns()` / `unified_url_patterns()`.
- **Dependency injection unification**
  ([#3628](https://github.com/kent8192/reinhardt-web/discussions/3628)):
  replace `#[inject] Arc<T>` with `#[inject] Depends<T>` across every
  injection site. `Depends<T>` adds caching, circular-dependency
  detection, and metadata that bare `Arc<T>` lacked.
- **Deprecate `Injected<T>`**
  ([#3631](https://github.com/kent8192/reinhardt-web/discussions/3631)):
  migrate `Injected<T>` and `OptionalInjected<T>` to `Depends<T>` and
  `Option<Depends<T>>`. Add an explicit `#[derive(Clone)]` if your type
  was relying on the previous auto-Clone behaviour.
- **`AdminUser` trait signature**
  ([#3615](https://github.com/kent8192/reinhardt-web/discussions/3615)):
  update `ModelAdmin` permission methods to accept `&dyn AdminUser`
  instead of `&(dyn std::any::Any + Send + Sync)`.
- **OAuth2 `exchange_code` redirect URI**
  ([#3609](https://github.com/kent8192/reinhardt-web/discussions/3609)):
  `exchange_code()` now requires a `redirect_uri` parameter; pass the
  callback URL as the fourth argument.
- **Typed TOML interpolation**
  ([#4241](https://github.com/kent8192/reinhardt-web/discussions/4241)):
  environment-variable interpolation in TOML (e.g.,
  `${REINHARDT_DB_PORT}`) now coerces to the target type. Opt out with
  `SettingsBuilder::with_typed_coercion(false)`.
- **URL resolver restructuring**
  ([#3918](https://github.com/kent8192/reinhardt-web/discussions/3918)):
  move `src/apps/<app>/ws_urls.rs` to `src/apps/<app>/urls/ws_urls.rs`
  and declare it in the `urls` submodule.
- **`define_views!` replaces `#[export_endpoints]`**
  ([#3768](https://github.com/kent8192/reinhardt-web/discussions/3768)):
  use the `define_views!` declarative macro for multi-file view
  modules — the attribute form was removed for stable-Rust
  compatibility (later renamed to `flatten_imports!`).
- **Apps relocate per-app handlers**
  ([#4476](https://github.com/kent8192/reinhardt-web/discussions/4476)):
  per-app `server_fn` and client UI moved from `commands/templates/...`
  into `apps/<app>/`. Update existing apps by relocating the matching
  source files; `reinhardt new` already emits the new layout.
- **`ClientLauncher::on_navigate`**
  ([#4117](https://github.com/kent8192/reinhardt-web/discussions/4117)):
  client SPA navigation now hooks through `Router::on_navigate` rather
  than launching a fresh router per navigation; remove any manual
  `ClientLauncher::launch` wiring tied to the old model.
- **`admin_routes_with_di()`**
  ([#3626](https://github.com/kent8192/reinhardt-web/discussions/3626)):
  use `admin_routes_with_di()` instead of the deprecated
  `admin_routes()` so middleware-contributed DI registrations are
  applied.

For the complete per-PR change list, see the
[Release-category Discussions](https://github.com/kent8192/reinhardt-web/discussions/categories/release).

### Added

#### ORM & Database

- `#[model]` derives Django-style model traits, including
  `Model::build()` typestate constructor, `ForeignKey<T>` and
  `ManyToMany<T>` relations, and `#[field(skip = true)]` for non-DB
  fields.
- Multi-backend support: PostgreSQL, MySQL, SQLite, and CockroachDB
  via TestContainers in tests; per-backend `ALTER COLUMN` dispatch and
  CockroachDB sentinel-row locks replace generic advisory-lock paths.
- Schema autodetection covers unique constraints, M2M relations, and
  qualified `to_model = "app::Model"` references; M2M autodetection
  keys on `table_name` and snake-cases through-tables consistently.
- `reinhardt-query`: in-house SQL builder wrapping SeaQuery, used in
  place of raw SQL across the workspace.
- Prefetch and `filter_by_target` honour canonical M2M naming rules.

#### URL Routing & Apps

- `#[url_patterns]` macro generates typed `urls::*` helpers per app
  with compile-time path verification; `#[routes]` consumes them.
- `#[viewset]` macro for REST-style endpoint groups; named, mount,
  and unified routing modes.
- `VersionedRouter` trait and `RouteVersionInfo` value type share a
  router abstraction between `reinhardt-urls` and `reinhardt-rest`
  without circular crate dependencies (introduced via
  `reinhardt-router`).
- Middleware DI registrations propagate through `with_middleware`,
  `group()`, and `pending_di` queues so the router rebuilds DI no
  matter the builder order.

#### Dependency Injection

- `Depends<T>` smart wrapper with caching, cycle detection, and
  metadata; `#[inject]` parameters express dependencies declaratively.
- `Middleware::di_registrations` hook + type-erased DI APIs allow
  middleware to contribute its own dependencies (e.g., `SessionStore`
  auto-registered by `SessionMiddleware`).
- `DependencyRegistration` is const-compatible for Rust 2024 edition.
- Testing helpers: `register_override` with `OverrideGuard`, and the
  `with_di_overrides!` macro shipped from `reinhardt-testkit-macros`.

#### Pages, WASM, and Server Functions

- `page!` declarative DSL for component bodies (70+ HTML elements
  validated at compile time) with `if`/`else`, `for`, `watch`, and
  reactive `Signal<T>` bindings.
- `head!` macro for SSR head metadata.
- `form!` macro: typed form fields, widgets, validators (client &
  server), CSRF protection, two-way Signal binding, computed values,
  field groups, slots, and dynamic `choices_loader`.
- `#[server_fn]` macro generates the WASM client stub and the
  server-side handler; codec selection (`json`, `url`, `msgpack`).
- `use_router()` hook, `RouterHandle`, `navigate()` free function,
  and `try_with_spa_router` for graceful SPA fallback.
- HMR (`reinhardt-pages` feature flag): WebSocket-driven CSS / DOM
  patching, scheme selected automatically based on
  `window.location.protocol`.

#### HTTP, REST, and Middleware

- `reinhardt-http` request / response abstractions; sanitization
  helpers (`validate_html_attr_name`, `is_safe_url`, anchor-link
  support, XSS prevention) and resource-limit configuration.
- `reinhardt-rest`: ViewSets with operation-level OpenAPI attributes,
  versioning via `VersioningSettings`, OpenAPI macros under
  `reinhardt-openapi-macros`.
- `reinhardt-middleware`: session middleware, CORS, gzip, etc.

#### Authentication & Authorization

- JWT + session-cookie auth, OAuth2 (`GenericOidcProvider` for
  arbitrary OIDC IdPs), and pluggable user managers.
- `SuperuserInit` trait and `SuperuserCreator` registry; auto-register
  via `inventory` for `#[user(full = true)]` + `#[model]` types.
- `AuthProtection` enum + `EndpointMetadata` propagate auth
  requirements; route macros detect auth parameters and produce the
  metadata automatically.

#### Admin Interface

- `#[model]`-driven admin pages with role-based permissions,
  type-safe query filters, and form-based CRUD.
- `reinhardt-admin-cli`: CLI for scaffolding admin pages,
  `cargo install --path crates/reinhardt-admin-cli` ships with the
  workspace.
- `admin_routes_with_di()` entry point + `AdminRoute` (now
  `#[non_exhaustive]`).

#### Configuration & Settings

- `SettingsBuilder` layered configuration with `TomlFileSource`,
  `EnvFileSource`, and typed interpolation enabled by default.
- `#[settings(...)]` macro requires explicit `CoreSettings` and emits
  `HasSettings<F>` impls for composed settings; `#[setting(...)]`
  attribute drives `field_policies()` generation.

#### Testing

- `reinhardt-test` fixtures for TestContainers-backed Postgres, MySQL,
  SQLite, CockroachDB, Redis, and Kafka instances.
- Module-scoped Kafka fixture with `KafkaConfig.partitions`.
- `reinhardt-testkit` and `reinhardt-testkit-macros` provide
  `with_di_overrides!` and `DiOverrideBuilder` for testing-only DI
  overrides.

#### Other Integrations

- GraphQL handler via `reinhardt-graphql` (+ macros).
- gRPC handler via `reinhardt-grpc` (+ macros).
- WebSocket router via `reinhardt-websockets`.
- Mail delivery via `reinhardt-mail` (pluggable transports).
- i18n via `reinhardt-i18n` (fluent-style messages).
- Throttling via `reinhardt-throttling`.
- Streaming integration via `reinhardt-streaming` (Kafka).
- Deep-link helpers via `reinhardt-deeplink`.
- Keyboard shortcuts via `reinhardt-shortcuts`.

#### Tooling & Examples

- `reinhardt new` scaffold emits the per-app layout matching
  [#4476](https://github.com/kent8192/reinhardt-web/discussions/4476).
- `examples-tutorial-basis` (Django-tutorial parity), `examples-rest`
  (REST-API tutorial with Bruno collections), and additional samples
  cover the most common entry points.

### Changed

- Cross-cutting refactor of the URL/routing macro internals for a
  smaller, easier-to-read generated TokenStream.
- ORM accessors and autodetector consolidate on a single
  `crate::naming::to_snake_case` source.
- Setting-related `glob` imports replaced with explicit `pub use`
  re-exports; explicit `rayon` trait imports.
- WASM compilation: client URL accessors namespaced per app on
  `wasm32`, with `#[cfg]` gating in generated tokens to avoid stale
  symbols.
- Migrations now dispatch `ALTER COLUMN TYPE` per backend (MySQL
  `MODIFY`, SQLite recreate, PostgreSQL / CockroachDB direct ALTER).

### Deprecated

- `Injected<T>` and `OptionalInjected<T>` — use `Depends<T>` /
  `Option<Depends<T>>` ([#3631](https://github.com/kent8192/reinhardt-web/discussions/3631)).
- `admin_routes()` — use `admin_routes_with_di()` ([#3626](https://github.com/kent8192/reinhardt-web/discussions/3626)).
- `JsonFileSource`, `auto_source` — use `TomlFileSource` ([#4120](https://github.com/kent8192/reinhardt-web/discussions/4120)).
- `define_views!` — renamed to `flatten_imports!` ([#3783](https://github.com/kent8192/reinhardt-web/discussions/3783)).
- `#[export_endpoints]` — removed in favour of `define_views!` /
  `flatten_imports!` ([#3768](https://github.com/kent8192/reinhardt-web/discussions/3768)).

### Fixed

- WASM compatibility for the reactive runtime: `Signal<T>` is `Sync`
  on native via `Arc<RwLock<T>>`; `flush_updates` unifies pending
  effects.
- `#[routes]` no longer panics on `extern` identifiers; reserved-ident
  set excludes `extern`.
- `form!` macro: `on_success` and `success_url` correctly hoist
  before navigation; `watch` and `initial` capture outer scope.
- `#[user]` integration tests opt out of auto-manager when the
  fixture supplies one.
- Migrations: split multi-statement reverse SQL; emit `ALTER COLUMN`
  reverse as a single comma-separated statement.
- Static-files handler disables immutable caching for bundle assets
  in debug builds so HMR can replace them.
- Per-app `installed_apps` state file is namespaced by
  `CARGO_CRATE_NAME` so concurrent macro expansion does not race.
- Type-erased proc-macro closures use an `if false { ... }`
  dead-code branch with `unreachable!()` arguments to unify the
  user-supplied generic at compile time (#4624 / #4627).

### Security

- HTML attribute / URL sanitization helpers shipped to prevent the
  most common XSS sinks.
- CSRF protection wired into `form!` for non-GET methods.
- OAuth2 `exchange_code` requires an explicit `redirect_uri` so
  attacker-controlled callbacks cannot be substituted post-issue.

### Performance

- Single-lock manager operations in core-macros generated code.
- Per-app endpoint inventory keyed on `CARGO_CRATE_NAME` removes a
  contention point during macro expansion.

For per-prerelease detail, see the
[Release-category Discussions](https://github.com/kent8192/reinhardt-web/discussions/categories/release)
(historical rc snapshots plus the alpha.7 / alpha.8 / alpha.9 announcement
posts). The rc snapshots are not treated as released versions.
