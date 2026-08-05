# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.4.0-alpha.2](https://github.com/kent8192/reinhardt-web/compare/reinhardt-websockets@v0.4.0-alpha.1...reinhardt-websockets@v0.4.0-alpha.2) - 2026-07-23

### Added

- *(db)* [**breaking**] add Copy-safe injected connection handles

## [0.4.0-alpha.1](https://github.com/kent8192/reinhardt-web/compare/reinhardt-websockets@v0.3.2...reinhardt-websockets@v0.4.0-alpha.1) - 2026-07-21

### Changed

- [**breaking**] remove remaining dynamic error dependencies

### Fixed

- *(release)* restore develop prerelease lifecycle

### Maintenance

- migrate dependency policy checks to cargo-deny
- merge develop/0.4.0 into retained effects
- merge develop/0.4.0 into fix/issue-5561-remove-anyhow
- merge develop/0.4.0 into remove-anyhow branch

## [0.3.2](https://github.com/kent8192/reinhardt-web/compare/reinhardt-websockets@v0.3.1...reinhardt-websockets@v0.3.2) - 2026-07-14

### Fixed

- *(ci)* allow intentional dependency-version duplicates

## [0.3.0](https://github.com/kent8192/reinhardt-web/compare/reinhardt-websockets@v0.2.0...reinhardt-websockets@v0.3.0) - 2026-06-28

Stable release of `reinhardt-websockets` for the Reinhardt 0.3.0 line. This
entry consolidates the 0.3.0 release-candidate series into one
stable release section.

### Migration Notes

- Review the root CHANGELOG and `instructions/MIGRATION_0.3.md` before upgrading from 0.2.x.

### Added

- feat!(di): introduce keyed injectable provider outputs

## [0.2.0](https://github.com/kent8192/reinhardt-web/compare/reinhardt-websockets@v0.1.3...reinhardt-websockets@v0.2.0) - 2026-06-11

Stable release of `reinhardt-websockets` for the Reinhardt 0.2.0 line. This
entry consolidates the 0.2.0 release-candidate series into one
stable release section.

### Migration Notes

- Review the breaking changes listed below before upgrading from 0.1.x.
- See the root CHANGELOG and `instructions/MIGRATION_0.2.md` for cross-crate migration guidance.

### Added

- *(websockets)* add settings fragments for connection, reconnection, origin, rate limit, and redis

### Changed

- [**breaking**] align develop/0.2.0 with main, preserving 8 feature crates

### Deprecated

- *(websockets)* deprecate ad-hoc XxxConfig structs in favor of settings fragments

### Fixed

- *(ci)* recover develop release-plz prerelease

### Maintenance

- *(websockets)* depend on reinhardt-conf for settings fragments

## [0.1.0](https://github.com/kent8192/reinhardt-web/compare/reinhardt-websockets@v0.1.0-rc.30...reinhardt-websockets@v0.1.0) - 2026-05-22

Initial stable release of `reinhardt-websockets` as part of the
reinhardt-web 0.1.0 release. Provides the framework's WebSocket
protocol surface — router, consumers, rooms, and the Redis-backed
channel layer — with the same DI, middleware, and URL-resolver
ergonomics as HTTP.

For the workspace-wide release narrative, see the [root CHANGELOG](https://github.com/kent8192/reinhardt-web/blob/main/CHANGELOG.md#010---2026-05-22).
Per-prerelease history is in the [Release Discussions](https://github.com/kent8192/reinhardt-web/discussions/categories/release).

### Capabilities at 0.1.0

- **Typed WebSocket router** — `WebSocketRouter::consumer()` builds
  consumer routes; `WebSocketEndpointInfo` and
  `WebSocketEndpointMetadata` (along with `substitute_ws_params`)
  give type-safe access to endpoint metadata, and a `reverse()`
  method mirrors HTTP URL reversal.
- **Unified mounting** — `WebSocketRoute`/`Router`/`EndpointInfo`
  live in reinhardt-core and mount through
  `UnifiedRouter::websocket()`, so HTTP and WebSocket routes share
  one resolver and one configuration model.
- **Hardened concurrency** — the rc cycle resolved an ABBA deadlock
  in `group_send`, released the connection slot on disconnect in
  `RateLimitMiddleware`, released locks before `Room::send_to`, and
  closed the registration-race in `get_or_init`.
- **Resilient connections** — auto-reconnect with exponential
  backoff, connection timeouts, graceful shutdown, complete
  state-machine match arms (`BinaryPayload`, `HeartbeatTimeout`,
  `SlowConsumer`), and partial-failure handling for room broadcasts.
- **Security defaults** — origin-header validation, configurable
  ping/pong keepalive intervals, default message-size limits,
  compression negotiation with size-bounded decompression,
  sanitized error messages, and authenticated Redis channel-layer
  connections.
- **Per-connection rate limiting** — a default rate limit is applied
  to WebSocket connections; per-route configuration is available via
  the consumer builder.

### Notable Breaking Changes

- **`Injected<T>` deprecated** ([#3631](https://github.com/kent8192/reinhardt-web/discussions/3631))
  — consumer parameter injection moves to `Depends<T>`.
- **Router move to reinhardt-core** — application code that imported
  `WebSocketRoute` / `WebSocketRouter` / `EndpointInfo` from this
  crate should re-import from `reinhardt::core` (re-exports remain
  in place for transitional builds).

### Migration Notes

See the [root CHANGELOG migration guide](https://github.com/kent8192/reinhardt-web/blob/main/CHANGELOG.md#migration-guide).
Mount WebSocket routes through `UnifiedRouter::websocket()`; existing
consumers continue to work, and `reverse()` lets you remove
hand-rolled URL templates from client code.
