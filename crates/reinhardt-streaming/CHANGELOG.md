# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.4.0-alpha.7](https://github.com/kent8192/reinhardt-web/compare/reinhardt-streaming@v0.4.0-alpha.6...reinhardt-streaming@v0.4.0-alpha.7) - 2026-08-16

### Maintenance

- update Cargo.toml dependencies

## [0.4.0-alpha.1](https://github.com/kent8192/reinhardt-web/compare/reinhardt-streaming@v0.3.2...reinhardt-streaming@v0.4.0-alpha.1) - 2026-07-21

### Fixed

- *(release)* restore develop prerelease lifecycle

## [0.3.0](https://github.com/kent8192/reinhardt-web/compare/reinhardt-streaming@v0.2.0...reinhardt-streaming@v0.3.0) - 2026-06-28

Stable release of `reinhardt-streaming` for the Reinhardt 0.3.0 line. This
crate moves with the coordinated Reinhardt 0.3.0 release train.

### Migration Notes

- Review the root CHANGELOG and `instructions/MIGRATION_0.3.md` before upgrading from 0.2.x.

### Maintenance

- align crate release metadata with the Reinhardt 0.3.0 stable release train.

## [0.2.0](https://github.com/kent8192/reinhardt-web/compare/reinhardt-streaming@v0.1.3...reinhardt-streaming@v0.2.0) - 2026-06-11

Stable release of `reinhardt-streaming` for the Reinhardt 0.2.0 line. This
entry consolidates the 0.2.0 release-candidate series into one
stable release section.

### Changed

- remove StreamingRef accessor and stale ResolvedUrls references

### Fixed

- *(ci)* recover develop release-plz prerelease

### Documentation

- *(release)* enforce public API doc coverage

## [0.1.0](https://github.com/kent8192/reinhardt-web/compare/reinhardt-streaming@v0.1.0-rc.30...reinhardt-streaming@v0.1.0) - 2026-05-22

Initial stable release of `reinhardt-streaming` as part of the
reinhardt-web 0.1.0 release. Provides a backend-agnostic async
streaming abstraction with a first-party Kafka backend (`rskafka`).

For the workspace-wide release narrative, see the [root CHANGELOG](https://github.com/kent8192/reinhardt-web/blob/main/CHANGELOG.md#010---2026-05-22).
Per-prerelease history is in the [Release Discussions](https://github.com/kent8192/reinhardt-web/discussions/categories/release).

### Capabilities at 0.1.0

- **Backend-agnostic stream API** — A `StreamBackend` trait with
  inventory-based registration lets applications swap producers and
  consumers without touching call sites, with strongly-typed payloads
  serialized via `serde_json`.
- **Kafka backend (rskafka, rustls)** — Behind the `kafka` feature
  flag, the Kafka backend exposes typed producer / consumer handles
  and the `KafkaConfig.partitions` knob for tuning publish targets.
- **TestContainers Kafka fixture** — A module-scoped Kafka fixture
  in `reinhardt-testkit` starts an ephemeral broker per test module,
  enabling deterministic integration tests for backend error paths
  and partitioning behavior.
- **Hardened error surface** — Kafka error paths are exercised with
  flakiness-resistant tests so transport errors propagate as typed
  `thiserror` variants rather than panicking.

### Notable Breaking Changes

`reinhardt-streaming` did not introduce its own framework-wide
breaking changes in 0.1.0. Workspace-level breaking changes are
tracked at the [Breaking Changes Discussions](https://github.com/kent8192/reinhardt-web/discussions/categories/breaking-changes)
and summarized in the [root CHANGELOG](https://github.com/kent8192/reinhardt-web/blob/main/CHANGELOG.md#010---2026-05-22).

### Migration Notes

See the workspace-level [Migration Guide](https://github.com/kent8192/reinhardt-web/blob/main/CHANGELOG.md#010---2026-05-22)
for the full upgrade flow. This crate has no crate-specific migration
steps for the 0.1.0 transition; consumers staying on the Kafka backend
need only enable the `kafka` feature flag and supply a `KafkaConfig`
with the new `partitions` field.
