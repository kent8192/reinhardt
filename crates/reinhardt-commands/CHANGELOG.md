# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.4.0-alpha.3](https://github.com/kent8192/reinhardt-web/compare/reinhardt-commands@v0.4.0-alpha.2...reinhardt-commands@v0.4.0-alpha.3) - 2026-07-27

### Fixed

- *(commands)* resolve static manifest aliases
- *(staticfiles)* harden manifest alias handling

## [0.4.0-alpha.2](https://github.com/kent8192/reinhardt-web/compare/reinhardt-commands@v0.4.0-alpha.1...reinhardt-commands@v0.4.0-alpha.2) - 2026-07-23

### Fixed

- *(i18n)* honor registered app catalogs
- *(i18n)* complete registered catalog workflow
- *(i18n)* preserve registered catalog domains
- *(i18n)* preserve structured PO entries
- *(i18n)* skip empty extracted messages

## [0.4.0-alpha.1](https://github.com/kent8192/reinhardt-web/compare/reinhardt-commands@v0.3.2...reinhardt-commands@v0.4.0-alpha.1) - 2026-07-21

### Added

- feat!(pages): require named component route names
- feat!(pages): resolve SSR resources asynchronously
- *(commands)* dispatch static page edits as HMR patches

### Fixed

- *(pages)* harden async SSR resource rendering
- *(docs)* remove redundant command links
- *(commands)* harden component style delivery
- *(commands)* harden component style delivery
- *(commands)* resolve target-aware style extraction
- *(commands)* retain metadata-refresh style context
- *(commands)* align style extraction with build cfgs
- *(commands)* align style extraction with Cargo build cfgs
- *(commands)* follow active component style sources
- *(commands)* allow styleless standalone runserver
- *(commands)* align component styles with wasm builds
- *(commands)* align selected Pages package rebuilds
- *(commands)* honor selected Pages package context
- *(commands)* preserve hot reload build context
- *(commands)* keep Pages rebuild artifacts in sync
- *(commands)* use library target names for Pages WASM
- *(tests)* align style delivery CI expectations
- harden component style delivery
- expose dependency-aware wasm freshness check
- compile dependency-aware wasm freshness check
- *(styles)* validate font and style extraction edges
- *(style)* validate generated component CSS
- *(style)* preserve pages style runtime boundaries
- *(commands)* preserve component style reload safety
- *(styles)* align generated and extracted scopes
- *(commands)* honor custom Pages static directories
- *(commands)* preserve component style lifecycle
- *(styles)* address component style review comments
- *(commands)* isolate Pages package selection
- *(commands)* fingerprint variable constraints
- *(commands)* use metadata for Pages cdylib targets
- *(manouche)* close PR 5641 media and extraction review gaps
- *(commands)* restore phase 1 CI gates
- *(release)* restore develop prerelease lifecycle

### Maintenance

- merge latest develop changes into typed JSON PR
- merge develop/0.4.0 into component style branch
- merge develop/0.4.0 into component-style branch
- auto-fix fmt and clippy

### Other

- resolve develop/0.4.0 into model enum fields
- sync develop/0.4.0 into page template hot patching

### Added

- *(pages)* compile component styles for collectstatic and development serving with stable CSS-only refresh URLs

### Removed

- *(pages)* remove the obsolete whole-root static HTML hot-patch parser and helper

### Fixed

- *(pages)* keep generated component styles aligned with compiled Cargo sources, configured static URLs, and successful rebuilds.
- *(collectstatic)* register generated assets before template rendering and validate every static source before clearing output.
- *(commands)* surface invalid migration rename destinations during generation

## [0.3.2](https://github.com/kent8192/reinhardt-web/compare/reinhardt-commands@v0.3.1...reinhardt-commands@v0.3.2) - 2026-07-14

### Fixed

- *(commands)* sort migrate plans by dependencies
- *(commands)* remove redundant rustdoc link target

## [0.3.0](https://github.com/kent8192/reinhardt-web/compare/reinhardt-commands@v0.2.0...reinhardt-commands@v0.3.0) - 2026-06-28

Stable release of `reinhardt-commands` for the Reinhardt 0.3.0 line. This
entry consolidates the 0.3.0 release-candidate series into one
stable release section.

### Migration Notes

- Review the root CHANGELOG and `instructions/MIGRATION_0.3.md` before upgrading from 0.2.x.

### Added

- *(urls)* [**breaking**] remove raw server route registration APIs

### Changed

- *(scaffolding)* align Pages app layout

### Fixed

- *(commands)* redact sqlite database paths in logs
- *(scaffolding)* default pages projects to sqlite
- *(commands)* repair pages quickstart scaffold defaults
- *(scaffolding)* generate target-neutral Pages apps
- *(scaffolding)* split generated Pages routers

### Documentation

- *(tutorial)* align pages scaffolding route gates

### Maintenance

- merge main into develop/0.3.0

## [0.2.0](https://github.com/kent8192/reinhardt-web/compare/reinhardt-commands@v0.1.3...reinhardt-commands@v0.2.0) - 2026-06-11

Stable release of `reinhardt-commands` for the Reinhardt 0.2.0 line. This
entry consolidates the 0.2.0 release-candidate series into one
stable release section.

### Migration Notes

- Regenerate or review project templates so dependency configuration, local infra state, and wasm-bindgen wiring match 0.2.0.
- See [`instructions/MIGRATION_0.2.md`](../../instructions/MIGRATION_0.2.md) for the workspace migration checklist.

### Breaking Changes

- *(commands)* [**breaking**] remove workspace templates and consolidate onto non-workspace templates

### Added

- *(commands)* add interactive dependency configuration
- *(commands)* add migrate-with-target direction detection
- *(commands)* pass get_settings() from generated manage.rs templates

### Changed

- *(auth)* make CurrentUser canonical extractor
- *(commands)* [**breaking**] remove workspace templates and consolidate onto non-workspace templates
- *(commands)* replace loose contains() assertions with exact-line checks
- *(commands)* simplify assert_eq!(expr, bool) to assert!(expr)

### Fixed

- *(commands)* adapt hot reload tests for develop
- *(auth)* replace InternalUser in UserManager public API with ManagedUser
- *(commands)* use project_crate_name for with_nav import in workspace pages template
- *(commands)* add parent project crate dependency to workspace app Cargo.toml
- *(commands)* compile migrate-with-target target handling
- *(commands)* honor --fake and --plan on migrate-with-target paths
- *(commands)* fail fast on recorder errors in migrate --plan
- *(commands)* resolve management-command database URL from project settings
- *(commands)* read [core.databases.default] in the settings disk loader
- *(commands)* update pages template CDN
- *(commands)* align wasm bindgen template
- *(commands)* verify runserver reachability after hot reload

- *(commands)* add pages app reverse template
- *(commands)* align startproject scaffold defaults
- *(commands)* use collectstatic no-input in pages template
- *(commands)* make generated model placeholders tutorial-safe
- *(commands)* ignore sqlite database in project templates
- *(ci)* repair admin dependency config checks
- *(build)* address CodeRabbit review feedback
- *(build)* port Codex review follow-ups
- address CodeRabbit review comments
- address follow-up CodeRabbit comments
- *(ci)* recover develop release-plz prerelease
- *(ci)* update WASM consumer fixture for URL routing simplification

### Performance

- *(commands)* skip unrelated hot reload rebuilds
- *(commands)* notify browsers after hot reload rebuilds
- *(commands)* use staleness check for pages wasm reuse
- *(pages)* hot patch static page edits
- *(build)* tune dev profile for hot reload

### Documentation

- align CLI install version examples
- *(release)* enforce public API doc coverage
- *(commands)* document migrate-with-target semantics
- *(commands)* clarify APP_LABEL/MIGRATION_NAME dependency
- *(commands)* make execute_from_command_line_with_settings doc example compile

- *(tutorial)* aggregate app URL routers

### Styling

- apply formatter fixes across workspace

### Maintenance

- *(commands)* ignore local infra state in templates
- forward merge main v0.1.1 changes into develop 0.2.0

### Testing

- *(commands)* verify hmr reload after rebuild
- *(commands)* drop stale InstalledApp import assertions in e2e_pages
- *(commands)* add migrate-with-target E2E coverage
- *(commands)* cover migrate --migrations-dir flag parsing
- *(commands)* drop needless #[allow(unreachable_patterns)] in migrate parse test
- *(commands)* cover settings-based database URL resolution
- *(ci)* refresh release CI expectations

### Other

- resolve conflicts with develop/0.2.0

## [0.1.0](https://github.com/kent8192/reinhardt-web/compare/reinhardt-commands@v0.1.0-rc.30...reinhardt-commands@v0.1.0) - 2026-05-22

Initial stable release of `reinhardt-commands` as part of the
reinhardt-web 0.1.0 release. This crate is Reinhardt's Django-style
management command framework: it ships the built-in commands
(`runserver`, `migrate`, `makemigrations`, `collectstatic`,
`createsuperuser`, `startproject`, `startapp`, `check`, `introspect`),
the `TemplateSource` trait that backs scaffolding, the hot-reload
WASM / server rebuild pipelines, and the per-app file templates that
`reinhardt new` emits.

For the workspace-wide release narrative (Highlights, Breaking
Changes, Migration Guide), see the [root CHANGELOG](https://github.com/kent8192/reinhardt-web/blob/main/CHANGELOG.md#010---2026-05-22).
Per-prerelease history is preserved in the
[Release Discussions](https://github.com/kent8192/reinhardt-web/discussions/categories/release).

### Capabilities at 0.1.0

- **`runserver` with built-in hot reload** — watches the workspace
  via `notify` and runs `WasmRebuildPipeline` + `ServerRebuildPipeline`
  in parallel on file change, restarting the server only after both
  artefacts are produced. Pipeline failures do not terminate the
  watcher: a fresh save retriggers the failed pipeline ([#4128](https://github.com/kent8192/reinhardt-web/issues/4128)).
  `--no-wasm-rebuild` opts out of the in-process WASM rebuild;
  `--no-override-wasm` reuses existing `dist/` artefacts when up to
  date ([#4205](https://github.com/kent8192/reinhardt-web/issues/4205)).
  Project `static/` is auto-mounted at `/static/`.
- **`createsuperuser` powered by `SuperuserCreator`** — works
  against any user type marked `#[user(full = true)]` + `#[model]`
  via the inventory-backed `SuperuserCreator` registry.
  `--noinput` reads the password from
  `REINHARDT_SUPERUSER_PASSWORD`, with the same minimum-length rule
  the interactive prompt enforces and an explicit mutually-exclusive
  check against `--no-password` ([#4233](https://github.com/kent8192/reinhardt-web/issues/4233)).
- **`startproject` / `startapp` with pluggable templates** — the
  `TemplateSource` trait has `Embedded`, `Filesystem`, and `Merged`
  implementations. `--template-dir` (or `REINHARDT_TEMPLATE_DIR`)
  switches templates per invocation. `startapp` appends the new app
  to the `installed_apps!` block automatically, and the
  Rust-2024-style `{name}.rs` module path is used (only `lib.rs` is
  renamed for default locations, not custom targets). Apps and
  projects with the `reinhardt_` prefix are rejected to prevent
  collisions with framework crates.
- **Per-app templates aligned with the [#4476](https://github.com/kent8192/reinhardt-web/discussions/4476) layout** —
  `apps/<app>/server_fn.rs.tpl`, `client.rs.tpl`,
  `client/components.rs.tpl`, `client/pages.rs.tpl`, plus the
  `urls/server_urls.rs.tpl` / `urls/client_urls.rs.tpl` /
  `urls/ws_urls.rs.tpl` triple. The project-level templates wire
  the client through `ClientRouter` and the `bootstrap.rs.tpl`
  entry point.
- **`makemigrations --merge`** — produces merge migrations for
  diverged branches without manual hand-editing. The `migrate`
  command auto-initializes the ORM dispatch and propagates the
  MySQL branch.
- **`introspect` and `check`** — `check` consumes
  `ProjectSettings` (no `env::var` reads) for typed access to
  configuration. `introspect` exposes `InfraSignals` with gRPC,
  storage, mail, session, graphql, admin, and i18n detection so
  CI / agent tooling can answer "which features are wired up".
- **`RunserverHook` for concurrent service startup** — registered
  via inventory; runs in parallel with the HTTP listener and is
  awaited before the listener accepts connections, replacing the
  ad-hoc startup-order coupling some integrations relied on.

### Notable Breaking Changes

- **Per-app handlers move to `apps/<app>/`** ([#4476](https://github.com/kent8192/reinhardt-web/discussions/4476))
  — `commands/templates/...` no longer carries handler code;
  existing projects must relocate matching source files.
  `reinhardt new` already emits the new layout.
- **`ProjectSettings` replaces `env::var`** ([#4295](https://github.com/kent8192/reinhardt-web/discussions/4295))
  — commands read configuration through `ProjectSettings` /
  `CommandContext::settings: Arc<dyn HasCommonSettings>` instead
  of touching `std::env`.
- **`runserver --with-pages` rebuilds WASM by default** ([#4205](https://github.com/kent8192/reinhardt-web/issues/4205))
  — the previous "skip if artefacts exist" behaviour is now opt-in
  via `--no-override-wasm`. `--force-wasm` is now redundant and
  emits a deprecation warning.
- **`cargo make watch` and friends removed** — the built-in
  hot-reload supersedes `bacon`-driven watch tasks ([#4128](https://github.com/kent8192/reinhardt-web/issues/4128)).

### Migration Notes

See the [root Migration Guide](https://github.com/kent8192/reinhardt-web/blob/main/CHANGELOG.md#migration-guide)
for project-wide guidance. Command-specific moves:

- Move per-app `server_fn` and client UI from
  `commands/templates/<app>/` into `apps/<app>/` per [#4476](https://github.com/kent8192/reinhardt-web/discussions/4476).
- Replace `std::env::var("REINHARDT_...")` calls in custom
  commands with reads from `CommandContext::settings`.
- Drop `--force-wasm` from `runserver` invocations and use
  `--no-override-wasm` if you intentionally pre-built WASM.
