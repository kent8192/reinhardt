# Changelog

All notable changes to `reinhardt-query` will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0-alpha.4](https://github.com/kent8192/reinhardt-web/compare/reinhardt-query@v0.1.0-alpha.3...reinhardt-query@v0.1.0-alpha.4) - 2026-02-23

### Added

- *(query)* add CTE (Common Table Expression) support
- *(query)* expose maintenance statement APIs (VACUUM, ANALYZE, materialized views)
- *(query)* add INSERT from subquery support

### Documentation

- *(query)* improve documentation for identifier quoting and Value enum

### Fixed

- *(security)* use parameterized queries and escape identifiers to prevent SQL injection
- *(query)* preserve subquery parameter values in FROM clause
- *(query)* implement proper handling for TableColumn, AsEnum, and Cast in MySQL/SQLite backends
- *(reinhardt-query-macros)* replace write_str unwrap with expect documenting infallibility
- *(reinhardt-query-macros)* emit errors for invalid #[iden] attribute arguments
- *(query-macros)* add compile-time Debug assertion for derive(Iden)
- *(release)* bump reinhardt-query-macros to v0.1.0-alpha.4 to skip yanked alpha.3

### Security

- *(query)* escape SQL identifiers in postgres backend
- *(reinhardt-query)* escape single quotes in Value::Char SQL literal

### Styling

- apply code formatting to security fix files
- fix formatting for query module changes

## [0.1.0-alpha.3](https://github.com/kent8192/reinhardt-web/compare/reinhardt-query@v0.1.0-alpha.2...reinhardt-query@v0.1.0-alpha.3) - 2026-02-15

### Documentation

- *(dcl)* update AlterUserStatement validate() doc example for new validation rules

### Fixed

- [**breaking**] standardize empty string validation across DCL statements

## [0.1.0-alpha.2](https://github.com/kent8192/reinhardt-web/compare/reinhardt-query@v0.1.0-alpha.1...reinhardt-query@v0.1.0-alpha.2) - 2026-02-14

### Maintenance

- updated the following local packages: reinhardt-query-macros

## [0.1.0-alpha.1] - Unreleased

### Added

#### Core Types
- `Iden` trait for SQL identifiers with `unquoted()` method
- `IdenStatic` marker trait for compile-time identifiers
- `Alias` for runtime-determined names
- `DynIden` type-erased identifier (`Rc`/`Arc` based on `thread-safe` feature)
- `ColumnRef` (simple, table-qualified, schema-qualified, asterisk)
- `TableRef` (simple, schema-qualified, aliased, subquery)
- `IntoIden`, `IntoColumnRef`, `IntoTableRef` conversion traits

#### Value System
- `Value` enum with 20+ variants (Bool, TinyInt, SmallInt, Int, BigInt, TinyUnsigned,
  SmallUnsigned, Unsigned, BigUnsigned, Float, Double, String, Bytes, and optional types)
- `IntoValue` trait for Rust type conversion
- `ValueTuple` for tuple values (IN clauses)
- `Values` wrapper for collected query parameters
- `ArrayType` for typed array values
- Optional type support via feature flags: `with-chrono`, `with-uuid`,
  `with-json`, `with-rust_decimal`, `with-bigdecimal`

#### Expression System
- `Expr` builder with column, value, and function expressions
- `SimpleExpr` AST for representing expression trees
- `ExprTrait` with arithmetic (`add`, `sub`, `mul`, `div`, `modulo`),
  comparison (`eq`, `ne`, `gt`, `gte`, `lt`, `lte`),
  logical (`and`, `or`, `not`), and pattern matching (`like`, `not_like`,
  `between`, `not_between`, `is_null`, `is_not_null`, `is_in`, `is_not_in`)
- `Condition` and `Cond` for building WHERE/HAVING clauses
- `CaseStatement` for CASE WHEN expressions
- `Keyword` enum for SQL keywords (`CurrentTimestamp`, `Null`, `Custom`)
- Subquery expressions (`exists`, `not_exists`, `in_subquery`, `not_in_subquery`)
- Tuple expressions for multi-value comparisons

#### Query Builders
- `SelectStatement` with columns, FROM, WHERE, ORDER BY, LIMIT, OFFSET
- `InsertStatement` with table, columns, values (single and multi-row)
- `UpdateStatement` with table, SET, WHERE
- `DeleteStatement` with table, WHERE
- `Query` factory with methods:
  - DML: `select()`, `insert()`, `update()`, `delete()`
  - DCL Privileges: `grant()`, `revoke()`, `grant_role()`, `revoke_role()`
  - DCL Roles: `create_role()`, `drop_role()`, `alter_role()`
  - DCL Users: `create_user()`, `drop_user()`, `alter_user()`, `rename_user()`
  - DCL Session: `set_role()`, `reset_role()`, `set_default_role()`

#### DCL (Data Control Language) Builders

**Privilege Management:**
- `GrantStatement` - GRANT statement builder with fluent API for object privileges
- `RevokeStatement` - REVOKE statement builder with fluent API for object privileges
- `GrantRoleStatement` - GRANT role membership statement builder
- `RevokeRoleStatement` - REVOKE role membership statement builder
- `RoleSpecification` enum - Role specifications (RoleName, CurrentRole, CurrentUser, SessionUser)
- `DropBehavior` enum - Drop behavior for REVOKE (Cascade, Restrict)
- `Privilege` enum - 16 privilege types (SELECT, INSERT, UPDATE, DELETE, REFERENCES, CREATE, ALL, TRUNCATE, TRIGGER, MAINTAIN, USAGE, CONNECT, TEMPORARY, EXECUTE, SET, ALTER SYSTEM)
  - **Breaking Change Prevention**: Added `#[non_exhaustive]` attribute to allow future extensions without breaking changes
- `ObjectType` enum - 15 database object types (Table, Database, Schema, Sequence, Function, Procedure, Routine, Type, Domain, ForeignDataWrapper, ForeignServer, Language, LargeObject, Tablespace, Parameter)
  - **Breaking Change Prevention**: Added `#[non_exhaustive]` attribute to allow future extensions without breaking changes
  - **Extended PostgreSQL Support**: Added 11 PostgreSQL-specific object types (Function, Procedure, Routine, Type, Domain, ForeignDataWrapper, ForeignServer, Language, LargeObject, Tablespace, Parameter)
  - Convenience methods: `on_function()`, `on_procedure()`, `on_routine()`, `on_type()`, `on_domain()`, `on_foreign_data_wrapper()`, `on_foreign_server()`, `on_language()`, `on_large_object()`, `on_tablespace()`, `on_parameter()` for GrantStatement
  - Convenience methods: `from_function()`, `from_procedure()`, `from_routine()`, `from_type()`, `from_domain()`, `from_foreign_data_wrapper()`, `from_foreign_server()`, `from_language()`, `from_large_object()`, `from_tablespace()`, `from_parameter()` for RevokeStatement
- `Grantee` enum - 6 grantee types (Role, User with host, Public, CurrentRole, CurrentUser, SessionUser)
- Privilege-object validation logic
- WITH GRANT OPTION support (object privileges)
- WITH ADMIN OPTION support (role membership)
- ADMIN OPTION FOR support (role membership revocation)
- GRANTED BY clause support (PostgreSQL)
- CASCADE and RESTRICT support (PostgreSQL)

**Role and User Management:**
- `CreateRoleStatement` - CREATE ROLE statement builder with fluent API
- `DropRoleStatement` - DROP ROLE statement builder with IF EXISTS support
- `AlterRoleStatement` - ALTER ROLE statement builder with attribute modification and RENAME TO (PostgreSQL)
- `CreateUserStatement` - CREATE USER statement builder (PostgreSQL alias, MySQL native)
- `DropUserStatement` - DROP USER statement builder with IF EXISTS support
- `AlterUserStatement` - ALTER USER statement builder (PostgreSQL alias, MySQL native)
- `RenameUserStatement` - RENAME USER statement builder (MySQL only)
- `RoleAttribute` enum - PostgreSQL role attributes (SUPERUSER, CREATEDB, CREATEROLE, INHERIT, LOGIN, REPLICATION, BYPASSRLS, CONNECTION LIMIT, PASSWORD, VALID UNTIL, IN ROLE, ROLE, ADMIN)
- `UserOption` enum - MySQL user options (IDENTIFIED BY, IDENTIFIED WITH, ACCOUNT LOCK/UNLOCK, PASSWORD EXPIRE, PASSWORD HISTORY, PASSWORD REUSE INTERVAL, PASSWORD REQUIRE CURRENT, FAILED_LOGIN_ATTEMPTS, PASSWORD_LOCK_TIME, COMMENT, ATTRIBUTE)

**Session Management:**
- `SetRoleStatement` - SET ROLE statement builder with support for specific roles, NONE, ALL, ALL EXCEPT (MySQL)
- `ResetRoleStatement` - RESET ROLE statement builder (PostgreSQL only)
- `SetDefaultRoleStatement` - SET DEFAULT ROLE statement builder (MySQL only)
- `RoleTarget` enum - Role targets for SET ROLE (Named, None, All, AllExcept, Default)
- `DefaultRoleSpec` enum - Default role specifications (RoleList, All, None)

**Database Support:**
- PostgreSQL and MySQL full support for all DCL operations
- SQLite not supported (panics with descriptive error messages)

#### DDL Operations
- `CreateTableStatement` with columns, constraints, indexes, IF NOT EXISTS
- `AlterTableStatement` with ADD/DROP/RENAME COLUMN, ADD/DROP CONSTRAINT, RENAME TABLE
- `DropTableStatement` with multiple tables, IF EXISTS, CASCADE/RESTRICT (PostgreSQL)
- `CreateIndexStatement` with UNIQUE, IF NOT EXISTS, WHERE clause (partial indexes), USING method
- `DropIndexStatement` with IF EXISTS, CASCADE/RESTRICT (PostgreSQL)
- `ColumnDef` for column definitions with type, constraints, default, check
- `ColumnType` enum with 30+ SQL types (Integer, String, Text, JSON, Array, etc.)
- `TableConstraint` enum (PRIMARY KEY, FOREIGN KEY, UNIQUE, CHECK)
- `IndexMethod` enum (BTree, Hash, Gist, Gin, Brin, FullText, Spatial)
- `AlterTableOperation` enum for ALTER TABLE operations
- `Query` factory extensions: `create_table()`, `alter_table()`, `drop_table()`, `create_index()`, `drop_index()`

#### Advanced SELECT Features
- JOIN support (INNER, LEFT, RIGHT, FULL OUTER, CROSS) with ON/USING
- GROUP BY with multiple columns
- HAVING clause
- DISTINCT, DISTINCT ON (PostgreSQL), DISTINCT ROW (MySQL)
- UNION, UNION ALL, INTERSECT, EXCEPT
- LOCK clauses (FOR UPDATE, FOR SHARE, FOR KEY SHARE, FOR NO KEY UPDATE)
- NULLS FIRST / NULLS LAST ordering
- Common Table Expressions (WITH, WITH RECURSIVE)
- Window functions (OVER, PARTITION BY, ORDER BY, frame clauses)
- Named window definitions (WINDOW clause)

#### Window Functions
- `WindowStatement` with partition_by, order_by, and frame
- `FrameClause` with frame_type, start, and end boundaries
- `FrameType` enum (Range, Rows, Groups)
- `Frame` enum (UnboundedPreceding, Preceding, CurrentRow, Following, UnboundedFollowing)
- `Expr::over()` for inline window specifications
- `Expr::over_named()` for named window references
- Ranking functions: `row_number()`, `rank()`, `dense_rank()`, `ntile()`
- Value functions: `lead()`, `lag()`, `first_value()`, `last_value()`, `nth_value()`

#### Backends
- `PostgresQueryBuilder` - double-quoted identifiers, `$N` placeholders,
  DISTINCT ON, GROUPS frame, `||` concatenation, RETURNING, NULLS FIRST/LAST,
  DDL with CASCADE/RESTRICT support, Full DCL support:
  - GRANT/REVOKE with GRANTED BY and CASCADE support
  - GRANT/REVOKE role membership with all PostgreSQL features
  - CREATE/DROP/ALTER ROLE with all PostgreSQL role attributes
  - CREATE/DROP/ALTER USER (aliases for ROLE operations)
  - SET ROLE and RESET ROLE for session management
- `MySqlQueryBuilder` - backtick-quoted identifiers, `?` placeholders,
  DISTINCT ROW, INSERT IGNORE, DDL with table-qualified DROP INDEX, Full DCL support:
  - GRANT/REVOKE with User@Host format
  - GRANT/REVOKE role membership with WITH ADMIN OPTION
  - CREATE/DROP/ALTER ROLE with MySQL-specific options
  - CREATE/DROP/ALTER USER with user@host specification
  - RENAME USER for user renaming
  - SET ROLE and SET DEFAULT ROLE for session management
- `SqliteQueryBuilder` - double-quoted identifiers, `?` placeholders,
  NULLS FIRST/LAST, `||` concatenation, DDL support, DCL not supported (panics)
- `SqlWriter` infrastructure for SQL string construction
- `QueryBuilder` trait for backend-agnostic query generation (DML, DDL, and DCL)

#### Operators
- `BinOper` for binary operators (arithmetic, comparison, logical, pattern)
- `UnOper` for unary operators (NOT, NEGATE, EXISTS)
- `LogicalChainOper` for AND/OR chaining
- `PgBinOper` for PostgreSQL-specific operators (concatenation, JSON, array)
- `SubQueryOper` for subquery operators (EXISTS, IN, ALL, ANY, SOME)

#### Documentation
- Comprehensive crate-level documentation with examples
- Module-level documentation for all public modules
- README with usage examples for all query types
- Doc comments on all public APIs
