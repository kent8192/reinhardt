# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.3.7](https://github.com/kent8192/reinhardt-web/compare/reinhardt-views@v0.3.6...reinhardt-views@v0.3.7) - 2026-08-07

### Maintenance

- update Cargo.toml dependencies

## [0.3.6](https://github.com/kent8192/reinhardt-web/compare/reinhardt-views@v0.3.5...reinhardt-views@v0.3.6) - 2026-08-04

### Maintenance

- update Cargo.toml dependencies

## [0.3.5](https://github.com/kent8192/reinhardt-web/compare/reinhardt-views@v0.3.4...reinhardt-views@v0.3.5) - 2026-08-02

### Maintenance

- update Cargo.toml dependencies

## [0.3.4](https://github.com/kent8192/reinhardt-web/compare/reinhardt-views@v0.3.3...reinhardt-views@v0.3.4) - 2026-07-30

### Maintenance

- update Cargo.toml dependencies

## [0.3.3](https://github.com/kent8192/reinhardt-web/compare/reinhardt-views@v0.3.2...reinhardt-views@v0.3.3) - 2026-07-28

### Fixed

- *(auth)* deny bare legacy extension authentication

## [0.3.2](https://github.com/kent8192/reinhardt-web/compare/reinhardt-views@v0.3.1...reinhardt-views@v0.3.2) - 2026-07-14

### Fixed

- *(auth)* preserve active status in request identities
- *(auth)* preserve active identity compatibility

## [0.3.1](https://github.com/kent8192/reinhardt-web/compare/reinhardt-views@v0.3.0...reinhardt-views@v0.3.1) - 2026-07-04

### Fixed

- *(ci)* unblock quick-xml security audit

## [0.3.0](https://github.com/kent8192/reinhardt-web/compare/reinhardt-views@v0.2.0...reinhardt-views@v0.3.0) - 2026-06-28

Stable release of `reinhardt-views` for the Reinhardt 0.3.0 line. This
entry consolidates the 0.3.0 release-candidate series into one
stable release section.

### Migration Notes

- Review the root CHANGELOG and `instructions/MIGRATION_0.3.md` before upgrading from 0.2.x.

### Added

- feat!(di): introduce keyed injectable provider outputs

### Fixed

- *(views)* wire declared viewset serializers
- *(tests)* align viewset serializer fixtures
- *(views)* align viewset serializer examples

### Maintenance

- merge main into develop/0.3.0

## [0.2.0](https://github.com/kent8192/reinhardt-web/compare/reinhardt-views@v0.1.3...reinhardt-views@v0.2.0) - 2026-06-11

Stable release of `reinhardt-views` for the Reinhardt 0.2.0 line. This
entry consolidates the 0.2.0 release-candidate series into one
stable release section.

### Migration Notes

- Review the breaking changes listed below before upgrading from 0.1.x.
- See the root CHANGELOG and `instructions/MIGRATION_0.2.md` for cross-crate migration guidance.

### Breaking Changes

- *(db,macros)* [**breaking**] unify custom managers with Model::objects() ([[#3984](https://github.com/kent8192/reinhardt-web/issues/3984)](https://github.com/kent8192/reinhardt-web/issues/3984))
- *(auth)* [**breaking**] migrate internal consumers from removed User/SimpleUser types

### Added

- *(orm)* support composite filter combinators
- *(db,macros)* [**breaking**] unify custom managers with Model::objects() ([[#3984](https://github.com/kent8192/reinhardt-web/issues/3984)](https://github.com/kent8192/reinhardt-web/issues/3984))

### Changed

- [**breaking**] align develop/0.2.0 with main, preserving 8 feature crates

### Fixed

- *(auth)* [**breaking**] migrate internal consumers from removed User/SimpleUser types
- *(db)* qualify Manager in rustdoc examples and add missing Objects type
- *(views)* route generic queryset fallback through objects
- *(ci)* recover develop release-plz prerelease

### Performance

- atomize facade dependency feature gates
- trim standard facade feature dependencies

### Testing

- *(views)* assert filter operator and value with matches! instead of Debug substring
- *(views)* cover explicit queryset override on retrieve-update and retrieve-destroy views

### Styling

- apply formatter fixes across workspace

## [0.1.0](https://github.com/kent8192/reinhardt-web/compare/reinhardt-views@v0.1.0-rc.30...reinhardt-views@v0.1.0) - 2026-05-22

Initial stable release of `reinhardt-views` as part of the
reinhardt-web 0.1.0 release. Provides the view helpers, ViewSet
handlers, response builders, and pagination plumbing that sit above
`reinhardt-rest` and `reinhardt-urls`.

For the workspace-wide release narrative, see the [root CHANGELOG](https://github.com/kent8192/reinhardt-web/blob/main/CHANGELOG.md#010---2026-05-22).
Per-prerelease history is in the [Release Discussions](https://github.com/kent8192/reinhardt-web/discussions/categories/release).

### Capabilities at 0.1.0

- **ViewSet handler dispatch** — `ModelViewSet` dispatches through a
  real CRUD handler (resolves [#3985](https://github.com/kent8192/reinhardt-web/issues/3985)),
  `viewsets/handler` is split by responsibility for legibility, and
  runtime action registration is emitted from the `#[viewset]`
  impl-form. Marker actions are bridged into the ViewSet so manual
  wiring is not needed.
- **HTTP-correct method handling** — Partial updates are inferred from
  the HTTP method (not a config field), unsupported methods return
  `405 Method Not Allowed` (not `400`), and non-object PATCH bodies
  are rejected with `400 Bad Request`.
- **Safe pagination** — Total count uses the actual row count rather
  than the page length, and pagination arithmetic saturates to
  prevent overflow under adversarial input.
- **Auto-trait preservation** — Public types on the ViewSet path
  preserve `UnwindSafe` / `RefUnwindSafe` so panic-in-handler crashes
  isolate cleanly; `parking_lot::RwLock` replaces `std::sync::RwLock`
  so the dispatcher never panics on poisoning.
- **Error model integration** — `From<ViewError> for core::Error`
  bridges view errors into the framework-wide exception type, and
  `(views, middleware)` share a PATCH-merge helper that uses the typed
  `SET_COOKIE` header.
- **`define_views!` declarative macro** — Multi-file view modules use
  `define_views!` (renamed later to `flatten_imports!`) for
  stable-Rust compatibility, replacing the removed `#[export_endpoints]`
  attribute form ([Discussion #3768](https://github.com/kent8192/reinhardt-web/discussions/3768)).

### Notable Breaking Changes

- **`Injected<T>` deprecated** ([Discussion #3631](https://github.com/kent8192/reinhardt-web/discussions/3631))
  — `Depends<T>` replaces `Injected<T>` in view handlers; the
  auto-`Clone` bound is removed.
- **`define_views!` replaces `#[export_endpoints]`** ([Discussion #3768](https://github.com/kent8192/reinhardt-web/discussions/3768))
  — Multi-file view modules must use the declarative macro.

### Migration Notes

This is the first stable release. See the [root CHANGELOG](https://github.com/kent8192/reinhardt-web/blob/main/CHANGELOG.md#010---2026-05-22)
for the cross-crate migration guide; for view-specific moves, follow
Discussions [#3631](https://github.com/kent8192/reinhardt-web/discussions/3631)
and [#3768](https://github.com/kent8192/reinhardt-web/discussions/3768).
