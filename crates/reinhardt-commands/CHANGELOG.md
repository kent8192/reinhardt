# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.4.0-alpha.8](https://github.com/kent8192/reinhardt-web/compare/reinhardt-commands@v0.4.0-alpha.7...reinhardt-commands@v0.4.0-alpha.8) - 2026-08-22

### Security

- *(commands)* upgrade evcxr to drop unmaintained json

## [0.4.0-alpha.7](https://github.com/kent8192/reinhardt-web/compare/reinhardt-commands@v0.4.0-alpha.6...reinhardt-commands@v0.4.0-alpha.7) - 2026-08-19

### Added

- *(commands)* export application contracts
- *(commands)* export application contract
- *(commands)* add deterministic contract verification
- *(commands)* define verification report v1
- *(commands)* add verify output formats
- *(commands)* render typed verification outcomes
- *(commands)* propagate verification exit codes

### Changed

- *(commands)* defer contract settings resolution

### Documentation

- *(contract)* publish application schema v0
- *(commands)* document contract verification
- *(commands)* document verification JSON protocol
- *(commands)* reorganize generated project guidance
- *(commands)* expand generated surface guidance
- *(commands)* document extractable app boundaries
- *(commands)* document page macro and target-neutral routes
- *(commands)* fix page macro guide ending
- *(commands)* expand generated project guidance
- *(commands)* add ORM query examples
- *(commands)* require field attributes in ORM guidance

### Fixed

- *(commands)* retain one-to-one contract references
- *(commands)* gate contract test metadata import
- *(commands)* use published varchar contract kind
- *(commands)* preserve contract feature compatibility
- *(commands)* resolve foreign key column types
- *(contract)* close export review findings
- *(contract)* close follow-up export review findings
- *(contract)* close mounted route follow-up findings
- *(contract)* resolve review findings
- *(contract)* preserve relative sqlite paths
- *(contract)* resolve application contract review findings
- resolve native protocol review findings
- *(commands)* compile deferred contract export
- *(commands)* harden deterministic contract verification
- *(commands)* use stable verification finding codes
- *(contract)* preserve mounted route metadata during export
- *(commands)* keep verification checks independent
- *(commands)* replay cargo feature and profile names
- *(commands)* make contract verification fail closed
- *(commands)* fail closed on process inspection errors
- *(commands)* compile shared dispatcher without contract
- satisfy format and clippy checks
- address contract verification review feedback
- close contract verification review gaps
- close contract verification review gaps
- refresh contract verification context
- close contract verification review gaps
- *(commands)* generate valid contract-aware project scaffolds
- *(contract)* close verification review gaps
- *(contract)* preserve resolved configuration semantics
- *(ci)* satisfy example build script clippy
- *(contract)* honor defaults and quoted manifest values
- *(ci)* parse multiline Cargo feature definitions
- *(contract)* preserve custom migration defaults
- *(commands)* complete verification report coverage
- *(commands)* preserve verification error details
- *(commands)* protect generated settings files

### Maintenance

- merge main into develop/0.4.0

### Other

- sync develop/0.4.0 into CI repair

### Testing

- *(contract)* exercise tutorial export consumer
- *(commands)* cover contract verification consumers
- *(commands)* make consumer replay host-portable
- *(commands)* cover both replay process failures
- *(commands)* cover both replay inspection stages
- *(commands)* reuse contract consumer fixture
- cover continued contract validators
- *(commands)* preserve report ordering fixture
- *(commands)* cover verification JSON protocol
- *(commands)* tighten verification JSON assertions
- *(commands)* remove generated guidance assertions

## [0.4.0-alpha.6](https://github.com/kent8192/reinhardt-web/compare/reinhardt-commands@v0.4.0-alpha.3...reinhardt-commands@v0.4.0-alpha.6) - 2026-08-06

### Added

- *(commands)* add ORM-aware Rust shell
- *(commands)* generate shell-enabled project wiring
- *(commands)* add squashmigrations CLI

### Documentation

- *(commands)* document Rust management shell migration
- *(commands)* document safe migration squashing
- *(commands)* clarify squash cleanup failures
- *(commands)* clarify inactive migration dependencies
- update version references to v0.4.0-alpha.5
- *(release)* restore coherent alpha.3 references

### Fixed

- *(commands)* harden shell evaluator lifecycle
- *(commands)* preserve shell evaluation output
- release shell and scoped database resources
- *(commands)* preserve shell bootstrap and interrupt output
- *(commands)* retain bootstrap evaluator resources
- *(commands)* close evaluator process trees
- *(commands)* harden shell evaluation boundaries
- *(commands)* harden shell bootstrap boundaries
- *(commands)* make shell startup interruptible
- *(commands)* await interrupted shell startup cleanup
- *(commands)* cancel shell startup promptly
- *(commands)* interrupt shell recovery startup
- *(commands)* preserve evaluator diagnostics
- *(commands)* preserve shell source prefixes
- *(commands)* avoid duplicate boundary diagnostics
- *(commands)* preserve shell bootstrap output and comments
- *(commands)* preserve bootstrap output streams
- *(commands)* preserve failed shell bootstrap output
- *(commands)* bound shell evaluator startup and output handling
- *(commands)* preserve shell evaluator boundary output
- *(commands)* guard evaluator process during startup
- *(commands)* preserve shell source and native gates
- *(commands)* preserve shell boundary output
- *(shell)* harden evaluator lifecycle
- *(shell)* preserve test database scopes
- *(commands)* preserve inner attributes after control whitespace
- *(commands)* preserve inner attributes after Rust whitespace
- *(commands)* gate shell-only helpers
- *(commands)* isolate shell evaluators by worker
- *(commands)* avoid unavailable shell prelude import
- *(commands)* handle shell evaluator lifecycle edges
- *(commands)* address shell review findings
- *(commands)* refine squashmigrations validation
- *(migrations)* harden squash range resolution
- *(migrations)* validate squash dependency context
- *(migrations)* harden squash generation
- *(commands)* plan replacement migrations
- *(migrations)* preserve replacement history semantics
- *(migrations)* resolve nested replacement histories
- *(migrations)* reconcile nested replacement histories
- *(migrations)* preserve replacement ancestry
- *(migrations)* retain partial replacement dependencies
- *(migrations)* resume fake replacement cleanup
- *(migrations)* expand fake replacement coverage
- *(migrations)* complete squash history reconciliation
- *(migrations)* handle squash review edge cases
- *(migrations)* cover squash review edge cases
- *(migrations)* preserve partial squash ordering
- *(migrations)* order partial squash descendants
- *(migrations)* retain partial replacement metadata
- *(commands)* restrict migration ordering helper to tests
- *(commands)* reconcile transitive replacement plans
- *(commands)* honor managed migration settings
- *(migrations)* address visibility review feedback
- *(ci)* restore visibility and wasm coverage
- *(commands)* reject keyless inspectdb objects
- *(commands)* harden dbshell diagnostics and MySQL transport
- *(release)* restore unpublished crates after partial release

### Maintenance

- auto-fix fmt and clippy

### Other

- sync develop/0.4.0 into pgvector branch
- integrate develop migration updates
- sync develop/0.4.0 and resolve review feedback

### Testing

- *(commands)* cover shell feature diagnostic
- *(commands)* retain shell evaluator failure output
- *(commands)* minimize real process exit probe
- *(commands)* align generated output expectations

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
## [0.3.10](https://github.com/kent8192/reinhardt-web/compare/reinhardt-commands@v0.3.9...reinhardt-commands@v0.3.10) - 2026-08-22

### Maintenance

- update Cargo.toml dependencies

## [0.3.9](https://github.com/kent8192/reinhardt-web/compare/reinhardt-commands@v0.3.8...reinhardt-commands@v0.3.9) - 2026-08-21

### Documentation

- *(security)* define runtime and operations boundaries
- *(security)* qualify remaining policy boundaries
- *(security)* qualify remaining boundary assumptions

### Fixed

- *(security)* qualify boundary control ownership

## [0.3.8](https://github.com/kent8192/reinhardt-web/compare/reinhardt-commands@v0.3.7...reinhardt-commands@v0.3.8) - 2026-08-16

### Maintenance

- update Cargo.toml dependencies

## [0.3.7](https://github.com/kent8192/reinhardt-web/compare/reinhardt-commands@v0.3.6...reinhardt-commands@v0.3.7) - 2026-08-12

### Changed

- *(commands)* add injectable process runner
- *(commands)* separate migration target planning
- *(commands)* separate CLI command planning

### Fixed

- *(commands)* preserve wasm process ordering
- *(commands)* yield server rebuild cargo build
- *(commands)* count compact custom command verbosity
- *(commands)* preserve workspace scaffold atomicity
- *(commands)* preserve PO headers and path exclusions
- *(commands)* detect PO header directives by line
- *(commands)* preserve introspection and static collection state
- *(commands)* satisfy strict clippy checks

### Testing

- *(commands)* cover process-backed build tooling
- *(commands)* cover server rebuild failures
- *(commands)* cover CLI resolution and dispatch
- *(commands)* cover built-in command decisions
- *(commands)* isolate built-in process state
- *(commands)* cover local infrastructure failures
- *(commands)* stabilize local infrastructure coverage
- *(commands)* cover workspace scaffold mutations
- *(commands)* cover project configuration and templates
- *(commands)* cover message extraction and compilation
- *(commands)* cover introspection and static collection
- *(commands)* cover virtual workspace introspection fallback
- *(commands)* cover watcher and feature failures
- *(commands)* verify watcher dispatcher outcomes
- *(commands)* raise behavior coverage above threshold
- *(commands)* harden coverage regression cases
- *(commands)* close remaining coverage gap
- *(commands)* reach crate coverage baseline
- *(commands)* cover router response conversion
- *(commands)* cover OpenAPI output failures
- *(commands)* cover SQLite migration targets
- *(commands)* cover fake SQLite migration targets
- *(commands)* cover noninteractive superuser creation
- *(commands)* cross coverage threshold
- *(commands)* cover runserver fallback paths
- *(commands)* assert complete settings fallback
- *(commands)* cover final fallback branches
- *(commands)* cover missing TLS key
- *(commands)* cover collectstatic failure
- *(commands)* stabilize collectstatic coverage matrix
- *(commands)* satisfy all-target clippy
- *(commands)* address review edge cases

## [0.3.6](https://github.com/kent8192/reinhardt-web/compare/reinhardt-commands@v0.3.5...reinhardt-commands@v0.3.6) - 2026-08-04

### Fixed

- *(commands)* validate local infrastructure state

### Security

- *(commands)* verify local infrastructure runtime state

## [0.3.5](https://github.com/kent8192/reinhardt-web/compare/reinhardt-commands@v0.3.4...reinhardt-commands@v0.3.5) - 2026-08-02

### Maintenance

- update Cargo.toml dependencies

## [0.3.4](https://github.com/kent8192/reinhardt-web/compare/reinhardt-commands@v0.3.3...reinhardt-commands@v0.3.4) - 2026-07-30

### Maintenance

- update Cargo.toml dependencies

## [0.3.3](https://github.com/kent8192/reinhardt-web/compare/reinhardt-commands@v0.3.2...reinhardt-commands@v0.3.3) - 2026-07-28

### Maintenance

- update Cargo.toml dependencies

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
