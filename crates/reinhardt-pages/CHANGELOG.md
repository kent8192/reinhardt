# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.4.0-alpha.2](https://github.com/kent8192/reinhardt-web/compare/reinhardt-pages@v0.4.0-alpha.1...reinhardt-pages@v0.4.0-alpha.2) - 2026-07-23

### Fixed

- *(pages)* ignore named interpolation keys in captures
- *(pages)* scope named macro argument capture skipping
- *(pages)* preserve captures for shadowable macros
- *(pages)* resolve named macro paths safely

### Testing

- *(pages)* refresh controlled bind diagnostic

## [0.4.0-alpha.1](https://github.com/kent8192/reinhardt-web/compare/reinhardt-pages@v0.3.2...reinhardt-pages@v0.4.0-alpha.1) - 2026-07-21

### Added

- *(pages)* compose resource values with action results
- *(pages)* parse page body form
- *(pages)* collect and generate implicit body captures
- feat!(pages): resolve SSR resources asynchronously
- feat!(pages): require explicit hook dependency modes
- *(pages)* validate deps macro hook arguments

### Changed

- *(pages)* migrate hooks to deps list syntax

### Documentation

- docs!(pages): clarify direct body migration
- *(pages)* explain hook dependency modes
- *(pages)* note hook dependency migration

### Fixed

- address 0.4 fast-path review comments
- [**breaking**] preserve server_fn codec metadata invariants
- *(pages)* complete implicit body capture analysis
- *(pages)* validate page body capture scopes
- *(pages)* harden async SSR resource rendering
- *(pages)* complete ssr suspense resource replay
- *(pages)* drain streamed ssr resource replays
- *(pages)* stabilize suspense resource replay
- *(pages)* address SSR resource review feedback
- *(pages)* reuse tracked SSR resources during replay
- *(pages)* harden streaming suspense resource scopes
- *(pages)* reuse pending tracked SSR resources
- *(pages)* scope suspense resource replay caches
- *(pages)* remove shared suspense head cache
- *(pages)* preserve SSR resource replay consistency
- *(pages)* keep buffered suspense head capture request scoped
- *(pages)* preserve marker render allocators
- *(pages)* satisfy resource hydration clippy
- *(pages)* harden suspense hydration state
- *(pages)* stabilize client form submit UI fixtures
- *(pages)* drop duplicate component tree arms
- *(ci)* restore upgraded dependency checks
- *(pages)* harden hook dependency migration
- *(pages)* harden dependency migration
- *(pages)* align dependency mode exports and tests
- *(pages)* update dynamic dependency test
- *(pages)* scope native resource dependency import
- *(pages)* remove duplicate native cfg
- *(pages)* repair hook dependency migration tests
- *(pages)* restore explicit deps compatibility
- *(ci)* terminate cfg aliases macro invocations
- *(pages)* retain reactive owners through review edge cases
- *(pages)* guard stale lifecycle handles
- *(pages)* retain mount scopes for events
- *(reactive)* enforce explicit dependencies and memo invalidation
- *(pages)* complete explicit dependency migration
- *(pages)* restore reactive CI compatibility
- *(pages)* update legacy hook dependency tests
- *(pages)* resolve controlled binding review feedback
- *(pages)* preserve controlled select projection
- *(pages)* address route loader review feedback
- *(pages)* resolve server function set review findings
- restore atomic ORM release compatibility
- *(pages)* restore WASM hydration compilation
- *(pages)* envelope server function failures
- *(pages)* envelope server function request failures
- *(pages)* skip stale server form errors
- *(pages)* classify server function failures
- *(pages)* preserve structured server form errors
- *(pages)* retain custom error status
- *(pages)* reconcile reactive attributes
- *(pages)* preserve reactive control hydration
- *(pages)* normalize boolean attribute names
- *(pages)* restore reactive attribute test checks
- *(pages)* stabilize empty head compile fixture
- *(pages)* simplify reactive boolean attribute predicates
- *(release)* restore develop prerelease lifecycle
- *(ci)* stabilize release test failures
- *(pages)* align hydration and SSR regression tests
- *(pages)* retain server DOM after hydration rollback
- *(pages)* scope document head wasm signals
- *(pages)* align server function error display test

### Maintenance

- merge develop/0.4.0 into component route branch
- merge remote async SSR resource branch
- merge develop/0.4.0 into client form bindings
- merge develop/0.4.0 into client form bindings
- merge develop/0.4.0 into typed JSON PR
- merge latest develop changes into typed JSON PR
- merge develop/0.4.0 into functional state setter
- migrate dependency policy checks to cargo-deny
- merge develop/0.4.0 into retained effects
- merge develop/0.4.0 into issue 5575 branch
- merge develop/0.4.0 into route loader branch

### Other

- sync develop/0.4.0 into hook dependency modes
- resolve develop/0.4.0 conflicts for [[#5676](https://github.com/kent8192/reinhardt-web/issues/5676)](https://github.com/kent8192/reinhardt-web/issues/5676)
- sync develop/0.4.0 into server function set
- sync develop/0.4.0 into structured server errors

### Performance

- use compact exact route tables
- revert compact exact route table experiment
- return server fn bytes directly
- borrow server fn request bodies
- avoid boxed server fn futures
- shrink server fn endpoint futures
- fast path json server fn content type
- streamline server fn responses
- [**breaking**] specialize json server fn hot path
- specialize server fn codec response path

### Testing

- update path parameter assertions
- *(pages)* cover ast page body reexports
- *(pages)* cover implicit page body captures
- *(pages)* unwrap HMR template metadata in keyed test

### Added

- Add named typed server function sets with mixed-codec marker chains,
  explicit router registration, and unchanged per-action metadata and mocks.
- Add opt-in model-backed server function sets with six typed CRUD RPCs,
  mandatory policies, typed list pagination, unique lookups, DTO mappings,
  checked overrides, transaction-bound custom actions, structured errors, and
  action-level component/MSW mock identity.
- Keep standard create overrides on the create authorization path with an
  executor-only context, without collection queryset scoping.
- *(pages)* add `SetStateExt::update` for previous-value state updates from
  `use_state` setters.
- Add native component testing utilities under
  `reinhardt_pages::testing::component`, including in-memory `Page` rendering,
  role/text/label queries, event helpers, async settling, pretty DOM output,
  and in-process `server_fn` mocks for `MockableServerFn` markers.

### Changed

- Sanitize authentication and internal extractor failures in generated server
  function handlers, and keep physical table names out of model not-found
  errors.
- **BREAKING**: Route-backed component macros now require the route name as
  `name = "..."`, for example `#[component("/path", name = "name")]`.

### Removed

- *(hmr)* remove the whole-root `HtmlReplace` message and client-side
  `innerHTML` replacement path in favor of state-preserving template patches.

### Migration Notes

- **BREAKING**: Dependency-aware hooks now require a named second argument.
  Use `deps![...]` for explicit dependencies, including `deps![]` for
  mount-only behavior. `deps_auto!()` is supported only by effects,
  layout-effects, and memos; callbacks, resources, and retained effects still
  require `deps![...]`. See the
  [0.4.0 hook dependency migration guide](../../docs/migration/0.4.0-hook-dependency-modes.md).

## [0.3.2](https://github.com/kent8192/reinhardt-web/compare/reinhardt-pages@v0.3.1...reinhardt-pages@v0.3.2) - 2026-07-14

### Fixed

- *(pages)* contain nested reactive DOM ownership
- *(pages)* gate nested reactive helper for wasm
- *(test)* render nested reactive wasm content
- *(ci)* allow intentional dependency-version duplicates
- *(tests)* use fallback service for SPA fixtures

### Other

- resolve main conflicts in nested reactive ownership

## [0.3.1](https://github.com/kent8192/reinhardt-web/compare/reinhardt-pages@v0.3.0...reinhardt-pages@v0.3.1) - 2026-07-04

### Fixed

- *(formatter)* wrap long page closure parameters

## [0.3.0](https://github.com/kent8192/reinhardt-web/compare/reinhardt-pages@v0.2.0...reinhardt-pages@v0.3.0) - 2026-06-28

Stable release of `reinhardt-pages` for the Reinhardt 0.3.0 line. This
entry consolidates the 0.3.0 release-candidate series into one
stable release section.

### Migration Notes

- Review the root CHANGELOG and `instructions/MIGRATION_0.3.md` before upgrading from 0.2.x.

### Added

- *(pages)* add explicit asset loading helpers
- Added route-backed component macro support via `#[component("/path", "name")]`,
  generated plain props builders, and `Path(...)` / `Query(...)` extractor-style
  component function arguments.

### Changed

- *(pages)* derive fetch credentials default
- [**breaking**] remove 0.3 deprecated public APIs

### Removed

- **BREAKING**: Removed the deprecated resource constructors
  `create_resource` and `create_resource_with_deps`. Use
  `use_resource(fetcher, deps)`.
- **BREAKING**: Removed the deprecated hooks `use_effect_event` and
  `use_effect_event_with`. Use `use_callback` / `use_callback_with` or read
  latest signal values with `.get_untracked()` inside effects.

### Fixed

- *(todo-check)* clear public api audit markers
- *(ci)* expose benchmark server function response
- *(pages)* allow wasm-opt fixture features
- *(ci)* update Rust 1.96 UI stderr expectations
- *(pages)* align asset head helpers with review feedback
- *(pages)* address CodeRabbit component macro review
- *(formatter)* handle reviewed page rustfmt islands
- add native stubs for client page functions
- *(pages)* export client_page from prelude
- *(ci)* satisfy pages clippy lint
- *(pages)* re-export use_callback_with from reactive::hooks
- Fixed route-backed component macro builders so downstream crates can use
  generated props without depending directly on `bon`.

### Performance

- *(pages)* reduce native endpoint dispatch allocations
- *(pages)* reduce fixture wasm release size

### Documentation

- *(pages)* document WASM server fn resolver
- *(pages)* classify React 19 parity outcomes

### Testing

- *(pages)* isolate static resolver state

### Maintenance

- merge main into develop/0.3.0
- merge develop 0.3.0 into build-time perf branch
- merge develop/0.3.0 into component route macros
- merge develop/0.3.0 into pr 5346

## [0.2.0](https://github.com/kent8192/reinhardt-web/compare/reinhardt-pages@v0.1.3...reinhardt-pages@v0.2.0) - 2026-06-11

Stable release of `reinhardt-pages` for the Reinhardt 0.2.0 line. This
entry consolidates the 0.2.0 release-candidate series into one
stable release section.

### Migration Notes

- Update hook call sites to pass explicit dependency tuples, using `()` for mount-only effects.
- Replace `create_resource*` with `use_resource(fetcher, deps)` and remove `watch { ... }` page bodies.
- Pass page macro captures through closure parameters or qualify free functions with multi-segment paths.
- See [`instructions/MIGRATION_0.2.md`](../../instructions/MIGRATION_0.2.md) for the workspace migration checklist.

### Breaking Changes

- `use_effect`, `use_layout_effect`, `use_memo`, `use_callback`, and
  `use_callback_with` now take an explicit dependency tuple as the second
  argument, aligning with React.js semantics (#4195, Manouche v2 Layer ②).
  The closure type for `use_effect` / `use_layout_effect` is now
  `FnMut() -> Option<C>` so cleanup functions can be returned, matching
  React `useEffect(() => () => cleanup())`. Pass `()` for mount-only.
- Hook closures now run with no active reactive Observer ("Option A"),
  so `Signal::get` inside the closure does NOT auto-subscribe.
  Subscriptions derive exclusively from the deps tuple.
- `impl Trackable for Resource<T, E>` lets `Resource` participate in
  hook deps tuples alongside `Signal` and `Memo`.
- `ServerFnRegistration` (native) and `MockableServerFn` (msw) now
  extend `ServerFnMetadata` instead of declaring `PATH`, `NAME`, and
  `CODEC` themselves. Existing consumers reach the constants through
  supertrait inheritance with no source change required
  ([#4711](https://github.com/kent8192/reinhardt-web/issues/4711)).
- **BREAKING**: `page!` macro now unconditionally wraps every `{expr}` and
  every `if` / `for` / `match` control-flow block in `Page::reactive(move || ...)`.
  Helper-routed Signal reads (`{helper(&signal)}`) re-render correctly without
  any opt-in. Spec §4.1. Resolves #4515.
- **BREAKING**: `page!` body identifiers must be declared in the closure
  parameter list. Implicit captures of outer Signal bindings are a hard
  compile error. Spec §3.7. Migration: pass the binding as a closure param
  or qualify free function calls with `self::` so the path is multi-segment.
- **BREAKING**: `page!` no longer accepts bare-identifier shorthand in
  element bodies (`div { name }`). Always use the explicit braced form:
  `div { {name} }`. Spec §3.6. Migration: codemod `cargo make
  migrate-manouche-v2` (PR3) handles this mechanically.
- **BREAKING**: `watch { ... }` block is removed. The body of `watch` can be
  inlined as-is; the new auto-wrap subsumes it. The validator emits a
  pointer at the `cargo make migrate-manouche-v2` codemod when it sees a
  surviving `watch` block.

### Added

- *(pages)* unify resource hooks into use_resource(fetcher, deps)
- *(forms)* add typed use_form ergonomics
- feat!(forms): route use_form through form definitions
- `callback_with_deps` internal helper backing `use_callback` /
  `use_callback_with`. Maintains stable `Arc<dyn Fn>` identity across
  re-entries at the same call site while listed deps are unchanged,
  matching React `useCallback`.
- "Reactivity semantics" rustdoc section on 8 closure-taking hooks
  (`use_reducer`, `use_action`, `use_action_state`,
  `use_sync_external_store{,_with_server}`, `use_transition`,
  `use_debug_value_with`, `use_websocket`) documenting that their
  closures run outside any active Observer by construction.
- Cross-hook integration test suite at
  `tests/hooks_deps_integration.rs` covering Memo→Effect propagation,
  unlisted-Signal isolation, cleanup ordering, and empty-deps
  mount-only behavior.
- Scaffolded `page!` macro pre-codegen pass `hook_deps_validator` (in
  the `macros` crate). Full Signal-read detection against Manouche's
  `PageBody` AST is deferred to a follow-up issue; the runtime contract
  is already enforced by the `*::new_with_deps` constructors.
- `ServerFnMetadata` cross-target supertrait carrying `PATH`, `NAME`,
  `CODEC`, and `INJECTED_PARAMS` for every `#[server_fn]`. Available
  on both native and wasm targets without any feature flag, so a
  `#[url_patterns(mode = unified)]` aggregator can name
  `my_fn::marker` from a closure body that compiles on either side
  of the cfg boundary ([#4711](https://github.com/kent8192/reinhardt-web/issues/4711)).
- `pub trait Trackable` in `reinhardt-pages::reactive` (re-exported as
  `reinhardt_pages::reactive::Trackable`). Implemented for `Signal<T>` and
  `Memo<T>`; consumed by the new auto-wrap visitor and the upcoming hook
  deps-tuple machinery (#4195).
- `NodeId::as_u64()` accessor in `reinhardt-core` so external callers (such
  as `Trackable::signal_id`) can obtain the underlying counter value.
- New `Component { prop: val, @event: handler, child_element { ... } }`
  invocation syntax inside `page!` bodies. Components are functions
  matching `fn <name>(props: <NameProps>) -> Page` where `<NameProps>`
  derives `bon::Builder`. The legacy positional form
  `{component_fn(args)}` continues to work unchanged. Spec §3.5.
- `bon` added as a `reinhardt-pages` runtime dependency. Staged for
  removal under spec §10 once `#[derive(PageProps)]` /
  `#[component]` proc-macros take over the prop-struct generation.
- `form!` macro fields now accept optional generic type parameters
  (`HiddenField<i64>`, `ChoiceField<bool>`, `MultipleChoiceField<String>`,
  `JsonField<MyStruct>`) to forward typed values to `#[server_fn]` handlers
  instead of always stringifying them (#4397)
- `IpAddressField` is now specialized to `Option<IpAddr>` in generated code
- Fields without a generic parameter default to `String` for backward
  compatibility
- `reinhardt_pages::router::request` submodule re-exports the
  Manouche DSL v2 spec §4.3 `FromRequest` building blocks
  (`FromRequest`, `RouteContext`, `ExtractError`, `PathParam<T>`,
  `QueryParam<T>`) from `reinhardt_urls::routers::client_router::from_request`
  so application code can write
  `use reinhardt_pages::router::request::FromRequest;` matching the
  spec's namespace. The legacy non-generic `PathParam` re-exported at
  `reinhardt_pages::router::PathParam` (deprecated since `0.1.0-rc.27`)
  is unrelated and remains in place during its deprecation cycle.
  (Refs #4668)
- `use_resource(fetcher, deps)` — unified async data-fetching hook, the
  resource counterpart of `use_effect`. `()` deps fetch once on mount; listed
  `Trackable` deps (`Signal` / `Memo` / `Resource`) drive automatic refetch.
  Available on all targets: the native/SSR path renders the `Loading` state and
  the client performs the real fetch after hydration, mirroring `use_action`.
  Supersedes the deprecated `create_resource` / `create_resource_with_deps`.

### Changed

- *(pages)* unify spawn into platform/, expose spawn_task from prelude

### Deprecated

- `use_effect_event` and `use_effect_event_with` are deprecated.
  Option A semantics make them structurally redundant — the wrapped
  closure of `use_effect`/`use_layout_effect` already runs without
  auto-tracking. Use `use_callback(f, deps)` for stable identity, or
  read latest values via `.get_untracked()` inside the closure.
  Scheduled for removal in v0.3.0.
- `create_resource` and `create_resource_with_deps` are deprecated in favor of
  the unified, cross-target `use_resource(fetcher, deps)` (`()` deps = fetch
  once; `(signal,)` deps = refetch on change). Thin forwarding shims are kept;
  scheduled for removal in v0.3.0.

### Removed

- **BREAKING**: `watch { ... }` block is removed. The body of `watch` can be
  inlined as-is; the new auto-wrap subsumes it. The validator emits a
  pointer at the `cargo make migrate-manouche-v2` codemod when it sees a
  surviving `watch` block.
- **`src/router/core.rs`** (6 items, all deprecated `0.1.0-rc.27`,
  refs #4234 / cloud#578) — `PathError`, `RouterError`,
  `ClientRouteMatch` (RouteMatch), `ClientRoute` (Route),
  `ClientRouter` (Router), `NavigationSubscription`. All relocated
  to `reinhardt_urls::routers`.
- **`src/router/pattern.rs`** (1 item, `0.1.0-rc.27`) — `ClientPathPattern`
  (PathPattern). Use `reinhardt_urls::routers::ClientPathPattern`.
- **`src/router/params.rs`** (1 item, `0.1.0-rc.27`) — `Path` extractor.
  Use `reinhardt_urls::routers::Path`.

#### BREAKING CHANGES — Router Relocation Cleanup

**First of two PRs** removing reinhardt-pages's 16 RC-deprecated items per
STABILITY_POLICY § SP-4 (umbrella Issue
[#4520](https://github.com/kent8192/reinhardt-web/issues/4520)).
This PR removes the 8 router-relocation items (relocated to
`reinhardt_urls::routers` since `0.1.0-rc.27`); the remaining 8
items (App struct, launcher legacy, use_reducer migration, MSW
migration, CSRF auto-inject) require selective Edit and ship in
the follow-up `feat(pages)!:` PR.

Removed in this PR (8 items):

- **`src/router/core.rs`** (6 items, all deprecated `0.1.0-rc.27`,
  refs #4234 / cloud#578) — `PathError`, `RouterError`,
  `ClientRouteMatch` (RouteMatch), `ClientRoute` (Route),
  `ClientRouter` (Router), `NavigationSubscription`. All relocated
  to `reinhardt_urls::routers`.
- **`src/router/pattern.rs`** (1 item, `0.1.0-rc.27`) — `ClientPathPattern`
  (PathPattern). Use `reinhardt_urls::routers::ClientPathPattern`.
- **`src/router/params.rs`** (1 item, `0.1.0-rc.27`) — `Path` extractor.
  Use `reinhardt_urls::routers::Path`.

### Fixed

- *(pages)* enable security feature for WASM builds
- *(auth)* replace InternalUser in UserManager public API with ManagedUser
- *(pages)* restore brace-form component invocation tests
- *(pages)* remove redundant #[builder(default)] from Option field
- *(pages)* add missing and regenerate stale trybuild .stderr files
- *(pages)* remove component_missing_required_prop compile-fail test
- *(pages)* correct component_missing_required_prop compile-fail test
- *(pages)* use brace-form Card {} inside page! for required-prop test
- *(pages)* document #[allow(dead_code)] on CardProps::item in compile-fail test
- *(docs)* resolve rustdoc intra-doc link errors on develop/0.2.0
- *(docs)* resolve additional rustdoc link errors for feature-gated types
- *(pages)* make SSR hydration IDs render-scoped
- *(pages)* keep deprecated reinhardt_pages::spawn re-export shim
- *(pages)* avoid reentrant reactive mount borrow
- *(pages)* rerender SPA links after cleanup
- *(pages)* render dynamic radio choices
- *(forms)* stabilize form runtime and validator parity
- Dynamic `ChoiceField { widget: RadioSelect, choices_from: ... }` forms now
  render one radio input per loaded choice, including labels and submitted
  `value` attributes. Client-side `form!` submissions also honor
  `success_url:` after successful server function calls (#5109).
- Resource dependency-change refetch now actually fires. The old
  `create_resource_with_deps` dropped its tracking `Effect` handle immediately
  after creation, so the `Effect` was disposed and never re-ran — automatic
  refetch on dependency change silently never happened (and the only covering
  test was excluded on every target by a contradictory `cfg`). The unified
  `use_resource` stores the `Effect` inside the returned `Resource` so it stays
  alive for the Resource's lifetime, and applies the `defer_yield` microtask
  deferral (#3316) on the dependency-driven path as well, not only the
  fetch-once path.

- *(commands)* align startproject scaffold defaults
- *(build)* address CodeRabbit review feedback
- *(ci)* recover develop release-plz prerelease
- *(auth,urls,pages)* remove stale references and fix latent clippy lints
- *(ci)* resolve all pre-existing compilation failures on develop/0.2.0
- *(ci)* update test snapshots and assertions for v0.2.0 breaking changes
- *(pages)* address CodeRabbit use_resource review
- *(forms)* address review and CI failures
- *(forms)* address review feedback
- *(forms)* address bot review feedback

### Performance

- *(commands)* notify browsers after hot reload rebuilds
- *(pages)* batch generated page attributes
- *(pages)* prune unused runtime parser deps
- *(pages)* trim wasm dependency graph
- *(pages)* gate non-browser wasm modules
- *(pages)* hot patch static page edits
- *(build)* tune dev profile for hot reload
- *(build)* measure cold workspace build
- atomize facade dependency feature gates

### Documentation

- *(release)* enforce public API doc coverage
- *(pages)* document spawn compat shim module
- *(pages)* make wasm spawn_task example testable (ignore -> no_run)

### Testing

- *(pages)* replace skeleton spawn_task test with behavior assertion
- *(forms)* align form runtime UI fixtures
- *(ci)* refresh release CI expectations
- *(pages)* address CodeRabbit review on hydration tests
- *(pages)* sync trybuild .stderr with rustfmt-collapsed empty braces

### Styling

- apply formatter fixes across workspace
- format files from merge resolution
- apply rustfmt to non-DSL files on develop/0.2.0
- *(pages)* reorder form component imports to satisfy rustfmt

### Maintenance

- forward merge main v0.1.1 changes into develop 0.2.0
- forward merge main v0.1.2 changes into develop 0.2.0
- *(ci)* merge develop into release docs fix

### Other

- resolve conflicts with develop/0.2.0

## [0.1.0](https://github.com/kent8192/reinhardt-web/compare/reinhardt-pages@v0.1.0-rc.30...reinhardt-pages@v0.1.0) - 2026-05-22

Initial stable release of `reinhardt-pages` as part of the
reinhardt-web 0.1.0 release. `reinhardt-pages` provides the
WASM-based frontend framework with a Django-like API: declarative
`page!` / `head!` / `form!` DSLs, a reactive `Signal<T>` runtime,
a SPA router and `ClientLauncher`, and `#[server_fn]` RPC stubs
shared with the server-side handler.

For the workspace-wide release narrative, see the [root CHANGELOG](https://github.com/kent8192/reinhardt-web/blob/main/CHANGELOG.md#010---2026-05-22).
Per-prerelease history is in the [Release Discussions](https://github.com/kent8192/reinhardt-web/discussions/categories/release).

### Capabilities at 0.1.0

- **`page!` declarative DSL** — anonymous component bodies with
  70+ compile-time-validated HTML elements, closure-style props,
  `@event` handlers, `if`/`else`/`for` control flow, reactive
  `watch` blocks driven by `Signal<T>`, and accessibility checks
  (img alt, button labels).
- **`head!` macro for SSR metadata** — title / meta / link /
  script / style elements with SSR head injection.
- **WASM SPA launcher** — `ClientLauncher` builder with built-in
  document-level link interception (opt out with
  `.intercept_links(false)`), `before_launch` / `after_launch`
  lifecycle hooks, `on_path` / `on_path_pattern` declarative
  effects with `PathCtx` portals, and JWT token management
  primitives for authenticated WASM clients.
- **Explicit navigation subscription API** — `Router::on_navigate`
  delivers a synchronous callback after every successful `push` /
  `replace` and on `popstate`, decoupled from Signal / Effect
  auto-tracking; `use_router()` hook, `RouterHandle`, free
  `navigate()`, and `try_with_spa_router` round out the SPA
  navigation surface.
- **Server Functions with DI** — `#[server_fn]` generates both the
  WASM client stub and the server-side handler, with JSON / URL /
  msgpack codecs, per-request DI context forking, `FromRequest`
  extractor support, CSRF auto-pass, and HTTP status code
  preservation for DI auth errors.
- **HMR (opt-in `hmr` feature)** — file-watcher-driven CSS / DOM
  updates pushed over a WebSocket scheme that is auto-selected
  based on `window.location.protocol` (secure variant on HTTPS).

### Notable Breaking Changes

- **`ClientLauncher::on_navigate`** ([#4117](https://github.com/kent8192/reinhardt-web/discussions/4117)) — SPA navigation routes through `Router::on_navigate` instead of relaunching the client per navigation; remove any manual `ClientLauncher::launch` wiring tied to the old model.
- **`client_router::history` dedup** ([#4219](https://github.com/kent8192/reinhardt-web/discussions/4219)) — the duplicate `history` module under `client_router` is removed; consume `pages::router::history` (or the canonical `reinhardt-urls` `client_router::history` on WASM) instead.
- **`pages::Router` deprecated** in favour of `urls::ClientRouter`; migration is summarised in [#4234](https://github.com/kent8192/reinhardt-web/issues/4234).
- **`form! on_success:` closure lift** ([#4624](https://github.com/kent8192/reinhardt-web/issues/4624)) — type-annotated `on_success: |value: T| ...` closures now lift to the outer scope (allowing route-parameter capture) and require `Send + Sync`; unannotated closures (`|value|`, `|_value|`) keep the historical inline emit.
- **MSW mocks replace `MockFetch`** — `MockFetch` and `mock_server_fn` are deprecated; opt into MSW-style mocking via the `msw` feature and the generated `MockableServerFn` trait.

### Migration Notes

- Replace `pages::Router` consumers with `urls::ClientRouter`; the
  former is `#[deprecated]` and downstream `#[allow(deprecated)]`
  attribute may be needed during the transition.
- Move ad-hoc SPA link interception into the built-in interceptor
  by removing your handler and letting `ClientLauncher::launch`
  install one by default — or pass `.intercept_links(false)` to
  preserve the previous behaviour.
- For dynamic `form! { on_success: |value: T| ... }` callbacks that
  need to capture an enclosing `qid` or other route parameters,
  add an explicit type annotation on the closure parameter.
- For the workspace-wide migration narrative, see the [root CHANGELOG](https://github.com/kent8192/reinhardt-web/blob/main/CHANGELOG.md#010---2026-05-22).
