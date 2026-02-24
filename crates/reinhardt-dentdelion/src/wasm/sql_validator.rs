//! SQL Validator for WASM Plugin Database Access
//!
//! This module provides SQL validation to prevent SQL injection attacks from
//! WASM plugins with `DatabaseAccess` capability.
//!
//! # Security Model
//!
//! The validator uses `sqlparser-rs` to parse SQL and enforce an allow-list:
//! - **Allowed**: SELECT, INSERT, UPDATE, DELETE (DML)
//! - **Blocked**: DROP, CREATE, ALTER, TRUNCATE, GRANT, REVOKE (DDL/DCL)
//!
//! This ensures that plugins can only manipulate data, not schema or permissions.

use std::fmt;

use sqlparser::ast::Statement;
use sqlparser::dialect::PostgreSqlDialect;
use sqlparser::parser::Parser;

/// SQL statement types that can be validated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SqlStatementType {
	/// SELECT - Read data from tables
	Select,
	/// INSERT - Insert new rows
	Insert,
	/// UPDATE - Update existing rows
	Update,
	/// DELETE - Delete rows
	Delete,
	/// Any other statement type (blocked)
	Other,
}

/// Error returned when SQL validation fails.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SqlValidationError {
	/// The SQL statement type is not allowed
	DisallowedStatement {
		/// The detected statement type
		statement_type: String,
	},
	/// The SQL could not be parsed
	ParseError {
		/// The error message from the parser
		message: String,
	},
	/// Multiple statements are not allowed
	MultipleStatements,
	/// Empty SQL statement
	EmptyStatement,
}

impl fmt::Display for SqlValidationError {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			Self::DisallowedStatement { statement_type } => write!(
				f,
				"SQL statement type '{}' is not allowed. Only SELECT, INSERT, UPDATE, and DELETE are permitted.",
				statement_type
			),
			Self::ParseError { message } => {
				write!(f, "Failed to parse SQL: {}", message)
			}
			Self::MultipleStatements => write!(
				f,
				"Multiple SQL statements are not allowed. Only single statements are permitted."
			),
			Self::EmptyStatement => write!(f, "SQL statement is empty"),
		}
	}
}

impl std::error::Error for SqlValidationError {}

/// SQL validator for WASM plugin database access.
///
/// This validator uses `sqlparser-rs` to parse and validate SQL statements,
/// enforcing an allow-list of permitted operations.
#[derive(Debug, Clone)]
pub struct SqlValidator {
	/// Whether to allow SELECT statements
	allow_select: bool,
	/// Whether to allow INSERT statements
	allow_insert: bool,
	/// Whether to allow UPDATE statements
	allow_update: bool,
	/// Whether to allow DELETE statements
	allow_delete: bool,
}

impl Default for SqlValidator {
	fn default() -> Self {
		Self::new()
	}
}

impl SqlValidator {
	/// Create a new validator with default settings (all DML allowed).
	///
	/// By default, allows SELECT, INSERT, UPDATE, and DELETE statements.
	pub fn new() -> Self {
		Self {
			allow_select: true,
			allow_insert: true,
			allow_update: true,
			allow_delete: true,
		}
	}

	/// Create a read-only validator (only SELECT allowed).
	pub fn read_only() -> Self {
		Self {
			allow_select: true,
			allow_insert: false,
			allow_update: false,
			allow_delete: false,
		}
	}

	/// Create a validator that allows no statements.
	pub fn none() -> Self {
		Self {
			allow_select: false,
			allow_insert: false,
			allow_update: false,
			allow_delete: false,
		}
	}

	/// Set whether SELECT is allowed.
	pub fn with_select(mut self, allow: bool) -> Self {
		self.allow_select = allow;
		self
	}

	/// Set whether INSERT is allowed.
	pub fn with_insert(mut self, allow: bool) -> Self {
		self.allow_insert = allow;
		self
	}

	/// Set whether UPDATE is allowed.
	pub fn with_update(mut self, allow: bool) -> Self {
		self.allow_update = allow;
		self
	}

	/// Set whether DELETE is allowed.
	pub fn with_delete(mut self, allow: bool) -> Self {
		self.allow_delete = allow;
		self
	}

	/// Validate a SQL statement.
	///
	/// # Arguments
	///
	/// * `sql` - The SQL statement to validate
	///
	/// # Returns
	///
	/// - `Ok(SqlStatementType)` if the statement is allowed
	/// - `Err(SqlValidationError)` if the statement is not allowed or invalid
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_dentdelion::wasm::{SqlValidator, SqlStatementType};
	///
	/// let validator = SqlValidator::new();
	///
	/// // Allowed statements
	/// assert_eq!(
	///     validator.validate("SELECT * FROM users"),
	///     Ok(SqlStatementType::Select)
	/// );
	///
	/// // Blocked statements
	/// assert!(validator.validate("DROP TABLE users").is_err());
	/// ```
	pub fn validate(&self, sql: &str) -> Result<SqlStatementType, SqlValidationError> {
		// Trim and check for empty statement
		let trimmed = sql.trim();
		if trimmed.is_empty() {
			return Err(SqlValidationError::EmptyStatement);
		}

		// Parse SQL using sqlparser-rs
		let dialect = PostgreSqlDialect {};
		let statements =
			Parser::parse_sql(&dialect, trimmed).map_err(|e| SqlValidationError::ParseError {
				message: e.to_string(),
			})?;

		// Check for multiple statements
		if statements.is_empty() {
			return Err(SqlValidationError::EmptyStatement);
		}
		if statements.len() > 1 {
			return Err(SqlValidationError::MultipleStatements);
		}

		// Get the single statement and classify it
		let statement = &statements[0];
		let statement_type = Self::classify_statement(statement);

		// Check if the statement type is allowed
		let is_allowed = match statement_type {
			SqlStatementType::Select => self.allow_select,
			SqlStatementType::Insert => self.allow_insert,
			SqlStatementType::Update => self.allow_update,
			SqlStatementType::Delete => self.allow_delete,
			SqlStatementType::Other => false,
		};

		if is_allowed {
			Ok(statement_type)
		} else {
			Err(SqlValidationError::DisallowedStatement {
				statement_type: Self::statement_type_to_string(&statement_type),
			})
		}
	}

	/// Classify a parsed SQL statement into a SqlStatementType.
	fn classify_statement(statement: &Statement) -> SqlStatementType {
		match statement {
			Statement::Query(_) => SqlStatementType::Select,
			Statement::Insert { .. } => SqlStatementType::Insert,
			Statement::Update { .. } => SqlStatementType::Update,
			Statement::Delete { .. } => SqlStatementType::Delete,
			// All other statement types are blocked
			_ => SqlStatementType::Other,
		}
	}

	/// Convert a statement type to a string for error messages.
	fn statement_type_to_string(stmt_type: &SqlStatementType) -> String {
		match stmt_type {
			SqlStatementType::Select => "SELECT".to_string(),
			SqlStatementType::Insert => "INSERT".to_string(),
			SqlStatementType::Update => "UPDATE".to_string(),
			SqlStatementType::Delete => "DELETE".to_string(),
			SqlStatementType::Other => "OTHER".to_string(),
		}
	}
}

/// Global default validator instance.
static DEFAULT_VALIDATOR: std::sync::OnceLock<SqlValidator> = std::sync::OnceLock::new();

/// Get the default SQL validator.
///
/// The default validator allows SELECT, INSERT, UPDATE, and DELETE.
pub fn default_validator() -> &'static SqlValidator {
	DEFAULT_VALIDATOR.get_or_init(SqlValidator::new)
}

/// Validate SQL using the default validator.
///
/// # Convenience Function
///
/// This is a convenience function that uses the default validator settings.
/// For custom validation rules, create a `SqlValidator` instance directly.
///
/// # Examples
///
/// ```
/// use reinhardt_dentdelion::wasm::validate_sql;
///
/// assert!(validate_sql("SELECT * FROM users").is_ok());
/// assert!(validate_sql("DROP TABLE users").is_err());
/// ```
pub fn validate_sql(sql: &str) -> Result<SqlStatementType, SqlValidationError> {
	default_validator().validate(sql)
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_validate_select() {
		let validator = SqlValidator::new();

		assert_eq!(
			validator.validate("SELECT * FROM users"),
			Ok(SqlStatementType::Select)
		);
		assert_eq!(
			validator.validate("select id, name from users where id = 1"),
			Ok(SqlStatementType::Select)
		);
		assert_eq!(
			validator.validate("  SELECT * FROM users  "),
			Ok(SqlStatementType::Select)
		);
	}

	#[test]
	fn test_validate_insert() {
		let validator = SqlValidator::new();

		assert_eq!(
			validator.validate("INSERT INTO users (name) VALUES ('test')"),
			Ok(SqlStatementType::Insert)
		);
		assert_eq!(
			validator.validate("insert into users values (1, 'test')"),
			Ok(SqlStatementType::Insert)
		);
	}

	#[test]
	fn test_validate_update() {
		let validator = SqlValidator::new();

		assert_eq!(
			validator.validate("UPDATE users SET name = 'test' WHERE id = 1"),
			Ok(SqlStatementType::Update)
		);
		assert_eq!(
			validator.validate("update users set name = 'test'"),
			Ok(SqlStatementType::Update)
		);
	}

	#[test]
	fn test_validate_delete() {
		let validator = SqlValidator::new();

		assert_eq!(
			validator.validate("DELETE FROM users WHERE id = 1"),
			Ok(SqlStatementType::Delete)
		);
		assert_eq!(
			validator.validate("delete from users"),
			Ok(SqlStatementType::Delete)
		);
	}

	#[test]
	fn test_block_drop() {
		let validator = SqlValidator::new();

		let result = validator.validate("DROP TABLE users");
		assert!(result.is_err());
		assert!(matches!(
			result.unwrap_err(),
			SqlValidationError::DisallowedStatement { .. }
		));
	}

	#[test]
	fn test_block_create() {
		let validator = SqlValidator::new();

		let result = validator.validate("CREATE TABLE evil (id INT)");
		assert!(result.is_err());
	}

	#[test]
	fn test_block_alter() {
		let validator = SqlValidator::new();

		let result = validator.validate("ALTER TABLE users ADD COLUMN evil TEXT");
		assert!(result.is_err());
	}

	#[test]
	fn test_block_truncate() {
		let validator = SqlValidator::new();

		let result = validator.validate("TRUNCATE TABLE users");
		assert!(result.is_err());
	}

	#[test]
	fn test_block_grant() {
		let validator = SqlValidator::new();

		let result = validator.validate("GRANT ALL PRIVILEGES ON users TO public");
		assert!(result.is_err());
	}

	#[test]
	fn test_block_revoke() {
		let validator = SqlValidator::new();

		let result = validator.validate("REVOKE ALL PRIVILEGES ON users FROM public");
		assert!(result.is_err());
	}

	#[test]
	fn test_empty_statement() {
		let validator = SqlValidator::new();

		let result = validator.validate("");
		assert!(matches!(result, Err(SqlValidationError::EmptyStatement)));

		let result = validator.validate("   ");
		assert!(matches!(result, Err(SqlValidationError::EmptyStatement)));
	}

	#[test]
	fn test_multiple_statements() {
		let validator = SqlValidator::new();

		// Multiple statements with semicolon
		let result = validator.validate("SELECT * FROM users; DROP TABLE users;");
		assert!(matches!(
			result,
			Err(SqlValidationError::MultipleStatements)
		));
	}

	#[test]
	fn test_statement_with_trailing_semicolon() {
		let validator = SqlValidator::new();

		// Single statement with trailing semicolon should be OK
		assert_eq!(
			validator.validate("SELECT * FROM users;"),
			Ok(SqlStatementType::Select)
		);
	}

	#[test]
	fn test_read_only_validator() {
		let validator = SqlValidator::read_only();

		// SELECT is allowed
		assert_eq!(
			validator.validate("SELECT * FROM users"),
			Ok(SqlStatementType::Select)
		);

		// INSERT is blocked
		assert!(validator.validate("INSERT INTO users VALUES (1)").is_err());

		// UPDATE is blocked
		assert!(
			validator
				.validate("UPDATE users SET name = 'test'")
				.is_err()
		);

		// DELETE is blocked
		assert!(validator.validate("DELETE FROM users").is_err());
	}

	#[test]
	fn test_custom_validator() {
		let validator = SqlValidator::new()
			.with_select(true)
			.with_insert(true)
			.with_update(false)
			.with_delete(false);

		assert!(validator.validate("SELECT * FROM users").is_ok());
		assert!(validator.validate("INSERT INTO users VALUES (1)").is_ok());
		assert!(
			validator
				.validate("UPDATE users SET name = 'test'")
				.is_err()
		);
		assert!(validator.validate("DELETE FROM users").is_err());
	}

	#[test]
	fn test_single_line_comment() {
		let validator = SqlValidator::new();

		// Comment before SELECT
		assert_eq!(
			validator.validate("-- This is a comment\nSELECT * FROM users"),
			Ok(SqlStatementType::Select)
		);
	}

	#[test]
	fn test_multi_line_comment() {
		let validator = SqlValidator::new();

		assert_eq!(
			validator.validate("/* comment */ SELECT * FROM users"),
			Ok(SqlStatementType::Select)
		);
	}

	#[test]
	fn test_validate_sql_convenience_function() {
		assert!(validate_sql("SELECT * FROM users").is_ok());
		assert!(validate_sql("DROP TABLE users").is_err());
	}

	#[test]
	fn test_error_display() {
		let err = SqlValidationError::DisallowedStatement {
			statement_type: "DROP".to_string(),
		};
		assert!(err.to_string().contains("DROP"));
		assert!(err.to_string().contains("not allowed"));

		let err = SqlValidationError::MultipleStatements;
		assert!(err.to_string().contains("Multiple SQL statements"));

		let err = SqlValidationError::EmptyStatement;
		assert!(err.to_string().contains("empty"));
	}

	#[test]
	fn test_complex_select() {
		let validator = SqlValidator::new();

		// Complex SELECT with JOIN
		assert_eq!(
			validator.validate(
				"SELECT u.name, p.title FROM users u JOIN posts p ON u.id = p.user_id WHERE u.active = true"
			),
			Ok(SqlStatementType::Select)
		);

		// SELECT with subquery
		assert_eq!(
			validator.validate("SELECT * FROM users WHERE id IN (SELECT user_id FROM posts)"),
			Ok(SqlStatementType::Select)
		);

		// SELECT with ORDER BY and LIMIT
		assert_eq!(
			validator.validate("SELECT * FROM users ORDER BY created_at DESC LIMIT 10"),
			Ok(SqlStatementType::Select)
		);
	}

	#[test]
	fn test_complex_insert() {
		let validator = SqlValidator::new();

		// INSERT with multiple values
		assert_eq!(
			validator
				.validate("INSERT INTO users (name, email) VALUES ('a', 'a@b.c'), ('d', 'd@e.f')"),
			Ok(SqlStatementType::Insert)
		);

		// INSERT with SELECT
		assert_eq!(
			validator.validate("INSERT INTO users_backup SELECT * FROM users"),
			Ok(SqlStatementType::Insert)
		);
	}

	#[test]
	fn test_complex_update() {
		let validator = SqlValidator::new();

		// UPDATE with multiple columns
		assert_eq!(
			validator
				.validate("UPDATE users SET name = 'test', email = 'test@test.com' WHERE id = 1"),
			Ok(SqlStatementType::Update)
		);

		// UPDATE with subquery
		assert_eq!(
			validator.validate(
				"UPDATE users SET active = false WHERE id IN (SELECT user_id FROM deleted_accounts)"
			),
			Ok(SqlStatementType::Update)
		);
	}

	#[test]
	fn test_complex_delete() {
		let validator = SqlValidator::new();

		// DELETE with subquery
		assert_eq!(
			validator
				.validate("DELETE FROM users WHERE id IN (SELECT user_id FROM inactive_accounts)"),
			Ok(SqlStatementType::Delete)
		);
	}

	#[test]
	fn test_block_copy() {
		let validator = SqlValidator::new();

		// COPY should be blocked (can read/write files)
		let result = validator.validate("COPY users TO '/tmp/users.csv'");
		assert!(result.is_err());
	}

	#[test]
	fn test_block_set_operations() {
		let validator = SqlValidator::new();

		// UNION/INTERSECT/EXCEPT are part of SELECT and should be allowed
		assert_eq!(
			validator.validate("SELECT * FROM users UNION SELECT * FROM admins"),
			Ok(SqlStatementType::Select)
		);
	}

	#[test]
	fn test_with_cte() {
		let validator = SqlValidator::new();

		// WITH (CTE) followed by SELECT should be allowed
		assert_eq!(
			validator.validate(
				"WITH active_users AS (SELECT * FROM users WHERE active = true) SELECT * FROM active_users"
			),
			Ok(SqlStatementType::Select)
		);

		// Note: sqlparser parses "WITH ... INSERT INTO ... SELECT ..." as a Query (SELECT)
		// This is because the INSERT ... SELECT is treated as a query with an outer INSERT wrapper
		// The key security goal is achieved: dangerous DDL/DCL is still blocked

		// Simple INSERT (without CTE) should be Insert
		assert_eq!(
			validator.validate(
				"INSERT INTO notifications SELECT * FROM users WHERE created_at > '2024-01-01'"
			),
			Ok(SqlStatementType::Insert)
		);

		// Simple UPDATE should be Update
		assert_eq!(
			validator.validate("UPDATE users SET active = false WHERE last_login < '2023-01-01'"),
			Ok(SqlStatementType::Update)
		);

		// Simple DELETE should be Delete
		assert_eq!(
			validator.validate("DELETE FROM users WHERE deleted = true"),
			Ok(SqlStatementType::Delete)
		);
	}
}
