# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.4.0-alpha.7](https://github.com/kent8192/reinhardt-web/compare/reinhardt-auth@v0.4.0-alpha.6...reinhardt-auth@v0.4.0-alpha.7) - 2026-08-19

### Documentation

- update version references to v0.3.7
- update version references to v0.3.8

### Fixed

- *(auth)* validate session accounts before authorization

### Maintenance

- auto-fix fmt and clippy
- merge main into develop/0.4.0

### Security

- *(auth)* restore JWT secret wrapper
- *(auth)* preserve JWT secret field compatibility

### Styling

- *(auth)* format macro helper

### Testing

- *(settings)* move JWT secret contract integration
- *(auth)* cover guard proc macro

## [0.4.0-alpha.6](https://github.com/kent8192/reinhardt-web/compare/reinhardt-auth@v0.4.0-alpha.5...reinhardt-auth@v0.4.0-alpha.6) - 2026-08-06

### Documentation

- *(release)* restore coherent alpha.3 references

## [0.4.0-alpha.3](https://github.com/kent8192/reinhardt-web/compare/reinhardt-auth@v0.4.0-alpha.2...reinhardt-auth@v0.4.0-alpha.3) - 2026-07-27

### Fixed

- *(auth)* use injected connection for session reads

## [0.4.0-alpha.2](https://github.com/kent8192/reinhardt-web/compare/reinhardt-auth@v0.4.0-alpha.1...reinhardt-auth@v0.4.0-alpha.2) - 2026-07-23

### Added

- *(db)* [**breaking**] add Copy-safe injected connection handles

### Fixed

- *(db)* restore URL-based backend owner connection

## [0.4.0-alpha.1](https://github.com/kent8192/reinhardt-web/compare/reinhardt-auth@v0.3.2...reinhardt-auth@v0.4.0-alpha.1) - 2026-07-21

### Changed

- *(orm)* migrate callers to explicit ORM executors

### Fixed

- *(release)* restore develop prerelease lifecycle

### Maintenance

- merge main into develop/0.4.0
- merge anyhow removal into atomic transactions
## [0.3.9](https://github.com/kent8192/reinhardt-web/compare/reinhardt-auth@v0.3.8...reinhardt-auth@v0.3.9) - 2026-08-21

### Documentation

- *(security)* define data and identity boundaries
- *(security)* qualify remaining raw boundaries
- *(security)* qualify auth and vault boundaries
- *(security)* document secret and csrf boundaries

### Fixed

- *(security)* qualify boundary control ownership
- *(security)* qualify boundary control ownership

## [0.3.8](https://github.com/kent8192/reinhardt-web/compare/reinhardt-auth@v0.3.7...reinhardt-auth@v0.3.8) - 2026-08-16

### Maintenance

- auto-fix fmt and clippy

### Styling

- *(auth)* format macro helper

### Testing

- *(auth)* cover guard proc macro

## [0.3.7](https://github.com/kent8192/reinhardt-web/compare/reinhardt-auth@v0.3.6...reinhardt-auth@v0.3.7) - 2026-08-12

### Fixed

- *(auth)* validate session accounts before authorization

## [0.3.6](https://github.com/kent8192/reinhardt-web/compare/reinhardt-auth@v0.3.5...reinhardt-auth@v0.3.6) - 2026-08-04

### Security

- *(auth)* restore JWT secret wrapper
- *(auth)* preserve JWT secret field compatibility

### Testing

- *(settings)* move JWT secret contract integration

### Security

- *(auth)* preserve the public JWT secret string API while classifying and redacting it as a secret

## [0.3.3](https://github.com/kent8192/reinhardt-web/compare/reinhardt-auth@v0.3.2...reinhardt-auth@v0.3.3) - 2026-07-28

### Fixed

- *(auth)* use injected connection for session reads
- *(auth)* use injected connection for session cleanup reads
- *(auth)* count prefixed sessions in database

### Testing

- *(auth)* cover injected session connections in integration

## [0.3.2](https://github.com/kent8192/reinhardt-web/compare/reinhardt-auth@v0.3.1...reinhardt-auth@v0.3.2) - 2026-07-14

### Fixed

- *(auth)* constrain generic OIDC discovery endpoints
- *(auth)* reject generic OIDC endpoint redirects
- *(auth)* redact jwt settings secrets

## [0.3.0](https://github.com/kent8192/reinhardt-web/compare/reinhardt-auth@v0.2.0...reinhardt-auth@v0.3.0) - 2026-06-28

Stable release of `reinhardt-auth` for the Reinhardt 0.3.0 line. This
entry consolidates the 0.3.0 release-candidate series into one
stable release section.

### Migration Notes

- Review the root CHANGELOG and `instructions/MIGRATION_0.3.md` before upgrading from 0.2.x.

### Changed

- [**breaking**] remove 0.3 deprecated public APIs

### Removed

- **BREAKING**: Removed the 0.2 compatibility extractor `AuthUser<U>`.
  Use `CurrentUser<U>` for full authenticated-user extraction.

### Fixed

- *(ci)* pin brotli allocator dependency

## [0.2.0](https://github.com/kent8192/reinhardt-web/compare/reinhardt-auth@v0.1.3...reinhardt-auth@v0.2.0) - 2026-06-11

Stable release of `reinhardt-auth` for the Reinhardt 0.2.0 line. This
entry consolidates the 0.2.0 release-candidate series into one
stable release section.

### Migration Notes

- Replace old auth user traits and fixture types with `AuthIdentity`, `BaseUser` / `FullUser`, `PermissionsMixin`, and application-owned `#[user]` models.
- Use `CurrentUser<U>` as the canonical extractor; `AuthUser<U>` is only a deprecated compatibility wrapper.
- See [`instructions/MIGRATION_0.2.md`](../../instructions/MIGRATION_0.2.md) for the workspace migration checklist.

### Breaking Changes

- *(auth)* [**breaking**] migrate internal consumers from removed User/SimpleUser types
- *(auth)* [**breaking**] remove RC-deprecated CurrentUser, DefaultUser, and User trait (refs [[#4520](https://github.com/kent8192/reinhardt-web/issues/4520)](https://github.com/kent8192/reinhardt-web/issues/4520), closes [[#4652](https://github.com/kent8192/reinhardt-web/issues/4652)](https://github.com/kent8192/reinhardt-web/issues/4652))
- *(db,macros)* [**breaking**] unify custom managers with Model::objects() ([[#3984](https://github.com/kent8192/reinhardt-web/issues/3984)](https://github.com/kent8192/reinhardt-web/issues/3984))
- *(model)* [**breaking**] make new an alias for build

### Added

- *(auth)* [**breaking**] remove RC-deprecated CurrentUser, DefaultUser, and User trait (refs [[#4520](https://github.com/kent8192/reinhardt-web/issues/4520)](https://github.com/kent8192/reinhardt-web/issues/4520), closes [[#4652](https://github.com/kent8192/reinhardt-web/issues/4652)](https://github.com/kent8192/reinhardt-web/issues/4652))
- *(db,macros)* [**breaking**] unify custom managers with Model::objects() ([[#3984](https://github.com/kent8192/reinhardt-web/issues/3984)](https://github.com/kent8192/reinhardt-web/issues/3984))
- *(model)* [**breaking**] make new an alias for build
- *(auth)* add settings fragments for session, jwt, token rotation

### Changed

- *(auth)* make CurrentUser canonical extractor
- [**breaking**] align develop/0.2.0 with main, preserving 8 feature crates

### Deprecated

- *(auth)* deprecate SessionConfig, JwtConfig, TokenRotationConfig

### Removed

- **`CurrentUser<U>` struct** (`src/current_user.rs`, deprecated
  `0.1.0-rc.12`) — entire module removed. Use the canonical
  `AuthUser<U>` extractor (`src/auth_user.rs`) directly. Closes
  Issue #4652.

  Note: `CurrentUser` could **not** be retained as a type alias
  (the original plan in #4652) because its on-the-wire shape
  (`Option<U>` + `Option<Uuid>`) differs from `AuthUser`'s
  tuple-struct shape — a type alias would break pattern-matching
  call sites. Migration is therefore a struct-replacement rather
  than a no-op alias.
- **`DefaultUser` struct** (`src/default_user.rs`, deprecated
  `0.1.0-rc.15`) — entire module removed. Define your own user type
  with the `#[user]` attribute macro.
- **`User` trait + `SimpleUser` + `AnonymousUser`** (`src/core/user.rs`,
  deprecated `0.1.0-rc.15`) — entire module removed. Use
  `AuthIdentity` + `BaseUser` / `FullUser` + `PermissionsMixin`
  instead.

#### BREAKING CHANGES

Removed all 3 RC-deprecated APIs from `reinhardt-auth` per
STABILITY_POLICY § SP-4 (umbrella Issue
[#4520](https://github.com/kent8192/reinhardt-web/issues/4520)) plus
closed companion Issue
[#4652](https://github.com/kent8192/reinhardt-web/issues/4652).

- **`CurrentUser<U>` struct** (`src/current_user.rs`, deprecated
  `0.1.0-rc.12`) — entire module removed. Use the canonical
  `AuthUser<U>` extractor (`src/auth_user.rs`) directly. Closes
  Issue #4652.

  Note: `CurrentUser` could **not** be retained as a type alias
  (the original plan in #4652) because its on-the-wire shape
  (`Option<U>` + `Option<Uuid>`) differs from `AuthUser`'s
  tuple-struct shape — a type alias would break pattern-matching
  call sites. Migration is therefore a struct-replacement rather
  than a no-op alias.

- **`DefaultUser` struct** (`src/default_user.rs`, deprecated
  `0.1.0-rc.15`) — entire module removed. Define your own user type
  with the `#[user]` attribute macro.

- **`User` trait + `SimpleUser` + `AnonymousUser`** (`src/core/user.rs`,
  deprecated `0.1.0-rc.15`) — entire module removed. Use
  `AuthIdentity` + `BaseUser` / `FullUser` + `PermissionsMixin`
  instead.

### Fixed

- stop implicit openapi schema macro output
- *(auth)* [**breaking**] migrate internal consumers from removed User/SimpleUser types
- *(auth)* replace InternalUser in UserManager public API with ManagedUser
- *(macros)* suppress missing_docs on generated Info companion types
- address CodeRabbit dependency gate review
- *(ci)* recover develop release-plz prerelease
- *(auth)* address CodeRabbit review feedback
- *(auth,urls,pages)* remove stale references and fix latent clippy lints
- *(ci)* update test snapshots and assertions for v0.2.0 breaking changes

### Performance

- atomize facade dependency feature gates

### Documentation

- *(release)* enforce public API doc coverage
- *(auth)* update core.rs and lib.rs doc references for removed types
- *(di,auth)* fix rustdoc link warnings on nightly

### Styling

- apply formatter fixes across workspace

### Maintenance

- *(auth)* add reinhardt-conf dependency for settings fragments

### Testing

- *(auth)* remove time-based permission clock flake

### Known consumer migration follow-up

This PR removes the symbols from `reinhardt-auth` itself. Workspace
consumers that still reference the removed types — including
`reinhardt-middleware`, `reinhardt-rest`, `reinhardt-http`,
`reinhardt-views`, the `examples-tutorial-basis` app (per #4652's
companion-PR section), and the workspace facade `reinhardt-web` — will
need a coordinated migration in a follow-up PR. CI on this PR is
expected to fail compilation on those crates until the follow-up lands.

See [`instructions/MIGRATION_0.2.md`](../../instructions/MIGRATION_0.2.md#reinhardt-auth)
for the migration guide.

## [0.1.0](https://github.com/kent8192/reinhardt-web/compare/reinhardt-auth@v0.1.0-rc.30...reinhardt-auth@v0.1.0) - 2026-05-22

Initial stable release of `reinhardt-auth` as part of the
reinhardt-web 0.1.0 release. This crate ships the
authentication / authorization surface — JWT, cookie sessions,
OAuth2 / OIDC, pluggable user managers, and Django-style
permission guards — that powers both `reinhardt-admin` and
end-user apps.

For the workspace-wide release narrative, see the [root CHANGELOG](https://github.com/kent8192/reinhardt-web/blob/main/CHANGELOG.md#010---2026-05-22).
Per-prerelease history is in the [Release Discussions](https://github.com/kent8192/reinhardt-web/discussions/categories/release).

### Capabilities at 0.1.0

- **JWT, sessions, and storage backends** — `JwtSessionBackend`
  with a `JwtError` enum that rejects expired tokens by default,
  cookie session backend with HMAC signed via the server secret,
  `CacheSessionBackend`, a database token storage with an O(1)
  SHA-256 digest index, and pluggable session storage
  (file / cookie / `redis-sessions`).
- **OAuth2 and OIDC** — `GenericOidcProvider` for arbitrary OIDC
  IdPs (in addition to GitHub / Google / etc.), HTTPS enforced
  on every OAuth2 / OIDC endpoint URL, `client_id` validated
  against the authorization code on exchange, and a GitHub
  `/user` transform that normalises responses into
  `StandardClaims`.
- **User and superuser management** — `AuthIdentity` trait
  (replaces the deprecated `User` trait), `Group` and
  `AuthPermission` ORM models, `GroupManager` integrated with
  `PermissionsMixin`, `SuperuserInit` trait + `SuperuserCreator`
  registry with `inventory` auto-registration for
  `#[user(full = true)]` + `#[model]` types.
- **Extractors and DI** — `AuthInfo` lightweight extractor,
  `AuthUser<U>` tuple-struct extractor, `CurrentUser` DI binding,
  and a `validate_auth_extractors` startup validation pass that
  fails closed when a route declares `AuthUser<U>` without a
  matching DI binding.
- **Permission guards** — `Guard<P>` runtime type with `Public`,
  `All`, `Any`, `Not` combinators, plus a `guard!()` proc macro
  whose winnow-based parser compiles permission expressions at
  attribute-expansion time.
- **Hardened defaults** — argon2 password hashing, constant-time
  comparison everywhere a token meets a secret, session
  rotation on login to defeat session fixation, and TOTP
  algorithm / proxy trust hardening for SSO flows.

### Notable Breaking Changes

- **OAuth2 `exchange_code` redirect URI** ([#3609](https://github.com/kent8192/reinhardt-web/discussions/3609)) — `exchange_code()` now requires the callback URL as its fourth argument, so the IdP-side `redirect_uri` is verified server-side.
- **`User` trait deprecated** in favour of `AuthIdentity`; `DefaultUser` carries the same deprecation. Update extractors and ORM bindings to the new trait.
- **JWT default rejects expired tokens** — the `JwtError` enum surfaces `Expired` distinctly from `Invalid`; callers that previously swallowed expiry must update their error matching.
- **`#[user(...)]` macro contract change** — the macro now emits a `BaseUserManager` impl and requires an explicit `LABEL` on the `AppLabel` implementor; fixtures and integration tests that need to supply their own manager must opt out via the macro attribute.
- **`Mutex` migration** — internal `std::Mutex` replaced with `tokio::Mutex` to prevent async deadlocks; downstream code that held the lock across an `.await` should re-verify boundaries.

### Migration Notes

- Pass the `redirect_uri` as the fourth argument to every
  `OAuth2Provider::exchange_code` call (covered in
  [#3609](https://github.com/kent8192/reinhardt-web/discussions/3609)).
- Migrate `#[user]`-annotated types to declare an explicit
  `LABEL` constant on their `AppLabel` impl; integration test
  fixtures that previously relied on a manual manager should
  opt out of the auto-emitted `BaseUserManager` impl.
- Replace `Injected<T>` / `OptionalInjected<T>` in auth wiring
  with `Depends<T>` / `Option<Depends<T>>` per [#3631](https://github.com/kent8192/reinhardt-web/discussions/3631).
- For the workspace-wide migration narrative, see the [root CHANGELOG](https://github.com/kent8192/reinhardt-web/blob/main/CHANGELOG.md#010---2026-05-22).
