# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.4.0-alpha.2](https://github.com/kent8192/reinhardt-web/compare/reinhardt-test@v0.4.0-alpha.1...reinhardt-test@v0.4.0-alpha.2) - 2026-07-23

### Added

- *(db)* [**breaking**] add Copy-safe injected connection handles

## [0.4.0-alpha.1](https://github.com/kent8192/reinhardt-web/compare/reinhardt-test@v0.3.2...reinhardt-test@v0.4.0-alpha.1) - 2026-07-21

### Added

- *(testkit)* add model-derived test database fixture

### Changed

- *(pages)* migrate model-set error adapters

### Fixed

- *(testkit)* complete model-derived schema fidelity
- *(ci)* terminate cfg aliases macro invocations
- *(pages)* envelope server function failures
- *(release)* restore develop prerelease lifecycle

### Maintenance

- merge develop/0.4.0 into server function set branch

## [0.3.0](https://github.com/kent8192/reinhardt-web/compare/reinhardt-test@v0.2.0...reinhardt-test@v0.3.0) - 2026-06-28

Stable release of `reinhardt-test` for the Reinhardt 0.3.0 line. This
entry consolidates the 0.3.0 release-candidate series into one
stable release section.

### Migration Notes

- Review the root CHANGELOG and `instructions/MIGRATION_0.3.md` before upgrading from 0.2.x.

### Added

- *(test)* add native msw mock server
- Native `reinhardt_test::msw::MockServiceWorker` support backed by a loopback
  HTTP mock server for explicit endpoint injection.

### Changed

- *(test)* share msw state across runtimes
- *(test)* adapt wasm msw to shared state

### Fixed

- *(scaffolding)* generate target-neutral Pages apps
- *(test)* consume native msw network error handlers
- *(test)* reject concurrent native msw startup
- *(test)* serialize native msw lifecycle state
- *(test)* stop native MSW keep-alive tasks

### Documentation

- *(test)* document native msw endpoint injection

### Testing

- *(test)* add native msw behavior coverage
- *(test)* tighten native msw coverage

### Styling

- *(test)* format native msw changes

## [0.2.0](https://github.com/kent8192/reinhardt-web/compare/reinhardt-test@v0.1.3...reinhardt-test@v0.2.0) - 2026-06-11

Stable release of `reinhardt-test` for the Reinhardt 0.2.0 line. This
entry consolidates the 0.2.0 release-candidate series into one
stable release section.

### Migration Notes

- Replace `MockFetch` / `mock_server_fn` with `MockServiceWorker` and use test-local user types instead of the built-in `TestUser` fixture.
- See [`instructions/MIGRATION_0.2.md`](../../instructions/MIGRATION_0.2.md) for the workspace migration checklist.

### Breaking Changes

- *(test)* [**breaking**] gate MockFetch and TestUser behind cfg(any()) (refs [[#4520](https://github.com/kent8192/reinhardt-web/issues/4520)](https://github.com/kent8192/reinhardt-web/issues/4520))

### Added

- *(test)* [**breaking**] gate MockFetch and TestUser behind cfg(any()) (refs [[#4520](https://github.com/kent8192/reinhardt-web/issues/4520)](https://github.com/kent8192/reinhardt-web/issues/4520))

### Changed

- *(pages)* delete deprecated mock_server_fn and use_action_state APIs
- [**breaking**] align develop/0.2.0 with main, preserving 8 feature crates

### Removed

- **`MockFetch` struct** (`src/wasm/mock.rs`, deprecated `0.1.0-rc.16`, refs #3283) — use `MockServiceWorker` from `reinhardt_test::msw`.
- **`TestUser` struct** (`src/fixtures/auth.rs`, deprecated `0.1.0-rc.16`) — define your own user type with `#[user]` macro and use `ForceLoginUser` trait.

#### BREAKING CHANGES

Removed both RC-deprecated items per STABILITY_POLICY § SP-4
(umbrella Issue [#4520](https://github.com/kent8192/reinhardt-web/issues/4520)):

- **`MockFetch` struct** (`src/wasm/mock.rs`, deprecated `0.1.0-rc.16`, refs #3283) — use `MockServiceWorker` from `reinhardt_test::msw`.
- **`TestUser` struct** (`src/fixtures/auth.rs`, deprecated `0.1.0-rc.16`) — define your own user type with `#[user]` macro and use `ForceLoginUser` trait.

Both items are gated with `#[cfg(any())]` (compile-excluded).

### Fixed

- *(test)* stabilize WASM MSW server_fn transport
- delete gated items instead of cfg-gating, update callers
- *(test)* restore missing pub use prefix in wasm mock re-export
- *(examples)* render basis tutorial vote choices
- Exposed `reinhardt::test::msw::MockServiceWorker` for WASM consumers that
  enable the root `msw` facade feature.
- *(build)* address CodeRabbit review feedback
- *(build)* port Codex review follow-ups
- *(ci)* recover develop release-plz prerelease

### Styling

- apply formatter fixes across workspace

## [0.1.0](https://github.com/kent8192/reinhardt-web/compare/reinhardt-test@v0.1.0-rc.30...reinhardt-test@v0.1.0) - 2026-05-22

Initial stable release of `reinhardt-test` as part of the
reinhardt-web 0.1.0 release. `reinhardt-test` is the user-facing
testing facade: it bundles `rstest` fixtures for TestContainers-
backed databases (PostgreSQL, MySQL, SQLite, CockroachDB), message
brokers (Redis, Kafka, RabbitMQ), and WASM / E2E browser harnesses
on top of the lower-level `reinhardt-testkit`.

For the workspace-wide release narrative (Highlights, Breaking
Changes, Migration Guide), see the [root CHANGELOG](https://github.com/kent8192/reinhardt-web/blob/main/CHANGELOG.md#010---2026-05-22).
Per-prerelease history is preserved in the
[Release Discussions](https://github.com/kent8192/reinhardt-web/discussions/categories/release).

### Capabilities at 0.1.0

- **TestContainers fixtures for every supported backend** — `rstest`
  fixtures bring up PostgreSQL 17, MySQL, SQLite, CockroachDB, Redis
  (single + cluster), Kafka, and RabbitMQ with deterministic
  lifecycles. Containers use module-scoped or per-test scopes as
  appropriate, with random UUID v7 suffixes to avoid collisions
  under `cargo nextest` parallelism.
- **WASM SPA test harness** — Feature-gated `wasm` / `wasm-full` /
  `msw` stacks wire `wasm-bindgen-test`, `web-sys`, `js-sys`, and
  `gloo-timers` into an integration suite that drives `reinhardt-
  pages` SPA UIs in a real browser. The MSW-style network-level
  request interceptor replaces the deprecated `MockFetch` /
  `mock_server_fn` mocks for new code.
- **E2E browser fixtures (fantoccini & CDP)** — `e2e` (WebDriver via
  `fantoccini`) and `e2e-cdp` (Chrome DevTools Protocol via
  `chromiumoxide`, paired with a containerised Chrome) provide two
  complementary E2E paths. The CDP fixture documents the Docker
  Engine 20.10 `host-gateway` requirement and resolves
  `host.docker.internal` for tests that target the host loopback.
- **Admin & auth integration fixtures** — Optional `admin` feature
  provisions Postgres + ORM + auth in one fixture so admin-panel
  permission tests can run end-to-end. Auth fixtures inject
  `is_staff` / `is_superuser` JWT claims and propagate handler-side
  session ID rotation through `Set-Cookie`.
- **Delegation to `reinhardt-testkit`** — Native targets re-export
  from `reinhardt-testkit`, so a test file consumes a single facade
  (`reinhardt_test::fixtures::*`) regardless of which underlying
  capability it touches.
- **Security-hardened helpers** — Path-traversal guards on
  `temp_file_url`, cookie-header injection prevention, URL encoding
  on query parameters, and `escape_html_content` /
  `escape_css_selector` on every WASM string-rendering path.

### Notable Breaking Changes

This crate does not introduce crate-level breaking changes at the
0.1.0 boundary beyond the deprecations listed below. See the
[root Migration Guide](https://github.com/kent8192/reinhardt-web/blob/main/CHANGELOG.md#010---2026-05-22)
for workspace-wide changes (e.g., `Depends<T>`, typed URL routing)
that affect the application code under test.

### Migration Notes

- **`MockFetch` / `mock_server_fn` → MSW interceptor**: Both are
  `#[deprecated(since = "0.1.0-rc.16")]`. Migrate to the
  `reinhardt_test::msw` module: it intercepts network requests at
  the boundary instead of stubbing function pointers, which keeps
  Server Function tests aligned with what the browser actually
  sends.
- **`target_arch = "wasm32"` → `target_family` + `target_os`**: If
  your downstream tests gated code on `target_arch = "wasm32"`,
  switch to `all(target_family = "wasm", target_os = "unknown")` —
  this matches the workspace-wide cfg layout and avoids future drift
  to WASIp1 / WASIp2.
- **Feature renamed `static` → `staticfiles`**: The `staticfiles`
  feature flag tracks the upstream `reinhardt-utils::staticfiles`
  module rename ([#114](https://github.com/kent8192/reinhardt-web/issues/114)).
  Update `Cargo.toml` feature lists accordingly.
