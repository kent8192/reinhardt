# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.3.7](https://github.com/kent8192/reinhardt-web/compare/reinhardt-apps@v0.3.6...reinhardt-apps@v0.3.7) - 2026-08-07

### Maintenance

- update Cargo.toml dependencies

## [0.3.6](https://github.com/kent8192/reinhardt-web/compare/reinhardt-apps@v0.3.5...reinhardt-apps@v0.3.6) - 2026-08-04

### Maintenance

- update Cargo.toml dependencies

## [0.3.5](https://github.com/kent8192/reinhardt-web/compare/reinhardt-apps@v0.3.4...reinhardt-apps@v0.3.5) - 2026-08-02

### Maintenance

- update Cargo.toml dependencies

## [0.3.4](https://github.com/kent8192/reinhardt-web/compare/reinhardt-apps@v0.3.3...reinhardt-apps@v0.3.4) - 2026-07-30

### Maintenance

- update Cargo.toml dependencies

## [0.3.3](https://github.com/kent8192/reinhardt-web/compare/reinhardt-apps@v0.3.2...reinhardt-apps@v0.3.3) - 2026-07-28

### Maintenance

- update Cargo.toml dependencies

## [0.3.0](https://github.com/kent8192/reinhardt-web/compare/reinhardt-apps@v0.2.0...reinhardt-apps@v0.3.0) - 2026-06-28

Stable release of `reinhardt-apps` for the Reinhardt 0.3.0 line. This
crate moves with the coordinated Reinhardt 0.3.0 release train.

### Migration Notes

- Review the root CHANGELOG and `instructions/MIGRATION_0.3.md` before upgrading from 0.2.x.

### Maintenance

- align crate release metadata with the Reinhardt 0.3.0 stable release train.

## [0.2.0](https://github.com/kent8192/reinhardt-web/compare/reinhardt-apps@v0.1.3...reinhardt-apps@v0.2.0) - 2026-06-11

Stable release of `reinhardt-apps` for the Reinhardt 0.2.0 line. This
entry consolidates the 0.2.0 release-candidate series into one
stable release section.

### Migration Notes

- Review the breaking changes listed below before upgrading from 0.1.x.
- See the root CHANGELOG and `instructions/MIGRATION_0.2.md` for cross-crate migration guidance.

### Changed

- *(conf)* delete deprecated Settings, AdvancedSettings, JsonFileSource, and related APIs
- [**breaking**] align develop/0.2.0 with main, preserving 8 feature crates

### Fixed

- *(docs)* resolve remaining cross-crate intra-doc link errors
- shield downstream consumers of newly deprecated config types
- complete downstream shielding for deprecated config re-exports
- *(ci)* recover develop release-plz prerelease

### Performance

- atomize facade dependency feature gates
- trim standard facade feature dependencies

### Maintenance

- update Cargo.toml dependencies

## [0.1.0](https://github.com/kent8192/reinhardt-web/compare/reinhardt-apps@v0.1.0-rc.30...reinhardt-apps@v0.1.0) - 2026-05-22

Initial stable release of `reinhardt-apps` as part of the reinhardt-web
0.1.0 release. This crate owns the `AppLabel` trait, the `AppConfig`
descriptor that every reinhardt app declares, the `Apps` registry that
the framework consults at runtime to enumerate installed apps, and the
vendor-asset registration surface used by the admin and pages crates.

For the workspace-wide release narrative (Highlights, Breaking
Changes, Migration Guide), see the [root CHANGELOG](https://github.com/kent8192/reinhardt-web/blob/main/CHANGELOG.md#010---2026-05-22).
Per-prerelease history is preserved in the
[Release Discussions](https://github.com/kent8192/reinhardt-web/discussions/categories/release).

### Capabilities at 0.1.0

- **`AppLabel` trait with explicit `LABEL` and `path()`** — every
  installed app implements `AppLabel` with an explicit `LABEL`
  constant (no implicit derivation from the type name) and a `path()`
  method for enum-style implementors. The `InstalledApp` enum that
  `reinhardt new` generates wires into this trait so typed
  `#[url_patterns(InstalledApp::...)]` invocations resolve to a real
  app at compile time.
- **`Apps` registry** — the framework's source of truth for which
  apps are installed at runtime. `populate()` rejects duplicates
  instead of silently overwriting, lookups are guarded against TOCTOU
  races, and lock poisoning is handled gracefully instead of
  propagating panics.
- **`ApplicationBuilder` with database-URL validation** —
  `ApplicationBuilder::build` validates database URL schemes before
  the application starts, surfacing misconfiguration at boot rather
  than on the first query.
- **Vendor-asset registration via `AppVendorAsset`** — apps register
  CDN-style vendor assets via inventory; consumers (notably the admin
  crate) query the registry to collect, integrity-verify, and serve
  the assets. The native-only `inventory` / `linkme` dependencies are
  gated behind the native target so the crate builds cleanly on WASM.
- **Cross-target build** — `reinhardt-apps` compiles under both
  native and `wasm32-unknown-unknown`, exposing data / trait
  dependencies on every target and gating native-only registration
  state behind `cfg`.
- **Testing hooks** — the `testing` feature exposes registry-reset
  helpers shared with `reinhardt-pages` so test suites can isolate
  per-test app registrations.

### Notable Breaking Changes

- **`AppLabel` implementors require explicit `LABEL`** — apps must
  declare `const LABEL: &'static str` (and the `#[app_config]` macro
  enforces it). This is the foundation that makes the typed
  `#[url_patterns(InstalledApp::*)]` rewrite ([#3770](https://github.com/kent8192/reinhardt-web/discussions/3770))
  predictable.
- **Per-app layout under `apps/<app>/`** ([#4476](https://github.com/kent8192/reinhardt-web/discussions/4476))
  — per-app `server_fn` and client UI moved from
  `commands/templates/...` into `apps/<app>/`; existing apps must
  relocate matching source files. `reinhardt new` already emits the
  new layout.

### Migration Notes

See the [root Migration Guide](https://github.com/kent8192/reinhardt-web/blob/main/CHANGELOG.md#migration-guide)
for project-wide guidance. App-specific moves:

- Add an explicit `const LABEL: &'static str = "<app_name>";` to
  every `AppLabel` implementor (or use the updated `#[app_config]`
  macro form).
- Relocate per-app handlers from `commands/templates/<app>/` to
  `apps/<app>/` per [#4476](https://github.com/kent8192/reinhardt-web/discussions/4476).
