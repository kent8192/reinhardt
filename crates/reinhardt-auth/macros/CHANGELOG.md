# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.4.0-alpha.7](https://github.com/kent8192/reinhardt-web/compare/reinhardt-auth-macros@v0.4.0-alpha.6...reinhardt-auth-macros@v0.4.0-alpha.7) - 2026-08-18

### Maintenance

- auto-fix fmt and clippy
- merge main into develop/0.4.0

### Styling

- *(auth)* format macro helper

### Testing

- *(auth)* cover guard proc macro

## [0.4.0-alpha.1](https://github.com/kent8192/reinhardt-web/compare/reinhardt-auth-macros@v0.3.2...reinhardt-auth-macros@v0.4.0-alpha.1) - 2026-07-21

### Fixed

- *(release)* restore develop prerelease lifecycle
## [0.3.8](https://github.com/kent8192/reinhardt-web/compare/reinhardt-auth-macros@v0.3.7...reinhardt-auth-macros@v0.3.8) - 2026-08-16

### Maintenance

- auto-fix fmt and clippy

### Styling

- *(auth)* format macro helper

### Testing

- *(auth)* cover guard proc macro

## [0.3.0](https://github.com/kent8192/reinhardt-web/compare/reinhardt-auth-macros@v0.2.0...reinhardt-auth-macros@v0.3.0) - 2026-06-28

Stable release of `reinhardt-auth-macros` for the Reinhardt 0.3.0 line. This
crate moves with the coordinated Reinhardt 0.3.0 release train.

### Migration Notes

- Review the root CHANGELOG and `instructions/MIGRATION_0.3.md` before upgrading from 0.2.x.

### Maintenance

- align crate release metadata with the Reinhardt 0.3.0 stable release train.

## [0.2.0](https://github.com/kent8192/reinhardt-web/compare/reinhardt-auth-macros@v0.1.3...reinhardt-auth-macros@v0.2.0) - 2026-06-11

Stable release of `reinhardt-auth-macros` for the Reinhardt 0.2.0 line. This
entry consolidates the 0.2.0 release-candidate series into one
stable release section.

### Migration Notes

- Regenerate auth models after moving to the final `CurrentUser<U>` and user trait contracts.
- See [`instructions/MIGRATION_0.2.md`](../../../instructions/MIGRATION_0.2.md) for the workspace migration checklist.

### Fixed

- *(ci)* recover develop release-plz prerelease

### Documentation

- *(release)* enforce public API doc coverage

## [0.1.0](https://github.com/kent8192/reinhardt-web/compare/reinhardt-auth-macros@v0.1.0-rc.30...reinhardt-auth-macros@v0.1.0) - 2026-05-22

Initial stable release of `reinhardt-auth-macros` as part of the
reinhardt-web 0.1.0 release. This crate hosts the proc macros that
back `reinhardt-auth` — primarily the `#[user]` attribute macro
and the `guard!()` permission-expression parser.

For the workspace-wide release narrative, see the [root CHANGELOG](https://github.com/kent8192/reinhardt-web/blob/main/CHANGELOG.md#010---2026-05-22).
Per-prerelease history is in the [Release Discussions](https://github.com/kent8192/reinhardt-web/discussions/categories/release).

### Capabilities at 0.1.0

- **`#[user(...)]` attribute** — annotates a `#[model]` type as
  the project's user identity. The macro emits a
  `BaseUserManager` impl, registers a `SuperuserCreator` via
  `inventory` for `#[user(full = true)]` types, and routes
  identity values through the canonical `reinhardt-auth`
  surface using a single-lock manager path.
- **`guard!()` proc macro** — a winnow-parsed mini-DSL that
  compiles permission expressions (`Public`, `All`, `Any`, `Not`,
  composition) into the `Guard<P>` runtime type from
  `reinhardt-auth`.
- **Test ergonomics** — opt-out attribute lets integration test
  fixtures supply their own user manager when the auto-emitted
  `BaseUserManager` is undesired (see [#3615](https://github.com/kent8192/reinhardt-web/discussions/3615)
  follow-up wiring).

### Notable Breaking Changes

- **`#[user(...)]` requires explicit `LABEL`** — `AppLabel`
  implementors targeted by `#[user]` must declare a `LABEL`
  constant; the macro no longer infers one. This pairs with the
  new `BaseUserManager` impl emission described above.
- **Auto-emitted `BaseUserManager`** — projects that previously
  hand-rolled `BaseUserManager` for a `#[user]` type either
  remove their manual impl or opt the type out of the
  auto-emitted impl in fixtures and integration tests.

### Migration Notes

- Add an explicit `LABEL` to each `AppLabel` impl reached by a
  `#[user]` annotation.
- For test fixtures that need a hand-rolled `BaseUserManager`,
  opt out of the auto-manager emission so the two impls do not
  collide at compile time.
- For the workspace-wide migration narrative, see the [root CHANGELOG](https://github.com/kent8192/reinhardt-web/blob/main/CHANGELOG.md#010---2026-05-22).
