# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.3.16](https://github.com/kent8192/reinhardt-web/compare/reinhardt-pages-macros@v0.3.15...reinhardt-pages-macros@v0.3.16) - 2026-09-08

### Fixed

- *(pages)* render form field attributes and help text
- *(pages)* preserve form ownership across hydration and reset
- *(pages)* evaluate static select choice expressions once
- *(pages)* preserve reconciled defaults during hydration

### Maintenance

- merge main into radio group accessibility
- *(pages)* merge main field metadata and radio groups

## [0.3.0](https://github.com/kent8192/reinhardt-web/compare/reinhardt-pages-macros@v0.2.0...reinhardt-pages-macros@v0.3.0) - 2026-06-28

Stable release of `reinhardt-pages-macros` for the Reinhardt 0.3.0 line. This
entry consolidates the 0.3.0 release-candidate series into one
stable release section.

### Migration Notes

- Review the root CHANGELOG and `instructions/MIGRATION_0.3.md` before upgrading from 0.2.x.

### Added

- *(params)* generalize cookie extractors
- *(pages)* add route-backed component macros
- *(forms)* add dynamic FieldArray runtime support
- Added `#[derive(FromRequest)]`, `#[page_props]`, and
  `#[component("/path", "name")]` macro codegen for route-backed page
  components.

### Fixed

- *(todo-check)* clear public api audit markers
- *(di)* support trait-based inject wrapper resolution
- *(di)* preserve Depends inject fallback
- *(pages)* resolve generated builder dependency path
- *(pages)* address CodeRabbit component macro review
- *(forms)* propagate boxed field arrays to page macros
- *(forms)* box page macro collection validation
- *(forms)* address field array review feedback
- *(forms)* harden field array review paths
- Fixed generated builder derives to resolve through `reinhardt-pages` so
  downstream crates do not need a direct `bon` dependency.

### Performance

- *(pages)* reduce native endpoint dispatch allocations

### Maintenance

- merge main into develop/0.3.0
- merge develop 0.3.0 into build-time perf branch
- merge develop/0.3.0 into component route macros

## [0.2.0](https://github.com/kent8192/reinhardt-web/compare/reinhardt-pages-macros@v0.1.3...reinhardt-pages-macros@v0.2.0) - 2026-06-11

Stable release of `reinhardt-pages-macros` for the Reinhardt 0.2.0 line. This
entry consolidates the 0.2.0 release-candidate series into one
stable release section.

### Migration Notes

- Run the Manouche v2 migration codemod for `page!` syntax changes, then review implicit captures manually.
- See [`instructions/MIGRATION_0.2.md`](../../../instructions/MIGRATION_0.2.md) for the workspace migration checklist.

### Breaking Changes

- *(pages-macros)* [**breaking**] implement compile-time hook deps verification

### Added

- *(pages-macros)* add typed value conversion in WASM bind listener
- *(pages)* support keyed page list rendering
- *(pages-macros)* [**breaking**] implement compile-time hook deps verification
- *(forms)* add typed use_form ergonomics
- feat!(forms): route use_form through form definitions

### Changed

- *(pages)* unify spawn into platform/, expose spawn_task from prelude
- `#[server_fn]` now emits the `marker` module on wasm
  unconditionally — previously the marker was only present when the
  `msw` feature was active, which forced `#[url_patterns(mode = unified)]`
  closure bodies that referenced `my_fn::marker` to be wrapped in
  `#[cfg(native)] { ... }` arms. The optional `Args` struct and
  `MockableServerFn` impl remain gated behind `#[cfg(feature = "msw")]`
  inside the marker module
  ([#4711](https://github.com/kent8192/reinhardt-web/issues/4711)).
- `#[server_fn]` emits `impl ServerFnMetadata for marker` on every
  emission path, providing a single source of truth for `PATH`,
  `NAME`, `CODEC`, and `INJECTED_PARAMS` across the cfg boundary.
  Duplicate constant declarations have been removed from
  `impl ServerFnRegistration` (native) and `impl MockableServerFn`
  (msw) blocks
  ([#4711](https://github.com/kent8192/reinhardt-web/issues/4711)).

### Fixed

- *(auth)* replace InternalUser in UserManager public API with ManagedUser
- *(pages)* render dynamic radio choices
- *(forms)* stabilize form runtime and validator parity

- *(forms)* omit unreachable focus path for empty forms
- *(ci)* recover develop release-plz prerelease
- *(forms)* address review and CI failures
- *(forms)* address review feedback
- *(forms)* address bot review feedback

### Performance

- *(pages)* batch generated page attributes

### Documentation

- *(release)* enforce public API doc coverage
- *(pages)* document Clone requirement for keyed for iterators

### Testing

- *(ci)* refresh release CI expectations

### Maintenance

- forward merge main v0.1.1 changes into develop 0.2.0
- *(ci)* merge develop into release docs fix

## [0.1.0](https://github.com/kent8192/reinhardt-web/compare/reinhardt-pages-macros@v0.1.0-rc.30...reinhardt-pages-macros@v0.1.0) - 2026-05-22

Initial stable release of `reinhardt-pages-macros` as part of the
reinhardt-web 0.1.0 release. This crate provides the procedural
macros that back `reinhardt-pages`: the `page!`, `head!`, and
`form!` declarative DSLs, plus the `#[server_fn]` attribute macro
that emits matched WASM client stubs and server-side handlers.

For the workspace-wide release narrative, see the [root CHANGELOG](https://github.com/kent8192/reinhardt-web/blob/main/CHANGELOG.md#010---2026-05-22).
Per-prerelease history is in the [Release Discussions](https://github.com/kent8192/reinhardt-web/discussions/categories/release).

### Capabilities at 0.1.0

- **`page!` macro** — anonymous component DSL with closure-style
  props, 70+ HTML elements validated at compile time, `@event:
  handler` syntax, `if`/`else`/`for` control flow, `watch` blocks
  for `Signal<T>`-driven re-rendering, dynamic expressions for
  `img src`, and accessibility / XSS validation at expansion time.
- **`head!` macro** — title, meta, link, script, and style
  elements for SSR head metadata injection.
- **`form!` macro** — typed form fields (`CharField`, `EmailField`,
  `IntegerField`, `FileField`, `ImageField`, `SubmitButton`, …),
  widget customization, server / client validators, `derived`
  computed values, `FieldGroup`, custom wrapper and SVG icon
  elements, slots, two-way `Signal` binding, CSRF protection,
  `initial_loader`, dynamic `choices_loader`, `autocomplete`
  attribute, and `strip_arguments` for downstream `server_fn`
  shaping.
- **`#[server_fn]` attribute macro** — WASM client stub plus
  server-side handler, custom endpoint paths, codec selection
  (`json` / `url` / `msgpack`), `#[reinhardt::inject]` parameter
  auto-detection, `FromRequest` extractor support, per-request DI
  context forking, and DI error → HTTP status mapping (with
  redacted 500 bodies).
- **MSW-style mocking (`msw` feature)** — generates a
  `MockableServerFn` trait per `#[server_fn]` so tests can stub
  RPC endpoints with the same typed surface they see in
  production.
- **Closure-lift pipeline** — `form! { on_success: |value: T|
  ... }` closures with explicit type annotations lift to the
  outer scope so the body can capture enclosing locals (e.g., a
  `qid` route parameter); `success_url:` and inner `watch`
  closures observe the same lift semantics. Unannotated closures
  keep the historical inline emit.

### Notable Breaking Changes

- **`form! on_success:` lift requires `Send + Sync`** ([#4623](https://github.com/kent8192/reinhardt-web/issues/4623), [#4624](https://github.com/kent8192/reinhardt-web/issues/4624)) — type-annotated `on_success: |value: T| ...` closures are now lifted to the outer construction block and therefore must be `Send + Sync`; unannotated closures (`|value|`, `|_value|`) are unaffected.
- **`#[export_endpoints]` removed in favour of `define_views!` / `flatten_imports!`** ([#3768](https://github.com/kent8192/reinhardt-web/discussions/3768), [#3783](https://github.com/kent8192/reinhardt-web/discussions/3783)) — multi-file view modules use the renamed declarative macro for stable-Rust compatibility.
- **`is_safe_url` inlined into `pages-macros`** — the macro no longer pulls in `reinhardt-core` as a dependency; downstream code that imported the helper through this crate should source it from `reinhardt-core` directly.

### Migration Notes

- Annotate `on_success` closure parameters with explicit types
  (`|value: T| ...`) when you need to capture enclosing-scope
  locals from `form!` bodies; ensure captured types are `Send +
  Sync`.
- Replace any `#[export_endpoints]` attribute usage with the
  `define_views!` / `flatten_imports!` declarative macro from
  `reinhardt-pages`.
- For the workspace-wide migration narrative, see the [root CHANGELOG](https://github.com/kent8192/reinhardt-web/blob/main/CHANGELOG.md#010---2026-05-22).
