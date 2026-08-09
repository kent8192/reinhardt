# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.3.7](https://github.com/kent8192/reinhardt-web/compare/reinhardt-i18n@v0.3.6...reinhardt-i18n@v0.3.7) - 2026-08-09

### Fixed

- *(i18n)* cover regional Portuguese plural boundaries

### Testing

- *(i18n)* raise coverage to 90%

## [0.3.0](https://github.com/kent8192/reinhardt-web/compare/reinhardt-i18n@v0.2.2...reinhardt-i18n@v0.3.0) - 2026-06-28

Stable release of `reinhardt-i18n` for the Reinhardt 0.3.0 line. This
entry consolidates the 0.3.0 release-candidate series into one
stable release section.

### Migration Notes

- Review the root CHANGELOG and `instructions/MIGRATION_0.3.md` before upgrading from 0.2.x.

### Maintenance

- merge main into develop/0.3.0

## [0.2.2](https://github.com/kent8192/reinhardt-web/compare/reinhardt-i18n@v0.2.1...reinhardt-i18n@v0.2.2) - 2026-06-25

### Documentation

- update version references to v0.2.1

## [0.2.0](https://github.com/kent8192/reinhardt-web/compare/reinhardt-i18n@v0.1.3...reinhardt-i18n@v0.2.0) - 2026-06-11

Stable release of `reinhardt-i18n` for the Reinhardt 0.2.0 line. This
entry consolidates the 0.2.0 release-candidate series into one
stable release section.

### Migration Notes

- Review the breaking changes listed below before upgrading from 0.1.x.
- See the root CHANGELOG and `instructions/MIGRATION_0.2.md` for cross-crate migration guidance.

### Changed

- [**breaking**] align develop/0.2.0 with main, preserving 8 feature crates

### Fixed

- *(ci)* recover develop release-plz prerelease

## [0.1.0](https://github.com/kent8192/reinhardt-web/compare/reinhardt-i18n@v0.1.0-rc.30...reinhardt-i18n@v0.1.0) - 2026-05-22

Initial stable release of `reinhardt-i18n` as part of the reinhardt-web
0.1.0 release. Provides locale activation, message catalog loading, and
plural-aware translation for Django-style i18n in Rust.

For the workspace-wide release narrative, see the [root CHANGELOG](https://github.com/kent8192/reinhardt-web/blob/main/CHANGELOG.md#010---2026-05-22).
Per-prerelease history is in the [Release Discussions](https://github.com/kent8192/reinhardt-web/discussions/categories/release).

### Capabilities at 0.1.0

- **Locale activation guards** — `TranslationGuard` activates a locale
  for the current task and restores the prior locale on drop, using
  `try_borrow_mut` so the destructor cannot panic re-entrantly.
- **Gettext-compatible PO parser** — Reads PO files with full
  `msgctxt` continuation-line support, enforces input-size limits, and
  refuses path traversal in `CatalogLoader::load` so untrusted locale
  directories cannot escape the catalog root.
- **Comprehensive plural rules** — Plural-form expressions cover the
  Unicode CLDR rule families (including the corrected non-plural
  language set without Hungarian) and validate plural indices to
  prevent memory exhaustion.
- **Locale and format validation** — `validate_locale()` is applied
  uniformly across every entry point, with a length cap; number and
  format-string handling rejects malformed inputs and special float
  values rather than panicking.
- **Optional DI integration** — Activated via the `di` feature, the
  crate registers translation services with `reinhardt-di` so message
  loaders can be injected via `Depends<T>`.

### Notable Breaking Changes

`reinhardt-i18n` did not introduce its own framework-wide breaking
changes in 0.1.0. Workspace-level breaking changes are tracked at the
[Breaking Changes Discussions](https://github.com/kent8192/reinhardt-web/discussions/categories/breaking-changes)
and summarized in the [root CHANGELOG](https://github.com/kent8192/reinhardt-web/blob/main/CHANGELOG.md#010---2026-05-22).

### Migration Notes

See the workspace-level [Migration Guide](https://github.com/kent8192/reinhardt-web/blob/main/CHANGELOG.md#010---2026-05-22)
for the full upgrade flow. Crate-specific notes:

- DI integration moved to `Depends<T>` ([#3628](https://github.com/kent8192/reinhardt-web/discussions/3628));
  update injection sites from `Arc<T>` accordingly.
- Eight previously unused dependencies were dropped from `Cargo.toml`;
  if you transitively depended on them through this crate, declare
  them directly.
