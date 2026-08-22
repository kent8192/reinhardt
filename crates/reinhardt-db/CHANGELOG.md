# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.4.0-alpha.9](https://github.com/kent8192/reinhardt-web/compare/reinhardt-db@v0.4.0-alpha.8...reinhardt-db@v0.4.0-alpha.9) - 2026-08-22

### Fixed

- *(orm)* make viewset creation insert-only

## [0.4.0-alpha.8](https://github.com/kent8192/reinhardt-web/compare/reinhardt-db@v0.4.0-alpha.7...reinhardt-db@v0.4.0-alpha.8) - 2026-08-22

### Documentation

- *(db)* remove redundant rustdoc link targets
- *(db)* restore rustdoc targets that do not resolve locally

### Fixed

- *(db)* execute scoped querysets through sessions
- *(db)* preserve typed temporal query values
- *(orm)* close request-scoping review gaps
- *(orm)* lock scoped mutation rows
- *(orm)* lock scoped mutation rows
- *(orm)* preserve typed field metadata
- *(orm)* preserve typed field metadata
- *(orm)* preserve scoped primary-key types
- *(orm)* parse displayed timestamp keys
- *(orm)* secure session-backed scoped reads
- *(orm)* use backend-safe mutation rechecks
- *(orm)* preserve typed manual primary keys
- *(orm)* avoid unsupported MySQL lock targets
- *(db)* avoid MySQL nullable relation locks
- *(db)* decode offset-free ISO timestamps
- *(db)* preserve mysql recheck locks
- *(orm)* lock scoped subqueries and classify datetime keys
- *(views)* serialize scoped subquery mutations
- *(orm)* serialize scoped join mutations
- *(orm)* reject unsafe mutation scopes
- *(orm)* render scope subqueries per backend
- *(orm)* preserve backend derived sources and lock guards
- *(orm)* support derived sources in session queries
- *(orm)* harden scoped model query locks
- *(orm)* allow prefetch-only model querysets
- *(orm)* close scoped query edge cases
- *(orm)* close request scoping review edge cases
- *(orm)* gate unused model select helper behind tests

### Styling

- *(orm)* format join alias collection

### Testing

- *(db)* use rstest for timestamp regression

## [0.4.0-alpha.7](https://github.com/kent8192/reinhardt-web/compare/reinhardt-db@v0.4.0-alpha.6...reinhardt-db@v0.4.0-alpha.7) - 2026-08-19

### Documentation

- update version references to v0.3.3
- update version references to v0.3.6
- update version references to v0.3.7
- update version references to v0.3.8

### Fixed

- *(orm)* bind explicit queryset parameters
- *(orm)* reject oversized queryset bind values
- *(orm)* keep UUID-looking filters as text
- *(orm)* bind typed temporal filter values
- *(orm)* retain UUID filter bindings
- *(orm)* preserve UUID primary key bindings
- *(db)* preserve primary key binding types
- *(db)* preserve composite model primary keys
- *(db)* support scalar primary key filters
- *(db)* preserve custom primary key lookups
- *(db)* retain numeric custom primary key filters
- *(db)* retain primary key fallback for aliases
- *(model)* preserve string primary key bindings
- *(orm)* preserve manual numeric primary key bindings
- *(orm)* retain primary key newtype compatibility
- *(orm)* bind many-to-many query values
- *(orm)* preserve many-to-many key types
- *(db)* update stale Jsonb test references

### Maintenance

- merge main into primary key binding fix
- merge main into develop/0.4.0

### Security

- *(orm)* preserve exact custom primary key bindings

### Testing

- *(db)* use rstest for numeric primary key fallback
- *(migrations)* cover operation token generation
- *(migrations)* cover remaining token branches
- *(orm)* cover typed query compilation
- *(orm)* cover custom manager contracts
- *(orm)* make custom manager defer observable
- *(db)* cover backend boundaries
- *(orm)* cover field deconstruction
- *(orm)* preserve related field case
- *(orm)* gate SQLite custom manager tests
- *(db)* cover pool contracts
- *(db)* assert connection timeout builder
- *(orm)* make pagination assertion deterministic
- *(db)* cover Codecov-reported ORM lines
- *(db)* cover aggregate expression branches
- *(db)* exercise pool event logger
- *(db)* cover migration AST parser
- *(db)* cover field migration operation paths
- *(db)* cover introspection parser branches
- *(db)* cover AST parser fallback

## [0.4.0-alpha.6](https://github.com/kent8192/reinhardt-web/compare/reinhardt-db@v0.4.0-alpha.5...reinhardt-db@v0.4.0-alpha.6) - 2026-08-06

### Documentation

- *(release)* restore coherent alpha.3 references

## [0.4.0-alpha.5](https://github.com/kent8192/reinhardt-web/compare/reinhardt-db@v0.4.0-alpha.4...reinhardt-db@v0.4.0-alpha.5) - 2026-08-05

### Fixed

- *(db)* align relationship imports with generated fields

### Other

- sync develop/0.4.0 and resolve review feedback

## [0.4.0-alpha.4](https://github.com/kent8192/reinhardt-web/compare/reinhardt-db@v0.4.0-alpha.3...reinhardt-db@v0.4.0-alpha.4) - 2026-08-04

### Added

- *(migrations)* add strict squash catalog
- *(migrations)* make squash optimization barrier-aware
- *(migrations)* render and safely write squash sources

### Changed

- *(db)* accept dynamic ORM executors

### Documentation

- *(db)* document atomic transaction outcomes
- *(migrations)* document squash app validation
- *(commands)* document safe migration squashing
- *(commands)* clarify squash cleanup failures
- *(inspectdb)* describe unique target relationships

### Fixed

- *(forms)* harden generated form persistence semantics
- *(db)* preserve naive timestamp wall-clock values
- *(forms)* complete model-backed form submission
- *(forms)* harden native model form decoding
- *(db)* preserve naive datetime query parameters
- *(forms)* prevent duplicate MySQL form inserts
- *(forms)* prevent duplicate create retries
- *(forms)* defer uncertain generated keys
- *(forms)* synchronize defaults and persistence state
- *(forms)* synchronize transaction-backed form state
- *(forms)* preserve transactional retry semantics
- *(forms)* preserve inline uncertain create state
- *(db)* scope atomic outcomes to savepoints
- *(db)* decode MySQL UTC fields with model metadata
- *(forms)* preserve nested form retries
- *(forms)* align native form validation
- *(db)* read PostgreSQL naive migration timestamps
- *(forms)* address model form review feedback
- *(pgvector)* preserve vector index metadata
- *(orm)* preserve typed vector NULL bindings
- *(ci)* repair pgvector test coverage
- *(pgvector)* close migration and binding gaps
- *(migrations)* harden squash catalog validation
- *(migrations)* refine squash range boundaries
- *(migrations)* preserve squash alter boundaries
- *(migrations)* harden squash source persistence
- *(migrations)* validate rendered source payloads
- *(migrations)* reject root identity changes
- *(migrations)* verify root snapshot identity
- *(migrations)* reject lossy strict parsing
- *(migrations)* validate squash root before writes
- *(migrations)* strictly parse nested column payloads
- *(db)* make squash operation parsing lossless
- *(db)* validate nested squash domain metadata
- *(migrations)* reject duplicate strict fields
- *(db)* parse legacy migration metadata and serial fields
- *(db)* use explicit empty segment check
- *(db)* mark standard alter-column rendering as supported
- *(migrations)* harden squash range resolution
- *(migrations)* validate squash execution contracts
- *(migrations)* ignore Rust module files
- *(migrations)* preserve RunRust source operations
- *(migrations)* validate migration directory identity
- *(migrations)* omit selected swappable dependencies
- *(migrations)* preserve public source operations
- *(migrations)* validate squash dependency context
- *(migrations)* preserve squash dependency semantics
- *(migrations)* harden squash generation
- *(migrations)* preserve squash dependency semantics
- *(migrations)* honor replacement execution
- *(migrations)* normalize squash dependencies
- *(migrations)* preserve squash reduction barriers
- *(migrations)* resolve replacement state history
- *(migrations)* normalize replacement histories
- *(migrations)* preserve replacement history semantics
- *(migrations)* resolve nested replacement histories
- *(migrations)* retain replacement rollback order
- *(migrations)* reconcile nested replacement histories
- *(migrations)* preserve replacement ancestry
- *(migrations)* preserve CreateTable backend options
- *(migrations)* complete squash history reconciliation
- *(migrations)* handle squash review edge cases
- *(migrations)* cover squash review edge cases
- *(migrations)* preserve partial squash ordering
- *(ci)* restore test fixture and lint compliance
- *(migrations)* restore public squash range construction
- *(migrations)* cover nested replacement histories
- *(migrations)* preserve squash source semantics
- *(migrations)* reject non-portable swappable squashes
- *(migrations)* address visibility review feedback
- *(orm)* reload composite MySQL upserts by lookup
- *(orm)* gate MySQL reload on generated primary key
- *(migrations)* validate replacement sets before adoption
- *(inspectdb)* preserve unique target relationships

### Maintenance

- *(pgvector)* propagate native vector feature flags

### Other

- sync develop/0.4.0 into pgvector branch
- sync develop/0.4.0 into pgvector branch
- integrate develop migration updates

### Styling

- *(migrations)* format squash execution tests
- *(migrations)* simplify dependency collection

### Testing

- *(migrations)* align strict parser diagnostic
- *(migrations)* verify backend option rendering
- *(migrations)* correct optional dependency assertions
- *(orm)* mark upsert fixture fields as generated
- *(migrations)* align SQL fixtures with safe rendering
- *(inspectdb)* cover unique relationship variants
- *(migrations)* assert complete MySQL table comment SQL

### Breaking Changes

- Replace the string-map `Manager::get_or_create` API and remove
  `get_or_create_with_conn`, `get_or_create_queries`, and `get_or_create_sql`.
  Use the typed `get_or_create` and `update_or_create` builders with generated
  model field accessors; caller-owned update/create execution requires a
  write-intent `AtomicTransaction`.
- Typed upsert builders now require model-generated `field_*()` accessors.
  The safe `FieldRef::new` constructor remains available for validated,
  lower-level dynamic filtering, but its unverified result cannot be supplied
  to proof-requiring APIs.

## [0.4.0-alpha.2](https://github.com/kent8192/reinhardt-web/compare/reinhardt-db@v0.4.0-alpha.1...reinhardt-db@v0.4.0-alpha.2) - 2026-07-23

### Added

- *(db)* [**breaking**] add Copy-safe injected connection handles

### Documentation

- *(db)* update database engine examples

### Fixed

- *(db)* restore URL-based backend owner connection

## [0.4.0-alpha.1](https://github.com/kent8192/reinhardt-web/compare/reinhardt-db@v0.3.2...reinhardt-db@v0.4.0-alpha.1) - 2026-07-21

### Added

- *(db)* add typed JSON model fields

### Fixed

- *(db)* normalize gb freephone numbers
- *(db)* normalize phone dialing variants
- *(db)* bind typed JSON scalars as JSON
- *(db)* address typed JSON review feedback
- *(db)* preserve typed JSON values across ORM paths
- *(db)* preserve JSON provenance across backend paths
- *(testkit)* preserve model schema metadata
- *(db)* avoid reconnecting initialized global database
- *(db)* harden typed relation traversal
- *(orm)* close typed relation traversal gaps
- *(orm)* support manual relation targets
- *(orm)* harden typed relation query SQL
- *(orm)* map typed relation columns
- *(orm)* align typed relation join aliases
- *(db)* qualify annotations after relation joins
- *(db)* qualify typed relation query clauses
- *(db)* harden typed relation query rendering
- *(db)* rebase typed relation query aliases
- *(db)* validate typed relation load paths
- *(db)* preserve typed eager load query semantics
- *(orm)* rebase aliases after manual joins
- *(db)* make aggregate field mapping explicit
- *(db)* preserve count wildcard with relation joins
- *(orm)* reserve typed relation aliases
- *(orm)* guard composite typed relation paths
- *(migrations)* preserve model table identity
- *(build)* remove cfg alias macro semicolons
- *(migrations)* preserve renamed table identity
- *(migrations)* retain FK creates after table renames
- *(migrations)* preserve create model table names
- *(commands)* scope global migration validation
- *(migrations)* preserve physical model state
- *(migrations)* handle moved model table ownership in PR 5673
- *(ci)* restore merged ORM compatibility
- *(db)* preserve typed ORM values
- preserve structured database errors
- *(pages)* resolve server function set review findings
- *(db)* update legacy executor test errors
- restore atomic ORM release compatibility
- *(orm)* decode string decimals as floating values
- *(release)* restore develop prerelease lifecycle
- *(ci)* stabilize release test failures

### Maintenance

- merge latest main into develop forward-merge
- *(auth)* merge develop into password policy branch
- merge latest develop changes into typed JSON PR
- merge develop/0.4.0 into typed traversal branch
- refresh main forward merge from develop/0.4.0
- merge develop/0.4.0 into table-name branch

### Other

- resolve develop/0.4.0 into model enum fields
- integrate latest enum field branch updates
- sync develop/0.4.0 into server function set

### Styling

- *(db)* format query annotation rendering

### Testing

- *(db)* preserve migration model identity on table rename
- *(migrations)* retain unrelated m2m constraints

### Added

- Add typed `UniqueFieldRef` descriptors for compile-time model and lookup-value
  matching, sealed against arbitrary downstream field-name construction by
  model-owned indexed proofs.
- Add executor-aware queryset and model operations plus backend-aware
  transaction executor behavior, including MySQL mutation paths that do not
  depend on `RETURNING`.
## [0.3.8](https://github.com/kent8192/reinhardt-web/compare/reinhardt-db@v0.3.7...reinhardt-db@v0.3.8) - 2026-08-16

### Testing

- *(db)* cover aggregate expression branches
- *(db)* exercise pool event logger
- *(db)* cover migration AST parser
- *(db)* cover field migration operation paths
- *(db)* cover introspection parser branches
- *(db)* cover AST parser fallback

## [0.3.7](https://github.com/kent8192/reinhardt-web/compare/reinhardt-db@v0.3.6...reinhardt-db@v0.3.7) - 2026-08-12

### Fixed

- *(orm)* bind many-to-many query values
- *(orm)* preserve many-to-many key types

### Testing

- *(migrations)* cover operation token generation
- *(migrations)* cover remaining token branches
- *(orm)* cover typed query compilation
- *(orm)* cover custom manager contracts
- *(orm)* make custom manager defer observable
- *(db)* cover backend boundaries
- *(orm)* cover field deconstruction
- *(orm)* preserve related field case
- *(orm)* gate SQLite custom manager tests
- *(db)* cover pool contracts
- *(db)* assert connection timeout builder
- *(orm)* make pagination assertion deterministic
- *(db)* cover Codecov-reported ORM lines

## [0.3.6](https://github.com/kent8192/reinhardt-web/compare/reinhardt-db@v0.3.5...reinhardt-db@v0.3.6) - 2026-08-04

### Fixed

- *(orm)* preserve manual numeric primary key bindings
- *(orm)* retain primary key newtype compatibility

### Maintenance

- merge main into primary key binding fix

### Security

- *(orm)* preserve exact custom primary key bindings

## [0.3.5](https://github.com/kent8192/reinhardt-web/compare/reinhardt-db@v0.3.4...reinhardt-db@v0.3.5) - 2026-08-02

### Maintenance

- update Cargo.toml dependencies

## [0.3.4](https://github.com/kent8192/reinhardt-web/compare/reinhardt-db@v0.3.3...reinhardt-db@v0.3.4) - 2026-07-30

### Fixed

- *(orm)* preserve typed keys in manager deletion

## [0.3.3](https://github.com/kent8192/reinhardt-web/compare/reinhardt-db@v0.3.2...reinhardt-db@v0.3.3) - 2026-07-28

### Fixed

- *(orm)* bind explicit queryset parameters
- *(orm)* reject oversized queryset bind values
- *(orm)* keep UUID-looking filters as text
- *(orm)* bind typed temporal filter values
- *(orm)* retain UUID filter bindings
- *(orm)* preserve UUID primary key bindings
- *(db)* preserve primary key binding types
- *(db)* preserve composite model primary keys
- *(db)* support scalar primary key filters
- *(db)* preserve custom primary key lookups
- *(db)* retain numeric custom primary key filters
- *(db)* retain primary key fallback for aliases
- *(model)* preserve string primary key bindings

### Testing

- *(db)* use rstest for numeric primary key fallback

## [0.3.2](https://github.com/kent8192/reinhardt-web/compare/reinhardt-db@v0.3.1...reinhardt-db@v0.3.2) - 2026-07-14

### Fixed

- *(orm)* preserve custom manager backend semantics

### Maintenance

- merge main into custom manager hooks branch

### Security

- *(orm)* enforce custom manager write hooks

## [0.3.1](https://github.com/kent8192/reinhardt-web/compare/reinhardt-db@v0.3.0...reinhardt-db@v0.3.1) - 2026-07-04

### Added

- *(db)* add scoped n+1 query detection

### Fixed

- *(db)* avoid listener map guards across awaits
- *(db)* address n+1 detector review gaps

## [0.3.0](https://github.com/kent8192/reinhardt-web/compare/reinhardt-db@v0.2.0...reinhardt-db@v0.3.0) - 2026-06-28

Stable release of `reinhardt-db` for the Reinhardt 0.3.0 line. This
entry consolidates the 0.3.0 release-candidate series into one
stable release section.

### Migration Notes

- Review the root CHANGELOG and `instructions/MIGRATION_0.3.md` before upgrading from 0.2.x.

### Added

- *(orm)* add QuerySet conditional partial updates

### Fixed

- *(db)* replay foreign key constraints and defaults
- *(db)* apply alter column defaults in migrations
- *(migrations)* handle field changes on renamed models
- *(migrations)* address autodetector review gaps
- *(todo-check)* clear public api audit markers
- *(db)* serialize CockroachDB migrations
- *(tutorial)* suppress basis runtime drift
- *(db)* suppress replayed migration drift
- add wasm safe model metadata substrate

### Performance

- *(db)* reduce pool acquire overhead
- *(db)* speed up migration graph topological sort

### Testing

- *(migrations)* tighten autodetector assertions

### Maintenance

- merge main into develop/0.3.0
- migrate Rust toolchain to 1.96.0

## [0.2.0](https://github.com/kent8192/reinhardt-web/compare/reinhardt-db@v0.1.3...reinhardt-db@v0.2.0) - 2026-06-11

Stable release of `reinhardt-db` for the Reinhardt 0.2.0 line. This
entry consolidates the 0.2.0 release-candidate series into one
stable release section.

### Migration Notes

- Update query/filter calls to the single-argument `Filter` contract and review generated migration diffs.
- Handle reverse migration SQL as `Vec<String>` where rollback may need multiple statements.
- See [`instructions/MIGRATION_0.2.md`](../../instructions/MIGRATION_0.2.md) for the workspace migration checklist.

### Breaking Changes

- **`FieldMetadata` gains type-safe `nullable: bool` field** ([#4439](https://github.com/kent8192/reinhardt-web/issues/4439)).
  `is_nullable()` reads the struct field. `with_nullable()` sets it as
  the canonical source of truth. `with_param("null", ...)` still works
  (auto-syncs the struct field) but should migrate to `with_nullable()`.
  `to_model_state()` no longer copies `"null"` into `FieldState.params`.
- *(db,macros)* [**breaking**] unify custom managers with Model::objects() ([[#3984](https://github.com/kent8192/reinhardt-web/issues/3984)](https://github.com/kent8192/reinhardt-web/issues/3984))
- *(model)* [**breaking**] make new an alias for build

### Added

- *(orm)* add Django-like lookup helpers
- *(orm)* support composite filter combinators
- *(db)* introduce type-safe nullable field on FieldMetadata
- *(db,macros)* [**breaking**] unify custom managers with Model::objects() ([[#3984](https://github.com/kent8192/reinhardt-web/issues/3984)](https://github.com/kent8192/reinhardt-web/issues/3984))
- *(model)* [**breaking**] make new an alias for build

### Changed

- [**breaking**] align develop/0.2.0 with main, preserving 8 feature crates

### Removed

- **`DatabaseConnection::get_database_url_from_env_or_settings(base_dir)`**
  (deprecated since `0.1.0-rc.29`) — removed per STABILITY_POLICY § SP-4
  and umbrella Issue [#4520](https://github.com/kent8192/reinhardt-web/issues/4520).
  The function reloaded `settings/<profile>.toml` from disk on every
  call, duplicating the framework's settings-loading logic. Use
  `DatabaseConnection::database_url_from(settings, env_override)` with
  a pre-built `ProjectSettings` instead.

#### BREAKING CHANGES

- **`DatabaseConnection::get_database_url_from_env_or_settings(base_dir)`**
  (deprecated since `0.1.0-rc.29`) — removed per STABILITY_POLICY § SP-4
  and umbrella Issue [#4520](https://github.com/kent8192/reinhardt-web/issues/4520).
  The function reloaded `settings/<profile>.toml` from disk on every
  call, duplicating the framework's settings-loading logic. Use
  `DatabaseConnection::database_url_from(settings, env_override)` with
  a pre-built `ProjectSettings` instead.

In-tree test deleted: `crates/reinhardt-db/tests/database_url_loader_interpolation.rs`.

Note: this PR keeps the consumer in `reinhardt-commands/src/builtin.rs`
(`get_database_url_from_settings`) unchanged. That helper still
references the removed entry point and will be migrated in a
follow-up `chore(commands)!: replace get_database_url_from_settings with
database_url_from` PR.

See [`instructions/MIGRATION_0.2.md`](../../instructions/MIGRATION_0.2.md#reinhardt-db)
for the migration guide.

### Fixed

- *(orm)* address lookup review edge cases
- *(db)* align LIKE escape SQL expectations
- *(db)* qualify Manager in rustdoc examples and add missing Objects type
- *(docs)* resolve remaining cross-crate intra-doc link errors
- repair release examples tests

- *(ci)* pin broken upstream transitive releases
- address CodeRabbit dependency gate review
- *(db)* address CodeRabbit review on CHANGELOG and with_param normalization
- *(ci)* recover develop release-plz prerelease

### Performance

- atomize facade dependency feature gates
- trim standard facade feature dependencies

### Documentation

- *(reinhardt-db)* fix QuerySet doctests for single-argument filter() API
- *(reinhardt-db)* qualify Filter path in with_db doctests

### Styling

- apply formatter fixes across workspace
- apply rustfmt to non-DSL files on develop/0.2.0

### Maintenance

- *(examples)* remove examples-twitter

## [0.1.0](https://github.com/kent8192/reinhardt-web/compare/reinhardt-db@v0.1.0-rc.30...reinhardt-db@v0.1.0) - 2026-05-22

Initial stable release of `reinhardt-db` as part of the reinhardt-web
0.1.0 release. `reinhardt-db` is the Django-style database layer for
the framework, providing an async ORM, schema migrations, a connection
pool, and a unified backend abstraction over PostgreSQL, MySQL,
SQLite, and CockroachDB.

For the workspace-wide release narrative (Highlights, Breaking
Changes, Migration Guide), see the [root CHANGELOG](https://github.com/kent8192/reinhardt-web/blob/main/CHANGELOG.md#010---2026-05-22).
Per-prerelease history is preserved in the
[Release Discussions](https://github.com/kent8192/reinhardt-web/discussions/categories/release).

### Capabilities at 0.1.0

- **Multi-backend support** — First-class support for PostgreSQL,
  MySQL, SQLite, and CockroachDB through a single async `sqlx`-backed
  connection abstraction. Backend-specific quirks (CockroachDB
  sentinel-row locks, SQLite `INTEGER PRIMARY KEY AUTOINCREMENT`,
  MySQL `MODIFY COLUMN`) are handled internally so application code
  stays portable.
- **Type-safe async ORM** — `#[model]`-driven model definitions with
  `QuerySet` filtering, `Manager` / `CustomManager` traits for custom
  query entry points, eager-loaded relations, and full Many-to-Many
  accessors with deterministic through-table naming. Built on top of
  `reinhardt-query` so no raw SQL strings appear in application code.
- **Django-style migrations** — `makemigrations` autodetector that
  diffs model state against the live database schema and emits typed
  migration operations including `CreateCompositePrimaryKey`,
  `SetAutoIncrementValue`, constraint diffs, and per-backend
  `ALTER COLUMN TYPE` dispatch. Migration conflict detection generates
  merge migration names automatically.
- **Connection pooling** — Pluggable pool implementation with
  `ManuallyDrop`-protected `PooledConnection` semantics, poison-safe
  lock recovery, and a `FilesystemSource` migration loader that warns
  rather than panics on missing migration directories.
- **NoSQL ODM scaffolding** — Optional `nosql` features expose a
  `Document` trait, `Repository<T>` for type-safe CRUD, and a builder-
  pattern `IndexModel`. Backends are gated behind individual features
  (`mongodb`, `redis`, `cassandra`, `dynamodb`, `neo4j`) so the
  default build remains SQL-only.
- **Configuration from `ProjectSettings`** — `database_url_from`
  accepts any type implementing `HasCoreSettings`, so connection
  strings are resolved through the typed-TOML settings pipeline
  instead of bare `env::var` calls. `DATABASE_URL` is opted into
  TOML interpolation explicitly.
- **SQL-injection-safe identifiers** — All identifier emission routes
  through `pg_escape`-style quoting (centralised in `crate::naming`),
  `BatchInsertBuilder` and `ON CONFLICT` quote column names, and
  `BatchUpdateBuilder`'s previously-injection-vulnerable methods were
  removed before stable.
- **UUID v7 by default** — Generated UUIDs use v7 across the crate
  for time-ordered primary keys.

### Notable Breaking Changes

- **`ProjectSettings` replaces `env::var`** ([#4295](https://github.com/kent8192/reinhardt-web/discussions/4295))
  — `commands` and `db` no longer read `DATABASE_URL` via `std::env`;
  pass a `&dyn HasCoreSettings` (or call `database_url_from`) so
  typed TOML interpolation governs the value.
- **`unique_together` propagates into `ModelMetadata`** ([#4027](https://github.com/kent8192/reinhardt-web/discussions/4027))
  — the autodetector now consumes `unique_together`, so existing
  migrations regenerate. Diff once, commit the regeneration.
- **SQL-injection-vulnerable `BatchUpdateBuilder` methods removed**
  — APIs that interpolated identifiers without quoting are gone.
  Switch to the parameterised builder methods.

### Migration Notes

- **DATABASE_URL via settings**: Replace
  `std::env::var("DATABASE_URL")` call sites with
  `reinhardt_db::database_url_from(settings)`; declare a
  `database_url` field on your settings struct (or rely on the
  default TOML key) and remove the explicit `env::var` import.
- **Regenerate migrations**: After upgrading, run `makemigrations`
  once on each app — `unique_together` and constraint diffs will
  appear as new migration files. Apply with `migrate`.
- **Audit raw-SQL call sites**: If you constructed `BatchUpdateBuilder`
  arguments by hand, replace removed methods with the parameterised
  builder; refer to the
  [reinhardt-query docs](https://docs.rs/reinhardt-query) for the
  current safe API.
