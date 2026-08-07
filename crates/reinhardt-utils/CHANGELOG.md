# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.3.7](https://github.com/kent8192/reinhardt-web/compare/reinhardt-utils@v0.3.6...reinhardt-utils@v0.3.7) - 2026-08-07

### Maintenance

- update Cargo.toml dependencies

## [0.3.6](https://github.com/kent8192/reinhardt-web/compare/reinhardt-utils@v0.3.5...reinhardt-utils@v0.3.6) - 2026-08-04

### Maintenance

- update Cargo.toml dependencies

## [0.3.5](https://github.com/kent8192/reinhardt-web/compare/reinhardt-utils@v0.3.4...reinhardt-utils@v0.3.5) - 2026-08-02

### Maintenance

- update Cargo.toml dependencies

## [0.3.4](https://github.com/kent8192/reinhardt-web/compare/reinhardt-utils@v0.3.3...reinhardt-utils@v0.3.4) - 2026-07-30

### Maintenance

- update Cargo.toml dependencies

## [0.3.3](https://github.com/kent8192/reinhardt-web/compare/reinhardt-utils@v0.3.2...reinhardt-utils@v0.3.3) - 2026-07-28

### Maintenance

- update Cargo.toml dependencies

## [0.3.0](https://github.com/kent8192/reinhardt-web/compare/reinhardt-utils@v0.2.0...reinhardt-utils@v0.3.0) - 2026-06-28

Stable release of `reinhardt-utils` for the Reinhardt 0.3.0 line. This
entry consolidates the 0.3.0 release-candidate series into one
stable release section.

### Migration Notes

- Review the root CHANGELOG and `instructions/MIGRATION_0.3.md` before upgrading from 0.2.x.

### Fixed

- *(staticfiles)* normalize passthrough prefix matching
- *(staticfiles)* harden passthrough prefix normalization
- *(utils)* verify vendor asset integrity before install
- *(ci)* pin brotli allocator dependency

### Maintenance

- merge main into develop/0.3.0

## [0.2.0](https://github.com/kent8192/reinhardt-web/compare/reinhardt-utils@v0.1.3...reinhardt-utils@v0.2.0) - 2026-06-11

Stable release of `reinhardt-utils` for the Reinhardt 0.2.0 line. This
entry consolidates the 0.2.0 release-candidate series into one
stable release section.

### Migration Notes

- Review the breaking changes listed below before upgrading from 0.1.x.
- See the root CHANGELOG and `instructions/MIGRATION_0.2.md` for cross-crate migration guidance.

### Changed

- [**breaking**] align develop/0.2.0 with main, preserving 8 feature crates

### Fixed

- *(build)* port strict hot patch regression assertion
- *(staticfiles)* inject wasm loader for directory index
- *(staticfiles)* preserve raw index in non-spa mode
- *(staticfiles)* inject wasm loader for directory index without spa mode

- *(ci)* pin broken upstream transitive releases
- *(build)* address CodeRabbit review feedback
- *(ci)* recover develop release-plz prerelease

### Performance

- *(commands)* notify browsers after hot reload rebuilds
- *(build)* measure cold workspace build
- trim standard facade feature dependencies

## [0.1.0](https://github.com/kent8192/reinhardt-web/compare/reinhardt-utils@v0.1.0-rc.30...reinhardt-utils@v0.1.0) - 2026-05-22

Initial stable release of `reinhardt-utils` as part of the
reinhardt-web 0.1.0 release. This crate is the workspace's shared
utility surface: the `staticfiles` subsystem, the `AppVendorAsset`
type that drives CDN-style vendor delivery, path / HTML sanitization
helpers consumed by every other Reinhardt crate, and lock-poisoning
recovery wrappers that replace `unwrap()` on `Mutex` / `RwLock` guards.

For the workspace-wide release narrative (Highlights, Breaking
Changes, Migration Guide), see the [root CHANGELOG](https://github.com/kent8192/reinhardt-web/blob/main/CHANGELOG.md#010---2026-05-22).
Per-prerelease history is preserved in the
[Release Discussions](https://github.com/kent8192/reinhardt-web/discussions/categories/release).

### Capabilities at 0.1.0

- **`staticfiles` subsystem with WASM auto-injection** — `StaticFilesConfig`
  collects per-app static directories under `STATIC_ROOT`, detects
  WASM entry points (`WasmEntry`), and auto-injects the SPA bootstrap
  script into the served HTML so applications using `reinhardt-pages`
  do not need to hand-write `<script>` tags. Bundle assets disable
  immutable cache headers in debug builds so HMR can replace them.
- **`passthrough_prefixes` and `index_file`** — `StaticFilesConfig`
  exposes `passthrough_prefixes` (validated as non-empty at builder
  time) and `index_file` so the runserver / collectstatic CLIs can
  serve SPA-style fallback files from external paths via
  `serve_direct_file`.
- **`AppVendorAsset` vendor delivery** — apps register CDN assets
  with SHA-256 integrity, path validation, and an async downloader
  that uses a lazy first-request guard. `inventory`-based query
  helpers let the admin crate enumerate every registered asset for
  `collectstatic`.
- **Path / URL / HTML sanitization helpers** — `is_safe_url`
  (with anchor-link support), path-traversal prevention on
  `LocalStorage`, redirect-URL validators, XSS-safe `format_html`,
  `linebreaks` / `linebreaksbr` / `strip_tags_safe` HTML escapers,
  and UTF-8-safe truncation that never panics on a multibyte
  boundary.
- **Numeric & lock-safety utilities** — checked arithmetic helpers
  for cursor encoding (preventing arithmetic underflow), DST-gap
  handling in `make_aware_local` without panic, bounded iterative
  cleanup replacing recursive variants, and `poll_until` helpers
  used in lieu of `reinhardt-test` from inside `reinhardt-utils` to
  avoid a circular publish chain.
- **Lock-poisoning recovery wrappers** — `RwLock` / `Mutex`
  helpers in `reinhardt-utils` centralize poison recovery (replace
  blocking `KEYS` with non-blocking `SCAN+UNLINK`, recover from
  poisoned guards instead of panicking) and back the same patterns
  that `reinhardt-core` and `reinhardt-conf` expose.
- **UUID v7 throughout** — every UUID generated inside Reinhardt
  flows through helpers in this crate and is emitted as UUID v7 for
  monotonic ordering.

### Notable Breaking Changes

- **`r#static` module renamed to `staticfiles`** (#114) — the
  module rename removes the raw-identifier prefix from
  `reinhardt_utils::r#static`; the feature flag is also renamed
  from `static` to `staticfiles`.

### Migration Notes

See the [root Migration Guide](https://github.com/kent8192/reinhardt-web/blob/main/CHANGELOG.md#migration-guide)
for project-wide guidance. The only utility-specific move is to
update imports:

- Replace `reinhardt_utils::r#static::*` with
  `reinhardt_utils::staticfiles::*`, and switch the feature flag
  from `static` to `staticfiles`.
