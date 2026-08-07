# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.3.7](https://github.com/kent8192/reinhardt-web/compare/reinhardt-testkit-macros@v0.3.6...reinhardt-testkit-macros@v0.3.7) - 2026-08-07

### Testing

- *(testkit)* cover DI override macro contracts

## [0.3.0](https://github.com/kent8192/reinhardt-web/compare/reinhardt-testkit-macros@v0.2.0...reinhardt-testkit-macros@v0.3.0) - 2026-06-28

Stable release of `reinhardt-testkit-macros` for the Reinhardt 0.3.0 line. This
crate moves with the coordinated Reinhardt 0.3.0 release train.

### Migration Notes

- Review the root CHANGELOG and `instructions/MIGRATION_0.3.md` before upgrading from 0.2.x.

### Maintenance

- align crate release metadata with the Reinhardt 0.3.0 stable release train.

## [0.2.0](https://github.com/kent8192/reinhardt-web/compare/reinhardt-testkit-macros@v0.1.3...reinhardt-testkit-macros@v0.2.0) - 2026-06-11

Stable release of `reinhardt-testkit-macros` for the Reinhardt 0.2.0 line. This
entry consolidates the 0.2.0 release-candidate series into one
stable release section.

### Migration Notes

- Use the final testkit override and auth helper APIs when regenerating fixtures.
- See [`instructions/MIGRATION_0.2.md`](../../instructions/MIGRATION_0.2.md) for the workspace migration checklist.

### Fixed

- *(ci)* recover develop release-plz prerelease

### Documentation

- *(release)* enforce public API doc coverage
- *(di)* update public docs to reflect per-context registry isolation

## [0.1.0](https://github.com/kent8192/reinhardt-web/compare/reinhardt-testkit-macros@v0.1.0-rc.30...reinhardt-testkit-macros@v0.1.0) - 2026-05-22

Initial stable release of `reinhardt-testkit-macros` as part of the
reinhardt-web 0.1.0 release. `reinhardt-testkit-macros` provides the
`with_di_overrides!` procedural macro that powers ergonomic DI
mocking in `reinhardt-testkit` tests.

For the workspace-wide release narrative (Highlights, Breaking
Changes, Migration Guide), see the [root CHANGELOG](https://github.com/kent8192/reinhardt-web/blob/main/CHANGELOG.md#010---2026-05-22).
Per-prerelease history is preserved in the
[Release Discussions](https://github.com/kent8192/reinhardt-web/discussions/categories/release).

### Capabilities at 0.1.0

- **`with_di_overrides!` macro** — One-line DI mocking inside a test:
  the macro expands to a `DiOverrideBuilder` chain that registers
  the requested overrides and yields a scoped `InjectionContext`
  ready to drive a handler or service.
- **Generated paths routed through `reinhardt-testkit`** — The
  macro emits fully-qualified paths that resolve through the
  `reinhardt-testkit` facade, so downstream tests do not have to
  add `reinhardt-testkit-macros` to their dependency list directly
  (it is re-exported).
- **Stable proc-macro toolchain** — Pinned to workspace `syn` /
  `quote` / `proc-macro2` versions with `trybuild` and `rstest`
  coverage; KI-2-compliant `[dev-dependencies]` (path-only
  `reinhardt-testkit`) so publish ordering remains correct.

### Notable Breaking Changes

This release does not introduce crate-level breaking changes.

### Migration Notes

This is the first stable release of the macro and no migration is
required from prerelease versions; the macro signature has been
stable since `0.1.0-rc.16`.
