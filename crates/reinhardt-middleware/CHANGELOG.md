# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.4.0-alpha.7](https://github.com/kent8192/reinhardt-web/compare/reinhardt-middleware@v0.4.0-alpha.6...reinhardt-middleware@v0.4.0-alpha.7) - 2026-08-19

### Documentation

- update version references to v0.3.7
- update version references to v0.3.8

### Fixed

- fix!(auth): require validated account state for sessions
- *(auth)* validate tutorial session accounts
- *(auth)* resolve tutorial sessions from shared store
- *(auth)* validate session accounts before authorization
- *(auth)* complete validated tutorial session setup

### Maintenance

- merge main into develop/0.4.0

### Reverted

- *(auth)* revert session authentication changes from [[#5905](https://github.com/kent8192/reinhardt-web/issues/5905)](https://github.com/kent8192/reinhardt-web/issues/5905)

## [0.4.0-alpha.6](https://github.com/kent8192/reinhardt-web/compare/reinhardt-middleware@v0.4.0-alpha.5...reinhardt-middleware@v0.4.0-alpha.6) - 2026-08-06

### Documentation

- *(release)* restore coherent alpha.3 references

## [0.4.0-alpha.1](https://github.com/kent8192/reinhardt-web/compare/reinhardt-middleware@v0.3.2...reinhardt-middleware@v0.4.0-alpha.1) - 2026-07-21

### Fixed

- *(release)* restore develop prerelease lifecycle

### Maintenance

- merge main into develop/0.4.0
## [0.3.9](https://github.com/kent8192/reinhardt-web/compare/reinhardt-middleware@v0.3.8...reinhardt-middleware@v0.3.9) - 2026-08-21

### Documentation

- *(security)* define request surface boundaries
- *(security)* qualify remaining policy boundaries
- *(security)* qualify remaining policy boundaries
- *(security)* document remaining trust boundaries
- *(security)* document secret and csrf boundaries
- *(security)* qualify remaining policy boundaries

### Fixed

- *(security)* qualify boundary control ownership
- *(security)* qualify boundary control ownership
- *(security)* qualify boundary control ownership

## [0.3.7](https://github.com/kent8192/reinhardt-web/compare/reinhardt-middleware@v0.3.6...reinhardt-middleware@v0.3.7) - 2026-08-12

### Fixed

- *(auth)* validate session accounts before authorization
- *(auth)* complete validated tutorial session setup

## [0.3.6](https://github.com/kent8192/reinhardt-web/compare/reinhardt-middleware@v0.3.5...reinhardt-middleware@v0.3.6) - 2026-08-04

### Fixed

- fix!(auth): require validated account state for sessions
- *(auth)* validate tutorial session accounts
- *(auth)* resolve tutorial sessions from shared store

### Reverted

- *(auth)* revert session authentication changes from [[#5905](https://github.com/kent8192/reinhardt-web/issues/5905)](https://github.com/kent8192/reinhardt-web/issues/5905)

## [0.3.2](https://github.com/kent8192/reinhardt-web/compare/reinhardt-middleware@v0.3.1...reinhardt-middleware@v0.3.2) - 2026-07-14

### Fixed

- *(auth)* preserve active status in request identities
- *(auth)* complete active identity propagation
- *(auth)* preserve active identity compatibility

## [0.3.0](https://github.com/kent8192/reinhardt-web/compare/reinhardt-middleware@v0.2.0...reinhardt-middleware@v0.3.0) - 2026-06-28

Stable release of `reinhardt-middleware` for the Reinhardt 0.3.0 line. This
entry consolidates the 0.3.0 release-candidate series into one
stable release section.

### Migration Notes

- Review the root CHANGELOG and `instructions/MIGRATION_0.3.md` before upgrading from 0.2.x.

### Added

- *(middleware)* populate AuthState from sessions
- feat!(di): introduce keyed injectable provider outputs

### Fixed

- *(middleware)* keep session cleanup active above threshold
- *(middleware)* amortize session cleanup
- *(ci)* pin brotli allocator dependency
- *(logging)* avoid zero-millisecond request durations

## [0.2.0](https://github.com/kent8192/reinhardt-web/compare/reinhardt-middleware@v0.1.3...reinhardt-middleware@v0.2.0) - 2026-06-11

Stable release of `reinhardt-middleware` for the Reinhardt 0.2.0 line. This
entry consolidates the 0.2.0 release-candidate series into one
stable release section.

### Migration Notes

- Replace `SessionStoreRef` injection with `Depends<SessionStore>`.
- See [`instructions/MIGRATION_0.2.md`](../../instructions/MIGRATION_0.2.md) for the workspace migration checklist.

### Breaking Changes

- *(auth)* [**breaking**] migrate internal consumers from removed User/SimpleUser types
- *(middleware)* [**breaking**] migrate session DI from Arc<SessionStore> to Depends<SessionStore>
- *(middleware)* [**breaking**] replace deprecated SecurityConfig with builder pattern on SecurityMiddleware
- [**breaking**] Migrate session DI resolution from `Arc<SessionStore>` to the
  canonical `Depends<SessionStore>` shape, aligning session middleware with the
  rest of reinhardt-di's `#[inject] Depends<T>` direction. See
  [#4437](https://github.com/kent8192/reinhardt-web/issues/4437).
  - `SessionMiddleware::di_registrations` now keys on `TypeId::of::<SessionStore>()`
    (previously `TypeId::of::<Arc<SessionStore>>()`), and stores the
    `Arc<SessionStore>` verbatim (no double-Arc envelope).
  - The `SessionStoreRef` newtype wrapper has been **removed**. Handlers that
    previously took `#[inject] store: SessionStoreRef` must now take
    `#[inject] store: Depends<SessionStore>`. Call sites for `SessionAuthExt`
    (`session.login(&store, …)`, `session.logout(&store)`) continue to work
    unchanged thanks to `Depends<T>: Deref<Target = T>` and Rust deref coercion.
  - `SessionAuthExt::login` / `SessionAuthExt::logout` now take `&SessionStore`
    directly instead of `&SessionStoreRef`.
  - **Migration**: replace `#[inject] store: SessionStoreRef` with
    `#[inject] store: Depends<SessionStore>` and update imports from
    `reinhardt::middleware::session::SessionStoreRef` to
    `reinhardt::middleware::session::SessionStore` + `reinhardt::di::Depends`.
    Manual DI setup that called `singleton.set::<Arc<SessionStore>>(store)`
    should call `singleton.set_arc(store)` instead.

### Added

- *(middleware)* [**breaking**] migrate session DI from Arc<SessionStore> to Depends<SessionStore>
- *(middleware)* bridge CorsConfig to CorsSettings fragment

### Changed

- *(conf)* delete deprecated Settings, AdvancedSettings, JsonFileSource, and related APIs
- *(middleware)* [**breaking**] replace deprecated SecurityConfig with builder pattern on SecurityMiddleware
- [**breaking**] Migrate session DI resolution from `Arc<SessionStore>` to the
  canonical `Depends<SessionStore>` shape, aligning session middleware with the
  rest of reinhardt-di's `#[inject] Depends<T>` direction. See
  [#4437](https://github.com/kent8192/reinhardt-web/issues/4437).
  - `SessionMiddleware::di_registrations` now keys on `TypeId::of::<SessionStore>()`
    (previously `TypeId::of::<Arc<SessionStore>>()`), and stores the
    `Arc<SessionStore>` verbatim (no double-Arc envelope).
  - The `SessionStoreRef` newtype wrapper has been **removed**. Handlers that
    previously took `#[inject] store: SessionStoreRef` must now take
    `#[inject] store: Depends<SessionStore>`. Call sites for `SessionAuthExt`
    (`session.login(&store, …)`, `session.logout(&store)`) continue to work
    unchanged thanks to `Depends<T>: Deref<Target = T>` and Rust deref coercion.
  - `SessionAuthExt::login` / `SessionAuthExt::logout` now take `&SessionStore`
    directly instead of `&SessionStoreRef`.
  - **Migration**: replace `#[inject] store: SessionStoreRef` with
    `#[inject] store: Depends<SessionStore>` and update imports from
    `reinhardt::middleware::session::SessionStoreRef` to
    `reinhardt::middleware::session::SessionStore` + `reinhardt::di::Depends`.
    Manual DI setup that called `singleton.set::<Arc<SessionStore>>(store)`
    should call `singleton.set_arc(store)` instead.
- [**breaking**] align develop/0.2.0 with main, preserving 8 feature crates

### Fixed

- stop implicit openapi schema macro output
- *(auth)* [**breaking**] migrate internal consumers from removed User/SimpleUser types
- *(docs)* resolve rustdoc intra-doc link errors on develop/0.2.0
- *(docs)* resolve additional rustdoc link errors for feature-gated types
- complete downstream shielding for deprecated config re-exports
- address CodeRabbit dependency gate review
- *(ci)* recover develop release-plz prerelease
- *(auth)* address CodeRabbit review feedback

### Performance

- atomize facade dependency feature gates
- trim standard facade feature dependencies

### Styling

- apply formatter fixes across workspace

### Maintenance

- *(examples)* remove examples-twitter

## [0.1.0](https://github.com/kent8192/reinhardt-web/compare/reinhardt-middleware@v0.1.0-rc.30...reinhardt-middleware@v0.1.0) - 2026-05-22

Initial stable release of `reinhardt-middleware` as part of the
reinhardt-web 0.1.0 release. Ships the canonical, batteries-included
HTTP middleware set: sessions, authentication, CORS, CSRF, security
headers, gzip, rate limiting, broken-link tracking, and more — every
component plugs into the request-scoped DI container via
`Middleware::di_registrations`.

For the workspace-wide release narrative, see the [root CHANGELOG](https://github.com/kent8192/reinhardt-web/blob/main/CHANGELOG.md#010---2026-05-22).
Per-prerelease history is in the [Release Discussions](https://github.com/kent8192/reinhardt-web/discussions/categories/release).

### Capabilities at 0.1.0

- **Pluggable session middleware** — `SessionMiddleware` auto-registers
  its `Arc<SessionStore>` through `di_registrations`, supports
  pluggable storage via the `AsyncSessionBackend` trait (with a
  Redis-backed `RedisSessionBackend` behind the `session-redis`
  feature), exposes typed `SessionValue<T>` / `OptionalSessionValue<T>`
  / `SessionValueNamed` / `OptionalSessionValueNamed<K, T>` extractors
  with Path-style auto-extraction, and propagates handler-side session
  ID rotation into `Set-Cookie`.
- **Authentication middleware family** — `JwtAuthMiddleware` for
  stateless tokens, `CookieSessionAuthMiddleware` for session cookies,
  `RemoteUserMiddleware` / `PersistentRemoteUserMiddleware` for proxy
  trust, and `LoginRequiredMiddleware` for unauthenticated redirect.
  JWT `Claims` carries `is_staff` / `is_superuser` so admin layers can
  authorize without an extra round-trip.
- **CSRF, CORS, CSP, and clickjacking protection** —
  `OriginGuardMiddleware` for CSRF, `CspMiddleware` with per-path
  `exempt_paths`, hardened `X-Frame-Options`, hardened CORS origin
  validation against the request per the Fetch Standard, and a `Vary`
  header injection when wildcard origins are combined with credentials.
  Handler-set CSP headers are respected. Cryptographic-random nonces
  and CSRF fallback secrets replace previous predictable values.
- **Operational middleware** — `gzip` encoding (asserted via typed
  handler tests), `HttpsRedirectMiddleware` with allowed-host
  validation, rate limiting with sliding-window statistics and stale
  bucket eviction, and `BrokenLinkConfig` driven from settings via
  `BrokenLinkConfig::from_settings` (no more env-var hot path).
- **Settings + DI integration** — Middleware reads configuration from
  the layered settings system rather than `env::var`, and the
  request-scoped session ID is injected into request extensions before
  the handler runs so downstream extractors see a consistent view.
- **UUIDv7 across security tokens** — Generation migrated from UUIDv4
  to UUIDv7 for ordered IDs; v4 is retained for security-sensitive
  tokens where unpredictability matters.

### Notable Breaking Changes

`SessionData` is `#[non_exhaustive]` to prevent source-breaking field
additions, and several middleware accessors gained DI-aware variants.
For the cross-crate view (including DI unification on `Depends<T>`)
see the [Breaking Changes Discussions](https://github.com/kent8192/reinhardt-web/discussions/categories/breaking-changes).

### Migration Notes

This is the first stable release, so there is no prior stable version
to migrate from. See the [root CHANGELOG](https://github.com/kent8192/reinhardt-web/blob/main/CHANGELOG.md#010---2026-05-22)
for the cross-crate migration guide.
