# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.4.0-alpha.12](https://github.com/kent8192/reinhardt-web/compare/reinhardt-openapi-macros@v0.4.0-alpha.11...reinhardt-openapi-macros@v0.4.0-alpha.12) - 2026-08-30

### Fixed

- *(macros)* address dto schema review feedback
- *(macros)* complete dto schema facade wiring

### Maintenance

- merge main into develop/0.4.0

## [0.4.0-alpha.1](https://github.com/kent8192/reinhardt-web/compare/reinhardt-openapi-macros@v0.3.2...reinhardt-openapi-macros@v0.4.0-alpha.1) - 2026-07-21

### Fixed

- *(release)* restore develop prerelease lifecycle

## [0.3.0](https://github.com/kent8192/reinhardt-web/compare/reinhardt-openapi-macros@v0.2.0...reinhardt-openapi-macros@v0.3.0) - 2026-06-28

Stable release of `reinhardt-openapi-macros` for the Reinhardt 0.3.0 line. This
crate moves with the coordinated Reinhardt 0.3.0 release train.

### Migration Notes

- Review the root CHANGELOG and `instructions/MIGRATION_0.3.md` before upgrading from 0.2.x.

### Maintenance

- align crate release metadata with the Reinhardt 0.3.0 stable release train.

## [0.2.0](https://github.com/kent8192/reinhardt-web/compare/reinhardt-openapi-macros@v0.1.3...reinhardt-openapi-macros@v0.2.0) - 2026-06-11

Stable release of `reinhardt-openapi-macros` for the Reinhardt 0.2.0 line. This
entry consolidates the 0.2.0 release-candidate series into one
stable release section.

### Migration Notes

- Regenerate REST/OpenAPI macro output after moving to settings fragments.
- See [`instructions/MIGRATION_0.2.md`](../../../instructions/MIGRATION_0.2.md) for the workspace migration checklist.

### Changed

- [**breaking**] align develop/0.2.0 with main, preserving 8 feature crates

### Fixed

- *(ci)* recover develop release-plz prerelease

### Documentation

- *(release)* enforce public API doc coverage

## [0.1.0](https://github.com/kent8192/reinhardt-web/compare/reinhardt-openapi-macros@v0.1.0-rc.30...reinhardt-openapi-macros@v0.1.0) - 2026-05-22

Initial stable release of `reinhardt-openapi-macros` as part of the
reinhardt-web 0.1.0 release. Procedural-macro companion to
`reinhardt-rest` that emits operation- and container-level OpenAPI
metadata directly from ViewSet attributes.

For the workspace-wide release narrative, see the [root CHANGELOG](https://github.com/kent8192/reinhardt-web/blob/main/CHANGELOG.md#010---2026-05-22).
Per-prerelease history is in the [Release Discussions](https://github.com/kent8192/reinhardt-web/discussions/categories/release).

### Capabilities at 0.1.0

- **Container-level OpenAPI attributes** — Attribute macros annotate
  ViewSet containers with OpenAPI schema metadata so the generated
  spec carries grouping, tagging, and documentation context.
- **Safe attribute parsing** — Parse errors propagate as proper
  `compile_error!` diagnostics instead of panicking; min/max
  constraints validate at expansion time; `get_ident()` replaces
  unchecked `expect()`.
- **Serde-aware validation** — Serde attribute handling and validation
  are normalized so OpenAPI output stays consistent with the runtime
  serialization shape.
- **Workspace-version pinning** — Proc-macro dependencies (including
  `native-tls`) align with workspace versions to keep the published
  graph reproducible.

### Notable Breaking Changes

This is a proc-macro crate consumed exclusively by `reinhardt-rest`;
breaking changes flow through that crate. See the [Breaking Changes
Discussions](https://github.com/kent8192/reinhardt-web/discussions/categories/breaking-changes).

### Migration Notes

This is the first stable release, so there is no prior stable version
to migrate from. See the [root CHANGELOG](https://github.com/kent8192/reinhardt-web/blob/main/CHANGELOG.md#010---2026-05-22)
for the cross-crate migration guide.
