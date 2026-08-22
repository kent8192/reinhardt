# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.4.0-alpha.9](https://github.com/kent8192/reinhardt-web/compare/reinhardt-urls@v0.4.0-alpha.8...reinhardt-urls@v0.4.0-alpha.9) - 2026-08-22

### Fixed

- *(urls)* enforce ViewSet routing policies

## [0.4.0-alpha.7](https://github.com/kent8192/reinhardt-web/compare/reinhardt-urls@v0.4.0-alpha.6...reinhardt-urls@v0.4.0-alpha.7) - 2026-08-19

### Added

- *(urls)* expose mounted route contracts
- *(core)* collect mounted endpoint security contracts
- *(urls)* declare class view authentication

### Fixed

- *(urls)* align mounted contracts with dispatch
- *(urls)* gate viewset contract metadata import
- *(commands)* retain one-to-one contract references
- *(urls)* satisfy route introspection lint
- *(contract)* close export review findings
- *(contract)* close follow-up export review findings
- *(contract)* close mounted route follow-up findings
- *(contract)* resolve review findings
- *(contract)* resolve application contract review findings
- resolve native protocol review findings
- *(contract)* preserve mounted route metadata during export
- *(commands)* make contract verification fail closed
- close contract verification review gaps
- *(contract)* close verification review gaps

### Other

- sync develop/0.4.0 into CI repair
- sync latest develop/0.4.0 into CI repair

### Testing

- *(ci)* align remaining release train fixtures

### Breaking Changes

- `ClientPathPattern::reverse` and `ClientRouter::reverse` now reject route
  parameters that can change URL meaning across browser normalization. Ordinary
  parameters must be non-empty, values must use URL-stable ASCII characters or
  valid percent-encoded triplets, and completed path segments must not be `.`
  or `..` (including percent-encoded variants). Wildcard parameters may still
  be empty and contain `/`.

### Migration Notes

- Audit client route reversal callers for values containing spaces, non-ASCII
  characters, malformed percent escapes, or standalone dot segments. Normalize
  those values before reversal and handle the `None` result from `reverse` as a
  route-resolution failure.

## [0.4.0-alpha.6](https://github.com/kent8192/reinhardt-web/compare/reinhardt-urls@v0.4.0-alpha.5...reinhardt-urls@v0.4.0-alpha.6) - 2026-08-06

### Documentation

- *(release)* restore coherent alpha.3 references

## [0.4.0-alpha.1](https://github.com/kent8192/reinhardt-web/compare/reinhardt-urls@v0.3.2...reinhardt-urls@v0.4.0-alpha.1) - 2026-07-21

### Added

- *(urls)* [**breaking**] match nested client layout routes
- *(urls)* [**breaking**] compose client layout guards and metadata
- *(pages)* [**breaking**] preserve client layout shells
- *(pages)* [**breaking**] add layout component macro
- *(urls)* expose matched route loader metadata
- *(urls)* split navigation match from commit
- *(pages)* hydrate server-prepared route loaders
- *(pages)* add lifecycle-aware document head management

### Documentation

- *(pages)* document route-level data loaders

### Fixed

- preserve route body and path param semantics
- address 0.4 fast-path review comments
- preserve custom method router dispatch
- *(urls)* harden client layout route matching
- *(urls)* reduce client route registration arguments
- *(routers)* align consumers with scoped Copy signals
- *(tests)* scope wasm router diagnostics
- *(urls)* defer native client router construction
- *(reactive)* retain scoped callback lifetimes
- *(reactive)* preserve deferred handle ownership
- *(reactive)* dispose retained lifecycle state
- *(reactive)* retain standalone owner scopes
- *(reactive)* close PR [[#5640](https://github.com/kent8192/reinhardt-web/issues/5640)](https://github.com/kent8192/reinhardt-web/issues/5640) CI and review gaps
- *(pages)* retain copy handle owners
- *(pages)* retain owner scopes for deferred work
- *(pages)* merge deferred owner scope fixes
- *(ci)* terminate cfg aliases macro invocations
- *(reactive)* enforce explicit dependencies and memo invalidation
- *(urls)* retain popstate subscription closure
- *(pages)* normalize initial loader history state
- *(urls)* satisfy route metadata clippy
- *(pages)* resolve route loader review findings
- *(urls)* preserve prepared route state
- *(pages)* address route loader review feedback
- *(release)* restore develop prerelease lifecycle

### Performance

- [**breaking**] inline path parameter dispatch values
- freeze compiled route tables
- skip empty router dispatch work
- borrow matched route handlers
- add concrete router dispatch fast path
- bypass router handler arc wrapper
- add synchronous handler fast path
- flatten server router backend dispatch
- add static server route fast path
- expose synchronous router dispatch
- use compact exact route tables
- revert compact exact route table experiment
- add requestless sync route path
- bypass exact route match materialization

### Testing

- tighten review regression assertions
- *(urls)* unwrap route metadata in tests

## [0.3.0](https://github.com/kent8192/reinhardt-web/compare/reinhardt-urls@v0.2.0...reinhardt-urls@v0.3.0) - 2026-06-28

Stable release of `reinhardt-urls` for the Reinhardt 0.3.0 line. This
entry consolidates the 0.3.0 release-candidate series into one
stable release section.

### Migration Notes

- Review the root CHANGELOG and `instructions/MIGRATION_0.3.md` before upgrading from 0.2.x.

### Added

- *(urls)* [**breaking**] remove raw server route registration APIs
- *(pages)* add route-backed component macros
- Added `ClientRouter::component(handler)` and component metadata traits for
  registering route-backed page components without repeating path/name
  metadata.

### Fixed

- *(ci)* satisfy server router clippy lint
- *(urls)* remove duplicate validate_routes doctest import
- *(urls)* hide route opt-out sigil in introspection

### Performance

- *(http)* reduce request allocation overhead

### Documentation

- define wasm native API parity contract
- clarify router parity level
- clarify generated API parity docs

### Maintenance

- merge develop/0.3.0 into component route macros

## [0.2.0](https://github.com/kent8192/reinhardt-web/compare/reinhardt-urls@v0.1.3...reinhardt-urls@v0.2.0) - 2026-06-11

Stable release of `reinhardt-urls` for the Reinhardt 0.2.0 line. This
entry consolidates the 0.2.0 release-candidate series into one
stable release section.

### Migration Notes

- Replace `reverse_single_pass` and `reverse_with_aho_corasick` with their `try_` variants.
- Use `route_path` with closure arity instead of numbered `route_pathN` helpers.
- See [`instructions/MIGRATION_0.2.md`](../../instructions/MIGRATION_0.2.md) for the workspace migration checklist.

### Breaking Changes

- *(urls)* [**breaking**] remove panicking reverse_* variants and UrlResolverUnprefixed (refs [[#4520](https://github.com/kent8192/reinhardt-web/issues/4520)](https://github.com/kent8192/reinhardt-web/issues/4520))
- *(urls)* [**breaking**] unify UnifiedRouter::server closure type to ServerRouter on wasm
- **[breaking]** Collapse `ClientRouter::route_pathN` / `named_route_pathN`
  into a single arity-generic `route_path` / `named_route_path` driven by
  a sealed `Handler<Args>` trait. The arity is inferred from the closure
  signature (1..=8 `Path<T>` extractors supported). Migration is a
  mechanical rename: drop the numeric suffix on every call site. Refs
  [#4637](https://github.com/kent8192/reinhardt-web/issues/4637).

### Added

- *(urls)* [**breaking**] remove panicking reverse_* variants and UrlResolverUnprefixed (refs [[#4520](https://github.com/kent8192/reinhardt-web/issues/4520)](https://github.com/kent8192/reinhardt-web/issues/4520))
- *(urls)* auto-populate global UrlReverser in register_router()
- *(urls)* [**breaking**] unify UnifiedRouter::server closure type to ServerRouter on wasm
- *(urls)* warn when a reverse URL name is not kebab-case
- Emit a `tracing::warn!` when a route name registered for URL reversal is not
  kebab-case (e.g. `user_detail`), suggesting the kebab-case form
  (`user-detail`) to match ViewSet-generated names. The warning is advisory, not
  an error: prefix the route-name segment with `!` to opt out (the sigil is
  stripped before storage, so reverse lookups use the clean name) or set
  `REINHARDT_URL_NAME_WARNINGS=0` to silence it globally. Refs
  [#4901](https://github.com/kent8192/reinhardt-web/issues/4901).
- `ClientRouter::page<F, P>(pattern, handler)` and
  `ClientRouter::named_page<F, P>(name, pattern, handler)` accepting any
  handler `Fn(P) -> Page` where `P: FromRequest`. The same Props struct
  can be used both as a Component prop bag (Manouche DSL v2 spec §4.3)
  and as a page function — "every page is a component." Path / query
  extraction errors surface as a `Page::Text` at the router boundary
  rather than panicking. (Refs #4668)
- `reinhardt_urls::routers::client_router::from_request` module
  exposing `FromRequest`, `RouteContext`, `ExtractError`,
  `PathParam<T>`, and `QueryParam<T>` — the manual building blocks for
  `ClientRouter::page` handlers. `#[derive(FromRequest)]` and
  `#[derive(PageProps)]` proc-macros are deferred to spec §10.
- `ClientRouteMatch::query: Option<String>` — populated by `match_path`
  after stripping an optional `?query` suffix from the path before
  pattern matching. Required for `QueryParam<T>` to see the query under
  real routing.
- `ParamContext::with_query(...)` / `ParamContext::query()` /
  `ParamContext::params()` — `render_current` threads the captured
  query through to the `RouteHandler` trait. Backward-compatible:
  existing `ParamContext::new(...)` keeps the previous signature and
  defaults the new field to `None`.

### Changed

- **[breaking]** Collapse `ClientRouter::route_pathN` / `named_route_pathN`
  into a single arity-generic `route_path` / `named_route_path` driven by
  a sealed `Handler<Args>` trait. The arity is inferred from the closure
  signature (1..=8 `Path<T>` extractors supported). Migration is a
  mechanical rename: drop the numeric suffix on every call site. Refs
  [#4637](https://github.com/kent8192/reinhardt-web/issues/4637).
- [**breaking**] align develop/0.2.0 with main, preserving 8 feature crates

### Removed

- **`reverse_with_aho_corasick(pattern, params)`** (`src/routers/reverse/runtime.rs`, deprecated `0.1.0-rc.29`) — use `try_reverse_with_aho_corasick` (the fallible variant).
- **`reverse_single_pass(pattern, params)`** (`src/routers/reverse/runtime.rs`, deprecated `0.1.0-rc.29`) — use `try_reverse_single_pass`.
- **`UrlResolverUnprefixed` trait** (`src/routers/resolver.rs`, deprecated `0.1.0-rc.29`, refs #4507 defect #2) — only supported the removed flat ViewSet trait accessors emitted by `#[viewset]`. Prefer `urls.server().<app>().<route>()`.

#### BREAKING CHANGES

Removed 3 RC-deprecated items per STABILITY_POLICY § SP-4 (umbrella
Issue [#4520](https://github.com/kent8192/reinhardt-web/issues/4520)):

- **`reverse_with_aho_corasick(pattern, params)`** (`src/routers/reverse/runtime.rs`, deprecated `0.1.0-rc.29`) — use `try_reverse_with_aho_corasick` (the fallible variant).
- **`reverse_single_pass(pattern, params)`** (`src/routers/reverse/runtime.rs`, deprecated `0.1.0-rc.29`) — use `try_reverse_single_pass`.
- **`UrlResolverUnprefixed` trait** (`src/routers/resolver.rs`, deprecated `0.1.0-rc.29`, refs #4507 defect #2) — only supported the removed flat ViewSet trait accessors emitted by `#[viewset]`. Prefer `urls.server().<app>().<route>()`.

### Fixed

- align urls routers macro feature graph
- *(urls)* fix broken intra-doc link in try_reverse_single_pass doc
- *(auth)* replace InternalUser in UserManager public API with ManagedUser
- *(urls)* migrate reverse tests from removed panicking helpers to try_ variants
- *(urls)* resolve UrlReverser Debug trait bound and dead code errors
- *(urls)* correct WASM register_globally doc comment
- *(urls)* panic on duplicate client route name registration
- *(urls)* update tests for page() and reverse() API changes
- *(urls)* populate global UrlReverser in register_router_arc() too
- *(docs)* resolve rustdoc intra-doc link errors on develop/0.2.0
- *(docs)* resolve remaining rustdoc doctest failures
- *(ci)* recover develop release-plz prerelease
- *(auth,urls,pages)* remove stale references and fix latent clippy lints

### Performance

- atomize facade dependency feature gates

### Documentation

- *(urls)* align WASM UnifiedRouter::server doc with no-invoke behavior

### Styling

- apply formatter fixes across workspace
- format files from merge resolution
- apply rustfmt to non-DSL files on develop/0.2.0

### Maintenance

- resolve merge conflict in reinhardt-urls CHANGELOG.md (develop/0.2.0)

## [0.1.0](https://github.com/kent8192/reinhardt-web/compare/reinhardt-urls@v0.1.0-rc.30...reinhardt-urls@v0.1.0) - 2026-05-22

Initial stable release of `reinhardt-urls` as part of the reinhardt-web
0.1.0 release. Provides the URL routing core: the `#[url_patterns]`
consumer, `ServerRouter` / `ClientRouter` / `UnifiedRouter` builders,
typed reverse resolution, and the radix-based path matcher shared with
client-side WASM.

For the workspace-wide release narrative, see the [root CHANGELOG](https://github.com/kent8192/reinhardt-web/blob/main/CHANGELOG.md#010---2026-05-22).
Per-prerelease history is in the [Release Discussions](https://github.com/kent8192/reinhardt-web/discussions/categories/release).

### Capabilities at 0.1.0

- **Server, client, and unified routers** — `ServerRouter`,
  `ClientRouter`, and `UnifiedRouter` cover native, WASM, and hybrid
  setups; `Debug` is implemented for both server-side variants, and
  `UnifiedRouter` carries a WASM-friendly `ServerRouterStub` so
  `#[url_patterns(mode = unified | server)]` closures compile on WASM.
  Native `mount_unified` correctly merges child client routes.
- **Typed URL resolution** — Compile-time type-safe URL resolution via
  extension traits, name-alias support on `UrlReverser` and
  `ServerRouter`, and a client-side `ClientUrlReverser`. The
  `url-resolver` capability is part of the standard, `api-only`, and
  `urls-full` feature sets.
- **Async-capable `#[routes]`** — `#[routes]` accepts async handlers,
  and the radix-based pattern matcher surfaces insertion errors and
  exposes fallible reverse helpers. Route registration accumulates
  prefixes correctly, strips leading slashes on action `url_path`, and
  normalizes ViewSet prefixes to prevent triple slashes.
- **Reactive client navigation** — `ClientRouter` carries reactive
  navigation observation, `render_current()` returns a `Page`, and
  `Clone` is derived so route resolution composes inside reactive
  scopes. `pages::router::history` is the single source of truth (the
  duplicate `client_router::history` module was removed).
- **Middleware-aware route assembly** — `with_middleware` harvests
  middleware-contributed DI registrations, `group()` drains pending
  DI on grouped routers, and child routers receive `with_di_context`
  propagation. `exclude()` provides declarative route exclusion on the
  builder side.
- **Streaming and viewset integration** — `UnifiedRouter::mount_streaming()`
  registers streaming handlers, `StreamingTopicResolver` resolves
  topics from URL patterns, and `viewset_with_actions` bridges
  `#[viewset]` actions through `ServerRouter` / `RouteGroup` builders.
- **Defensive runtime** — Lock poisoning is recovered (no `unwrap()` on
  `RwLock` guards), `RwLock` guards are never held across `.await`
  points, the LRU route cache enforces memory-bounded eviction with
  periodic compaction, and path validation rejects traversal, ambiguous
  parameters, ReDoS-prone patterns, and parameter injection at compile
  time.

### Notable Breaking Changes

- **Typed `#[url_patterns]`** ([Discussion #3770](https://github.com/kent8192/reinhardt-web/discussions/3770))
  — Accepts `InstalledApp::*` identifiers with `mode = server | client
  | unified`; pattern functions are renamed accordingly.
- **`urls/` directory layout** ([Discussion #3918](https://github.com/kent8192/reinhardt-web/discussions/3918))
  — `ws_urls.rs` and friends move under `src/apps/<app>/urls/`.
- **`client_router::history` dedup** ([Discussion #4219](https://github.com/kent8192/reinhardt-web/discussions/4219))
  — The duplicate `history` module under `client_router` is removed;
  consume `pages::router::history` instead.
- **Apps relocation** ([Discussion #4476](https://github.com/kent8192/reinhardt-web/discussions/4476))
  — Per-app handlers move into `apps/<app>/`, which affects how URL
  patterns are declared and mounted.
- **Async `#[routes]`** — Route handler signatures may be `async fn`;
  sync ABIs remain supported but are no longer the canonical shape.
- **Radix insertion errors surface** — `#[routes]` no longer panics on
  pattern conflicts; fallible reverse helpers return `Result` values
  callers must handle.

### Migration Notes

See the [root CHANGELOG](https://github.com/kent8192/reinhardt-web/blob/main/CHANGELOG.md#010---2026-05-22)
for the consolidated migration guide. The most disruptive moves are
covered in Discussions [#3770](https://github.com/kent8192/reinhardt-web/discussions/3770),
[#3918](https://github.com/kent8192/reinhardt-web/discussions/3918),
[#4219](https://github.com/kent8192/reinhardt-web/discussions/4219),
and [#4476](https://github.com/kent8192/reinhardt-web/discussions/4476).
