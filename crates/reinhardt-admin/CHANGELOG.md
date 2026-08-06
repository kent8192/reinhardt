# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.4.0-alpha.2](https://github.com/kent8192/reinhardt-web/compare/reinhardt-admin@v0.4.0-alpha.1...reinhardt-admin@v0.4.0-alpha.2) - 2026-07-23

### Added

- *(db)* [**breaking**] add Copy-safe injected connection handles

### Fixed

- *(admin)* restore Arc import in database tests

### Maintenance

- auto-fix fmt and clippy

## [0.4.0-alpha.1](https://github.com/kent8192/reinhardt-web/compare/reinhardt-admin@v0.3.2...reinhardt-admin@v0.4.0-alpha.1) - 2026-07-21

### Added

- *(manouche,pages)* lower typed intrinsic event handlers

### Changed

- *(pages)* migrate model-set error adapters

### Fixed

- *(manouche)* handle composed accessibility labels
- *(admin)* satisfy strict clippy checks
- *(pages)* align native callbacks with scoped handles
- *(pages)* retain owner scopes for deferred work
- *(pages)* merge deferred owner scope fixes
- *(ci)* terminate cfg aliases macro invocations
- *(pages)* retain mount scopes for events
- *(pages)* scope error-kind imports to tests
- *(release)* restore develop prerelease lifecycle

### Maintenance

- merge latest develop/0.4.0 into forward-merge branch
- merge latest main into develop forward-merge
- *(pages)* merge develop/0.4.0 into typed events
- merge develop/0.4.0 into retained effects
- merge develop/0.4.0 into arena copy handles branch

### Performance

- [**breaking**] inline path parameter dispatch values

### Testing

- update path parameter assertions

## [0.3.1](https://github.com/kent8192/reinhardt-web/compare/reinhardt-admin@v0.3.0...reinhardt-admin@v0.3.1) - 2026-07-04

### Fixed

- *(formatter)* wrap long page closure parameters
- *(formatter)* normalize admin feature page closures

## [0.3.0](https://github.com/kent8192/reinhardt-web/compare/reinhardt-admin@v0.2.0...reinhardt-admin@v0.3.0) - 2026-06-28

Stable release of `reinhardt-admin` for the Reinhardt 0.3.0 line. This
entry consolidates the 0.3.0 release-candidate series into one
stable release section.

### Migration Notes

- Review the root CHANGELOG and `instructions/MIGRATION_0.3.md` before upgrading from 0.2.x.

### Added

- *(urls)* [**breaking**] remove raw server route registration APIs
- *(formatter)* rustfmt page expression islands
- feat!(di): introduce keyed injectable provider outputs

### Changed

- *(admin)* use keyed depends for server injections

### Fixed

- *(admin)* count zero-limit list pages
- *(tutorial)* suppress basis runtime drift
- *(di)* honor cache false for keyed wrappers

### Performance

- *(admin)* combine admin list count query

## [0.2.0](https://github.com/kent8192/reinhardt-web/compare/reinhardt-admin@v0.1.3...reinhardt-admin@v0.2.0) - 2026-06-11

Stable release of `reinhardt-admin` for the Reinhardt 0.2.0 line. This
entry consolidates the 0.2.0 release-candidate series into one
stable release section.

### Migration Notes

- Replace the removed `reinhardt_admin::core::vendor` shim with `reinhardt_utils::staticfiles::vendor`.
- See [`instructions/MIGRATION_0.2.md`](../../instructions/MIGRATION_0.2.md) for the workspace migration checklist.

### Breaking Changes

- *(db,macros)* [**breaking**] unify custom managers with Model::objects() ([[#3984](https://github.com/kent8192/reinhardt-web/issues/3984)](https://github.com/kent8192/reinhardt-web/issues/3984))

### Added

- *(orm)* add Django-like lookup helpers
- *(db)* introduce type-safe nullable field on FieldMetadata
- *(db,macros)* [**breaking**] unify custom managers with Model::objects() ([[#3984](https://github.com/kent8192/reinhardt-web/issues/3984)](https://github.com/kent8192/reinhardt-web/issues/3984))
- *(forms)* add typed use_form ergonomics
- feat!(forms): route use_form through form definitions

### Changed

- *(auth)* make CurrentUser canonical extractor
- *(admin)* migrate create_resource to use_resource
- [**breaking**] align develop/0.2.0 with main, preserving 8 feature crates

### Removed

- **`reinhardt-admin::core::vendor`** module gated with `#![cfg(any())]`
  — contains the deprecated `VendorAsset`, `Verbosity`,
  `verify_integrity`, `download_vendor_assets`,
  `ensure_vendor_assets`, `admin_vendor_assets` (all deprecated since
  `0.1.0-rc.27`). All items moved to
  `reinhardt_utils::staticfiles::vendor`. Admin's own assets are
  declared via `inventory::submit!` in `crates/reinhardt-admin/src/lib.rs`.

#### BREAKING CHANGES

**Final PR closing umbrella Issue
[#4520](https://github.com/kent8192/reinhardt-web/issues/4520).**

Removed all 6 RC-deprecated vendor-asset shim items from
`reinhardt-admin` per STABILITY_POLICY § SP-4:

- **`reinhardt-admin::core::vendor`** module gated with `#![cfg(any())]`
  — contains the deprecated `VendorAsset`, `Verbosity`,
  `verify_integrity`, `download_vendor_assets`,
  `ensure_vendor_assets`, `admin_vendor_assets` (all deprecated since
  `0.1.0-rc.27`). All items moved to
  `reinhardt_utils::staticfiles::vendor`. Admin's own assets are
  declared via `inventory::submit!` in `crates/reinhardt-admin/src/lib.rs`.

With this PR merged, every `#[deprecated(since = "0.1.0-rc.X", ...)]`
item documented in Issue #4520 has been removed across all 12 affected
crates. The `since = "0.1.0-rc.*"` namespace is retired.

### Fixed

- *(orm)* address lookup review edge cases
- *(admin)* enable tutorial admin CRUD
- *(admin)* build CRUD URLs with router reverse
- *(admin)* preserve multi-select mutation values
- *(auth)* replace InternalUser in UserManager public API with ManagedUser
- *(admin)* compile admin wasm browser tests
- *(ci)* recover develop release-plz prerelease
- *(forms)* address review and CI failures
- *(forms)* address bot review feedback

### Documentation

- *(admin)* remove broken DefaultUser intra-doc links

### Styling

- *(admin)* format admin feature controls
- apply formatter fixes across workspace
- apply rustfmt to non-DSL files on develop/0.2.0

### Maintenance

- include all main v0.1.1 PR changes
- forward merge main v0.1.2 changes into develop 0.2.0

### Other

- resolve conflicts with develop/0.2.0

## [0.1.0](https://github.com/kent8192/reinhardt-web/compare/reinhardt-admin@v0.1.0-rc.30...reinhardt-admin@v0.1.0) - 2026-05-22

Initial stable release of `reinhardt-admin` as part of the
reinhardt-web 0.1.0 release. This crate is the built-in admin
panel: a WASM SPA shell, `ModelAdmin`-driven CRUD pages,
role-based permissions, and type-safe query filters, all wired
through the framework's DI and routing surface.

For the workspace-wide release narrative, see the [root CHANGELOG](https://github.com/kent8192/reinhardt-web/blob/main/CHANGELOG.md#010---2026-05-22).
Per-prerelease history is in the [Release Discussions](https://github.com/kent8192/reinhardt-web/discussions/categories/release).

### Capabilities at 0.1.0

- **`#[model]`-driven admin pages** — `ModelAdmin` derives admin
  CRUD pages directly from your `#[model]` types, with role-based
  permission methods that accept `&dyn AdminUser`, type-safe
  query filters, and a `FormFieldSpec` enum that preserves the
  field's choice set through to widget rendering.
- **`admin_routes_with_di()` entry point** — a single mount call
  that applies middleware-contributed DI registrations, replacing
  the legacy `admin_routes()` and `AdminRouter` struct. The
  `AdminRoute` enum is now `#[non_exhaustive]` and its `Login`
  variant has moved position to make room for new routes.
- **WASM SPA shell** — `admin_routes_with_di()` serves the admin
  SPA HTML, embeds the WASM client with `init()`, applies CSP
  security headers, and supports `HEAD` requests for the static
  asset handler. The SPA uses `mount()` rendering with the
  reactive scheduler initialised at boot.
- **Configurable `AdminSettings`** — `SettingsFragment` impl with
  `from_str` parsing on `FrameOptions` and `ReferrerPolicy`,
  CSP and security-header validation warnings, and a
  `SecurityHeaders` conversion that wires CSP into the SPA
  response.
- **Login surface** — a typed `form!`-driven login page with a
  JWT-issuing server function; the JWT embeds `is_staff` and
  `is_superuser` so the admin gate can authorise without a
  database round-trip on every request.
- **Inventory-based vendor assets** — admin's UnoCSS runtime,
  Open Props, Animate.css, and Google Fonts are registered as
  inventory entries that `collectstatic` downloads and serves as
  local vendor paths, with embedded fallbacks via
  `Bytes::from_static` for zero-copy delivery.
- **Hardened CRUD operations** — parameterised queries
  everywhere, escaped LIKE patterns, deterministic INSERT
  column ordering, UUID primary key handling in the `RETURNING`
  clause, `auto_now` / `auto_now_add` timestamp injection, and
  CSV / TSV export via `write_record` for map-based records.

### Notable Breaking Changes

- **`AdminUser` trait signature** ([#3615](https://github.com/kent8192/reinhardt-web/discussions/3615)) — `ModelAdmin` permission methods now accept `&dyn AdminUser` instead of `&(dyn std::any::Any + Send + Sync)`.
- **`admin_routes_with_di()` replaces `admin_routes()`** ([#3626](https://github.com/kent8192/reinhardt-web/discussions/3626)) — middleware-contributed DI registrations are applied through the new entry point; `AdminRouter` struct and the deprecated `AdminSite` shim methods are removed.
- **`AdminRoute` is `#[non_exhaustive]`** — match arms over `AdminRoute` must include a default fallback, and the `Login` variant has moved position.
- **Admin form widget HTML elements** ([#3771](https://github.com/kent8192/reinhardt-web/discussions/3771)) — `TextArea`, `Select`, and `MultiSelect` render as their semantic HTML elements rather than `<input>`.
- **`#[inject]` accepts `Depends<T>`** ([#3628](https://github.com/kent8192/reinhardt-web/discussions/3628)) — `Arc<T>` parameters in `#[inject]` are unsupported; integration tests and admin handlers have been updated to `Depends<T>`.

### Migration Notes

- Switch admin mount points from `admin_routes()` to
  `admin_routes_with_di()` so middleware DI contributions are
  applied. Remove any `AdminRouter` struct references —
  routes are mounted directly.
- Update `ModelAdmin` permission methods to the
  `&dyn AdminUser` signature; the type-erased loader accepts
  any `#[user]` type for admin authentication.
- Add a wildcard arm to any `match` over `AdminRoute`
  (it is now `#[non_exhaustive]`) and recheck arm ordering if
  you matched on `AdminRoute::Login`.
- For `form!` fields that previously rendered as `<input>`,
  expect the new semantic HTML element when the field type is
  `TextArea`, `Select`, or `MultiSelect`.
- For the workspace-wide migration narrative, see the [root CHANGELOG](https://github.com/kent8192/reinhardt-web/blob/main/CHANGELOG.md#010---2026-05-22).
