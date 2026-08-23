# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.4.0-alpha.7](https://github.com/kent8192/reinhardt-web/compare/reinhardt-core@v0.4.0-alpha.6...reinhardt-core@v0.4.0-alpha.7) - 2026-08-19

### Documentation

- update version references to v0.3.6
- update version references to v0.3.7
- update version references to v0.3.8

### Fixed

- *(settings)* preserve explicit secret references

### Maintenance

- merge main into develop/0.4.0

### Security

- *(auth)* preserve JWT secret field compatibility

### Testing

- *(settings)* move JWT secret contract integration
- *(macros)* align model UI support with filter bindings
- *(core)* cover localized validation messages
- *(core)* cover i18n fallback formatting
- *(core)* cover schema draft metadata
- *(core)* cover URL validator messages
- *(core)* cover range custom errors

## [0.4.0-alpha.6](https://github.com/kent8192/reinhardt-web/compare/reinhardt-core@v0.4.0-alpha.5...reinhardt-core@v0.4.0-alpha.6) - 2026-08-06

### Documentation

- *(release)* restore coherent alpha.3 references

## [0.4.0-alpha.4](https://github.com/kent8192/reinhardt-web/compare/reinhardt-core@v0.4.0-alpha.3...reinhardt-core@v0.4.0-alpha.4) - 2026-08-04

### Added

- *(core)* add model form schema contracts
- *(macros)* generate typed model form schemas
- *(forms)* [**breaking**] make generated model forms async
- *(db)* add typed queryset retrieval proofs

### Documentation

- *(forms)* document generated model forms

### Fixed

- *(macros)* preserve model form relation schemas
- *(macros)* reject repeated setter name collisions
- *(forms)* enforce model form persistence invariants
- *(forms)* align model form automatic defaults
- *(forms)* harden generated form persistence semantics
- *(forms)* close residual model form review gaps
- *(macros)* omit generated fields from model forms
- *(forms)* enforce generated model constraints
- *(forms)* handle temporal and assigned model inputs
- *(forms)* preserve model form field contracts
- *(forms)* preserve specialized field constraints
- *(forms)* preserve exact generated constraints
- *(forms)* complete model-backed form submission
- *(forms)* enforce model form submission policies
- *(forms)* harden native model form decoding
- *(forms)* preserve model form defaults
- *(forms)* harden model form input handling
- *(forms)* preserve secure model form defaults
- *(forms)* honor relation form editability
- *(forms)* preserve nullable relation clears
- *(forms)* complete model form submission contracts
- *(forms)* validate model form submission boundaries
- *(forms)* preserve native model form semantics
- *(forms)* preserve native defaults and control values
- *(forms)* preserve model-backed form state
- *(forms)* reserve the native csrf field name
- *(forms)* enforce policy in typed setters
- *(forms)* harden generated model form boundaries
- *(forms)* preserve optional model form state
- *(forms)* enforce model form overrides
- *(forms)* reserve generated form namespaces
- *(forms)* preflight deferred child validators
- *(forms)* require generated relation ids
- *(forms)* prevent duplicate MySQL form inserts
- *(forms)* preserve native range defaults
- *(forms)* prevent duplicate create retries
- *(forms)* synchronize defaults and persistence state
- *(forms)* synchronize transaction-backed form state
- *(forms)* preserve transactional retry semantics
- *(forms)* support trusted inline foreign keys
- *(forms)* preserve model form control semantics
- *(forms)* preserve nested form retries
- *(forms)* validate inline formset retries
- *(forms)* align native form validation
- *(forms)* use serde-json for trusted fields
- *(forms)* support nullable model form relations
- *(forms)* preserve trusted non-editable model values
- *(forms)* address model form review feedback
- *(orm)* harden typed retrieval helpers
- *(orm)* harden queryset retrieval helpers
- *(db)* validate typed queryset ordering fields
- *(db)* preserve queryset bulk lookup columns
- *(db)* retain typed retrieval field provenance
- *(db)* preserve typed queryset retrieval compatibility
- *(db)* preserve generated field reference defaults
- *(db)* resolve typed retrieval review feedback
- *(db)* integrate develop/0.4.0 with typed upsert builders
- *(db)* merge concurrent upsert review updates
- *(commands)* honor managed migration settings

### Maintenance

- merge develop/0.4.0 into issue [[#5845](https://github.com/kent8192/reinhardt-web/issues/5845)](https://github.com/kent8192/reinhardt-web/issues/5845)

### Other

- sync develop/0.4.0 into inspectdb
- integrate develop migration updates

### Testing

- *(macros)* align model support with field proofs

## [0.4.0-alpha.2](https://github.com/kent8192/reinhardt-web/compare/reinhardt-core@v0.4.0-alpha.1...reinhardt-core@v0.4.0-alpha.2) - 2026-07-23

### Fixed

- *(pages)* preserve borrowed signal bindings
- *(pages)* accept mutable signal borrows

## [0.4.0-alpha.1](https://github.com/kent8192/reinhardt-web/compare/reinhardt-core@v0.3.2...reinhardt-core@v0.4.0-alpha.1) - 2026-07-21

### Added

- *(query,migrations)* [**breaking**] support typed generated column expressions

### Fixed

- *(benchmark,macros)* address develop merge review feedback
- *(migrations)* address generated column review feedback
- *(migrations)* handle generated column followups
- *(db)* preserve generated-column replacement metadata
- *(db)* address generated column review feedback
- *(db)* address generated column review follow-up
- *(db)* complete generated column review repairs
- *(db)* harden generated column edge cases
- *(db)* reject invalid generated column definitions
- *(macros)* gate MySQL generated-column test
- *(macros)* preserve model schema metadata
- *(testkit)* harden model-derived schema metadata
- *(db)* harden typed relation traversal
- *(orm)* close typed relation traversal gaps
- *(orm)* support manual relation targets
- *(macros)* honor reverse relation to_field
- *(db)* validate typed relation load paths
- *(macros)* reject ambiguous composite reverse relations
- *(orm)* guard composite typed relation paths
- *(pages)* restore explicit deps compatibility
- *(ci)* terminate cfg aliases macro invocations
- *(pages)* retain reactive owners through review edge cases
- *(reactive)* enforce explicit dependencies and memo invalidation
- *(pages)* complete explicit dependency migration
- *(reactive)* isolate scope cleanup observers
- *(core)* restore reactive notification coverage
- *(pages)* resolve controlled binding review feedback
- restore atomic ORM release compatibility
- *(core)* ignore stale reactive subscribers
- *(pages)* bind action button attributes reactively
- *(pages)* dispose reactive attribute effects
- *(pages)* materialize reactive test attributes
- *(pages)* preserve reactive attribute precedence
- *(pages)* reconcile reactive attributes
- *(pages)* normalize boolean attribute names
- *(core)* satisfy reactive attribute lint requirements
- *(core)* update PageElement parts regression test
- *(release)* restore develop prerelease lifecycle
- *(orm)* resolve to-field physical columns

### Maintenance

- merge latest main into develop forward-merge
- merge latest develop changes into typed JSON PR
- merge develop/0.4.0 into typed traversal branch
- merge develop/0.4.0 into issue 5575 branch

### Other

- resolve develop/0.4.0 conflicts for [[#5676](https://github.com/kent8192/reinhardt-web/issues/5676)](https://github.com/kent8192/reinhardt-web/issues/5676)
- resolve develop/0.4.0 into model enum fields
- sync develop/0.4.0 into server function set

### Testing

- *(reactive)* cover explicit deps macro compatibility
- *(macros)* repair model UI fixture contracts

### Added

- Generate typed `unique_<field>()` accessors for single-column primary keys,
  fields declared with `unique = true`, and unconditional single-field unique
  constraints, backed by model-owned compile-time field proofs.
- Add development-only `Page` template metadata and dynamic-slot carriers for
  state-preserving Pages HMR.
## [0.3.10](https://github.com/kent8192/reinhardt-web/compare/reinhardt-core@v0.3.9...reinhardt-core@v0.3.10) - 2026-08-22

### Maintenance

- update Cargo.toml dependencies

## [0.3.9](https://github.com/kent8192/reinhardt-web/compare/reinhardt-core@v0.3.8...reinhardt-core@v0.3.9) - 2026-08-21

### Added

- *(core)* add structured field error formatters
- *(core)* format errors across serializer fields
- *(rest)* expose configurable field errors
- *(serializers)* store error formatters on serializer fields

### Documentation

- *(serializers)* link FieldErrorMessages from module rustdoc

### Fixed

- *(serializers)* alias field error formatter to satisfy type complexity lint
- *(serializers)* keep field structs constructible with error messages

### Other

- bring main into configurable field error messages
- keep field error formatters with JSON extraction

### Testing

- *(core)* satisfy boolean assertion lint
- *(serializers)* cover required, fallback, and field-type stability
- *(macros)* keep server_only Info compile-fail on SecretInfo
- *(macros)* isolate server_only Info compile-fail from serde bounds

## [0.3.8](https://github.com/kent8192/reinhardt-web/compare/reinhardt-core@v0.3.7...reinhardt-core@v0.3.8) - 2026-08-16

### Testing

- *(core)* cover localized validation messages
- *(core)* cover i18n fallback formatting
- *(core)* cover schema draft metadata
- *(core)* cover URL validator messages
- *(core)* cover range custom errors

## [0.3.7](https://github.com/kent8192/reinhardt-web/compare/reinhardt-core@v0.3.6...reinhardt-core@v0.3.7) - 2026-08-12

### Maintenance

- update Cargo.toml dependencies

## [0.3.6](https://github.com/kent8192/reinhardt-web/compare/reinhardt-core@v0.3.5...reinhardt-core@v0.3.6) - 2026-08-04

### Fixed

- *(settings)* preserve explicit secret references

### Security

- *(auth)* preserve JWT secret field compatibility

### Testing

- *(settings)* move JWT secret contract integration
- *(macros)* align model UI support with filter bindings

## [0.3.5](https://github.com/kent8192/reinhardt-web/compare/reinhardt-core@v0.3.4...reinhardt-core@v0.3.5) - 2026-08-02

### Maintenance

- update Cargo.toml dependencies

## [0.3.4](https://github.com/kent8192/reinhardt-web/compare/reinhardt-core@v0.3.3...reinhardt-core@v0.3.4) - 2026-07-30

### Maintenance

- update Cargo.toml dependencies

## [0.3.3](https://github.com/kent8192/reinhardt-web/compare/reinhardt-core@v0.3.2...reinhardt-core@v0.3.3) - 2026-07-28

### Added

- *(macros)* pass raw requests alongside extractors

### Fixed

- *(macros)* prevent extractor shadowing of raw requests
- *(macros)* preserve generated raw request bindings
- *(macros)* classify structured route extractors
- *(macros)* harden raw request extractor forwarding
- *(macros)* extract raw route parameters hygienically
- *(macros)* preserve raw request names in wrappers
- *(macros)* classify wrapped request body extractors
- *(macros)* classify optional session extractors
- *(macros)* bind scalar model primary keys
- *(macros)* preserve ambiguous custom primary key fallbacks
- *(routes)* preserve request aliases named as extractors
- *(routes)* identify raw requests by type
- *(db)* retain primary key fallback for aliases
- *(routes)* preserve request aliases
- *(routes)* preserve injected and body parameter bindings
- *(routes)* preserve raw aliases and route attributes
- *(routes)* filter conditional wrapper attributes
- *(routes)* preserve cfg gates and multipart routes
- *(routes)* keep body extractors name-independent
- *(model)* emit typed primary key filters
- *(model)* restrict typed timestamp filters
- *(routes)* preserve safe wrapper attributes
- *(model)* preserve string primary key bindings
- *(routes)* filter nested wrapper instrumentation
- *(model)* restrict string primary key filters
- *(model)* normalize string primary key bindings
- *(macros)* preserve safe key and extractor fallbacks

### Testing

- *(routes)* assert body forwarding exactly
- *(model)* assert string key conversion exactly

## [0.3.1](https://github.com/kent8192/reinhardt-web/compare/reinhardt-core@v0.3.0...reinhardt-core@v0.3.1) - 2026-07-04

### Fixed

- *(ci)* unblock quick-xml security audit
- *(ci)* adapt XML parser to quick-xml 0.41

## [0.3.0](https://github.com/kent8192/reinhardt-web/compare/reinhardt-core@v0.2.0...reinhardt-core@v0.3.0) - 2026-06-28

Stable release of `reinhardt-core` for the Reinhardt 0.3.0 line. This
entry consolidates the 0.3.0 release-candidate series into one
stable release section.

### Migration Notes

- Review the root CHANGELOG and `instructions/MIGRATION_0.3.md` before upgrading from 0.2.x.

### Added

- *(pages)* add explicit asset loading helpers

### Changed

- [**breaking**] remove 0.3 deprecated public APIs

### Fixed

- *(ci)* pin brotli allocator dependency
- *(ci)* update Rust 1.96 UI stderr expectations
- *(pages)* align asset head helpers with review feedback
- add wasm safe model metadata substrate
- emit shared model info for parity
- keep server-only model PK metadata

### Maintenance

- migrate Rust toolchain to 1.96.0

## [0.2.0](https://github.com/kent8192/reinhardt-web/compare/reinhardt-core@v0.1.3...reinhardt-core@v0.2.0) - 2026-06-11

Stable release of `reinhardt-core` for the Reinhardt 0.2.0 line. This
entry consolidates the 0.2.0 release-candidate series into one
stable release section.

### Migration Notes

- Remove typed URL helper and resolver imports generated by older route macros.
- Use explicit reverse lookups and app-local helper functions instead of `ResolvedUrls` or flat accessor traits.
- See [`instructions/MIGRATION_0.2.md`](../../instructions/MIGRATION_0.2.md) for the workspace migration checklist.

### Breaking Changes

- *(core)* scaffold MIGRATION_0.2.md and document BREAKING CHANGES (refs [[#4520](https://github.com/kent8192/reinhardt-web/issues/4520)](https://github.com/kent8192/reinhardt-web/issues/4520))
- *(core)* [**breaking**] remove 0.1.0-rc deprecated URL resolver codegen (refs [[#4520](https://github.com/kent8192/reinhardt-web/issues/4520)](https://github.com/kent8192/reinhardt-web/issues/4520))

### Added

- *(core)* [**breaking**] remove 0.1.0-rc deprecated URL resolver codegen (refs [[#4520](https://github.com/kent8192/reinhardt-web/issues/4520)](https://github.com/kent8192/reinhardt-web/issues/4520))
- *(pages)* support keyed page list rendering
- *(macros)* compile-time kebab-case URL-name warning; drop dead url-resolver codegen
- *(pages)* unify resource hooks into use_resource(fetcher, deps)
- `reactive::deps` module with `Trackable` trait, `Deps` opaque container, and
  `IntoDeps` for tuples arity 0..=12. Enables the React-aligned
  `(closure, deps)` hook signatures in `reinhardt-pages` (#4195).
- `Effect::new_with_deps` and `Effect::new_with_deps_and_timing` constructors
  with Option A semantics (closure runs without active Observer; only listed
  deps subscribe) and optional `FnOnce` cleanup return.
- `Memo::new_with_deps` constructor mirroring the same Option A semantics for
  derived values. Adds an internal `MEMO_DIRTY` thread-local for type-agnostic
  invalidation by a hidden Layout-timing Effect that subscribes to the deps.
- `impl Trackable for Signal<T>` and `impl Trackable for Memo<T>`, enabling
  these primitives to participate in hook deps tuples.

- *(macros)* expose model info companions to wasm

### Changed

- [**breaking**] align develop/0.2.0 with main, preserving 8 feature crates

### Removed

- **`#[routes]` deprecated 2-level URL accessor codegen** (rc.16) —
  `urls.<app>()` is removed. Use the namespaced gateway
  `urls.server().<app>()` instead. Affects every project that depends
  on `#[routes]` and called the 2-level accessor.
- **`#[routes]` deprecated 2-level client URL accessor codegen**
  (rc.16) — `urls.<app>_client()` is removed. Use
  `urls.client().<app>()` instead.
- **`#[get(name = "...")]` / `#[post(name = "...")]` deprecated per-route
  resolver-trait codegen** (rc.16) — the legacy `Resolve<Name>` trait
  blanket-impl that produced flat `urls.<name>(...)` calls is removed.
  Use the namespaced accessors `urls.server().<app>().<name>(...)`
  emitted by the same macros.
- **`#[viewset]` flat ViewSet accessor codegen** (rc.29, Issue
  [#4507](https://github.com/kent8192/reinhardt-web/issues/4507)) —
  the `Resolve<Pascal>List` / `Resolve<Pascal>Detail` traits and the
  matching `urls.<basename>_list()` / `urls.<basename>_detail(id)`
  flat accessors are removed (4 generated items). Use
  `urls.server().<app>().<basename>_list()` /
  `urls.server().<app>().<basename>_detail(id)` instead.
- **`impl UrlResolverUnprefixed for ResolvedUrls`** override emitted by
  `#[routes]` — removed because the flat ViewSet accessor that
  required namespace-iterating fallback no longer exists. The
  `UrlResolverUnprefixed` trait itself is removed in
  `reinhardt-urls` PR.

#### BREAKING CHANGES

All `reinhardt-core` public APIs deprecated during the `0.1.0-rc.*`
cycle have been removed per STABILITY_POLICY § SP-4 ("APIs deprecated
during RC MUST survive until the next major version"). Refs umbrella
Issue [#4520](https://github.com/kent8192/reinhardt-web/issues/4520).

`reinhardt-core` removals (8 macro-emitted items):

- **`#[routes]` deprecated 2-level URL accessor codegen** (rc.16) —
  `urls.<app>()` is removed. Use the namespaced gateway
  `urls.server().<app>()` instead. Affects every project that depends
  on `#[routes]` and called the 2-level accessor.
- **`#[routes]` deprecated 2-level client URL accessor codegen**
  (rc.16) — `urls.<app>_client()` is removed. Use
  `urls.client().<app>()` instead.
- **`#[get(name = "...")]` / `#[post(name = "...")]` deprecated per-route
  resolver-trait codegen** (rc.16) — the legacy `Resolve<Name>` trait
  blanket-impl that produced flat `urls.<name>(...)` calls is removed.
  Use the namespaced accessors `urls.server().<app>().<name>(...)`
  emitted by the same macros.
- **`#[viewset]` flat ViewSet accessor codegen** (rc.29, Issue
  [#4507](https://github.com/kent8192/reinhardt-web/issues/4507)) —
  the `Resolve<Pascal>List` / `Resolve<Pascal>Detail` traits and the
  matching `urls.<basename>_list()` / `urls.<basename>_detail(id)`
  flat accessors are removed (4 generated items). Use
  `urls.server().<app>().<basename>_list()` /
  `urls.server().<app>().<basename>_detail(id)` instead.
- **`impl UrlResolverUnprefixed for ResolvedUrls`** override emitted by
  `#[routes]` — removed because the flat ViewSet accessor that
  required namespace-iterating fallback no longer exists. The
  `UrlResolverUnprefixed` trait itself is removed in
  `reinhardt-urls` PR.

See [`instructions/MIGRATION_0.2.md`](../../instructions/MIGRATION_0.2.md#reinhardt-core)
for the full migration guide.

### Fixed

- stop implicit openapi schema macro output
- *(core)* drop leftover empty test definition in viewset_macro tests (refs [[#4520](https://github.com/kent8192/reinhardt-web/issues/4520)](https://github.com/kent8192/reinhardt-web/issues/4520))
- *(auth)* replace InternalUser in UserManager public API with ManagedUser
- *(macros)* propagate serde derives to Info companion struct via model_config
- *(macros)* remove unused has_derive_trait from model_derive
- *(macros)* suppress missing_docs on generated Info companion types
- *(core)* dispose Memo only on last clone drop
- *(core)* drop disposed-flag clone from Memo compute closure
- *(core)* drop unused mut on Memo::new parameter
- *(macros)* keep unnamed EndpointMetadata.name None across codegen paths
- *(pages)* rerender SPA links after cleanup
- *(conf)* emit fragment self settings impls
- *(core)* address Copilot review feedback on PR [[#4713](https://github.com/kent8192/reinhardt-web/issues/4713)](https://github.com/kent8192/reinhardt-web/issues/4713)
- *(ci)* recover develop release-plz prerelease
- *(ci)* resolve all pre-existing compilation failures on develop/0.2.0
- *(ci)* update test snapshots and assertions for v0.2.0 breaking changes
- *(pages)* address CodeRabbit use_resource review

### Performance

- *(pages)* batch generated page attributes
- *(pages)* trim wasm dependency graph
- *(build)* measure cold workspace build
- atomize facade dependency feature gates
- trim standard facade feature dependencies

### Documentation

- *(release)* enforce public API doc coverage
- *(core)* scaffold MIGRATION_0.2.md and document BREAKING CHANGES (refs [[#4520](https://github.com/kent8192/reinhardt-web/issues/4520)](https://github.com/kent8192/reinhardt-web/issues/4520))

### Other

- resolve conflicts with develop/0.2.0

## [0.1.0](https://github.com/kent8192/reinhardt-web/compare/reinhardt-core@v0.1.0-rc.30...reinhardt-core@v0.1.0) - 2026-05-22

Initial stable release of `reinhardt-core` as part of the reinhardt-web
0.1.0 release. This crate is the foundation of the framework: it owns
the cross-cutting type system, the reactive signal runtime, the request
dispatch surface that route / action / WebSocket macros expand into,
and the security primitives (sanitization, validation, resource limits)
that every other Reinhardt crate consumes.

For the workspace-wide release narrative (Highlights, Breaking
Changes, Migration Guide), see the [root CHANGELOG](https://github.com/kent8192/reinhardt-web/blob/main/CHANGELOG.md#010---2026-05-22).
Per-prerelease history is preserved in the
[Release Discussions](https://github.com/kent8192/reinhardt-web/discussions/categories/release).

### Capabilities at 0.1.0

- **Reactive signal runtime** — `Signal<T>`, `Effect`, and `Resource`
  power the reinhardt-pages reactive layer. Signals are `Sync` on
  native via `Arc<RwLock<T>>`, are WASM-compatible, and the runtime
  exposes `#[doc(hidden)]` diagnostic accessors (`debug_subscribers`,
  `debug_dependencies`, `debug_observer_stack`, `debug_pending_updates`)
  for cross-crate WASM tests ([#4088](https://github.com/kent8192/reinhardt-web/issues/4088)).
- **Request dispatch primitives for route / action / WebSocket macros**
  — sets the task-local resolve context, forks the per-request DI
  context, surfaces async-capable `#[routes]` handlers, and exposes
  `AuthProtection` plus `EndpointMetadata` so route macros can detect
  auth parameters and propagate the resulting metadata automatically.
- **`use_endpoint!` and `flatten_imports!`** — multi-file view modules
  expose their endpoints through `use_endpoint!` for resolver re-export,
  and `flatten_imports!` (renamed from `define_views!`) replaces the
  removed `#[export_endpoints]` attribute for stable-Rust compatibility
  ([#3783](https://github.com/kent8192/reinhardt-web/discussions/3783)).
- **Auth scaffolding (`SuperuserInit`, `SuperuserCreator`)** — the
  registry-backed `SuperuserCreator` is auto-populated via `inventory`
  whenever a `#[user(full = true)]` + `#[model]` type is declared,
  enabling `manage createsuperuser` to bootstrap any user model.
- **Compile-time security primitives** — `validate_html_attr_name`,
  `is_safe_url` (with anchor-link support), redirect-URL validation,
  HTML / CSS / script escaping, multipart body limits, decompression-
  bomb prevention, HMAC-SHA256 cursor integrity, and a runtime resource-
  limits configuration shared by `reinhardt-http` / `reinhardt-pages` /
  `reinhardt-rest`.
- **Settings primitives backing `#[settings]`** — `CoreSettings` is the
  required base fragment, and the macro now generates `HasSettings<F>`
  impls and `field_policies()` from `#[setting(...)]` attribute blocks
  so consumers can compose fragments without losing per-field policy
  data.
- **OpenAPI / REST hooks** — operation-level `#[rest::*]` route
  attributes contribute OpenAPI metadata to `reinhardt-rest` without
  forcing a hard dependency on the REST crate.
- **Workspace-wide invariants** — UUIDs are emitted as v7 throughout
  the codebase, glob imports have been replaced with explicit `pub use`
  re-exports across the validators / rayon preludes, and all relative
  paths beyond `../` are eliminated per project policy.

### Notable Breaking Changes

- **`#[url_patterns]` becomes typed** ([#3770](https://github.com/kent8192/reinhardt-web/discussions/3770))
  — accepts `InstalledApp::*` identifiers and `mode = server|client|unified|ws`;
  pattern functions are renamed accordingly. `reinhardt-core`'s
  dispatch macros consume the typed form.
- **DI unifies on `Depends<T>`** ([#3628](https://github.com/kent8192/reinhardt-web/discussions/3628))
  and **`Injected<T>` is deprecated** ([#3631](https://github.com/kent8192/reinhardt-web/discussions/3631))
  — `#[inject]` no longer accepts `Arc<T>` directly; `Depends<T>`
  adds caching, cycle detection, and DI metadata. The auto-`Clone`
  bound is removed.
- **`#[routes]` is async-capable** — handler signatures may be
  `async fn`; synchronous handlers remain supported.
- **`DependencyRegistration` is const-compatible** for Rust 2024
  edition.
- **`#[settings]` requires explicit `CoreSettings`** and emits
  `HasSettings<F>` impls in both attribute forms.
- **`flatten_imports!` replaces `define_views!`** ([#3783](https://github.com/kent8192/reinhardt-web/discussions/3783)),
  which itself replaced `#[export_endpoints]` ([#3768](https://github.com/kent8192/reinhardt-web/discussions/3768)).

### Migration Notes

See the [root Migration Guide](https://github.com/kent8192/reinhardt-web/blob/main/CHANGELOG.md#migration-guide)
for the full per-feature migration steps. The high-value moves for
`reinhardt-core` consumers are:

- Switch every `#[inject] Arc<T>` site to `#[inject] Depends<T>` and
  drop redundant `#[derive(Clone)]` bounds.
- Replace `Injected<T>` / `OptionalInjected<T>` with `Depends<T>` /
  `Option<Depends<T>>`.
- Add an explicit `CoreSettings` fragment to any `#[settings]` block
  that previously relied on the implicit one, and migrate
  `#[export_endpoints]` views to `flatten_imports!`.
