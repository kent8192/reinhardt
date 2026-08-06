# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.4.0-alpha.2](https://github.com/kent8192/reinhardt-web/compare/reinhardt-testkit@v0.4.0-alpha.1...reinhardt-testkit@v0.4.0-alpha.2) - 2026-07-23

### Added

- *(db)* [**breaking**] add Copy-safe injected connection handles

### Fixed

- *(db)* retain connection leases in CI consumers

## [0.4.0-alpha.1](https://github.com/kent8192/reinhardt-web/compare/reinhardt-testkit@v0.3.2...reinhardt-testkit@v0.4.0-alpha.1) - 2026-07-21

### Added

- *(testkit)* add model-derived test database fixture

### Fixed

- *(testkit)* harden test database fixture startup
- *(testkit)* preserve model schema metadata
- *(testkit)* harden model-derived schema metadata
- *(testkit)* complete model-derived schema fidelity
- *(ci)* terminate cfg aliases macro invocations
- *(release)* restore develop prerelease lifecycle

### Maintenance

- migrate dependency policy checks to cargo-deny

## [0.3.2](https://github.com/kent8192/reinhardt-web/compare/reinhardt-testkit@v0.3.1...reinhardt-testkit@v0.3.2) - 2026-07-14

### Fixed

- *(ci)* allow intentional dependency-version duplicates

## [0.3.0](https://github.com/kent8192/reinhardt-web/compare/reinhardt-testkit@v0.2.0...reinhardt-testkit@v0.3.0) - 2026-06-28

Stable release of `reinhardt-testkit` for the Reinhardt 0.3.0 line. This
entry consolidates the 0.3.0 release-candidate series into one
stable release section.

### Migration Notes

- Review the root CHANGELOG and `instructions/MIGRATION_0.3.md` before upgrading from 0.2.x.

### Added

- feat!(di): introduce keyed injectable provider outputs

### Fixed

- *(di)* honor cache false for keyed wrappers

## [0.2.0](https://github.com/kent8192/reinhardt-web/compare/reinhardt-testkit@v0.1.3...reinhardt-testkit@v0.2.0) - 2026-06-11

Stable release of `reinhardt-testkit` for the Reinhardt 0.2.0 line. This
entry consolidates the 0.2.0 release-candidate series into one
stable release section.

### Migration Notes

- Replace `force_authenticate` helpers and `with_authenticated_user` with the fluent auth APIs.
- Move old migration fixture usage to `postgres_with_migrations_from_dir(...)`.
- See [`instructions/MIGRATION_0.2.md`](../../instructions/MIGRATION_0.2.md) for the workspace migration checklist.

### Breaking Changes

- *(testkit)* [**breaking**] gate 9 RC-deprecated items behind cfg(any()) (refs [[#4520](https://github.com/kent8192/reinhardt-web/issues/4520)](https://github.com/kent8192/reinhardt-web/issues/4520))
- *(db,macros)* [**breaking**] unify custom managers with Model::objects() ([[#3984](https://github.com/kent8192/reinhardt-web/issues/3984)](https://github.com/kent8192/reinhardt-web/issues/3984))

### Added

- *(testkit)* [**breaking**] gate 9 RC-deprecated items behind cfg(any()) (refs [[#4520](https://github.com/kent8192/reinhardt-web/issues/4520)](https://github.com/kent8192/reinhardt-web/issues/4520))
- *(db,macros)* [**breaking**] unify custom managers with Model::objects() ([[#3984](https://github.com/kent8192/reinhardt-web/issues/3984)](https://github.com/kent8192/reinhardt-web/issues/3984))

### Changed

- *(testkit)* remove residual doc references to deleted deprecated APIs
- [**breaking**] align develop/0.2.0 with main, preserving 8 feature crates

### Removed

- **`APIRequestFactory::force_authenticate`** (`src/factory.rs`, deprecated `0.1.0-rc.16`) — use `client.auth().session()` or `client.auth().jwt()`.
- **`APIClient::force_authenticate`** (`src/client.rs`, deprecated `0.1.0-rc.16`) — same migration.
- **`ServerFnTestContext::with_authenticated_user`** (`src/server_fn/context.rs`, deprecated `0.1.0-rc.16`) — use `.auth().session(&user).done()`.
- **6 testcontainers fixtures + helpers** (`src/fixtures/testcontainers.rs`, deprecated `0.1.0-rc.16`) — use `postgres_with_migrations_from_dir()` and the filesystem-based migration loader.

#### BREAKING CHANGES

Removed all 9 RC-deprecated items from `reinhardt-testkit` per
STABILITY_POLICY § SP-4 (umbrella Issue
[#4520](https://github.com/kent8192/reinhardt-web/issues/4520)):

- **`APIRequestFactory::force_authenticate`** (`src/factory.rs`, deprecated `0.1.0-rc.16`) — use `client.auth().session()` or `client.auth().jwt()`.
- **`APIClient::force_authenticate`** (`src/client.rs`, deprecated `0.1.0-rc.16`) — same migration.
- **`ServerFnTestContext::with_authenticated_user`** (`src/server_fn/context.rs`, deprecated `0.1.0-rc.16`) — use `.auth().session(&user).done()`.
- **6 testcontainers fixtures + helpers** (`src/fixtures/testcontainers.rs`, deprecated `0.1.0-rc.16`) — use `postgres_with_migrations_from_dir()` and the filesystem-based migration loader.

All 9 items are gated with `#[cfg(any())]` so they no longer compile;
this preserves git blame readability for one release. A subsequent
cleanup PR can delete the gated code outright.

### Fixed

- delete gated items instead of cfg-gating, update callers
- *(testkit)* shield server fixtures from deprecated RateLimitConfig
- *(ci)* recover develop release-plz prerelease

### Styling

- apply rustfmt to non-DSL files on develop/0.2.0

### Maintenance

- update Cargo.toml dependencies

## [0.1.0](https://github.com/kent8192/reinhardt-web/compare/reinhardt-testkit@v0.1.0-rc.30...reinhardt-testkit@v0.1.0) - 2026-05-22

Initial stable release of `reinhardt-testkit` as part of the
reinhardt-web 0.1.0 release. `reinhardt-testkit` is the lower-level
test infrastructure crate that `reinhardt-test` builds on; it owns
the DI override machinery, the TestContainers fixtures, and the
in-process HTTP transport used to exercise handlers without a real
network socket.

For the workspace-wide release narrative (Highlights, Breaking
Changes, Migration Guide), see the [root CHANGELOG](https://github.com/kent8192/reinhardt-web/blob/main/CHANGELOG.md#010---2026-05-22).
Per-prerelease history is preserved in the
[Release Discussions](https://github.com/kent8192/reinhardt-web/discussions/categories/release).

### Capabilities at 0.1.0

- **`with_di_overrides!` & `DiOverrideBuilder`** — A typed builder
  plus a macro re-exported from `reinhardt-testkit-macros` lets a
  test swap a `Depends<T>` registration for a mock in one call. The
  generated context is isolated per test via
  `with_test_di_context()` so parallel `cargo nextest` runs do not
  share DI state.
- **TestContainers fixtures** — `rstest`-friendly fixtures for
  Postgres (unified version pin, pool close, cleanup backoff),
  Kafka (`apache/kafka` image, module-scoped, configurable
  partitions), Redis (single + cluster), RabbitMQ, and friends.
  `postgres_with_migrations_from_dir` loads migrations through
  `FilesystemSource` and initialises ORM global state so a fresh
  schema is ready before the first `await`.
- **In-process APIClient transport** — `APIClient` can run against
  any `Handler` directly, bypassing the network. Path parameter
  insertion order is preserved through the DI / URLs / HTTP
  pipeline so route matching is byte-identical to the production
  router.
- **Auth & session fixtures** — Builder-based auth API with JWT
  session handling; SessionData migrated to a `new()` constructor
  for `#[non_exhaustive]` compatibility, and handler-side session
  rotation is reflected back in `Set-Cookie`.
- **Cross-feature gating** — Feature flags (`testcontainers`,
  `websockets`, `graphql`, `viewsets`, `messages`, `admin`,
  `property-based`, `static`) keep heavy optional deps out of
  default builds; `full` turns them all on.
- **Security-hardened diagnostics** — Cookie validation panic
  messages were stripped of sensitive values; floor pins on
  `astral-tokio-tar` clear RUSTSEC-2026-0145; explanatory comments
  document every remaining `#[allow(dead_code)]`.

### Notable Breaking Changes

- **`global_registry`-based migration fixtures deprecated** — They
  remain available but emit `#[deprecated(since = "0.1.0-rc.16")]`.
  New code should call `postgres_with_migrations_from_dir(...)`.

### Migration Notes

- **Adopt `with_di_overrides!`**: For DI mocking, replace
  hand-rolled `InjectionContext` construction with
  `with_di_overrides! { MyTrait => Arc::new(mock) }` (or the
  builder form). This routes through
  `injection_context_with_di_overrides`, which keeps the override
  table isolated per test.
- **Migrate migration fixtures**: Replace the deprecated
  `global_registry`-based migration helpers with
  `postgres_with_migrations_from_dir(path)`. The new helper accepts
  a workspace-relative `CARGO_MANIFEST_DIR`-based path and avoids
  the global-state leakage of the legacy registry.
- **Reuse the in-process transport**: If your tests currently
  spawn a real listener to exercise handlers, consider switching
  to the `APIClient` in-process transport via the `Handler` trait —
  it's faster and avoids port-binding races on CI.
