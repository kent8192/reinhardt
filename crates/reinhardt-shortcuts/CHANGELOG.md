# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.4.0-alpha.1](https://github.com/kent8192/reinhardt-web/compare/reinhardt-shortcuts@v0.3.2...reinhardt-shortcuts@v0.4.0-alpha.1) - 2026-07-21

### Fixed

- *(release)* restore develop prerelease lifecycle

### Maintenance

- merge main into develop/0.4.0

## [0.3.0](https://github.com/kent8192/reinhardt-web/compare/reinhardt-shortcuts@v0.2.2...reinhardt-shortcuts@v0.3.0) - 2026-06-28

Stable release of `reinhardt-shortcuts` for the Reinhardt 0.3.0 line. This
entry consolidates the 0.3.0 release-candidate series into one
stable release section.

### Migration Notes

- Review the root CHANGELOG and `instructions/MIGRATION_0.3.md` before upgrading from 0.2.x.

### Maintenance

- merge main into develop/0.3.0

## [0.2.2](https://github.com/kent8192/reinhardt-web/compare/reinhardt-shortcuts@v0.2.1...reinhardt-shortcuts@v0.2.2) - 2026-06-25

### Documentation

- update version references to v0.2.1

## [0.2.0](https://github.com/kent8192/reinhardt-web/compare/reinhardt-shortcuts@v0.1.3...reinhardt-shortcuts@v0.2.0) - 2026-06-11

Stable release of `reinhardt-shortcuts` for the Reinhardt 0.2.0 line. This
entry consolidates the 0.2.0 release-candidate series into one
stable release section.

### Migration Notes

- Review the breaking changes listed below before upgrading from 0.1.x.
- See the root CHANGELOG and `instructions/MIGRATION_0.2.md` for cross-crate migration guidance.

### Breaking Changes

- *(db,macros)* [**breaking**] unify custom managers with Model::objects() ([[#3984](https://github.com/kent8192/reinhardt-web/issues/3984)](https://github.com/kent8192/reinhardt-web/issues/3984))

### Added

- *(db,macros)* [**breaking**] unify custom managers with Model::objects() ([[#3984](https://github.com/kent8192/reinhardt-web/issues/3984)](https://github.com/kent8192/reinhardt-web/issues/3984))

### Changed

- [**breaking**] align develop/0.2.0 with main, preserving 8 feature crates

### Fixed

- *(build)* port Codex review follow-ups
- *(ci)* recover develop release-plz prerelease

## [0.1.0](https://github.com/kent8192/reinhardt-web/compare/reinhardt-shortcuts@v0.1.0-rc.30...reinhardt-shortcuts@v0.1.0) - 2026-05-22

Initial stable release of `reinhardt-shortcuts` as part of the
reinhardt-web 0.1.0 release. Provides Django-style shortcut functions
(`redirect`, `render`, `get_object_or_404`) on top of `reinhardt-http`
and `reinhardt-views`.

For the workspace-wide release narrative, see the [root CHANGELOG](https://github.com/kent8192/reinhardt-web/blob/main/CHANGELOG.md#010---2026-05-22).
Per-prerelease history is in the [Release Discussions](https://github.com/kent8192/reinhardt-web/discussions/categories/release).

### Capabilities at 0.1.0

- **Django-style shortcuts** — `redirect`, `render`, `render_to_string`,
  and `get_object_or_404` shortcuts that compose with typed URL
  resolvers and the template/page rendering stack, removing
  boilerplate from view handlers.
- **Hardened redirect and rendering** — Open-redirect attacks are
  blocked at the shortcut layer, hardcoded headers use
  `HeaderValue::from_static`, and `render_to_string` preserves data
  integrity while sanitizing 404 error output.
- **XSS-safe template rendering** — `render_html` documents its XSS
  contract and routes user input through `reinhardt_core`'s
  `escape_html` so shortcut callers cannot accidentally bypass
  escaping.
- **Optional ORM shortcut helpers** — Behind the `database` feature
  flag, helpers like `get_object_or_404` integrate with `reinhardt-db`
  without leaking database error messages into HTTP responses.
- **TemplateContext with capacity guard** — Template contexts are
  bounded by a configurable capacity to prevent runaway accumulation
  inside long-lived render pipelines.

### Notable Breaking Changes

`reinhardt-shortcuts` did not introduce its own framework-wide
breaking changes in 0.1.0. Workspace-level breaking changes are
tracked at the [Breaking Changes Discussions](https://github.com/kent8192/reinhardt-web/discussions/categories/breaking-changes)
and summarized in the [root CHANGELOG](https://github.com/kent8192/reinhardt-web/blob/main/CHANGELOG.md#010---2026-05-22).

### Migration Notes

See the workspace-level [Migration Guide](https://github.com/kent8192/reinhardt-web/blob/main/CHANGELOG.md#010---2026-05-22)
for the full upgrade flow. Crate-specific notes:

- The `database` feature no longer pins the PostgreSQL backend; enable
  the desired backend feature on `reinhardt-db` explicitly.
- `From` conversions that previously allowed unvalidated URLs into
  `redirect()` were removed ([#726](https://github.com/kent8192/reinhardt-web/issues/726));
  validated `Url` types are now required at the boundary.
