# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.3.7](https://github.com/kent8192/reinhardt-web/compare/reinhardt-manouche@v0.3.6...reinhardt-manouche@v0.3.7) - 2026-08-07

### Maintenance

- update Cargo.toml dependencies

## [0.3.6](https://github.com/kent8192/reinhardt-web/compare/reinhardt-manouche@v0.3.5...reinhardt-manouche@v0.3.6) - 2026-08-04

### Maintenance

- update Cargo.toml dependencies

## [0.3.5](https://github.com/kent8192/reinhardt-web/compare/reinhardt-manouche@v0.3.4...reinhardt-manouche@v0.3.5) - 2026-08-02

### Maintenance

- update Cargo.toml dependencies

## [0.3.4](https://github.com/kent8192/reinhardt-web/compare/reinhardt-manouche@v0.3.3...reinhardt-manouche@v0.3.4) - 2026-07-30

### Maintenance

- update Cargo.toml dependencies

## [0.3.3](https://github.com/kent8192/reinhardt-web/compare/reinhardt-manouche@v0.3.2...reinhardt-manouche@v0.3.3) - 2026-07-28

### Maintenance

- update Cargo.toml dependencies

## [0.3.0](https://github.com/kent8192/reinhardt-web/compare/reinhardt-manouche@v0.2.0...reinhardt-manouche@v0.3.0) - 2026-06-28

Stable release of `reinhardt-manouche` for the Reinhardt 0.3.0 line. This
entry consolidates the 0.3.0 release-candidate series into one
stable release section.

### Migration Notes

- Review the root CHANGELOG and `instructions/MIGRATION_0.3.md` before upgrading from 0.2.x.

### Added

- *(forms)* expand generated widget coverage

### Fixed

- *(forms)* align generated widget contracts

## [0.2.0](https://github.com/kent8192/reinhardt-web/compare/reinhardt-manouche@v0.1.3...reinhardt-manouche@v0.2.0) - 2026-06-11

Stable release of `reinhardt-manouche` for the Reinhardt 0.2.0 line. This
entry consolidates the 0.2.0 release-candidate series into one
stable release section.

### Added

- *(pages)* support keyed page list rendering
- *(forms)* add typed use_form ergonomics
- feat!(forms): route use_form through form definitions

### Fixed

- *(auth)* replace InternalUser in UserManager public API with ManagedUser
- *(docs)* resolve remaining rustdoc doctest failures
- *(forms)* stabilize form runtime and validator parity
- *(ci)* recover develop release-plz prerelease
- *(ci)* resolve all pre-existing compilation failures on develop/0.2.0
- *(forms)* address review and CI failures
- *(forms)* address review feedback
- *(forms)* address bot review feedback

### Performance

- *(pages)* trim wasm dependency graph

### Styling

- apply formatter fixes across workspace

### Maintenance

- forward merge main v0.1.1 changes into develop 0.2.0

## [0.1.0](https://github.com/kent8192/reinhardt-web/compare/reinhardt-manouche@v0.1.0-rc.30...reinhardt-manouche@v0.1.0) - 2026-05-22

Initial stable release of `reinhardt-manouche` as part of the
reinhardt-web 0.1.0 release. Provides the AST, parser, validator, and
codegen for the Manouche DSL that powers the `page!` and `form!`
procedural macros in `reinhardt-pages`.

For the workspace-wide release narrative, see the [root CHANGELOG](https://github.com/kent8192/reinhardt-web/blob/main/CHANGELOG.md#010---2026-05-22).
Per-prerelease history is in the [Release Discussions](https://github.com/kent8192/reinhardt-web/discussions/categories/release).

### Capabilities at 0.1.0

- **Unified page / form / head DSL** — A single parser stack handles
  `page!`, `form!`, and `<head>` content with shared AST types,
  reactive trait definitions, and consistent diagnostic spans driven
  by `syn` and `darling`.
- **Typed form fields with validator scope** — Form fields declare
  client/server scope via `ValidatorScope` and `ClientTrigger`. Typed
  validator rules carry scope information so codegen can place
  each validator on the appropriate side of the network boundary.
- **First-class navigation and submit ergonomics** — `success_url`
  for post-submit navigation, `on_success` / `on_success_ref` callback
  forms, `strip_arguments`, `autocomplete`, and `SubmitButton` are
  parsed and lowered without per-field boilerplate.
- **Compile-time safety hardening** — `js_condition` expressions are
  validated at compile time to prevent injection, `<head>` element
  attribute extraction is validated for safe rendering, and the page
  parser uses `assert!` (not `debug_assert!`) so contracts hold in
  release builds too.

### Notable Breaking Changes

- **`manouche` IR / `IRVisitor` removed** ([#3900](https://github.com/kent8192/reinhardt-web/discussions/3900))
  — the unused intermediate-representation layer and its visitor
  scaffold were removed. Codegen now lowers directly from AST to
  generated tokens. External consumers of the IR types must migrate to
  the AST surface.

Workspace-level breaking changes are tracked at the
[Breaking Changes Discussions](https://github.com/kent8192/reinhardt-web/discussions/categories/breaking-changes)
and summarized in the [root CHANGELOG](https://github.com/kent8192/reinhardt-web/blob/main/CHANGELOG.md#010---2026-05-22).

### Migration Notes

See the workspace-level [Migration Guide](https://github.com/kent8192/reinhardt-web/blob/main/CHANGELOG.md#010---2026-05-22)
for the full upgrade flow. Crate-specific notes:

- Replace any direct dependency on `manouche`'s IR types or
  `IRVisitor` trait with the AST node types ([#3900](https://github.com/kent8192/reinhardt-web/discussions/3900));
  most downstream users only ever consumed the procedural macros and
  are unaffected.
- Form-level client validators that previously emitted JavaScript are
  rejected at parse time with a migration error. Move them to
  scope-annotated typed validator rules.
