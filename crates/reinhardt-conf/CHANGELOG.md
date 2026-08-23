# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.4.0-alpha.7](https://github.com/kent8192/reinhardt-web/compare/reinhardt-conf@v0.4.0-alpha.6...reinhardt-conf@v0.4.0-alpha.7) - 2026-08-19

### Documentation

- update version references to v0.3.3
- update version references to v0.3.4
- update version references to v0.3.5
- update version references to v0.3.6
- update version references to v0.3.7
- update version references to v0.3.8

### Maintenance

- merge main into develop/0.4.0

## [0.4.0-alpha.6](https://github.com/kent8192/reinhardt-web/compare/reinhardt-conf@v0.4.0-alpha.5...reinhardt-conf@v0.4.0-alpha.6) - 2026-08-06

### Documentation

- *(release)* restore coherent alpha.3 references

## [0.4.0-alpha.4](https://github.com/kent8192/reinhardt-web/compare/reinhardt-conf@v0.4.0-alpha.3...reinhardt-conf@v0.4.0-alpha.4) - 2026-08-04

### Fixed

- *(migrations)* harden squash generation
- *(migrations)* cover squash review edge cases
- *(commands)* honor managed migration settings

### Other

- integrate develop migration updates

## [0.4.0-alpha.1](https://github.com/kent8192/reinhardt-web/compare/reinhardt-conf@v0.3.2...reinhardt-conf@v0.4.0-alpha.1) - 2026-07-21

### Changed

- [**breaking**] remove remaining dynamic error dependencies

### Fixed

- *(release)* restore develop prerelease lifecycle

### Maintenance

- merge develop/0.4.0 into remove-anyhow branch
## [0.3.10](https://github.com/kent8192/reinhardt-web/compare/reinhardt-conf@v0.3.9...reinhardt-conf@v0.3.10) - 2026-08-22

### Maintenance

- update Cargo.toml dependencies

## [0.3.9](https://github.com/kent8192/reinhardt-web/compare/reinhardt-conf@v0.3.8...reinhardt-conf@v0.3.9) - 2026-08-21

### Documentation

- *(security)* define runtime and operations boundaries
- *(security)* clarify encryption policy
- *(security)* qualify caller-enforced boundaries
- *(security)* qualify remaining boundary assumptions
- *(security)* qualify auth and vault boundaries
- *(security)* qualify remaining policy boundaries

### Fixed

- *(security)* qualify boundary control ownership

## [0.3.8](https://github.com/kent8192/reinhardt-web/compare/reinhardt-conf@v0.3.7...reinhardt-conf@v0.3.8) - 2026-08-16

### Maintenance

- update Cargo.toml dependencies

## [0.3.7](https://github.com/kent8192/reinhardt-web/compare/reinhardt-conf@v0.3.6...reinhardt-conf@v0.3.7) - 2026-08-12

### Maintenance

- update Cargo.toml dependencies

## [0.3.6](https://github.com/kent8192/reinhardt-web/compare/reinhardt-conf@v0.3.5...reinhardt-conf@v0.3.6) - 2026-08-04

### Maintenance

- update Cargo.toml dependencies

## [0.3.5](https://github.com/kent8192/reinhardt-web/compare/reinhardt-conf@v0.3.4...reinhardt-conf@v0.3.5) - 2026-08-02

### Maintenance

- update Cargo.toml dependencies

## [0.3.4](https://github.com/kent8192/reinhardt-web/compare/reinhardt-conf@v0.3.3...reinhardt-conf@v0.3.4) - 2026-07-30

### Maintenance

- update Cargo.toml dependencies

## [0.3.3](https://github.com/kent8192/reinhardt-web/compare/reinhardt-conf@v0.3.2...reinhardt-conf@v0.3.3) - 2026-07-28

### Maintenance

- update Cargo.toml dependencies

## [0.3.0](https://github.com/kent8192/reinhardt-web/compare/reinhardt-conf@v0.2.0...reinhardt-conf@v0.3.0) - 2026-06-28

Stable release of `reinhardt-conf` for the Reinhardt 0.3.0 line. This
entry consolidates the 0.3.0 release-candidate series into one
stable release section.

### Migration Notes

- Review the root CHANGELOG and `instructions/MIGRATION_0.3.md` before upgrading from 0.2.x.

### Fixed

- *(todo-check)* clear public api audit markers
- *(conf)* keep sectionless settings nodes embedded

### Maintenance

- migrate Rust toolchain to 1.96.0

## [0.2.0](https://github.com/kent8192/reinhardt-web/compare/reinhardt-conf@v0.1.3...reinhardt-conf@v0.2.0) - 2026-06-11

Stable release of `reinhardt-conf` for the Reinhardt 0.2.0 line. This
entry consolidates the 0.2.0 release-candidate series into one
stable release section.

### Migration Notes

- Replace legacy `Settings`, `AdvancedSettings`, `JsonFileSource`, and `auto_source` usage with composed settings fragments and `TomlFileSource`.
- See [`instructions/MIGRATION_0.2.md`](../../instructions/MIGRATION_0.2.md) for the workspace migration checklist.

### Breaking Changes

- *(conf)* [**breaking**] remove legacy advanced settings types

### Added

- *(settings)* add typed schema primitives
- *(settings)* expose composed schema roots
- Typed settings schema references now expose embedded settings nodes, including
  wrapper-aware paths for `Option`, `Vec`, `HashMap<String, _>`,
  `BTreeMap<String, _>`, `IndexMap<String, _>`, and `Box`.

### Changed

- *(conf)* delete deprecated Settings, AdvancedSettings, JsonFileSource, and related APIs
- [**breaking**] align develop/0.2.0 with main, preserving 8 feature crates

### Deprecated

- *(conf)* deprecate TemplateConfig in favor of TemplateSettings fragment

### Fixed

- *(settings)* preserve root section precedence
- *(settings)* classify embedded config nodes
- *(settings)* require explicit nested settings nodes
- *(settings)* keep schema accessor compatibility
- *(conf)* support secret source maps
- *(conf)* escape secret source test paths
- Required-field validation now reports missing nested required leaves as
  `BuildError::MissingRequiredPath` while preserving
  `BuildError::MissingRequiredField` for missing direct section fields.
- *(conf)* [**breaking**] remove legacy advanced settings types
- *(conf)* emit fragment self settings impls

- *(ci)* pin broken upstream transitive releases
- *(ci)* recover develop release-plz prerelease
- *(ci)* resolve all pre-existing compilation failures on develop/0.2.0

### Performance

- atomize facade dependency feature gates

### Documentation

- *(conf)* document embedded settings nodes
- *(settings)* document embedded schema nodes
- *(mail,conf)* fix unresolved intra-doc links to settings fragments

### Styling

- apply formatter fixes across workspace

## [0.1.0](https://github.com/kent8192/reinhardt-web/compare/reinhardt-conf@v0.1.0-rc.30...reinhardt-conf@v0.1.0) - 2026-05-22

Initial stable release of `reinhardt-conf` as part of the reinhardt-web
0.1.0 release. This crate is Reinhardt's Django-inspired settings
framework: it owns the `SettingsBuilder` layered configuration model,
the composable fragment system (`CoreSettings`, `SecuritySettings`,
`I18nSettings`, ...), the TOML / env source priority stack, and the
secrets / encryption primitives that protect sensitive values in
memory.

For the workspace-wide release narrative (Highlights, Breaking
Changes, Migration Guide), see the [root CHANGELOG](https://github.com/kent8192/reinhardt-web/blob/main/CHANGELOG.md#010---2026-05-22).
Per-prerelease history is preserved in the
[Release Discussions](https://github.com/kent8192/reinhardt-web/discussions/categories/release).

### Capabilities at 0.1.0

- **Composable settings fragments** — `SettingsFragment` is the
  trait every fragment implements, with a `type Accessor` projection
  and a `field_policies()` hook. The crate ships `CoreSettings` as
  the required base plus Django-compat fragments (`I18nSettings`,
  `TemplateSettings`, `ContactSettings`, `OpenApiSettings`, ...).
  `ComposedSettings`, `HasSettings<F>`, and `HasCommonSettings`
  expose composition to user code.
- **Layered `SettingsBuilder` with priority sources** — the
  builder merges `TomlFileSource`, `EnvFileSource`,
  `DefaultSource`, `HighPriorityEnvSource`, and user-supplied
  sources by priority. `MergeStrategy::Deep` is the default
  (`build_composed`), and per-test override sources let
  TestContainers integration tests inject configuration without
  touching real files.
- **Typed TOML interpolation** ([#4241](https://github.com/kent8192/reinhardt-web/discussions/4241),
  [#4229](https://github.com/kent8192/reinhardt-web/discussions/4229))
  — TOML strings support `${VAR}`, `${VAR:-default}`, `${VAR:-}`
  (explicit empty), and `${VAR:?message}` placeholders. The
  interpolator walks the full TOML AST (including strings nested in
  tables and arrays), and placeholders coerce to the destination
  field's type (e.g., `${REINHARDT_DB_PORT}` becomes a `u16`). Opt
  out of coercion with `SettingsBuilder::with_typed_coercion(false)`
  or disable interpolation entirely with `without_interpolation()`.
- **Field policies via `#[setting(...)]`** — the
  `FieldRequirement` / `FieldPolicy` types drive
  `BuildError::MissingRequiredField` and feed `build_composed()` so
  required-field errors surface at boot rather than as `None`
  values mid-request.
- **Secrets & encryption primitives** — `DatabaseUrl` redacts
  passwords in `Debug` output, the secrets module uses
  `ZeroizeOnDrop` with `ManuallyDrop` to preserve drop safety
  through `into_inner`, and credentials are URL-encoded when
  reassembled. Encryption-key exposure via CLI arguments is
  prevented, and hot-reload uses `tokio::sync::Mutex` to keep async
  reload paths free of `parking_lot` blocking.
- **Database-URL scheme validation as public API** — consumers
  (notably `reinhardt-apps::ApplicationBuilder::build`) reuse the
  same validator that the crate uses internally.

### Notable Breaking Changes

- **TOML interpolation is on by default** — `TomlFileSource::new(path)`
  now enables `${VAR}` interpolation; the previous opt-in behavior
  caused silent failures when a literal `${DB_PASSWORD}` landed in
  the merged tree. `with_interpolation()` is a no-arg explicitness
  marker; `without_interpolation()` opts out (issue #4224).
- **`set_interpolation(bool)` is deprecated** — use
  `with_interpolation()` / `without_interpolation()` instead; the
  boolean setter will be removed in 0.2.0.
- **`JsonFileSource` and `auto_source` are deprecated** ([#4120](https://github.com/kent8192/reinhardt-web/discussions/4120))
  — TOML is the canonical Reinhardt configuration format. Migrate
  `.json` configuration files to `.toml` or implement
  `ConfigSource` against `serde_json` to keep JSON support
  out-of-tree.
- **`Settings.installed_apps` is deprecated** — installed apps
  flow through the `reinhardt-apps` registry; the legacy
  `Settings`-level field remains as a serde-flattened bridge but
  emits a deprecation warning.
- **Built-in fragments extracted from `AdvancedSettings`** —
  `AdvancedSettings` is deprecated in favour of explicit fragments
  (`SecuritySettings`, `I18nSettings`, `TemplateSettings`,
  `ContactSettings`, `OpenApiSettings`, `CoreSettings`).

### Migration Notes

See the [root Migration Guide](https://github.com/kent8192/reinhardt-web/blob/main/CHANGELOG.md#migration-guide)
for project-wide guidance. Settings-specific moves:

- If you relied on literal `${...}` strings in TOML, append
  `.without_interpolation()` to your `TomlFileSource` constructor.
- Replace `JsonFileSource::new(...)` / `auto_source(...)` with
  `TomlFileSource::new(...)` against a `.toml` file.
- Migrate from `AdvancedSettings` to the matching fragments
  (`SecuritySettings`, `I18nSettings`, ...) and add an explicit
  `CoreSettings` fragment to every composed-settings declaration.
