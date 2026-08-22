# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.4.0-alpha.7](https://github.com/kent8192/reinhardt-web/compare/reinhardt-di@v0.4.0-alpha.6...reinhardt-di@v0.4.0-alpha.7) - 2026-08-19

### Documentation

- update version references to v0.3.3
- update version references to v0.3.4
- update version references to v0.3.5
- update version references to v0.3.6
- update version references to v0.3.7
- update version references to v0.3.8

### Fixed

- *(di)* gate multipart extraction tests by feature

### Maintenance

- merge main into develop/0.4.0

### Testing

- *(di)* cover request parameter extraction
- *(di)* make parameter extraction tests feature-safe
- *(di)* harden multipart extraction coverage

## [0.4.0-alpha.6](https://github.com/kent8192/reinhardt-web/compare/reinhardt-di@v0.4.0-alpha.5...reinhardt-di@v0.4.0-alpha.6) - 2026-08-06

### Documentation

- *(release)* restore coherent alpha.3 references

## [0.4.0-alpha.2](https://github.com/kent8192/reinhardt-web/compare/reinhardt-di@v0.4.0-alpha.1...reinhardt-di@v0.4.0-alpha.2) - 2026-07-23

### Documentation

- *(di)* document mutable injection patterns

### Fixed

- *(di)* forward provider injection patterns

## [0.4.0-alpha.1](https://github.com/kent8192/reinhardt-web/compare/reinhardt-di@v0.3.2...reinhardt-di@v0.4.0-alpha.1) - 2026-07-21

### Fixed

- *(release)* restore develop prerelease lifecycle

### Maintenance

- merge main into develop/0.4.0
## [0.3.9](https://github.com/kent8192/reinhardt-web/compare/reinhardt-di@v0.3.8...reinhardt-di@v0.3.9) - 2026-08-21

### Documentation

- *(security)* define data and identity boundaries
- *(security)* document secret and csrf boundaries

### Fixed

- *(security)* qualify boundary control ownership

## [0.3.7](https://github.com/kent8192/reinhardt-web/compare/reinhardt-di@v0.3.6...reinhardt-di@v0.3.7) - 2026-08-12

### Fixed

- *(di)* gate multipart extraction tests by feature

### Testing

- *(di)* cover request parameter extraction
- *(di)* make parameter extraction tests feature-safe
- *(di)* harden multipart extraction coverage

## [0.3.0](https://github.com/kent8192/reinhardt-web/compare/reinhardt-di@v0.2.0...reinhardt-di@v0.3.0) - 2026-06-28

Stable release of `reinhardt-di` for the Reinhardt 0.3.0 line. This
entry consolidates the 0.3.0 release-candidate series into one
stable release section.

### Migration Notes

- Review the root CHANGELOG and `instructions/MIGRATION_0.3.md` before upgrading from 0.2.x.

### Added

- *(params)* generalize cookie extractors
- feat!(di): introduce keyed injectable provider outputs
- *(di)* add wasm parity stubs for injectable macros

### Changed

- [**breaking**] remove 0.3 deprecated public APIs

### Fixed

- *(di)* support trait-based inject wrapper resolution
- *(di)* preserve Depends inject fallback
- *(di)* honor cache false for keyed wrappers

## [0.2.0](https://github.com/kent8192/reinhardt-web/compare/reinhardt-di@v0.1.3...reinhardt-di@v0.2.0) - 2026-06-11

Stable release of `reinhardt-di` for the Reinhardt 0.2.0 line. This
entry consolidates the 0.2.0 release-candidate series into one
stable release section.

### Migration Notes

- Replace `Injected<T>` and `OptionalInjected<T>` with `Depends<T>` and `Option<Depends<T>>`.
- See [`instructions/MIGRATION_0.2.md`](../../instructions/MIGRATION_0.2.md) for the workspace migration checklist.

### Breaking Changes

- *(di)* [**breaking**] remove Injected and OptionalInjected (refs [[#4520](https://github.com/kent8192/reinhardt-web/issues/4520)](https://github.com/kent8192/reinhardt-web/issues/4520))
- *(di)* [**breaking**] enforce scope hierarchy at resolution time
- *(di)* [**breaking**] make InjectionContext registry-aware for per-test isolation

### Added

- *(di)* [**breaking**] remove Injected and OptionalInjected (refs [[#4520](https://github.com/kent8192/reinhardt-web/issues/4520)](https://github.com/kent8192/reinhardt-web/issues/4520))
- *(di)* [**breaking**] enforce scope hierarchy at resolution time
- *(di)* [**breaking**] make InjectionContext registry-aware for per-test isolation
- *(di)* add DependsResult and DependsOption sugar type aliases

### Changed

- *(auth)* make CurrentUser canonical extractor
- *(di)* delete deprecated Injected<T> and OptionalInjected<T> types
- [**breaking**] align develop/0.2.0 with main, preserving 8 feature crates

### Removed

- **`Injected<T>` struct** (`src/injected.rs`, deprecated since
  `0.1.0-rc.16`) — the FastAPI-inspired wrapper that previously coexisted
  with [`Depends<T>`](src/depends.rs). All injection codegen now goes
  through `Depends<T>` exclusively.
- **`OptionalInjected<T>` type alias** (`src/injected.rs`, deprecated
  since `0.1.0-rc.16`) — use `Option<Depends<T>>` instead.

#### BREAKING CHANGES

All public APIs deprecated during the `0.1.0-rc.*` cycle have been
removed per STABILITY_POLICY § SP-4. Refs umbrella Issue
[#4520](https://github.com/kent8192/reinhardt-web/issues/4520).

`reinhardt-di` removals (2 items):

- **`Injected<T>` struct** (`src/injected.rs`, deprecated since
  `0.1.0-rc.16`) — the FastAPI-inspired wrapper that previously coexisted
  with [`Depends<T>`](src/depends.rs). All injection codegen now goes
  through `Depends<T>` exclusively.
- **`OptionalInjected<T>` type alias** (`src/injected.rs`, deprecated
  since `0.1.0-rc.16`) — use `Option<Depends<T>>` instead.

#### Macro behavior change

`#[injectable]` no longer accepts `Injected<T>` / `OptionalInjected<T>`
fields. The error message reads:

```text
#[inject] field must have type Depends<T> or Option<Depends<T>>
```

`InjectionMetadata` and `DependencyScope` (the supporting metadata
types that previously co-resided with `Injected<T>`) remain in
`crates/reinhardt-di/src/injected.rs` because they are still used by
`Depends<T>`. The module's `Injected<T>`/`OptionalInjected<T>`
content is gone but the file name (`injected.rs`) is preserved this
release to keep the diff focused on RC-deprecated removals — a rename
is a candidate for a follow-up PR.

See [`instructions/MIGRATION_0.2.md`](../../instructions/MIGRATION_0.2.md#reinhardt-di)
for the migration guide.

### Fixed

- *(di)* enforce scope check on cache-hit path
- *(di)* enforce scope check on pre-seeded request cache and bypass path
- *(di)* collapse nested if-let into let-chain
- *(di)* resolve DependsResult/DependsOption field injection from registry
- *(ci)* recover develop release-plz prerelease
- *(di)* address Copilot review feedback on scope hierarchy tests
- apply CodeRabbit auto-fixes

### Documentation

- *(release)* enforce public API doc coverage
- recommend Result return types for injectable factories
- *(di)* document Injected removal in CHANGELOG and migration guide (refs [[#4520](https://github.com/kent8192/reinhardt-web/issues/4520)](https://github.com/kent8192/reinhardt-web/issues/4520))
- *(di)* update public docs to reflect per-context registry isolation
- *(di,auth)* fix rustdoc link warnings on nightly

### Styling

- format files from merge resolution

### Other

- resolve conflicts with develop/0.2.0

## [0.1.0](https://github.com/kent8192/reinhardt-web/compare/reinhardt-di@v0.1.0-rc.30...reinhardt-di@v0.1.0) - 2026-05-22

Initial stable release of `reinhardt-di` as part of the reinhardt-web
0.1.0 release. Provides the framework's dependency-injection runtime:
the `Depends<T>` extractor, the `#[inject]` parameter attribute, and the
global `DependencyRegistry` that powers DI across HTTP handlers, server
functions, GraphQL/gRPC, and WebSocket consumers.

For the workspace-wide release narrative, see the [root CHANGELOG](https://github.com/kent8192/reinhardt-web/blob/main/CHANGELOG.md#010---2026-05-22).
Per-prerelease history is in the [Release Discussions](https://github.com/kent8192/reinhardt-web/discussions/categories/release).

### Capabilities at 0.1.0

- **Unified `Depends<T>` extractor** — every `#[inject]` parameter is
  typed as `Depends<T>`; the framework caches resolution per request,
  detects cycles via a task-local guard, and surfaces typed metadata
  the legacy `Arc<T>` shape could not carry.
- **Scope-aware `InjectionContext`** — context forking for per-request
  scopes, a deep-cloned `request_scope`, a fallback path for
  pre-seeded types, and a protocol-agnostic `fork()` shared across
  HTTP, GraphQL, gRPC, and WebSocket entry points.
- **Middleware-contributed DI** — `Middleware::di_registrations()`
  lets middleware (admin, auth, session) attach DI bindings that the
  framework picks up at route-server construction time, removing a
  long-standing route-vs-server scope gap.
- **Typed errors with HTTP mapping** — `DiError::Authentication` /
  `Authorization` map to 401 / 403 responses through
  `ParamError::Authentication`; both enums are `#[non_exhaustive]`.
- **Optional and validated extraction** — blanket `Injectable` for
  `Option<T>` enables optional dependencies; `Validated<T>`
  auto-validates extracted payloads before handler dispatch.
- **Per-request and per-test isolation** — the `testing` feature
  exposes `register_override` and the RAII `OverrideGuard` so mocks
  can be installed for a single test without leaking into other
  threads.
- **Hardened proc-macro output** — the `#[injectable]` and
  `#[injectable_factory]` expansions auto-derive `Clone`, register
  qualified type names, route `async_trait` through reinhardt-core,
  enforce attribute ordering, and reject unknown arguments at compile
  time.
- **Security-hardened registry** — `RegistryValidator`'s
  `FrameworkTypeOverride` check rejects accidental shadowing of
  framework types; ReDoS-safe pattern length limits and body-size
  caps were added to parameter extractors during the alpha cycle.

### Notable Breaking Changes

- **`Arc<T>` → `Depends<T>` on `#[inject]`** — see
  [#3628](https://github.com/kent8192/reinhardt-web/discussions/3628).
- **`Injected<T>` deprecated** — replaced by `Depends<T>`; the
  auto-`Clone` bound is removed (see
  [#3631](https://github.com/kent8192/reinhardt-web/discussions/3631)).
- **`Middleware::di_registrations()` hook** introduced; non-auth
  `DiError` variants now map to `ParamError::Internal` so 500s are
  not silently relabeled.
- **`DiError` and `ParamError` are `#[non_exhaustive]`** — match arms
  on these enums must include a default fallback.

### Migration Notes

See the workspace-wide migration guide in the [root CHANGELOG](https://github.com/kent8192/reinhardt-web/blob/main/CHANGELOG.md#migration-guide)
for `Arc<T>` → `Depends<T>` and `Injected<T>` → `Depends<T>`
walkthroughs. The key per-site change is mechanical: replace
`#[inject] Arc<T>` with `#[inject] Depends<T>` and add an explicit
`#[derive(Clone)]` if your concrete type was relying on the previous
auto-`Clone` behaviour.
