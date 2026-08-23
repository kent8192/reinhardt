# Changelog

All notable changes to `reinhardt-query` will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.4.0-alpha.8](https://github.com/kent8192/reinhardt-web/compare/reinhardt-query@v0.4.0-alpha.7...reinhardt-query@v0.4.0-alpha.8) - 2026-08-22

### Fixed

- *(orm)* lock scoped mutation rows

## [0.4.0-alpha.7](https://github.com/kent8192/reinhardt-web/compare/reinhardt-query@v0.4.0-alpha.6...reinhardt-query@v0.4.0-alpha.7) - 2026-08-19

### Documentation

- update version references to v0.3.3
- update version references to v0.3.4
- update version references to v0.3.5
- update version references to v0.3.6
- update version references to v0.3.7
- update version references to v0.3.8

### Maintenance

- merge main into develop/0.4.0

### Testing

- *(query)* cover MySQL event builders
- *(query)* cover qualified tuple references
- *(query)* cover remaining reference helpers
- *(query)* cover builder defaults and iterators
- *(query)* cover remaining pure builders

## [0.4.0-alpha.6](https://github.com/kent8192/reinhardt-web/compare/reinhardt-query@v0.4.0-alpha.5...reinhardt-query@v0.4.0-alpha.6) - 2026-08-06

### Documentation

- *(release)* restore coherent alpha.3 references

## [0.4.0-alpha.4](https://github.com/kent8192/reinhardt-web/compare/reinhardt-query@v0.4.0-alpha.3...reinhardt-query@v0.4.0-alpha.4) - 2026-08-04

### Added

- *(db)* add backend-aware QuerySet explain

### Fixed

- *(pgvector)* preserve vector index metadata
- *(db)* preserve queryset semantics in explain
- *(db)* preserve safe explain plans
- *(db)* harden backend-aware explain
- *(db)* validate explain backend capabilities
- *(query)* validate explain backend capabilities
- *(query)* allow typed LIKE explain predicates
- *(query)* validate explain query shapes
- *(query)* harden explain validation
- *(query)* preserve MySQL explain syntax
- *(query)* harden backend explain rendering
- *(query)* preserve structural explain annotations
- *(query)* reject invalid explain expression shapes
- *(ci)* repair all-feature unit regressions
- *(query)* satisfy WASM backend validation lint

### Maintenance

- *(pgvector)* propagate native vector feature flags

### Other

- sync develop/0.4.0 into pgvector branch
- sync develop/0.4.0 into queryset explain

## [0.4.0-alpha.1](https://github.com/kent8192/reinhardt-web/compare/reinhardt-query@v0.3.2...reinhardt-query@v0.4.0-alpha.1) - 2026-07-21

### Fixed

- *(db)* alias renamed executor return columns
- resolve server function set review findings
- *(release)* restore develop prerelease lifecycle

### Maintenance

- merge main into develop/0.4.0
- merge develop/0.4.0 into server function set branch
## [0.3.9](https://github.com/kent8192/reinhardt-web/compare/reinhardt-query@v0.3.8...reinhardt-query@v0.3.9) - 2026-08-21

### Fixed

- *(orm)* bind scoped mutations atomically
- *(orm)* preserve declared array element types
- *(orm)* preserve model session query state

## [0.3.8](https://github.com/kent8192/reinhardt-web/compare/reinhardt-query@v0.3.7...reinhardt-query@v0.3.8) - 2026-08-16

### Testing

- *(query)* cover MySQL event builders
- *(query)* cover qualified tuple references
- *(query)* cover remaining reference helpers
- *(query)* cover builder defaults and iterators
- *(query)* cover remaining pure builders

## [0.3.0](https://github.com/kent8192/reinhardt-web/compare/reinhardt-query@v0.2.0...reinhardt-query@v0.3.0) - 2026-06-28

Stable release of `reinhardt-query` for the Reinhardt 0.3.0 line. This
entry consolidates the 0.3.0 release-candidate series into one
stable release section.

### Migration Notes

- Review the root CHANGELOG and `instructions/MIGRATION_0.3.md` before upgrading from 0.2.x.

### Fixed

- *(todo-check)* clear public api audit markers

## [0.2.0](https://github.com/kent8192/reinhardt-web/compare/reinhardt-query@v0.1.3...reinhardt-query@v0.2.0) - 2026-06-11

Stable release of `reinhardt-query` for the Reinhardt 0.2.0 line. This
entry consolidates the 0.2.0 release-candidate series into one
stable release section.

### Migration Notes

- Replace `SeaRc<T>` with `SharedRc<T>` and move call sites to the 0.2 filter-builder API.
- See [`instructions/MIGRATION_0.2.md`](../../instructions/MIGRATION_0.2.md) for the workspace migration checklist.

### Breaking Changes

- *(query)* [**breaking**] remove SeaRc type alias (refs [[#4520](https://github.com/kent8192/reinhardt-web/issues/4520)](https://github.com/kent8192/reinhardt-web/issues/4520))

### Added

- *(query)* [**breaking**] remove SeaRc type alias (refs [[#4520](https://github.com/kent8192/reinhardt-web/issues/4520)](https://github.com/kent8192/reinhardt-web/issues/4520))

### Changed

- [**breaking**] align develop/0.2.0 with main, preserving 8 feature crates

### Removed

- **`SeaRc<T>` type alias** (`src/types/iden.rs`, deprecated since
  `0.1.0-rc.16`) — removed per STABILITY_POLICY § SP-4. Use
  [`SharedRc`](src/types/iden.rs) directly instead, which expands to
  `Arc<T>` with the `thread-safe` feature and `Rc<T>` without it. The
  `pub use iden::SeaRc;` re-export in `src/types.rs` is also dropped.

  Refs umbrella Issue
  [#4520](https://github.com/kent8192/reinhardt-web/issues/4520).
  See [`instructions/MIGRATION_0.2.md`](../../instructions/MIGRATION_0.2.md#reinhardt-query)
  for the migration guide.

#### BREAKING CHANGES

- **`SeaRc<T>` type alias** (`src/types/iden.rs`, deprecated since
  `0.1.0-rc.16`) — removed per STABILITY_POLICY § SP-4. Use
  [`SharedRc`](src/types/iden.rs) directly instead, which expands to
  `Arc<T>` with the `thread-safe` feature and `Rc<T>` without it. The
  `pub use iden::SeaRc;` re-export in `src/types.rs` is also dropped.

  Refs umbrella Issue
  [#4520](https://github.com/kent8192/reinhardt-web/issues/4520).
  See [`instructions/MIGRATION_0.2.md`](../../instructions/MIGRATION_0.2.md#reinhardt-query)
  for the migration guide.

### Fixed

- *(ci)* recover develop release-plz prerelease

### Documentation

- *(query)* document SeaRc removal in CHANGELOG and migration guide (refs [[#4520](https://github.com/kent8192/reinhardt-web/issues/4520)](https://github.com/kent8192/reinhardt-web/issues/4520))

## [0.1.0](https://github.com/kent8192/reinhardt-web/compare/reinhardt-query@v0.1.0-rc.30...reinhardt-query@v0.1.0) - 2026-05-22

Initial stable release of `reinhardt-query` as part of the
reinhardt-web 0.1.0 release. `reinhardt-query` is the in-house
SQL query builder that backs `reinhardt-db`; it produces backend-
specific SQL (PostgreSQL, MySQL, SQLite) for DML, DDL, and DCL
statements without exposing applications to raw strings.

For the workspace-wide release narrative (Highlights, Breaking
Changes, Migration Guide), see the [root CHANGELOG](https://github.com/kent8192/reinhardt-web/blob/main/CHANGELOG.md#010---2026-05-22).
Per-prerelease history is preserved in the
[Release Discussions](https://github.com/kent8192/reinhardt-web/discussions/categories/release).

### Capabilities at 0.1.0

- **Typed query AST** — `Iden`, `IdenStatic`, `ColumnRef`, `TableRef`,
  `Value`, `Expr`, `SimpleExpr`, `Condition`, and `CaseStatement`
  form a fully typed expression algebra. The `Value` enum covers 20+
  variants (signed and unsigned integer widths, bytes, optional types)
  and integrates with `chrono`, `uuid`, `serde_json`, `rust_decimal`,
  and `bigdecimal` through feature flags.
- **DML builders** — `SelectStatement`, `InsertStatement`,
  `UpdateStatement`, and `DeleteStatement` with JOINs (INNER / LEFT /
  RIGHT / FULL OUTER / CROSS), GROUP BY, HAVING, DISTINCT (plus
  `DISTINCT ON` for PostgreSQL and `DISTINCT ROW` for MySQL), set
  operations (UNION / INTERSECT / EXCEPT), CTEs (`WITH RECURSIVE`
  included), window functions with frame clauses, and lock clauses
  (`FOR UPDATE`, `FOR SHARE`, …). `INSERT … FROM SELECT` is supported.
- **DDL builders** — `CreateTableStatement`,
  `AlterTableStatement` (ADD / DROP / RENAME COLUMN, ADD / DROP
  CONSTRAINT, RENAME TABLE), `DropTableStatement`,
  `CreateIndexStatement` (UNIQUE, partial `WHERE`, USING method), and
  `DropIndexStatement`. `ColumnType` covers 30+ SQL types and
  `IndexMethod` covers BTree / Hash / Gist / Gin / Brin / FullText /
  Spatial.
- **Full DCL surface** — GRANT / REVOKE for object privileges
  (16 privilege types, 15 object types, `WITH GRANT OPTION`,
  `GRANTED BY`, CASCADE), role membership (`WITH ADMIN OPTION`,
  `ADMIN OPTION FOR`), CREATE / DROP / ALTER ROLE and USER, and
  session management (`SET ROLE`, `RESET ROLE`,
  `SET DEFAULT ROLE`). PostgreSQL and MySQL are fully supported;
  SQLite panics with descriptive messages where DCL doesn't apply.
- **NoSQL Redis command builder** — Optional `nosql-redis` feature
  exposes typed builders for the Redis RESP command set, including a
  typestate-protected `ZAddBuilder` (`only_if_greater` /
  `only_if_less`) and a compile-fail trybuild suite to keep the API
  honest.
- **Pluggable backends** — `PostgresQueryBuilder`,
  `MySqlQueryBuilder`, and `SqliteQueryBuilder` implement the
  `QueryBuilder` trait so the same AST renders to backend-appropriate
  SQL (placeholder syntax, identifier quoting, `||` vs `CONCAT`,
  `NULLS FIRST/LAST` handling). All identifier emission is
  injection-safe: identifiers and values are escaped through
  `SqlWriter` helpers shared across statements.
- **Derive macros** — Optional `derive` feature pulls in
  `reinhardt-query-macros` for `#[derive(Iden)]`, accepting
  struct-level `#[iden]` attributes and emitting per-variant
  `Iden` impls (including the special `Table` variant).
- **`Rc` / `Arc` switch via `thread-safe`** — The `thread-safe`
  feature flips `DynIden` from `Rc`-based to `Arc`-based so the
  builder can be shared across threads when required (used by
  `reinhardt-db`).
- **`#[non_exhaustive]` future-proofing** — `Privilege` and
  `ObjectType` are `#[non_exhaustive]` so future variants can be
  added without a breaking-change wave.

### Notable Breaking Changes

- **Empty-string validation standardised across DCL** ([0.1.0-alpha.3](https://github.com/kent8192/reinhardt-web/blob/main/crates/reinhardt-query/CHANGELOG.md))
  — every DCL statement now rejects empty role / user / object names
  uniformly. Callers that previously passed `""` will see an
  `Err(...)` instead of malformed SQL.

### Migration Notes

- **DCL string validation**: Audit code paths that construct DCL
  statements from user input — empty strings now produce `Err`. Wrap
  inputs in `if name.is_empty()` guards before invoking the builder
  if you previously relied on permissive behaviour.
- **Renamed `SeaRc` → `SharedRc`**: The old `SeaRc` alias is
  deprecated. Update imports to `SharedRc`; the deprecated alias is
  kept for one release cycle.
- **Renamed Redis builder methods**: `ZAddBuilder::gt` / `lt` were
  renamed to `only_if_greater` / `only_if_less` for self-documenting
  intent.
