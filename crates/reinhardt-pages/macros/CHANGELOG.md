# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Fixed

- *(pages)* preserve rejected SPA navigation and external HTTPS destinations in generated form redirects

## [0.4.0-alpha.4](https://github.com/kent8192/reinhardt-web/compare/reinhardt-pages-macros@v0.4.0-alpha.3...reinhardt-pages-macros@v0.4.0-alpha.4) - 2026-08-04

### Added

- *(pages)* generate model-backed forms
- *(pages)* lower typed custom event handlers

### Documentation

- *(pages)* document typed custom event payloads

### Fixed

- *(forms)* harden generated form persistence semantics
- *(pages)* preserve valid boolean and fractional controls
- *(pages)* render stored model form values
- *(pages)* reject unsupported model form widgets
- *(forms)* enforce generated model constraints
- *(forms)* handle temporal and assigned model inputs
- *(forms)* preserve model form field contracts
- *(pages)* retain native model form submission
- *(pages)* validate model widget overrides
- *(forms)* complete model-backed form submission
- *(forms)* enforce model form submission policies
- *(forms)* harden native model form decoding
- *(forms)* preserve model form defaults
- *(forms)* harden model form input handling
- *(forms)* preserve secure model form defaults
- *(forms)* preserve untouched model controls
- *(forms)* synchronize hydrated model submits
- *(forms)* complete model form submission contracts
- *(forms)* validate model form submission boundaries
- *(forms)* reject unsupported model reactivity
- *(forms)* preserve native model form semantics
- *(forms)* validate inline and runtime model form state
- *(forms)* preserve native defaults and control values
- *(forms)* preserve model-backed form state
- *(forms)* reset omitted model form fields
- *(forms)* retain untouched boolean defaults
- *(pages)* reject unsupported model form callbacks
- *(forms)* harden generated model form boundaries
- *(forms)* preserve optional model form state
- *(forms)* enforce model form overrides
- *(forms)* reserve generated form namespaces
- *(pages)* keep model form controls in sync
- *(forms)* preserve native range defaults
- *(forms)* synchronize defaults and persistence state
- *(forms)* preserve transactional retry semantics
- *(forms)* preserve model form control semantics
- *(forms)* validate inline formset retries
- *(forms)* align native form validation
- *(pages)* preserve server function fallback syntax
- *(forms)* restore model form CI coverage
- *(forms)* address model form review feedback
- *(pages)* preserve typed custom handler spans
- *(pages)* assert typed custom event parameter path
- *(pages)* address typed custom event review
- *(pages)* match custom event token spacing in tests
- *(pages)* retain query key wrapper types
- *(ci)* repair all-feature unit regressions

### Maintenance

- auto-fix fmt and clippy

### Other

- sync develop/0.4.0 into query client v2

### Styling

- *(pages)* format typed custom event codegen

### Testing

- *(forms)* align generated model form expectations
- *(pages)* verify async custom event lowering
- *(pages)* compare zero-argument custom event lowering
- *(pages)* relax typed custom-event lowering assertion
- *(pages)* harden typed custom event coverage

## [0.4.0-alpha.2](https://github.com/kent8192/reinhardt-web/compare/reinhardt-pages-macros@v0.4.0-alpha.1...reinhardt-pages-macros@v0.4.0-alpha.2) - 2026-07-23

### Fixed

- *(pages)* ignore named interpolation keys in captures
- *(pages)* scope named macro argument capture skipping
- *(pages)* preserve captures for shadowable macros
- *(pages)* resolve named macro paths safely

### Fixed

- Preserve mutable and destructuring `#[inject]` bindings in `#[server_fn]`
  implementation signatures while forwarding resolved values by hygienic
  identifiers that cannot collide with user parameter bindings.

## [0.4.0-alpha.1](https://github.com/kent8192/reinhardt-web/compare/reinhardt-pages-macros@v0.3.2...reinhardt-pages-macros@v0.4.0-alpha.1) - 2026-07-21

### Added

- *(pages)* apply transactional template hot patches

### Fixed

- *(pages)* preserve controlled binding state
- *(pages)* preserve controlled select projection
- resolve server function set review findings
- *(pages)* resolve server function set review findings
- *(pages)* envelope server function failures
- *(pages)* envelope server function request failures
- *(pages)* classify server function failures
- *(pages)* preserve structured server form errors
- *(pages)* retain custom error status
- *(release)* restore develop prerelease lifecycle
- *(pages)* prevent invalid head macro doc link

### Maintenance

- merge main into develop/0.4.0
- refresh main forward merge from develop/0.4.0
- merge develop/0.4.0 into server function set branch

### Other

- resolve develop/0.4.0 conflicts for [[#5676](https://github.com/kent8192/reinhardt-web/issues/5676)](https://github.com/kent8192/reinhardt-web/issues/5676)
- sync develop/0.4.0 into server function set
- sync develop/0.4.0 into structured server errors

### Added

- Add fn-form `#[server_fnset]` generation for named low-level marker sets and
  model-backed CRUD namespaces.
- Add impl-form checked standard overrides and custom actions with detail and
  transaction metadata, normalized endpoints, and compile-time diagnostics for
  invalid names, links, lookups, signatures, collisions, and REST-only options.
- Generate all six checked standard override paths, including the dedicated
  transaction-only create context.

### Changed

- **BREAKING**: `#[component]` now requires `name = "..."` for route names and
  rejects positional string or bare identifier route names.

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
  `#[component("/path", name = "name")]` macro codegen for route-backed page
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
