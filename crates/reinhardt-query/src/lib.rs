//! # reinhardt-query
//!
//! A type-safe SQL query builder for the Reinhardt framework.
//!
//! This crate provides a fluent API for constructing SQL queries that target
//! PostgreSQL, MySQL, and SQLite databases. It generates parameterized queries
//! with proper identifier escaping and value placeholders for each backend.
//!
//! ## Features
//!
//! ### DML (Data Manipulation Language)
//! - **Type-safe query construction** - Build SELECT, INSERT, UPDATE, DELETE statements
//! - **DCL (Data Control Language) support** - Build GRANT and REVOKE statements
//! - **Expression system** - Rich expression API with arithmetic, comparison, and logical operators
//! - **Advanced SQL features** - JOINs, GROUP BY, HAVING, DISTINCT, UNION, CTEs, Window functions
//!
//! ### DDL (Data Definition Language)
//! - **Schema management** - CREATE/ALTER/DROP SCHEMA (PostgreSQL, CockroachDB)
//! - **Sequence operations** - CREATE/ALTER/DROP SEQUENCE (PostgreSQL, CockroachDB)
//! - **Database operations** - CREATE/ALTER/DROP DATABASE (all backends)
//! - **Functions & Procedures** - CREATE/ALTER/DROP FUNCTION/PROCEDURE (PostgreSQL, MySQL, CockroachDB)
//! - **Custom types** - CREATE/ALTER/DROP TYPE (PostgreSQL, CockroachDB)
//! - **Materialized views** - CREATE/ALTER/DROP/REFRESH MATERIALIZED VIEW (PostgreSQL, CockroachDB)
//! - **Events** - CREATE/ALTER/DROP EVENT (MySQL)
//! - **Comments** - COMMENT ON for all database objects (PostgreSQL, CockroachDB)
//! - **Maintenance** - VACUUM, ANALYZE, OPTIMIZE/REPAIR/CHECK TABLE
//!
//! ### Multi-Backend Support
//! - **PostgreSQL** - Full DDL and DML support with advanced features
//! - **MySQL** - DML, Functions, Procedures, Events, and table maintenance
//! - **SQLite** - DML and basic DDL operations
//! - **CockroachDB** - Full PostgreSQL compatibility with distributed database features
//! - **Parameterized queries** - Automatic placeholder generation (`$1` for PostgreSQL, `?` for MySQL/SQLite)
//!
//! ## Architecture
//!
//! The crate is organized into several modules:
//!
//! - [`value`]: Core value types for representing SQL values
//! - [`types`]: Identifier, column reference, table reference, and operator types
//! - [`expr`]: Expression building with the [`ExprTrait`] system
//! - [`query`]: Query builders ([`SelectStatement`],
//!   [`InsertStatement`], [`UpdateStatement`],
//!   [`DeleteStatement`])
//! - [`dcl`]: DCL (Data Control Language) builders ([`GrantStatement`],
//!   [`RevokeStatement`], [`Privilege`], [`ObjectType`], [`Grantee`])
//! - [`backend`]: Database backend implementations
//!   ([`PostgresQueryBuilder`],
//!   [`MySqlQueryBuilder`],
//!   [`SqliteQueryBuilder`])
//!
//! ## Quick Start
//!
//! ```rust
//! use reinhardt_query::prelude::*;
//!
//! // Build a SELECT query
//! let mut stmt = Query::select();
//! stmt.column("name")
//!     .column("email")
//!     .from("users")
//!     .and_where(Expr::col("active").eq(true))
//!     .order_by("name", Order::Asc)
//!     .limit(10);
//!
//! // Generate SQL for PostgreSQL
//! let builder = PostgresQueryBuilder::new();
//! let (sql, values) = builder.build_select(&stmt);
//! assert_eq!(
//!     sql,
//!     r#"SELECT "name", "email" FROM "users" WHERE "active" = $1 ORDER BY "name" ASC LIMIT $2"#
//! );
//! assert_eq!(values.len(), 2);
//! ```
//!
//! ## Backend Differences
//!
//! ### DML Features
//! | Feature | PostgreSQL | MySQL | SQLite | CockroachDB |
//! |---------|-----------|-------|--------|-------------|
//! | Identifier quoting | `"name"` | `` `name` `` | `"name"` | `"name"` |
//! | Placeholders | `$1, $2, ...` | `?, ?, ...` | `?, ?, ...` | `$1, $2, ...` |
//! | NULLS FIRST/LAST | ✅ Native | ❌ | ✅ Native | ✅ Native |
//! | DISTINCT ON | ✅ | ❌ | ❌ | ✅ |
//! | Window functions | ✅ Full | ✅ Full | ✅ Full | ✅ Full |
//! | CTEs (WITH) | ✅ | ✅ | ✅ | ✅ |
//!
//! ### DDL Features
//! | Feature | PostgreSQL | MySQL | SQLite | CockroachDB |
//! |---------|-----------|-------|--------|-------------|
//! | CREATE/ALTER/DROP SCHEMA | ✅ | ❌ | ❌ | ✅ |
//! | CREATE/ALTER/DROP SEQUENCE | ✅ | ❌ | ❌ | ✅ |
//! | CREATE/ALTER/DROP DATABASE | ✅ | ✅ | ✅ | ✅ |
//! | CREATE/ALTER/DROP FUNCTION | ✅ | ✅ | ❌ | ✅ |
//! | CREATE/ALTER/DROP PROCEDURE | ✅ | ✅ | ❌ | ✅ |
//! | CREATE/ALTER/DROP TYPE | ✅ | ❌ | ❌ | ✅ |
//! | CREATE/ALTER/DROP EVENT | ❌ | ✅ | ❌ | ❌ |
//! | MATERIALIZED VIEW | ✅ | ❌ | ❌ | ✅ |
//! | COMMENT ON | ✅ | ❌ | ❌ | ✅ |
//! | VACUUM/ANALYZE | ✅ | ❌ | ✅ | ✅ |
//! | OPTIMIZE/REPAIR/CHECK | ❌ | ✅ | ❌ | ❌ |
//!
//! ### DCL Features
//! | Feature | PostgreSQL | MySQL | SQLite | CockroachDB |
//! |---------|-----------|-------|--------|-------------|
//! | GRANT/REVOKE | ✅ | ✅ | ❌ | ✅ |
//! | CREATE/DROP/ALTER ROLE | ✅ | ✅ | ❌ | ✅ |
//! | CREATE/DROP/ALTER USER | ✅ | ✅ | ❌ | ✅ |
//! | RENAME USER | ❌ | ✅ | ❌ | ❌ |
//! | SET ROLE | ✅ | ✅ | ❌ | ✅ |
//! | RESET ROLE | ✅ | ❌ | ❌ | ✅ |
//! | SET DEFAULT ROLE | ❌ | ✅ | ❌ | ❌ |
//!
//! ## Expression Examples
//!
//! ```rust
//! use reinhardt_query::prelude::*;
//!
//! // Arithmetic expressions
//! let expr = Expr::col("price").mul(Expr::col("quantity"));
//!
//! // Comparison with chaining
//! let cond = Expr::col("age").gte(18i32).and(Expr::col("active").eq(true));
//!
//! // CASE WHEN expressions
//! let case_expr = Expr::case()
//!     .when(Expr::col("score").gte(90i32), "A")
//!     .when(Expr::col("score").gte(80i32), "B")
//!     .else_result("C");
//!
//! // LIKE pattern matching
//! let like_expr = Expr::col("email").like("%@example.com");
//! ```
//!
//! ## DDL Examples
//!
//! ```rust,ignore
//! use reinhardt_query::prelude::*;
//!
//! // Create a schema (PostgreSQL, CockroachDB)
//! let mut stmt = Query::create_schema();
//! stmt.name("app_schema").if_not_exists();
//!
//! // Create a sequence (PostgreSQL, CockroachDB)
//! let mut stmt = Query::create_sequence();
//! stmt.name("user_id_seq").start_with(1000).increment_by(1);
//!
//! // Create a function (PostgreSQL, MySQL, CockroachDB)
//! use reinhardt_query::types::function::FunctionLanguage;
//! let mut stmt = Query::create_function();
//! stmt.name("add_numbers")
//!     .add_parameter("a", "INTEGER")
//!     .add_parameter("b", "INTEGER")
//!     .returns("INTEGER")
//!     .language(FunctionLanguage::Sql)
//!     .body("SELECT $1 + $2");
//!
//! // Create a procedure (PostgreSQL, MySQL, CockroachDB)
//! let mut stmt = Query::create_procedure();
//! stmt.name("log_event")
//!     .add_parameter("message", "text")
//!     .language(FunctionLanguage::Sql)
//!     .body("INSERT INTO event_log (message) VALUES ($1)");
//!
//! // Create a custom ENUM type (PostgreSQL, CockroachDB)
//! let mut stmt = Query::create_type();
//! stmt.name("status")
//!     .as_enum(vec!["pending".to_string(), "active".to_string(), "completed".to_string()]);
//!
//! // Create a COMPOSITE type (PostgreSQL, CockroachDB)
//! let mut stmt = Query::create_type();
//! stmt.name("address")
//!     .as_composite(vec![
//!         ("street".to_string(), "text".to_string()),
//!         ("city".to_string(), "text".to_string()),
//!     ]);
//!
//! // Create a materialized view (PostgreSQL, CockroachDB)
//! let select = Query::select()
//!     .column(Expr::col("id"))
//!     .column(Expr::col("name"))
//!     .from("users")
//!     .and_where(Expr::col("active").eq(true));
//!
//! let mut stmt = Query::create_materialized_view();
//! stmt.name("active_users").as_select(select);
//!
//! // Add a comment (PostgreSQL, CockroachDB)
//! let mut stmt = Query::comment();
//! stmt.target(CommentTarget::Table("users".into_iden()))
//!     .comment("Stores user account information");
//! ```
//!
//! ## DCL Examples
//!
//! ### Privilege Management
//!
//! ```rust
//! use reinhardt_query::prelude::*;
//!
//! // GRANT privileges
//! let grant_stmt = Query::grant()
//!     .privilege(Privilege::Select)
//!     .privilege(Privilege::Insert)
//!     .on_table("users")
//!     .to("app_user")
//!     .with_grant_option(true);
//!
//! let builder = PostgresQueryBuilder::new();
//! let (sql, values) = builder.build_grant(&grant_stmt);
//! // sql = r#"GRANT SELECT, INSERT ON TABLE "users" TO "app_user" WITH GRANT OPTION"#
//!
//! // REVOKE privileges
//! let revoke_stmt = Query::revoke()
//!     .privilege(Privilege::Insert)
//!     .from_table("users")
//!     .from("app_user")
//!     .cascade(true);
//!
//! let (sql, values) = builder.build_revoke(&revoke_stmt);
//! // sql = r#"REVOKE INSERT ON TABLE "users" FROM "app_user" CASCADE"#
//! ```
//!
//! ### Role Management
//!
//! ```rust
//! use reinhardt_query::prelude::*;
//!
//! // PostgreSQL: CREATE ROLE with attributes
//! let create_role = Query::create_role()
//!     .role("app_admin")
//!     .attribute(RoleAttribute::Login)
//!     .attribute(RoleAttribute::CreateDb)
//!     .attribute(RoleAttribute::Password("secure_password".to_string()));
//!
//! let builder = PostgresQueryBuilder::new();
//! let (sql, _) = builder.build_create_role(&create_role);
//! // sql = r#"CREATE ROLE "app_admin" WITH LOGIN CREATEDB PASSWORD $1"#
//!
//! // ALTER ROLE
//! let alter_role = Query::alter_role()
//!     .role("app_admin")
//!     .attribute(RoleAttribute::CreateRole);
//!
//! let (sql, _) = builder.build_alter_role(&alter_role);
//! // sql = r#"ALTER ROLE "app_admin" WITH CREATEROLE"#
//!
//! // DROP ROLE
//! let drop_role = Query::drop_role()
//!     .role("app_admin")
//!     .if_exists(true);
//!
//! let (sql, _) = builder.build_drop_role(&drop_role);
//! // sql = r#"DROP ROLE IF EXISTS "app_admin""#
//! ```
//!
//! ### User Management
//!
//! ```rust
//! use reinhardt_query::prelude::*;
//!
//! // MySQL: CREATE USER with options
//! let create_user = Query::create_user()
//!     .user("webapp@localhost")
//!     .if_not_exists(true)
//!     .option(UserOption::Password("webapp_pass".to_string()))
//!     .option(UserOption::AccountUnlock);
//!
//! let builder = MySqlQueryBuilder::new();
//! let (sql, _) = builder.build_create_user(&create_user);
//! // sql = r#"CREATE USER IF NOT EXISTS `webapp@localhost` IDENTIFIED BY ? ACCOUNT UNLOCK"#
//!
//! // MySQL: RENAME USER
//! let rename = Query::rename_user()
//!     .rename("old_user", "new_user");
//!
//! let (sql, _) = builder.build_rename_user(&rename);
//! // sql = r#"RENAME USER `old_user` TO `new_user`"#
//! ```
//!
//! ### Session Management
//!
//! ```rust
//! use reinhardt_query::prelude::*;
//!
//! // PostgreSQL: SET ROLE
//! let set_role = Query::set_role()
//!     .role(RoleTarget::Named("admin".to_string()));
//!
//! let builder = PostgresQueryBuilder::new();
//! let (sql, _) = builder.build_set_role(&set_role);
//! // sql = r#"SET ROLE "admin""#
//!
//! // PostgreSQL: RESET ROLE
//! let reset_role = Query::reset_role();
//!
//! let (sql, _) = builder.build_reset_role(&reset_role);
//! // sql = r#"RESET ROLE"#
//!
//! // MySQL: SET DEFAULT ROLE
//! let set_default = Query::set_default_role()
//!     .roles(DefaultRoleSpec::All)
//!     .user("webapp");
//!
//! let builder = MySqlQueryBuilder::new();
//! let (sql, _) = builder.build_set_default_role(&set_default);
//! // sql = r#"SET DEFAULT ROLE ALL TO `webapp`"#
//! ```
//!
//! ## Feature Flags
//!
//! - `thread-safe`: Use `Arc` instead of `Rc` for `DynIden` (enables thread-safe identifiers)
//! - `with-chrono`: Enable chrono date/time types in `Value`
//! - `with-uuid`: Enable UUID type in `Value`
//! - `with-json`: Enable JSON type in `Value`
//! - `with-rust_decimal`: Enable Decimal type in `Value`
//! - `with-bigdecimal`: Enable BigDecimal type in `Value`
//! - `full`: Enable all optional features

// Core modules
pub mod types;
pub mod value;

// Expression module
pub mod expr;

// Query builders
pub mod query;

// DCL (Data Control Language)
pub mod dcl;

// Backend implementations
pub mod backend;

/// Prelude module for convenient imports.
///
/// Import everything from this module to get started quickly:
///
/// ```rust
/// use reinhardt_query::prelude::*;
/// ```
pub mod prelude {
	// Backend builders
	pub use crate::backend::{
		MySqlQueryBuilder, PostgresQueryBuilder, QueryBuilder, SqlWriter, SqliteQueryBuilder,
	};
	// DCL statements
	pub use crate::dcl::{
		AlterRoleStatement, AlterUserStatement, CreateRoleStatement, CreateUserStatement,
		DefaultRoleSpec, DropRoleStatement, DropUserStatement, GrantRoleStatement, GrantStatement,
		Grantee, ObjectType, Privilege, RenameUserStatement, ResetRoleStatement,
		RevokeRoleStatement, RevokeStatement, RoleAttribute, RoleSpecification, RoleTarget,
		SetDefaultRoleStatement, SetRoleStatement, UserOption,
	};
	// Expression system
	pub use crate::expr::{
		CaseExprBuilder, CaseStatement, Cond, Condition, ConditionExpression, ConditionHolder,
		ConditionType, Expr, ExprTrait, Func, IntoCondition, Keyword, SimpleExpr,
	};
	// DML query builders
	pub use crate::query::{
		DeleteStatement, ForeignKey, ForeignKeyCreateStatement, InsertStatement, OnConflict, Query,
		QueryBuilderTrait, QueryStatementBuilder, QueryStatementWriter, SelectStatement,
		UpdateStatement,
	};
	// DDL query builders
	pub use crate::query::{
		AlterIndexStatement, AlterTableStatement, CreateIndexStatement, CreateTableStatement,
		CreateViewStatement, DropIndexStatement, DropTableStatement, DropViewStatement,
		TruncateTableStatement,
	};
	// Function/Procedure/Type DDL
	pub use crate::query::{
		AlterFunctionStatement, AlterProcedureStatement, AlterTypeStatement,
		CreateFunctionStatement, CreateProcedureStatement, CreateTypeStatement,
		DropFunctionStatement, DropProcedureStatement, DropTypeStatement,
	};
	// Type system
	pub use crate::types::{
		Alias, ColumnRef, DynIden, Iden, IdenStatic, IntoColumnRef, IntoIden, IntoTableRef, Order,
		TableRef,
	};
	// DDL types
	pub use crate::types::{BinOper, JoinType};
	pub use crate::types::{ColumnDef, ColumnType, ForeignKeyAction, IndexDef, TableConstraint};
	// Value system
	pub use crate::value::{ArrayType, IntoValue, Value, ValueTuple, Values};
	// Iden derive macro (feature-gated)
	#[cfg(feature = "derive")]
	pub use reinhardt_query_macros::Iden;
}

// Re-export commonly used types at crate root
pub use prelude::*;
