//! Query statement builders
//!
//! This module provides builders for SQL query statements (SELECT, INSERT, UPDATE, DELETE)
//! and DDL statements (CREATE/ALTER/DROP for tables, indexes, views, schemas, sequences, databases).
//!
//! # DML Usage
//!
//! - Query Select: [`SelectStatement`]
//! - Query Insert: [`InsertStatement`]
//! - Query Update: [`UpdateStatement`]
//! - Query Delete: [`DeleteStatement`]
//!
//! # DDL Usage - Table Operations
//!
//! - Create Table: [`CreateTableStatement`]
//! - Alter Table: [`AlterTableStatement`]
//! - Drop Table: [`DropTableStatement`]
//!
//! # DDL Usage - Index Operations
//!
//! - Create Index: [`CreateIndexStatement`]
//! - Alter Index: [`AlterIndexStatement`]
//! - Drop Index: [`DropIndexStatement`]
//! - Reindex: [`ReindexStatement`]
//!
//! # DDL Usage - View Operations
//!
//! - Create View: [`CreateViewStatement`]
//! - Drop View: [`DropViewStatement`]
//!
//! # DDL Usage - Schema Operations
//!
//! - Create Schema: [`CreateSchemaStatement`]
//! - Alter Schema: [`AlterSchemaStatement`]
//! - Drop Schema: [`DropSchemaStatement`]
//!
//! # DDL Usage - Sequence Operations
//!
//! - Create Sequence: [`CreateSequenceStatement`]
//! - Alter Sequence: [`AlterSequenceStatement`]
//! - Drop Sequence: [`DropSequenceStatement`]
//!
//! # DDL Usage - Database Operations
//!
//! - Create Database: [`CreateDatabaseStatement`]
//! - Alter Database: [`AlterDatabaseStatement`]
//! - Drop Database: [`DropDatabaseStatement`]
//!
//! # DDL Usage - Comment Operations
//!
//! - Comment: [`CommentStatement`]
//!
//! # Examples
//!
//! ```rust,ignore
//! use reinhardt_query::prelude::*;
//!
//! // SELECT query
//! let select_query = Query::select()
//!     .column(Expr::col("name"))
//!     .from("users")
//!     .and_where(Expr::col("active").eq(true));
//!
//! // INSERT query
//! let insert_query = Query::insert()
//!     .into_table("users")
//!     .columns(["name", "email"])
//!     .values_panic(["Alice", "alice@example.com"]);
//!
//! // UPDATE query
//! let update_query = Query::update()
//!     .table("users")
//!     .value("active", false)
//!     .and_where(Expr::col("id").eq(1));
//!
//! // DELETE query
//! let delete_query = Query::delete()
//!     .from_table("users")
//!     .and_where(Expr::col("active").eq(false));
//! ```

pub mod alter_index;
pub mod alter_table;
pub mod comment;
pub mod create_index;
pub mod create_table;
pub mod create_trigger;
pub mod create_view;
pub mod database;
pub mod delete;
pub mod drop_index;
pub mod drop_table;
pub mod drop_trigger;
pub mod drop_view;
pub mod event;
pub mod foreign_key;
pub mod function;
pub mod insert;
pub mod maintenance;
pub mod materialized_view;
pub mod on_conflict;
pub mod procedure;
pub mod reindex;
pub mod returning;
pub mod schema;
pub mod select;
pub mod sequence;
pub mod traits;
pub mod truncate_table;
pub mod type_def;
pub mod update;

// Re-export all public types from submodules
pub use alter_index::AlterIndexStatement;
pub use alter_table::{AlterTableOperation, AlterTableStatement};
pub use comment::CommentStatement;
pub use create_index::{CreateIndexStatement, IndexColumn, IndexMethod};
pub use create_table::CreateTableStatement;
pub use create_trigger::CreateTriggerStatement;
pub use create_view::CreateViewStatement;
pub use database::{
	AlterDatabaseStatement, AttachDatabaseStatement, CreateDatabaseStatement,
	DetachDatabaseStatement, DropDatabaseStatement,
};
pub use delete::DeleteStatement;
pub use drop_index::DropIndexStatement;
pub use drop_table::DropTableStatement;
pub use drop_trigger::DropTriggerStatement;
pub use drop_view::DropViewStatement;
pub use event::{AlterEventStatement, CreateEventStatement, DropEventStatement};
pub use foreign_key::{ForeignKey, ForeignKeyCreateStatement};
pub use function::{AlterFunctionStatement, CreateFunctionStatement, DropFunctionStatement};
pub use insert::{InsertSource, InsertStatement};
pub use maintenance::{
	AnalyzeStatement, CheckTableStatement, OptimizeTableStatement, RepairTableStatement,
	VacuumStatement,
};
pub use materialized_view::{
	AlterMaterializedViewStatement, CreateMaterializedViewStatement, DropMaterializedViewStatement,
	RefreshMaterializedViewStatement,
};
pub use on_conflict::{OnConflict, OnConflictAction, OnConflictTarget};
pub use procedure::{AlterProcedureStatement, CreateProcedureStatement, DropProcedureStatement};
pub use reindex::{ReindexStatement, ReindexTarget};
pub use returning::ReturningClause;
pub use schema::{
	AlterSchemaOperation, AlterSchemaStatement, CreateSchemaStatement, DropSchemaStatement,
};
pub use select::{
	CommonTableExpr, LockClause, SelectDistinct, SelectExpr, SelectStatement, UnionType,
};
pub use sequence::{AlterSequenceStatement, CreateSequenceStatement, DropSequenceStatement};
pub use traits::{QueryBuilderTrait, QueryStatementBuilder, QueryStatementWriter};
pub use truncate_table::TruncateTableStatement;
pub use type_def::{AlterTypeStatement, CreateTypeStatement, DropTypeStatement};
pub use update::UpdateStatement;

/// Shorthand for constructing any table query
///
/// This struct provides static constructor methods for creating query statements.
///
/// # Examples
///
/// ```rust,ignore
/// use reinhardt_query::prelude::*;
///
/// // Create a SELECT statement
/// let select = Query::select()
///     .column(Expr::col("id"))
///     .column(Expr::col("name"))
///     .from("users");
///
/// // Create an INSERT statement
/// let insert = Query::insert()
///     .into_table("users")
///     .columns(["name", "email"])
///     .values(["Alice"]);
///
/// // Create an UPDATE statement
/// let update = Query::update()
///     .table("users")
///     .value("name", "Bob")
///     .and_where(Expr::col("id").eq(1));
///
/// // Create a DELETE statement
/// let delete = Query::delete()
///     .from_table("users")
///     .and_where(Expr::col("id").eq(1));
/// ```
#[derive(Debug, Clone)]
pub struct Query;

impl Query {
	/// Construct a new [`SelectStatement`]
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::select()
	///     .column(Expr::col("id"))
	///     .column(Expr::col("name"))
	///     .from("users");
	/// ```
	pub fn select() -> SelectStatement {
		SelectStatement::new()
	}

	/// Construct a new [`InsertStatement`]
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::insert()
	///     .into_table("users")
	///     .columns(["name", "email"])
	///     .values(["Alice"]);
	/// ```
	pub fn insert() -> InsertStatement {
		InsertStatement::new()
	}

	/// Construct a new [`UpdateStatement`]
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::update()
	///     .table("users")
	///     .value("name", "Bob")
	///     .and_where(Expr::col("id").eq(1));
	/// ```
	pub fn update() -> UpdateStatement {
		UpdateStatement::new()
	}

	/// Construct a new [`DeleteStatement`]
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::delete()
	///     .from_table("users")
	///     .and_where(Expr::col("id").eq(1));
	/// ```
	pub fn delete() -> DeleteStatement {
		DeleteStatement::new()
	}

	/// Construct a new [`CreateTableStatement`]
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	/// use reinhardt_query::types::ddl::{ColumnDef, ColumnType};
	///
	/// let query = Query::create_table()
	///     .table("users")
	///     .if_not_exists()
	///     .col(
	///         ColumnDef::new("id")
	///             .column_type(ColumnType::Integer)
	///             .primary_key(true)
	///     );
	/// ```
	pub fn create_table() -> CreateTableStatement {
		CreateTableStatement::new()
	}

	/// Construct a new [`AlterTableStatement`]
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	/// use reinhardt_query::types::ddl::{ColumnDef, ColumnType};
	///
	/// let query = Query::alter_table()
	///     .table("users")
	///     .add_column(
	///         ColumnDef::new("age")
	///             .column_type(ColumnType::Integer)
	///     );
	/// ```
	pub fn alter_table() -> AlterTableStatement {
		AlterTableStatement::new()
	}

	/// Construct a new [`DropTableStatement`]
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::drop_table()
	///     .table("users")
	///     .if_exists();
	/// ```
	pub fn drop_table() -> DropTableStatement {
		DropTableStatement::new()
	}

	/// Construct a new [`CreateIndexStatement`]
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::create_index()
	///     .name("idx_email")
	///     .table("users")
	///     .col("email")
	///     .unique();
	/// ```
	pub fn create_index() -> CreateIndexStatement {
		CreateIndexStatement::new()
	}

	/// Construct a new [`DropIndexStatement`]
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::drop_index()
	///     .name("idx_email")
	///     .table("users")
	///     .if_exists();
	/// ```
	pub fn drop_index() -> DropIndexStatement {
		DropIndexStatement::new()
	}

	/// Construct a new [`CreateViewStatement`]
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// let select = Query::select()
	///     .column(Expr::col("name"))
	///     .from("users");
	///
	/// let query = Query::create_view()
	///     .name("user_names")
	///     .as_select(select)
	///     .if_not_exists();
	/// ```
	pub fn create_view() -> CreateViewStatement {
		CreateViewStatement::new()
	}

	/// Construct a new [`DropViewStatement`]
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::drop_view()
	///     .name("user_names")
	///     .if_exists();
	/// ```
	pub fn drop_view() -> DropViewStatement {
		DropViewStatement::new()
	}

	/// Construct a new [`TruncateTableStatement`]
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::truncate_table()
	///     .table("users");
	/// ```
	pub fn truncate_table() -> TruncateTableStatement {
		TruncateTableStatement::new()
	}

	/// Construct a new [`CreateTriggerStatement`]
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	/// use reinhardt_query::types::{TriggerTiming, TriggerEvent, TriggerScope};
	///
	/// let query = Query::create_trigger()
	///     .name("audit_insert")
	///     .timing(TriggerTiming::After)
	///     .event(TriggerEvent::Insert)
	///     .on_table("users")
	///     .for_each(TriggerScope::Row)
	///     .execute_function("audit_log_insert");
	/// ```
	pub fn create_trigger() -> CreateTriggerStatement {
		CreateTriggerStatement::new()
	}

	/// Construct a new [`DropTriggerStatement`]
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::drop_trigger()
	///     .name("audit_insert")
	///     .on_table("users")
	///     .if_exists();
	/// ```
	pub fn drop_trigger() -> DropTriggerStatement {
		DropTriggerStatement::new()
	}

	/// Construct a new [`AlterIndexStatement`]
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// // PostgreSQL: Rename index
	/// let query = Query::alter_index()
	///     .name("idx_users_email")
	///     .rename_to("idx_users_email_new");
	///
	/// // MySQL: Rename index (requires table name)
	/// let query = Query::alter_index()
	///     .table("users")
	///     .name("idx_email")
	///     .rename_to("idx_email_new");
	/// ```
	pub fn alter_index() -> AlterIndexStatement {
		AlterIndexStatement::new()
	}

	/// Construct a new [`ReindexStatement`]
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// // Reindex a specific index
	/// let query = Query::reindex()
	///     .index("idx_users_email");
	///
	/// // Reindex all indexes in a table
	/// let query = Query::reindex()
	///     .table("users");
	/// ```
	pub fn reindex() -> ReindexStatement {
		ReindexStatement::new()
	}

	/// Construct a new [`CreateSchemaStatement`]
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// // CREATE SCHEMA my_schema
	/// let query = Query::create_schema()
	///     .name("my_schema");
	///
	/// // CREATE SCHEMA IF NOT EXISTS my_schema AUTHORIZATION owner_user
	/// let query = Query::create_schema()
	///     .name("my_schema")
	///     .if_not_exists()
	///     .authorization("owner_user");
	/// ```
	pub fn create_schema() -> CreateSchemaStatement {
		CreateSchemaStatement::new()
	}

	/// Construct a new [`AlterSchemaStatement`]
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// // ALTER SCHEMA old_name RENAME TO new_name
	/// let query = Query::alter_schema()
	///     .name("old_name")
	///     .rename_to("new_name");
	///
	/// // ALTER SCHEMA my_schema OWNER TO new_owner
	/// let query = Query::alter_schema()
	///     .name("my_schema")
	///     .owner_to("new_owner");
	/// ```
	pub fn alter_schema() -> AlterSchemaStatement {
		AlterSchemaStatement::new()
	}

	/// Construct a new [`DropSchemaStatement`]
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// // DROP SCHEMA my_schema
	/// let query = Query::drop_schema()
	///     .name("my_schema");
	///
	/// // DROP SCHEMA IF EXISTS my_schema CASCADE
	/// let query = Query::drop_schema()
	///     .name("my_schema")
	///     .if_exists()
	///     .cascade();
	/// ```
	pub fn drop_schema() -> DropSchemaStatement {
		DropSchemaStatement::new()
	}

	/// Construct a new [`CreateSequenceStatement`]
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// // CREATE SEQUENCE user_id_seq
	/// let query = Query::create_sequence()
	///     .name("user_id_seq");
	///
	/// // CREATE SEQUENCE user_id_seq START WITH 1000 INCREMENT BY 1
	/// let query = Query::create_sequence()
	///     .name("user_id_seq")
	///     .start_with(1000)
	///     .increment_by(1);
	/// ```
	pub fn create_sequence() -> CreateSequenceStatement {
		CreateSequenceStatement::new()
	}

	/// Construct a new [`AlterSequenceStatement`]
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// // ALTER SEQUENCE user_id_seq RESTART WITH 2000
	/// let query = Query::alter_sequence()
	///     .name("user_id_seq")
	///     .restart_with(2000);
	///
	/// // ALTER SEQUENCE user_id_seq RENAME TO new_user_id_seq
	/// let query = Query::alter_sequence()
	///     .name("user_id_seq")
	///     .rename_to("new_user_id_seq");
	/// ```
	pub fn alter_sequence() -> AlterSequenceStatement {
		AlterSequenceStatement::new()
	}

	/// Construct a new [`DropSequenceStatement`]
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// // DROP SEQUENCE user_id_seq
	/// let query = Query::drop_sequence()
	///     .name("user_id_seq");
	///
	/// // DROP SEQUENCE IF EXISTS user_id_seq CASCADE
	/// let query = Query::drop_sequence()
	///     .name("user_id_seq")
	///     .if_exists()
	///     .cascade();
	/// ```
	pub fn drop_sequence() -> DropSequenceStatement {
		DropSequenceStatement::new()
	}

	/// Construct a new [`CommentStatement`]
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	/// use reinhardt_query::types::CommentTarget;
	///
	/// // COMMENT ON TABLE "users" IS 'User account information'
	/// let query = Query::comment()
	///     .target(CommentTarget::Table("users".into_iden()))
	///     .comment("User account information");
	///
	/// // COMMENT ON COLUMN "users"."email" IS 'User email address'
	/// let query = Query::comment()
	///     .target(CommentTarget::Column("users".into_iden(), "email".into_iden()))
	///     .comment("User email address");
	/// ```
	pub fn comment() -> CommentStatement {
		CommentStatement::new()
	}

	/// Construct a new [`AlterDatabaseStatement`]
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// // ALTER DATABASE mydb RENAME TO newdb
	/// let query = Query::alter_database()
	///     .name("mydb")
	///     .rename_to("newdb");
	///
	/// // CockroachDB: ALTER DATABASE mydb ADD REGION 'us-west-1'
	/// let query = Query::alter_database()
	///     .name("mydb")
	///     .add_region("us-west-1");
	/// ```
	pub fn alter_database() -> AlterDatabaseStatement {
		AlterDatabaseStatement::new()
	}

	/// Construct a new [`CreateDatabaseStatement`]
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::create_database()
	///     .name("testdb")
	/// ```
	pub fn create_database() -> CreateDatabaseStatement {
		CreateDatabaseStatement::new()
	}

	/// Construct a new [`DropDatabaseStatement`]
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// // DROP DATABASE mydb
	/// let query = Query::drop_database()
	///     .name("mydb");
	/// ```
	pub fn drop_database() -> DropDatabaseStatement {
		DropDatabaseStatement::new()
	}

	/// Construct a new [`OptimizeTableStatement`]
	///
	/// **MySQL-only feature**: This statement is specific to MySQL.
	/// Other backends will panic with a helpful message.
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// // OPTIMIZE TABLE users
	/// let query = Query::optimize_table()
	///     .table("users");
	///
	/// // OPTIMIZE TABLE users, posts
	/// let query = Query::optimize_table()
	///     .table("users")
	///     .table("posts");
	/// ```
	pub fn optimize_table() -> OptimizeTableStatement {
		OptimizeTableStatement::new()
	}

	/// Construct a new [`RepairTableStatement`]
	///
	/// **MySQL-only feature**: This statement is specific to MySQL.
	/// Other backends will panic with a helpful message.
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// // REPAIR TABLE users
	/// let query = Query::repair_table()
	///     .table("users");
	///
	/// // REPAIR TABLE QUICK users
	/// let query = Query::repair_table()
	///     .table("users")
	///     .quick();
	/// ```
	pub fn repair_table() -> RepairTableStatement {
		RepairTableStatement::new()
	}

	/// Construct a new [`CheckTableStatement`]
	///
	/// **MySQL-only feature**: This statement is specific to MySQL.
	/// Other backends will panic with a helpful message.
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	/// use reinhardt_query::types::CheckTableOption;
	///
	/// // CHECK TABLE users
	/// let query = Query::check_table()
	///     .table("users");
	///
	/// // CHECK TABLE users QUICK
	/// let query = Query::check_table()
	///     .table("users")
	///     .option(CheckTableOption::Quick);
	/// ```
	pub fn check_table() -> CheckTableStatement {
		CheckTableStatement::new()
	}

	/// Construct a new [`CreateEventStatement`]
	///
	/// **MySQL-only feature**: This statement is specific to MySQL.
	/// Other backends will panic with a helpful message.
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	/// use reinhardt_query::types::{TriggerTiming, TriggerEvent, TriggerScope};
	///
	/// // CREATE EVENT cleanup_logs
	/// let query = Query::create_event()
	///     .name("cleanup_logs")
	///     .on_schedule_every("1 DAY")
	///     .do_body("DELETE FROM logs WHERE created_at < NOW() - INTERVAL 7 DAY");
	/// ```
	pub fn create_event() -> CreateEventStatement {
		CreateEventStatement::new()
	}

	/// Construct a new [`AlterEventStatement`]
	///
	/// **MySQL-only feature**: This statement is specific to MySQL.
	/// Other backends will panic with a helpful message.
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// // ALTER EVENT cleanup_logs RENAME TO purge_old_logs
	/// let query = Query::alter_event()
	///     .name("cleanup_logs")
	///     .rename_to("purge_old_logs");
	/// ```
	pub fn alter_event() -> AlterEventStatement {
		AlterEventStatement::new()
	}

	/// Construct a new [`DropEventStatement`]
	///
	/// **MySQL-only feature**: This statement is specific to MySQL.
	/// Other backends will panic with a helpful message.
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// // DROP EVENT cleanup_logs
	/// let query = Query::drop_event()
	///     .name("cleanup_logs")
	///     .if_exists();
	/// ```
	pub fn drop_event() -> DropEventStatement {
		DropEventStatement::new()
	}

	/// Construct a new [`CreateFunctionStatement`]
	///
	/// **Backend support**: PostgreSQL, MySQL, CockroachDB only.
	/// SQLite will panic with a helpful message.
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	/// use reinhardt_query::types::function::FunctionLanguage;
	///
	/// // Basic SQL function:
	///
	/// let query = Query::create_function()
	///     .name("add_one")
	///     .add_parameter("x", "integer")
	///     .returns("integer")
	///     .language(FunctionLanguage::Sql)
	///     .body("SELECT $1 + 1");
	/// ```
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	/// use reinhardt_query::types::function::*;
	///
	/// // PL/pgSQL function with OR REPLACE and options:
	///
	/// let query = Query::create_function()
	///     .name("calculate_total")
	///     .or_replace()
	///     .add_parameter("price", "numeric")
	///     .add_parameter("quantity", "integer")
	///     .returns("numeric")
	///     .language(FunctionLanguage::PlPgSql)
	///     .behavior(FunctionBehavior::Immutable)
	///     .security(FunctionSecurity::Definer)
	///     .body("BEGIN RETURN price * quantity; END;");
	/// ```
	pub fn create_function() -> CreateFunctionStatement {
		CreateFunctionStatement::new()
	}

	/// Construct a new [`AlterFunctionStatement`]
	///
	/// **Backend support**: PostgreSQL, MySQL, CockroachDB only.
	/// SQLite will panic with a helpful message.
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// // Rename function:
	///
	/// let query = Query::alter_function()
	///     .name("old_func")
	///     .rename_to("new_func");
	/// ```
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// // Change function owner:
	///
	/// let query = Query::alter_function()
	///     .name("my_func")
	///     .owner_to("new_owner");
	/// ```
	pub fn alter_function() -> AlterFunctionStatement {
		AlterFunctionStatement::new()
	}

	/// Construct a new [`DropFunctionStatement`]
	///
	/// **Backend support**: PostgreSQL, MySQL, CockroachDB only.
	/// SQLite will panic with a helpful message.
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// // Simple DROP FUNCTION:
	///
	/// let query = Query::drop_function()
	///     .name("my_func");
	/// ```
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// // DROP FUNCTION with IF EXISTS and CASCADE:
	///
	/// let query = Query::drop_function()
	///     .name("my_func")
	///     .if_exists()
	///     .cascade();
	/// ```
	pub fn drop_function() -> DropFunctionStatement {
		DropFunctionStatement::new()
	}

	/// Construct a new [`CreateProcedureStatement`]
	///
	/// **Backend support**: PostgreSQL, MySQL, CockroachDB only.
	/// SQLite will panic with a helpful message.
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	/// use reinhardt_query::types::function::FunctionLanguage;
	///
	/// // Basic SQL procedure:
	///
	/// let query = Query::create_procedure()
	///     .name("log_event")
	///     .add_parameter("message", "text")
	///     .language(FunctionLanguage::Sql)
	///     .body("INSERT INTO event_log (message) VALUES ($1)");
	/// ```
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	/// use reinhardt_query::types::function::*;
	///
	/// // PL/pgSQL procedure with OR REPLACE and options:
	///
	/// let query = Query::create_procedure()
	///     .name("update_inventory")
	///     .or_replace()
	///     .add_parameter("product_id", "integer")
	///     .add_parameter("quantity", "integer")
	///     .language(FunctionLanguage::PlPgSql)
	///     .behavior(FunctionBehavior::Volatile)
	///     .security(FunctionSecurity::Invoker)
	///     .body("BEGIN UPDATE inventory SET stock = stock - quantity WHERE id = product_id; END;");
	/// ```
	pub fn create_procedure() -> CreateProcedureStatement {
		CreateProcedureStatement::new()
	}

	/// Construct a new [`AlterProcedureStatement`]
	///
	/// **Backend support**: PostgreSQL, MySQL, CockroachDB only.
	/// SQLite will panic with a helpful message.
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// // Rename procedure:
	///
	/// let query = Query::alter_procedure()
	///     .name("old_proc")
	///     .rename_to("new_proc");
	/// ```
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// // Change procedure owner:
	///
	/// let query = Query::alter_procedure()
	///     .name("my_proc")
	///     .owner_to("new_owner");
	/// ```
	pub fn alter_procedure() -> AlterProcedureStatement {
		AlterProcedureStatement::new()
	}

	/// Construct a new [`DropProcedureStatement`]
	///
	/// **Backend support**: PostgreSQL, MySQL, CockroachDB only.
	/// SQLite will panic with a helpful message.
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// // Simple DROP PROCEDURE:
	///
	/// let query = Query::drop_procedure()
	///     .name("my_proc");
	/// ```
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// // DROP PROCEDURE with IF EXISTS and CASCADE:
	///
	/// let query = Query::drop_procedure()
	///     .name("my_proc")
	///     .if_exists()
	///     .cascade();
	/// ```
	pub fn drop_procedure() -> DropProcedureStatement {
		DropProcedureStatement::new()
	}

	/// Construct a new [`CreateTypeStatement`]
	///
	/// **Backend support**: PostgreSQL, CockroachDB only.
	/// MySQL and SQLite will panic with a helpful message.
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// // Create ENUM type:
	///
	/// let query = Query::create_type()
	///     .name("mood")
	///     .as_enum(vec!["happy".to_string(), "sad".to_string(), "neutral".to_string()]);
	/// ```
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// // Create COMPOSITE type:
	///
	/// let query = Query::create_type()
	///     .name("address")
	///     .as_composite(vec![
	///         ("street".to_string(), "text".to_string()),
	///         ("city".to_string(), "varchar(10)".to_string()),
	///         ("zip".to_string(), "varchar(10)".to_string()),
	///     ]);
	/// ```
	pub fn create_type() -> CreateTypeStatement {
		CreateTypeStatement::new()
	}

	/// Construct a new [`AlterTypeStatement`]
	///
	/// **Backend support**: PostgreSQL, CockroachDB only.
	/// MySQL and SQLite will panic with a helpful message.
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// // Add value to ENUM type:
	///
	/// let query = Query::alter_type()
	///     .name("mood")
	///     .add_value("excited", Some("happy"));
	/// ```
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// // Rename type:
	///
	/// let query = Query::alter_type()
	///     .name("old_type")
	///     .rename_to("new_type");
	/// ```
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// // Rename ENUM value:
	///
	/// let query = Query::alter_type()
	///     .name("mood")
	///     .rename_value("happy", "joyful");
	/// ```
	pub fn alter_type() -> AlterTypeStatement {
		AlterTypeStatement::new()
	}

	/// Construct a new [`DropTypeStatement`]
	///
	/// **Backend support**: PostgreSQL, CockroachDB only.
	/// MySQL and SQLite will panic with a helpful message.
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// // Simple DROP TYPE:
	///
	/// let query = Query::drop_type()
	///     .name("mood");
	/// ```
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// // DROP TYPE with IF EXISTS and CASCADE:
	///
	/// let query = Query::drop_type()
	///     .name("mood")
	///     .if_exists()
	///     .cascade();
	/// ```
	pub fn drop_type() -> DropTypeStatement {
		DropTypeStatement::new()
	}

	/// Construct a new [`VacuumStatement`]
	///
	/// **Backend support**: PostgreSQL, SQLite, CockroachDB only.
	/// MySQL will panic with a helpful message (use `Query::optimize_table()` instead).
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// // VACUUM (all tables)
	/// let query = Query::vacuum();
	///
	/// // VACUUM users
	/// let query = Query::vacuum()
	///     .table("users");
	///
	/// // VACUUM FULL ANALYZE users
	/// let query = Query::vacuum()
	///     .table("users")
	///     .full()
	///     .analyze();
	/// ```
	pub fn vacuum() -> VacuumStatement {
		VacuumStatement::new()
	}

	/// Construct a new [`AnalyzeStatement`]
	///
	/// **Backend support**: PostgreSQL, MySQL, SQLite, CockroachDB.
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// // ANALYZE (all tables)
	/// let query = Query::analyze();
	///
	/// // ANALYZE users
	/// let query = Query::analyze()
	///     .table("users");
	///
	/// // ANALYZE VERBOSE users
	/// let query = Query::analyze()
	///     .table("users")
	///     .verbose();
	///
	/// // ANALYZE users (email, name)
	/// let query = Query::analyze()
	///     .table_columns("users", ["email", "name"]);
	/// ```
	pub fn analyze() -> AnalyzeStatement {
		AnalyzeStatement::new()
	}

	/// Construct a new [`CreateMaterializedViewStatement`]
	///
	/// **Backend support**: PostgreSQL, CockroachDB only.
	/// MySQL and SQLite will panic with a helpful message.
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// let select = Query::select()
	///     .column(Expr::col("id"))
	///     .column(Expr::col("name"))
	///     .from("users")
	///     .and_where(Expr::col("active").eq(true));
	///
	/// let query = Query::create_materialized_view()
	///     .name("active_users")
	///     .as_select(select)
	///     .if_not_exists();
	/// ```
	pub fn create_materialized_view() -> CreateMaterializedViewStatement {
		CreateMaterializedViewStatement::new()
	}

	/// Construct a new [`AlterMaterializedViewStatement`]
	///
	/// **Backend support**: PostgreSQL, CockroachDB only.
	/// MySQL and SQLite will panic with a helpful message.
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// // ALTER MATERIALIZED VIEW old_mv RENAME TO new_mv
	/// let query = Query::alter_materialized_view()
	///     .name("old_mv")
	///     .rename_to("new_mv");
	///
	/// // ALTER MATERIALIZED VIEW my_mv OWNER TO new_owner
	/// let query = Query::alter_materialized_view()
	///     .name("my_mv")
	///     .owner_to("new_owner");
	/// ```
	pub fn alter_materialized_view() -> AlterMaterializedViewStatement {
		AlterMaterializedViewStatement::new()
	}

	/// Construct a new [`DropMaterializedViewStatement`]
	///
	/// **Backend support**: PostgreSQL, CockroachDB only.
	/// MySQL and SQLite will panic with a helpful message.
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// // DROP MATERIALIZED VIEW my_mv
	/// let query = Query::drop_materialized_view()
	///     .name("my_mv");
	///
	/// // DROP MATERIALIZED VIEW IF EXISTS my_mv CASCADE
	/// let query = Query::drop_materialized_view()
	///     .name("my_mv")
	///     .if_exists()
	///     .cascade();
	/// ```
	pub fn drop_materialized_view() -> DropMaterializedViewStatement {
		DropMaterializedViewStatement::new()
	}

	/// Construct a new [`RefreshMaterializedViewStatement`]
	///
	/// **Backend support**: PostgreSQL, CockroachDB only.
	/// MySQL and SQLite will panic with a helpful message.
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// // REFRESH MATERIALIZED VIEW my_mv
	/// let query = Query::refresh_materialized_view()
	///     .name("my_mv");
	///
	/// // REFRESH MATERIALIZED VIEW CONCURRENTLY my_mv
	/// let query = Query::refresh_materialized_view()
	///     .name("my_mv")
	///     .concurrently();
	///
	/// // REFRESH MATERIALIZED VIEW my_mv WITH NO DATA
	/// let query = Query::refresh_materialized_view()
	///     .name("my_mv")
	///     .with_data(false);
	/// ```
	pub fn refresh_materialized_view() -> RefreshMaterializedViewStatement {
		RefreshMaterializedViewStatement::new()
	}

	/// Construct a new [`AttachDatabaseStatement`]
	///
	/// **SQLite-only feature**: This statement is specific to SQLite.
	/// Other backends will panic with a helpful message.
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// // ATTACH DATABASE 'other.db' AS other_db
	///
	/// let query = Query::attach_database()
	///     .file_path("other.db")
	///     .schema_name("other_db");
	/// ```
	pub fn attach_database() -> AttachDatabaseStatement {
		AttachDatabaseStatement::new()
	}

	/// Construct a new [`DetachDatabaseStatement`]
	///
	/// **SQLite-only feature**: This statement is specific to SQLite.
	/// Other backends will panic with a helpful message.
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// // DETACH DATABASE other_db
	///
	/// let query = Query::detach_database()
	///     .schema_name("other_db");
	/// ```
	pub fn detach_database() -> DetachDatabaseStatement {
		DetachDatabaseStatement::new()
	}

	/// Construct a new GRANT statement
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	/// use reinhardt_query::dcl::{GrantStatement, Privilege};
	///
	/// let grant = Query::grant()
	///     .privilege(Privilege::Select)
	///     .on_table("users")
	///     .to("app_user");
	/// ```
	pub fn grant() -> crate::dcl::GrantStatement {
		crate::dcl::GrantStatement::new()
	}

	/// Construct a new REVOKE statement
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	/// use reinhardt_query::dcl::{RevokeStatement, Privilege};
	///
	/// let revoke = Query::revoke()
	///     .privilege(Privilege::Insert)
	///     .from_table("users")
	///     .from("app_user");
	/// ```
	pub fn revoke() -> crate::dcl::RevokeStatement {
		crate::dcl::RevokeStatement::new()
	}

	/// Construct a new CREATE ROLE statement
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	/// use reinhardt_query::dcl::{CreateRoleStatement, RoleAttribute};
	///
	/// let create_role = Query::create_role()
	///     .role("developer")
	///     .attribute(RoleAttribute::Login)
	///     .attribute(RoleAttribute::Password("secret".to_string()));
	/// ```
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	/// use reinhardt_query::dcl::*;
	///
	/// // MySQL CREATE ROLE:
	///
	/// let query = Query::create_role()
	///     .role("app_role")
	///     .if_not_exists(true)
	///     .option(UserOption::Comment("Application role".to_string()));
	/// ```
	pub fn create_role() -> crate::dcl::CreateRoleStatement {
		crate::dcl::CreateRoleStatement::new()
	}

	/// Construct a new DROP ROLE statement
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	/// use reinhardt_query::dcl::DropRoleStatement;
	///
	/// let drop_role = Query::drop_role()
	///     .role("old_role")
	///     .if_exists(true);
	/// ```
	pub fn drop_role() -> crate::dcl::DropRoleStatement {
		crate::dcl::DropRoleStatement::new()
	}

	/// Construct a new ALTER ROLE statement
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	/// use reinhardt_query::dcl::{AlterRoleStatement, RoleAttribute};
	///
	/// // PostgreSQL:
	///
	/// let query = Query::alter_role()
	///     .role("developer")
	///     .attribute(RoleAttribute::NoLogin);
	/// ```
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	/// use reinhardt_query::dcl::*;
	///
	/// // MySQL:
	///
	/// let query = Query::alter_role()
	///     .role("app_role")
	///     .option(UserOption::AccountLock);
	/// ```
	pub fn alter_role() -> crate::dcl::AlterRoleStatement {
		crate::dcl::AlterRoleStatement::new()
	}

	/// Construct a new SET ROLE statement (PostgreSQL only)
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::set_role()
	///     .role_name("developer")
	/// ```
	pub fn set_role() -> crate::dcl::SetRoleStatement {
		crate::dcl::SetRoleStatement::new()
	}

	/// Construct a new RESET ROLE statement (PostgreSQL only)
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::reset_role()
	///     .role_name("developer")
	/// ```
	pub fn reset_role() -> crate::dcl::ResetRoleStatement {
		crate::dcl::ResetRoleStatement::new()
	}

	/// Construct a new SET DEFAULT ROLE statement (MySQL only)
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::set_default_role()
	///     .role_name("developer")
	/// ```
	pub fn set_default_role() -> crate::dcl::SetDefaultRoleStatement {
		crate::dcl::SetDefaultRoleStatement::new()
	}

	/// Construct a new CREATE USER statement
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::query::Query;
	///
	/// let stmt = Query::create_user();
	/// ```
	pub fn create_user() -> crate::dcl::CreateUserStatement {
		crate::dcl::CreateUserStatement::new()
	}

	/// Construct a new DROP USER statement
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::query::Query;
	///
	/// let stmt = Query::drop_user();
	/// ```
	pub fn drop_user() -> crate::dcl::DropUserStatement {
		crate::dcl::DropUserStatement::new()
	}

	/// Construct a new ALTER USER statement
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::query::Query;
	///
	/// let stmt = Query::alter_user();
	/// ```
	pub fn alter_user() -> crate::dcl::AlterUserStatement {
		crate::dcl::AlterUserStatement::new()
	}

	/// Construct a new RENAME USER statement (MySQL only)
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::query::Query;
	///
	/// let stmt = Query::rename_user();
	/// ```
	pub fn rename_user() -> crate::dcl::RenameUserStatement {
		crate::dcl::RenameUserStatement::new()
	}
}
