# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.4.0-alpha.7](https://github.com/kent8192/reinhardt-web/compare/reinhardt-rest@v0.4.0-alpha.6...reinhardt-rest@v0.4.0-alpha.7) - 2026-08-19

### Documentation

- update version references to v0.3.3
- update version references to v0.3.4
- update version references to v0.3.5
- update version references to v0.3.6
- update version references to v0.3.7
- update version references to v0.3.8

### Maintenance

- merge main into develop/0.4.0

## [0.4.0-alpha.6](https://github.com/kent8192/reinhardt-web/compare/reinhardt-rest@v0.4.0-alpha.5...reinhardt-rest@v0.4.0-alpha.6) - 2026-08-06

### Documentation

- *(release)* restore coherent alpha.3 references

## [0.4.0-alpha.4](https://github.com/kent8192/reinhardt-web/compare/reinhardt-rest@v0.4.0-alpha.3...reinhardt-rest@v0.4.0-alpha.4) - 2026-08-04

### Fixed

- *(ci)* repair pgvector test coverage

## [0.4.0-alpha.2](https://github.com/kent8192/reinhardt-web/compare/reinhardt-rest@v0.4.0-alpha.1...reinhardt-rest@v0.4.0-alpha.2) - 2026-07-23

### Added

- *(db)* [**breaking**] add Copy-safe injected connection handles

## [0.4.0-alpha.1](https://github.com/kent8192/reinhardt-web/compare/reinhardt-rest@v0.3.2...reinhardt-rest@v0.4.0-alpha.1) - 2026-07-21

### Changed

- [**breaking**] replace dynamic domain error adapters

### Fixed

- *(rest)* preserve domain error boundaries
- preserve internal error classification
- *(rest)* classify invalid model shapes as internal errors
- *(errors)* preserve database retry classifications
- preserve structured database errors
- *(release)* restore develop prerelease lifecycle

### Maintenance

- merge latest main into develop forward-merge
- merge develop/0.4.0 into fix/issue-5561-remove-anyhow
- merge develop/0.4.0 into remove-anyhow branch
## [0.3.10](https://github.com/kent8192/reinhardt-web/compare/reinhardt-rest@v0.3.9...reinhardt-rest@v0.3.10) - 2026-08-22

### Maintenance

- update Cargo.toml dependencies

## [0.3.9](https://github.com/kent8192/reinhardt-web/compare/reinhardt-rest@v0.3.8...reinhardt-rest@v0.3.9) - 2026-08-21

### Added

- *(rest)* expose configurable field errors
- *(serializers)* store error formatters on serializer fields

### Fixed

- *(serializers)* keep field structs constructible with error messages

### Other

- bring main into configurable field error messages
- keep field error formatters with JSON extraction

### Testing

- *(serializers)* cover required, fallback, and field-type stability

## [0.3.8](https://github.com/kent8192/reinhardt-web/compare/reinhardt-rest@v0.3.7...reinhardt-rest@v0.3.8) - 2026-08-16

### Maintenance

- update Cargo.toml dependencies

## [0.3.7](https://github.com/kent8192/reinhardt-web/compare/reinhardt-rest@v0.3.6...reinhardt-rest@v0.3.7) - 2026-08-12

### Maintenance

- update Cargo.toml dependencies

## [0.3.6](https://github.com/kent8192/reinhardt-web/compare/reinhardt-rest@v0.3.5...reinhardt-rest@v0.3.6) - 2026-08-04

### Maintenance

- update Cargo.toml dependencies

## [0.3.5](https://github.com/kent8192/reinhardt-web/compare/reinhardt-rest@v0.3.4...reinhardt-rest@v0.3.5) - 2026-08-02

### Maintenance

- update Cargo.toml dependencies

## [0.3.4](https://github.com/kent8192/reinhardt-web/compare/reinhardt-rest@v0.3.3...reinhardt-rest@v0.3.4) - 2026-07-30

### Maintenance

- update Cargo.toml dependencies

## [0.3.3](https://github.com/kent8192/reinhardt-web/compare/reinhardt-rest@v0.3.2...reinhardt-rest@v0.3.3) - 2026-07-28

### Maintenance

- update Cargo.toml dependencies

## [0.3.1](https://github.com/kent8192/reinhardt-web/compare/reinhardt-rest@v0.3.0...reinhardt-rest@v0.3.1) - 2026-07-04

### Added

- *(db)* add scoped n+1 query detection

### Fixed

- *(rest)* keep validator doctests feature neutral
- *(ci)* unblock quick-xml security audit

## [0.3.0](https://github.com/kent8192/reinhardt-web/compare/reinhardt-rest@v0.2.0...reinhardt-rest@v0.3.0) - 2026-06-28

Stable release of `reinhardt-rest` for the Reinhardt 0.3.0 line. This
entry consolidates the 0.3.0 release-candidate series into one
stable release section.

### Migration Notes

- Review the root CHANGELOG and `instructions/MIGRATION_0.3.md` before upgrading from 0.2.x.

### Added

- *(urls)* [**breaking**] remove raw server route registration APIs

### Fixed

- *(todo-check)* clear public api audit markers

### Maintenance

- merge main into develop/0.3.0

## [0.2.0](https://github.com/kent8192/reinhardt-web/compare/reinhardt-rest@v0.1.3...reinhardt-rest@v0.2.0) - 2026-06-11

Stable release of `reinhardt-rest` for the Reinhardt 0.2.0 line. This
entry consolidates the 0.2.0 release-candidate series into one
stable release section.

### Migration Notes

- Replace `OpenApiConfig` with `OpenApiSettings` and review versioning settings fragments.
- See [`instructions/MIGRATION_0.2.md`](../../instructions/MIGRATION_0.2.md) for the workspace migration checklist.

### Breaking Changes

- *(auth)* [**breaking**] migrate internal consumers from removed User/SimpleUser types
- *(rest)* [**breaking**] remove deprecated OpenApiConfig struct (refs [[#4520](https://github.com/kent8192/reinhardt-web/issues/4520)](https://github.com/kent8192/reinhardt-web/issues/4520))
- *(db,macros)* [**breaking**] unify custom managers with Model::objects() ([[#3984](https://github.com/kent8192/reinhardt-web/issues/3984)](https://github.com/kent8192/reinhardt-web/issues/3984))

### Added

- *(rest)* [**breaking**] remove deprecated OpenApiConfig struct (refs [[#4520](https://github.com/kent8192/reinhardt-web/issues/4520)](https://github.com/kent8192/reinhardt-web/issues/4520))
- *(db,macros)* [**breaking**] unify custom managers with Model::objects() ([[#3984](https://github.com/kent8192/reinhardt-web/issues/3984)](https://github.com/kent8192/reinhardt-web/issues/3984))

### Changed

- [**breaking**] align develop/0.2.0 with main, preserving 8 feature crates

### Removed

- **`OpenApiConfig` struct** (`src/openapi/config.rs`, deprecated since
  `0.1.0-rc.16`) — removed per STABILITY_POLICY § SP-4. Use
  `OpenApiSettings` from `reinhardt_conf::settings::openapi` instead.
  Refs [#4520](https://github.com/kent8192/reinhardt-web/issues/4520).

#### BREAKING CHANGES

- **`OpenApiConfig` struct** (`src/openapi/config.rs`, deprecated since
  `0.1.0-rc.16`) — removed per STABILITY_POLICY § SP-4. Use
  `OpenApiSettings` from `reinhardt_conf::settings::openapi` instead.
  Refs [#4520](https://github.com/kent8192/reinhardt-web/issues/4520).

No workspace consumers referenced `OpenApiConfig` directly, so no
follow-up consumer migration is required.

### Fixed

- keep openapi facade feature standalone
- *(rest)* remove deletion-history comments from openapi.rs
- *(auth)* [**breaking**] migrate internal consumers from removed User/SimpleUser types
- *(ci)* recover develop release-plz prerelease
- *(auth)* address CodeRabbit review feedback

### Performance

- atomize facade dependency feature gates
- trim standard facade feature dependencies

### Documentation

- *(reinhardt-rest)* fix ModelSerializer doctests after DefaultUser removal

### Maintenance

- update Cargo.toml dependencies
- *(examples)* remove examples-twitter

## [0.1.0](https://github.com/kent8192/reinhardt-web/compare/reinhardt-rest@v0.1.0-rc.30...reinhardt-rest@v0.1.0) - 2026-05-22

Initial stable release of `reinhardt-rest` as part of the reinhardt-web
0.1.0 release. Provides the Django-REST-Framework-style API layer:
serializers with declarative validation, ViewSets, filter backends, a
browsable API, configurable versioning, and OpenAPI schema generation.

For the workspace-wide release narrative, see the [root CHANGELOG](https://github.com/kent8192/reinhardt-web/blob/main/CHANGELOG.md#010---2026-05-22).
Per-prerelease history is in the [Release Discussions](https://github.com/kent8192/reinhardt-web/discussions/categories/release).

### Capabilities at 0.1.0

- **RESTful framework primitives** — Serializers (including
  `ModelSerializer` honoring `MetaConfig`, and a contract-correct
  `WritableNestedSerializer`), ViewSets, browsable API interface, and
  filter backends that preserve existing `ORDER BY` / `WHERE` clauses
  and use parameterized queries in `SimpleSearchBackend`.
- **Settings-driven versioning** — `VersioningSettings` replaces the
  previous `VersioningConfig::from_env` constructor; routers wire into
  `reinhardt_router::VersionedRouter` so versioning strategies
  introspect routes without depending on `reinhardt-urls` directly
  (resolves the urls ↔ rest cycle tracked in [#4321](https://github.com/kent8192/reinhardt-web/issues/4321)).
  See [Discussion #4294](https://github.com/kent8192/reinhardt-web/discussions/4294).
- **Operation-level OpenAPI attributes** — Route-level OpenAPI
  attributes attach metadata to ViewSet operations, integrated with
  `AuthProtection` and `EndpointMetadata` for automatic OpenAPI
  security-scheme generation.
- **Filter and search safety** — Version-prefix regex enforces segment
  boundaries, operator state resets between filter evaluations,
  UTF-8-safe length checks, case-insensitive keyword scanning via
  `eq_ignore_ascii_case`, and PostgreSQL / MySQL dialect support in
  search backends.
- **Performance-tuned dispatch** — Per-request allocations reduced in
  the ViewSet dispatch hot path, with compiled regex caches in
  `NamespaceVersioning`.

### Notable Breaking Changes

- **`VersioningSettings`** ([Discussion #4294](https://github.com/kent8192/reinhardt-web/discussions/4294))
  — REST versioning is configured through the typed settings system;
  `VersioningConfig::from_env` is gone.
- **`define_views!` replaces `#[export_endpoints]`** ([Discussion #3768](https://github.com/kent8192/reinhardt-web/discussions/3768))
  — Multi-file view modules now use the declarative macro for
  stable-Rust compatibility.
- **Apps relocation** ([Discussion #4476](https://github.com/kent8192/reinhardt-web/discussions/4476))
  — Per-app `server_fn` and client UI moved into `apps/<app>/`;
  affects how REST endpoints are wired into a project.

### Migration Notes

This is the first stable release. See the [root CHANGELOG](https://github.com/kent8192/reinhardt-web/blob/main/CHANGELOG.md#010---2026-05-22)
for the cross-crate migration guide; for REST-specific moves, follow
Discussions [#4294](https://github.com/kent8192/reinhardt-web/discussions/4294)
and [#3768](https://github.com/kent8192/reinhardt-web/discussions/3768).
