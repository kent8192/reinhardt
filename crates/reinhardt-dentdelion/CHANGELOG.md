# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.4.0-alpha.1](https://github.com/kent8192/reinhardt-web/compare/reinhardt-dentdelion@v0.3.2...reinhardt-dentdelion@v0.4.0-alpha.1) - 2026-07-21

### Changed

- [**breaking**] remove remaining dynamic error dependencies

### Fixed

- *(release)* restore develop prerelease lifecycle

### Maintenance

- merge develop/0.4.0 into remove-anyhow branch

## [0.3.0](https://github.com/kent8192/reinhardt-web/compare/reinhardt-dentdelion@v0.2.2...reinhardt-dentdelion@v0.3.0) - 2026-06-28

Stable release of `reinhardt-dentdelion` for the Reinhardt 0.3.0 line. This
entry consolidates the 0.3.0 release-candidate series into one
stable release section.

### Migration Notes

- Review the root CHANGELOG and `instructions/MIGRATION_0.3.md` before upgrading from 0.2.x.

### Maintenance

- merge main into develop/0.3.0

## [0.2.2](https://github.com/kent8192/reinhardt-web/compare/reinhardt-dentdelion@v0.2.1...reinhardt-dentdelion@v0.2.2) - 2026-06-25

### Documentation

- update version references to v0.2.1

## [0.2.0](https://github.com/kent8192/reinhardt-web/compare/reinhardt-dentdelion@v0.1.3...reinhardt-dentdelion@v0.2.0) - 2026-06-11

Stable release of `reinhardt-dentdelion` for the Reinhardt 0.2.0 line. This
entry consolidates the 0.2.0 release-candidate series into one
stable release section.

### Migration Notes

- Review the breaking changes listed below before upgrading from 0.1.x.
- See the root CHANGELOG and `instructions/MIGRATION_0.2.md` for cross-crate migration guidance.

### Changed

- [**breaking**] align develop/0.2.0 with main, preserving 8 feature crates

### Fixed

- *(ci)* recover develop release-plz prerelease

### Maintenance

- update Cargo.toml dependencies

## [0.1.0](https://github.com/kent8192/reinhardt-web/compare/reinhardt-dentdelion@v0.1.0-rc.30...reinhardt-dentdelion@v0.1.0) - 2026-05-22

Initial stable release of `reinhardt-dentdelion` (Delion) as part of
the reinhardt-web 0.1.0 release. Provides the plugin system that
loads, sandboxes, and runs framework extensions — including a WASM
component runtime built on `wasmtime`.

For the workspace-wide release narrative, see the [root CHANGELOG](https://github.com/kent8192/reinhardt-web/blob/main/CHANGELOG.md#010---2026-05-22).
Per-prerelease history is in the [Release Discussions](https://github.com/kent8192/reinhardt-web/discussions/categories/release).

### Capabilities at 0.1.0

- **`Plugin` trait + manifest** — A first-class plugin contract with
  TOML manifest metadata, deterministic loading, and topologically
  sorted initialization, so plugins can declare dependencies on one
  another without ordering bugs.
- **WASM Component Model sandbox** — Behind the `wasm` feature, plugins
  run inside a `wasmtime` Component Model sandbox with a 128 MiB
  default memory limit, 30 s execution timeout, and per-plugin
  configuration. The models capability is intentionally excluded from
  WASM (compile-time integration only).
- **Defense-in-depth security controls** — Plugin names are validated
  against path-traversal and log-injection vectors, SSRF guards filter
  outbound URLs from WASM hosts, SQL queries from WASM plugins go
  through a `sqlparser` allowlist, and script-tag escaping in
  hydration blocks XSS in rendered output.
- **Optional JavaScript SSR** — The `ts` feature enables a pure-Rust
  `boa_engine` JavaScript runtime for React/Preact SSR, with
  `scraper`-based CSS/meta extraction for the rendered output.
- **`crates_io` integration** — The `cli` feature surfaces helpers for
  publishing and pulling Delion plugins from crates.io, with a proper
  User-Agent and `reqwest::Client` shared across `HostState`
  instances.

### Notable Breaking Changes

- **`ColumnType` and `TsError` are `#[non_exhaustive]`** — Match arms
  on these enums must include a wildcard fallback.

Workspace-level breaking changes are tracked at the
[Breaking Changes Discussions](https://github.com/kent8192/reinhardt-web/discussions/categories/breaking-changes)
and summarized in the [root CHANGELOG](https://github.com/kent8192/reinhardt-web/blob/main/CHANGELOG.md#010---2026-05-22).

### Migration Notes

See the workspace-level [Migration Guide](https://github.com/kent8192/reinhardt-web/blob/main/CHANGELOG.md#010---2026-05-22)
for the full upgrade flow. Crate-specific notes:

- Add a `_ => …` fallback to existing `match` expressions on
  `ColumnType` and `TsError`.
- `wasmtime` is pinned at `36.0.6` for the lifetime of 0.1.x to clear
  the security advisories that motivated the downgrade; align your
  workspace overrides accordingly.
- The `unsafe impl Send/Sync for TsRuntime` was removed; if you held a
  `TsRuntime` across `.await` points, restructure to keep it
  task-local.
