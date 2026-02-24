//! # DCL (Data Control Language) Test Fixtures
//!
//! This module provides rstest fixtures for DCL integration testing.
//!
//! ## Available Fixtures
//!
//! - `dcl_test_table` - Creates a test table name for privilege testing
//! - `test_role` - Creates a test role name
//! - `test_role_with_attrs` - Creates a role name with specific attributes
//! - `test_user` - Creates a test user name
//! - `test_user_with_password` - Creates a user name with password
//! - `dcl_tracker` - Per-instance object tracker for cleanup
//! - `test_database` - Creates a test database name
//! - `test_schema` - Creates a test schema name (PostgreSQL only)
//!
//! ## Usage
//!
//! ```rust,no_run
//! use reinhardt_test::fixtures::dcl::*;
//! use rstest::rstest;
//!
//! #[rstest]
//! #[tokio::test]
//! async fn test_grant_select(
//!     dcl_test_table: String,
//!     test_role: String,
//!     mut dcl_tracker: DclTracker,
//! ) {
//!     dcl_tracker.track(format!("TABLE:{}", dcl_test_table));
//!     dcl_tracker.track(format!("ROLE:{}", test_role));
//!     // After test, dcl_tracker.cleanup_list() returns tracked objects
//! }
//! ```
//!
//! ## Migration from Global State
//!
//! The previous implementation used a global `Mutex<Vec<String>>` for tracking
//! DCL objects, which caused race conditions in parallel test execution.
//! The new design uses per-instance `DclTracker` for thread-safe tracking
//! and UUID-based naming to prevent name collisions. (Fixes #870)

use reinhardt_query::prelude::{
	Alias, ColumnDef, CreateTableStatement, ForeignKey, ForeignKeyAction, Query,
};
use uuid::Uuid;

/// Per-instance tracker for DCL objects created during a test
///
/// Replaces the previous global `Mutex<Vec<String>>` approach to eliminate
/// race conditions between parallel tests. Each test gets its own tracker
/// instance through the `dcl_tracker` rstest fixture.
///
/// ## Object Format
///
/// Each tracked entry is formatted as `<TYPE>:<name>` where TYPE is one of:
/// - TABLE
/// - ROLE
/// - USER
/// - DATABASE
/// - SCHEMA
///
/// # Example
///
/// ```rust,no_run
/// use reinhardt_test::fixtures::dcl::DclTracker;
///
/// let mut tracker = DclTracker::new();
/// tracker.track("ROLE:test_role_abc123".to_string());
/// tracker.track("TABLE:dcl_test_def456".to_string());
///
/// let objects = tracker.cleanup_list();
/// assert_eq!(objects.len(), 2);
/// ```
pub struct DclTracker {
	objects: Vec<String>,
}

impl DclTracker {
	/// Create a new empty tracker
	pub fn new() -> Self {
		Self {
			objects: Vec::new(),
		}
	}

	/// Track a DCL object for later cleanup
	pub fn track(&mut self, object_name: String) {
		self.objects.push(object_name);
	}

	/// Return all tracked objects and clear the internal list
	pub fn cleanup_list(&mut self) -> Vec<String> {
		std::mem::take(&mut self.objects)
	}
}

impl Default for DclTracker {
	fn default() -> Self {
		Self::new()
	}
}

/// rstest fixture providing a per-instance DCL object tracker
///
/// # Example
///
/// ```rust,no_run
/// use reinhardt_test::fixtures::dcl::{dcl_tracker, DclTracker};
/// use rstest::rstest;
///
/// #[rstest]
/// fn test_with_tracker(mut dcl_tracker: DclTracker) {
///     dcl_tracker.track("ROLE:my_role".to_string());
///     let cleanup = dcl_tracker.cleanup_list();
///     assert_eq!(cleanup.len(), 1);
/// }
/// ```
#[rstest::fixture]
pub fn dcl_tracker() -> DclTracker {
	DclTracker::new()
}

/// Generate a short unique suffix from UUID for naming
fn unique_suffix() -> String {
	Uuid::new_v4().simple().to_string()[..12].to_string()
}

/// Create a test table for DCL privilege testing
///
/// Returns a unique table name using UUID-based suffix.
///
/// # Table Schema
///
/// ```text
/// dcl_test_<uuid> (
///     id BIGINT PRIMARY KEY,
///     name VARCHAR(100) NOT NULL,
///     value TEXT,
///     created_at TIMESTAMP
/// )
/// ```
///
/// # Example
///
/// ```rust,no_run
/// use reinhardt_test::fixtures::dcl::dcl_test_table;
/// use rstest::rstest;
///
/// #[rstest]
/// fn test_table_operations(dcl_test_table: String) {
///     assert!(dcl_test_table.starts_with("dcl_test_"));
/// }
/// ```
pub fn dcl_test_table() -> String {
	format!("dcl_test_{}", unique_suffix())
}

/// Create a test role name
///
/// Returns a unique role name using UUID-based suffix.
///
/// # Naming Convention
///
/// Format: `test_role_<uuid>`
///
/// # Example
///
/// ```rust,no_run
/// use reinhardt_test::fixtures::dcl::test_role;
/// use rstest::rstest;
///
/// #[rstest]
/// fn test_role_creation(test_role: String) {
///     assert!(test_role.starts_with("test_role_"));
/// }
/// ```
pub fn test_role() -> String {
	format!("test_role_{}", unique_suffix())
}

/// Create a test role with specific attributes
///
/// Returns a tuple of (role_name, attributes) where attributes is a comma-separated
/// string of role attributes (e.g., "LOGIN,CREATEDB").
///
/// # Supported Attributes
///
/// - PostgreSQL: LOGIN, NOLOGIN, CREATEDB, NOCREATEDB, CREATEROLE, NOCREATEROLE,
///   SUPERUSER, NOSUPERUSER, INHERIT, NOINHERIT, REPLICATION, NOREPLICATION
/// - MySQL: (none - MySQL doesn't support role attributes)
///
/// # Example
///
/// ```rust,no_run
/// use reinhardt_test::fixtures::dcl::test_role_with_attrs;
/// use rstest::rstest;
///
/// #[rstest]
/// fn test_role_with_attributes(test_role_with_attrs: (String, String)) {
///     let (role_name, attrs) = test_role_with_attrs;
///     assert!(role_name.starts_with("test_role_attrs_"));
///     assert!(attrs.contains("LOGIN"));
/// }
/// ```
pub fn test_role_with_attrs() -> (String, String) {
	let role_name = format!("test_role_attrs_{}", unique_suffix());
	let attributes = "LOGIN,CREATEDB".to_string();
	(role_name, attributes)
}

/// Create a test user name
///
/// Returns a unique user name using UUID-based suffix.
///
/// # Naming Convention
///
/// Format: `test_user_<uuid>`
///
/// # Example
///
/// ```rust,no_run
/// use reinhardt_test::fixtures::dcl::test_user;
/// use rstest::rstest;
///
/// #[rstest]
/// fn test_user_creation(test_user: String) {
///     assert!(test_user.starts_with("test_user_"));
/// }
/// ```
pub fn test_user() -> String {
	format!("test_user_{}", unique_suffix())
}

/// Create a test user with password
///
/// Returns a tuple of (username, password).
///
/// # Password
///
/// The password is auto-generated and unique for each test.
///
/// # Security Note
///
/// **WARNING**: These passwords are for testing only. Never use in production.
///
/// # Example
///
/// ```rust,no_run
/// use reinhardt_test::fixtures::dcl::test_user_with_password;
/// use rstest::rstest;
///
/// #[rstest]
/// fn test_user_auth(test_user_with_password: (String, String)) {
///     let (username, password) = test_user_with_password;
///     assert!(username.starts_with("test_user_pass_"));
///     assert!(!password.is_empty());
/// }
/// ```
pub fn test_user_with_password() -> (String, String) {
	let suffix = unique_suffix();
	let user_name = format!("test_user_pass_{}", suffix);
	let password = format!("test_password_{}", suffix);
	(user_name, password)
}

/// Create a test database name
///
/// Returns a unique database name using UUID-based suffix.
///
/// # Naming Convention
///
/// Format: `test_db_<uuid>`
///
/// # Example
///
/// ```rust,no_run
/// use reinhardt_test::fixtures::dcl::test_database;
/// use rstest::rstest;
///
/// #[rstest]
/// fn test_database_creation(test_database: String) {
///     assert!(test_database.starts_with("test_db_"));
/// }
/// ```
pub fn test_database() -> String {
	format!("test_db_{}", unique_suffix())
}

/// Create a test schema name (PostgreSQL only)
///
/// Returns a unique schema name using UUID-based suffix.
///
/// # Naming Convention
///
/// Format: `test_schema_<uuid>`
///
/// # Database Support
///
/// - PostgreSQL: Supported
/// - MySQL: Not supported (use CREATE DATABASE instead)
/// - SQLite: Not supported
///
/// # Example
///
/// ```rust,no_run
/// use reinhardt_test::fixtures::dcl::test_schema;
/// use rstest::rstest;
///
/// #[rstest]
/// fn test_schema_creation(test_schema: String) {
///     assert!(test_schema.starts_with("test_schema_"));
/// }
/// ```
pub fn test_schema() -> String {
	format!("test_schema_{}", unique_suffix())
}

/// Generate reinhardt-query `CreateTableStatement` for DCL test table
///
/// Returns a table creation statement that can be built into SQL for any backend.
///
/// # Schema
///
/// ```text
/// dcl_test_<uuid> (
///     id BIGINT PRIMARY KEY,
///     name VARCHAR(100) NOT NULL,
///     value TEXT,
///     created_at TIMESTAMP
/// )
/// ```
///
/// # Example
///
/// ```rust,no_run
/// use reinhardt_test::fixtures::dcl::dcl_test_table_stmt;
/// use reinhardt_query::prelude::{PostgresQueryBuilder, MySqlQueryBuilder, QueryStatementBuilder};
///
/// #[test]
/// fn test_table_sql_generation() {
///     let stmt = dcl_test_table_stmt();
///
///     let sql = stmt.to_string(PostgresQueryBuilder::new());
///     assert!(sql.contains("CREATE TABLE"));
///
///     let sql = stmt.to_string(MySqlQueryBuilder::new());
///     assert!(sql.contains("CREATE TABLE"));
/// }
/// ```
pub fn dcl_test_table_stmt() -> CreateTableStatement {
	let table_name = dcl_test_table();

	let mut stmt = Query::create_table();
	stmt.table(Alias::new(&table_name))
		.col(
			ColumnDef::new(Alias::new("id"))
				.big_integer()
				.not_null(true)
				.primary_key(true),
		)
		.col(
			ColumnDef::new(Alias::new("name"))
				.string_len(100)
				.not_null(true),
		)
		.col(ColumnDef::new(Alias::new("value")).text())
		.col(ColumnDef::new(Alias::new("created_at")).timestamp());
	stmt.take()
}

/// Generate reinhardt-query `CreateTableStatement` for DCL test table with foreign key
///
/// Returns a table creation statement with a foreign key constraint for testing
/// privilege management on related tables.
///
/// # Schema
///
/// ```text
/// dcl_test_parent_<uuid> (
///     id BIGINT PRIMARY KEY,
///     name VARCHAR(100) NOT NULL
/// )
///
/// dcl_test_child_<uuid> (
///     id BIGINT PRIMARY KEY,
///     parent_id BIGINT NOT NULL,
///     value TEXT,
///     FOREIGN KEY (parent_id) REFERENCES dcl_test_parent(id) ON DELETE CASCADE
/// )
/// ```
///
/// # Returns
///
/// A tuple of (parent_table_stmt, child_table_stmt, parent_name, child_name)
///
/// # Example
///
/// ```rust,no_run
/// use reinhardt_test::fixtures::dcl::dcl_test_table_with_fk;
/// use reinhardt_query::prelude::{PostgresQueryBuilder, QueryStatementBuilder};
///
/// #[test]
/// fn test_foreign_key_table() {
///     let (parent_stmt, child_stmt, parent_name, child_name) = dcl_test_table_with_fk();
///
///     let sql = child_stmt.to_string(PostgresQueryBuilder::new());
///     assert!(sql.contains("FOREIGN KEY"));
/// }
/// ```
pub fn dcl_test_table_with_fk() -> (CreateTableStatement, CreateTableStatement, String, String) {
	// Use same suffix for parent and child to make the relationship clear
	let suffix = unique_suffix();
	let parent_name = format!("dcl_test_parent_{}", suffix);
	let child_name = format!("dcl_test_child_{}", suffix);

	let mut parent_stmt = Query::create_table();
	parent_stmt
		.table(Alias::new(&parent_name))
		.col(
			ColumnDef::new(Alias::new("id"))
				.big_integer()
				.not_null(true)
				.primary_key(true),
		)
		.col(
			ColumnDef::new(Alias::new("name"))
				.string_len(100)
				.not_null(true),
		);

	let mut fk = ForeignKey::create();
	fk.name(Alias::new(format!("fk_{}_parent", child_name)))
		.from_tbl(Alias::new(&child_name))
		.from_col(Alias::new("parent_id"))
		.to_tbl(Alias::new(&parent_name))
		.to_col(Alias::new("id"))
		.on_delete(ForeignKeyAction::Cascade)
		.on_update(ForeignKeyAction::Cascade);

	let mut child_stmt = Query::create_table();
	child_stmt
		.table(Alias::new(&child_name))
		.col(
			ColumnDef::new(Alias::new("id"))
				.big_integer()
				.not_null(true)
				.primary_key(true),
		)
		.col(
			ColumnDef::new(Alias::new("parent_id"))
				.big_integer()
				.not_null(true),
		)
		.col(ColumnDef::new(Alias::new("value")).text())
		.foreign_key_from_builder(&mut fk);

	(
		parent_stmt.take(),
		child_stmt.take(),
		parent_name,
		child_name,
	)
}

#[cfg(test)]
mod tests {
	use super::*;
	use reinhardt_query::prelude::{
		MySqlQueryBuilder, PostgresQueryBuilder, QueryStatementBuilder,
	};
	use rstest::rstest;

	#[rstest]
	fn test_dcl_test_table_format() {
		// Arrange & Act
		let table = dcl_test_table();

		// Assert
		assert!(table.starts_with("dcl_test_"));
	}

	#[rstest]
	fn test_dcl_test_table_uniqueness() {
		// Arrange & Act
		let table1 = dcl_test_table();
		let table2 = dcl_test_table();

		// Assert
		assert_ne!(table1, table2, "Each call must generate a unique name");
	}

	#[rstest]
	fn test_test_role_format() {
		// Arrange & Act
		let role = test_role();

		// Assert
		assert!(role.starts_with("test_role_"));
	}

	#[rstest]
	fn test_test_role_with_attrs_format() {
		// Arrange & Act
		let (role, attrs) = test_role_with_attrs();

		// Assert
		assert!(role.starts_with("test_role_attrs_"));
		assert_eq!(attrs, "LOGIN,CREATEDB");
	}

	#[rstest]
	fn test_test_user_format() {
		// Arrange & Act
		let user = test_user();

		// Assert
		assert!(user.starts_with("test_user_"));
	}

	#[rstest]
	fn test_test_user_with_password_format() {
		// Arrange & Act
		let (user, pass) = test_user_with_password();

		// Assert
		assert!(user.starts_with("test_user_pass_"));
		assert!(pass.starts_with("test_password_"));
	}

	#[rstest]
	fn test_test_database_format() {
		// Arrange & Act
		let db = test_database();

		// Assert
		assert!(db.starts_with("test_db_"));
	}

	#[rstest]
	fn test_test_schema_format() {
		// Arrange & Act
		let schema = test_schema();

		// Assert
		assert!(schema.starts_with("test_schema_"));
	}

	#[rstest]
	fn test_dcl_tracker_tracks_objects() {
		// Arrange
		let mut tracker = DclTracker::new();
		let role = test_role();
		let user = test_user();
		let table = dcl_test_table();

		// Act
		tracker.track(format!("ROLE:{}", role));
		tracker.track(format!("USER:{}", user));
		tracker.track(format!("TABLE:{}", table));

		let cleanup = tracker.cleanup_list();

		// Assert
		assert_eq!(cleanup.len(), 3);
		assert!(cleanup.iter().any(|o| o.starts_with("ROLE:")));
		assert!(cleanup.iter().any(|o| o.starts_with("USER:")));
		assert!(cleanup.iter().any(|o| o.starts_with("TABLE:")));
	}

	#[rstest]
	fn test_dcl_tracker_clears_after_cleanup() {
		// Arrange
		let mut tracker = DclTracker::new();
		tracker.track(format!("ROLE:{}", test_role()));
		tracker.track(format!("USER:{}", test_user()));

		// Act
		let cleanup1 = tracker.cleanup_list();
		let cleanup2 = tracker.cleanup_list();

		// Assert
		assert_eq!(cleanup1.len(), 2);
		assert_eq!(cleanup2.len(), 0);
	}

	#[rstest]
	fn test_dcl_test_table_stmt_generates_valid_sql() {
		// Arrange & Act
		let stmt = dcl_test_table_stmt();

		// Assert
		assert!(
			stmt.to_string(PostgresQueryBuilder::new())
				.contains("CREATE TABLE")
		);
		assert!(
			stmt.to_string(MySqlQueryBuilder::new())
				.contains("CREATE TABLE")
		);
	}

	#[rstest]
	fn test_dcl_test_table_with_fk_format() {
		// Arrange & Act
		let (parent_stmt, child_stmt, parent_name, child_name) = dcl_test_table_with_fk();

		// Assert
		assert!(parent_name.starts_with("dcl_test_parent_"));
		assert!(child_name.starts_with("dcl_test_child_"));

		let parent_sql = parent_stmt.to_string(PostgresQueryBuilder::new());
		assert!(parent_sql.contains("CREATE TABLE"));
		assert!(parent_sql.contains(&parent_name));

		let child_sql = child_stmt.to_string(PostgresQueryBuilder::new());
		assert!(child_sql.contains("CREATE TABLE"));
		assert!(child_sql.contains(&child_name));
		assert!(child_sql.contains("FOREIGN KEY"));
	}

	#[rstest]
	fn test_dcl_test_table_with_fk_uses_same_suffix() {
		// Arrange & Act
		let (_parent_stmt, _child_stmt, parent_name, child_name) = dcl_test_table_with_fk();

		// Assert - parent and child share the same suffix
		let parent_suffix = parent_name.strip_prefix("dcl_test_parent_").unwrap();
		let child_suffix = child_name.strip_prefix("dcl_test_child_").unwrap();
		assert_eq!(parent_suffix, child_suffix);
	}
}
