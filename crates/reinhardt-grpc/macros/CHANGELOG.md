# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.3.7](https://github.com/kent8192/reinhardt-web/compare/reinhardt-grpc-macros@v0.3.6...reinhardt-grpc-macros@v0.3.7) - 2026-08-07

### Maintenance

- update Cargo.toml dependencies

## [0.3.6](https://github.com/kent8192/reinhardt-web/compare/reinhardt-grpc-macros@v0.3.5...reinhardt-grpc-macros@v0.3.6) - 2026-08-04

### Maintenance

- update Cargo.toml dependencies

## [0.3.5](https://github.com/kent8192/reinhardt-web/compare/reinhardt-grpc-macros@v0.3.4...reinhardt-grpc-macros@v0.3.5) - 2026-08-02

### Fixed

- *(grpc)* restore facade-only handler DI
- *(grpc)* support package-only facade dependencies
- *(grpc)* preserve explicit facade aliases

## [0.3.0](https://github.com/kent8192/reinhardt-web/compare/reinhardt-grpc-macros@v0.2.0...reinhardt-grpc-macros@v0.3.0) - 2026-06-28

Stable release of `reinhardt-grpc-macros` for the Reinhardt 0.3.0 line. This
entry consolidates the 0.3.0 release-candidate series into one
stable release section.

### Migration Notes

- Review the root CHANGELOG and `instructions/MIGRATION_0.3.md` before upgrading from 0.2.x.

### Added

- feat!(di): introduce keyed injectable provider outputs

## [0.2.0](https://github.com/kent8192/reinhardt-web/compare/reinhardt-grpc-macros@v0.1.3...reinhardt-grpc-macros@v0.2.0) - 2026-06-11

Stable release of `reinhardt-grpc-macros` for the Reinhardt 0.2.0 line. This
entry consolidates the 0.2.0 release-candidate series into one
stable release section.

### Migration Notes

- Review the breaking changes listed below before upgrading from 0.1.x.
- See the root CHANGELOG and `instructions/MIGRATION_0.2.md` for cross-crate migration guidance.

### Changed

- [**breaking**] align develop/0.2.0 with main, preserving 8 feature crates

### Fixed

- *(ci)* recover develop release-plz prerelease

### Documentation

- *(release)* enforce public API doc coverage

## [0.1.0](https://github.com/kent8192/reinhardt-web/compare/reinhardt-grpc-macros@v0.1.0-rc.30...reinhardt-grpc-macros@v0.1.0) - 2026-05-22

Initial stable release of `reinhardt-grpc-macros` as part of the
reinhardt-web 0.1.0 release. Provides the procedural macros that wire
tonic-generated gRPC services to the framework's DI runtime.

For the workspace-wide release narrative, see the [root CHANGELOG](https://github.com/kent8192/reinhardt-web/blob/main/CHANGELOG.md#010---2026-05-22).
Per-prerelease history is in the [Release Discussions](https://github.com/kent8192/reinhardt-web/discussions/categories/release).

### Capabilities at 0.1.0

- **Per-request DI context** — gRPC handler macros fork the
  `InjectionContext` per request, so each call obtains an isolated
  request scope rather than sharing the service-level scope.
- **Strict attribute validation** — unknown `inject` attribute
  options surface as compile errors; trait-impl name collisions in
  generated code were resolved during the alpha cycle.
- **Async-aware validation codegen** — generated input validators
  participate in the async control flow so service implementations
  can `.await` validation without resorting to ad-hoc spawning.
- **Hardened generated types** — macro-emitted code applies
  workspace-uniform native-tls pinning and workspace-version
  alignment, and tightens type-checks against malicious input.

### Notable Breaking Changes

- **`Injected<T>` deprecated** ([#3631](https://github.com/kent8192/reinhardt-web/discussions/3631))
  — generated handler wrappers expose `Depends<T>` injection sites.

### Migration Notes

See the [root CHANGELOG migration guide](https://github.com/kent8192/reinhardt-web/blob/main/CHANGELOG.md#migration-guide).
Macro-side migration is mechanical: existing `#[inject]` parameters
move from `Injected<T>` / `Arc<T>` to `Depends<T>`.
