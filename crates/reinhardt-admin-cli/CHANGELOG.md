# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.4.0-alpha.1](https://github.com/kent8192/reinhardt-web/compare/reinhardt-admin-cli@v0.3.2...reinhardt-admin-cli@v0.4.0-alpha.1) - 2026-07-21

### Changed

- *(pages)* migrate hooks to deps list syntax

### Fixed

- *(pages)* harden hook dependency migration
- *(pages)* harden dependency migration
- *(pages)* harden dependency migration edge cases
- *(pages)* harden dependency migration edge cases
- *(admin)* satisfy migration clippy lints
- *(admin)* preserve unresolved local hook calls
- *(admin)* preserve unresolved omitted hook calls
- *(pages)* repair hook dependency migration tests
- *(release)* restore develop prerelease lifecycle

### Maintenance

- migrate dependency policy checks to cargo-deny
- merge develop/0.4.0 into fix/issue-5561-remove-anyhow
- merge develop/0.4.0 into remove-anyhow branch
- merge develop/0.4.0 into anyhow removal branch

### Added

- *(formatter)* delegate component-scoped `style!` formatting to `reinhardt-formatter`

## [0.3.2](https://github.com/kent8192/reinhardt-web/compare/reinhardt-admin-cli@v0.3.1...reinhardt-admin-cli@v0.3.2) - 2026-07-14

### Fixed

- *(ci)* allow intentional dependency-version duplicates

## [0.3.1](https://github.com/kent8192/reinhardt-web/compare/reinhardt-admin-cli@v0.3.0...reinhardt-admin-cli@v0.3.1) - 2026-07-04

### Fixed

- *(formatter)* wrap long page closure parameters

## [0.3.0](https://github.com/kent8192/reinhardt-web/compare/reinhardt-admin-cli@v0.2.0...reinhardt-admin-cli@v0.3.0) - 2026-06-28

Stable release of `reinhardt-admin-cli` for the Reinhardt 0.3.0 line. This
entry consolidates the 0.3.0 release-candidate series into one
stable release section.

### Migration Notes

- Review the root CHANGELOG and `instructions/MIGRATION_0.3.md` before upgrading from 0.2.x.

### Documentation

- *(release)* expose 0.3 migration guide
- *(formatter)* document page rustfmt islands

## [0.2.0](https://github.com/kent8192/reinhardt-web/compare/reinhardt-admin-cli@v0.1.3...reinhardt-admin-cli@v0.2.0) - 2026-06-11

Stable release of `reinhardt-admin-cli` for the Reinhardt 0.2.0 line. This
entry consolidates the 0.2.0 release-candidate series into one
stable release section.

### Migration Notes

- Refresh generated project templates if the project vendors Reinhardt scaffold output.
- See [`instructions/MIGRATION_0.2.md`](../../instructions/MIGRATION_0.2.md) for the workspace migration checklist.

### Added

- *(commands)* add interactive dependency configuration
- `reinhardt-admin migrate-manouche-v2 [PATH]` subcommand applying the
  Manouche v1 → v2 codemod (spec §6.1 + §6.2). Available as `cargo make
  migrate-manouche-v2`. Supports `--dry-run` and `--skip <rule>`. Rules:
  `bare_ident`, `watch_unwrap`, `use_effect_deps`, `component_props`.

### Changed

- *(admin-cli)* replace the imperative AST formatter with a tree-sitter and Topiary pipeline for `page!`, `form!`, and `head!` DSL macros.

### Fixed

- *(admin-cli)* keep `migrate-manouche-v2` idempotent for already wrapped page expression slots.
- *(admin-cli)* preserve item attributes when `migrate-manouche-v2` rewrites attributed items.
- *(admin-cli)* avoid treating control-flow syntax variables as page children in `migrate-manouche-v2`.
- *(admin-cli)* keep `migrate-manouche-v2` from rewriting `match` patterns and `let` bindings as page children.
- *(admin-cli)* preserve inner module docs and migrate element bodies inside `let` initializers in `migrate-manouche-v2`.
- *(admin-cli)* restore Topiary formatter wiring from main
- *(admin-cli)* run rustfmt on DSL-skipped files in fmt command
- *(admin-cli)* pass ignore-all files through rustfmt in fmt command
- *(admin-cli)* update migrate_v2 expected fixtures to match prettyplease output
- *(admin-cli)* reject commented-out `#![rustfmt::skip]` in `rustfmt_skip_attr_matches`
- split formatter from admin cli

- *(commands)* align startproject scaffold defaults
- *(ci)* repair admin dependency config checks
- *(admin-cli)* revert rustfmt-damaged migrate_v2 fixtures
- *(admin-cli)* preserve migrate_v2 fixtures during fmt-all
- *(admin-cli)* skip nested workspaces in fmt-all

### Documentation

- *(release)* enforce public API doc coverage

### Styling

- apply rustfmt to non-DSL files on develop/0.2.0

### Maintenance

- forward merge main v0.1.2 changes into develop 0.2.0

## [0.1.1](https://github.com/kent8192/reinhardt-web/compare/reinhardt-admin-cli@v0.1.0...reinhardt-admin-cli@v0.1.1) - 2026-05-24
### Added

- `reinhardt-admin migrate-manouche-v2 [PATH]` subcommand applying the
  Manouche v1 → v2 codemod (spec §6.1 + §6.2). Available as `cargo make
  migrate-manouche-v2`. Supports `--dry-run` and `--skip <rule>`. Rules:
  `bare_ident`, `watch_unwrap`, `use_effect_deps`, `component_props`.

## [0.1.0](https://github.com/kent8192/reinhardt-web/compare/reinhardt-admin-cli@v0.1.0-rc.30...reinhardt-admin-cli@v0.1.0) - 2026-05-22

Initial stable release of `reinhardt-admin-cli` as part of the
reinhardt-web 0.1.0 release. This crate ships the `reinhardt-admin`
binary — the project's scaffolding, formatter, and plugin
manager — driven from a single `clap`-based entry point.

For the workspace-wide release narrative, see the [root CHANGELOG](https://github.com/kent8192/reinhardt-web/blob/main/CHANGELOG.md#010---2026-05-22).
Per-prerelease history is in the [Release Discussions](https://github.com/kent8192/reinhardt-web/discussions/categories/release).

### Capabilities at 0.1.0

- **Project and app scaffolding** — `startproject` generates a
  new Reinhardt project (matching the per-app layout from
  [#4476](https://github.com/kent8192/reinhardt-web/discussions/4476)), `startapp` adds an application module, and both
  accept `--template-dir` to override the embedded templates.
- **Interactive dependency selection** — `startproject` can prompt
  for the Reinhardt version and feature flags, and `configure`
  applies the same dependency selection to an existing project.
- **Template selection via `ArgGroup`** — `--template /
  --with-pages / --with-rest` replaces the previous
  `--template-type` flag; the selection is enforced by a `clap`
  `ArgGroup` so invalid combinations fail at parse time.
- **AST-based formatter** — `fmt` recursively walks nested
  `page!` macros, formats them through `prettyplease`, and
  hardens the parser against false-positive `page!`-shaped
  substrings inside strings and comments. Files without a
  `page!` macro are a no-op (not a skip). Writes are atomic
  with permission preservation and automatic backup cleanup.
- **Plugin manager** — `plugin install / remove / list / search /
  enable / disable / update / info` subcommands provide a single
  workflow for managing third-party extensions during the RC
  phase.
- **Secure defaults** — atomic file writes, recursion-depth guard
  on the AST formatter, sanitised error messages (no leaked
  paths), input validation on mutation paths, secret zeroing on
  drop via `zeroize`, and `.env` parsing with TOCTOU
  mitigations.

### Notable Breaking Changes

- **`--template-type` removed** — replaced by `--template /
  --with-pages / --with-rest` enforced via a `clap` `ArgGroup`.
  Scripts that pass `--template-type` must be updated to one of
  the new flags.
- **Per-app layout in `startproject`** ([#4476](https://github.com/kent8192/reinhardt-web/discussions/4476)) — scaffolding emits the new layout where per-app `server_fn` and client UI live under `apps/<app>/` instead of `commands/templates/...`.

### Migration Notes

- Replace `reinhardt-admin startproject --template-type <kind>`
  invocations with `--template <kind>` (or the matching
  `--with-pages` / `--with-rest` flag).
- Existing projects that pre-date [#4476](https://github.com/kent8192/reinhardt-web/discussions/4476) should
  relocate their per-app handlers manually; the scaffold emits
  the new layout automatically for newly created apps.
- For the workspace-wide migration narrative, see the [root CHANGELOG](https://github.com/kent8192/reinhardt-web/blob/main/CHANGELOG.md#010---2026-05-22).
