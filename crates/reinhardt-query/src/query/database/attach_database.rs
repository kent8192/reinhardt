//! ATTACH DATABASE statement builder
//!
//! This module provides the `AttachDatabaseStatement` type for building SQL ATTACH DATABASE queries.
//! ATTACH DATABASE is a SQLite-specific feature for connecting additional database files.

use crate::types::{DynIden, IntoIden};
use crate::value::Value;

use crate::query::traits::{QueryBuilderTrait, QueryStatementBuilder, QueryStatementWriter};

/// ATTACH DATABASE statement builder (SQLite-specific)
///
/// This struct provides a fluent API for constructing ATTACH DATABASE queries.
/// ATTACH DATABASE allows attaching additional database files to the current connection.
///
/// # Examples
///
/// ```rust
/// use reinhardt_query::prelude::*;
///
/// // ATTACH DATABASE 'path/to/db.sqlite' AS auxiliary
/// let query = Query::attach_database()
///     .file_path("path/to/db.sqlite")
///     .as_name("auxiliary");
/// ```
#[derive(Debug, Clone)]
pub struct AttachDatabaseStatement {
	pub(crate) file_path: Option<String>,
	pub(crate) database_name: Option<DynIden>,
}

impl AttachDatabaseStatement {
	/// Create a new ATTACH DATABASE statement
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::attach_database();
	/// ```
	pub fn new() -> Self {
		Self {
			file_path: None,
			database_name: None,
		}
	}

	/// Take the ownership of data in the current [`AttachDatabaseStatement`]
	pub fn take(&mut self) -> Self {
		Self {
			file_path: self.file_path.take(),
			database_name: self.database_name.take(),
		}
	}

	/// Set the file path to the database file
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::attach_database()
	///     .file_path("path/to/db.sqlite");
	/// ```
	pub fn file_path<S>(&mut self, path: S) -> &mut Self
	where
		S: Into<String>,
	{
		self.file_path = Some(path.into());
		self
	}

	/// Set the alias name for the attached database
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::attach_database()
	///     .file_path("path/to/db.sqlite")
	///     .as_name("auxiliary");
	/// ```
	pub fn as_name<N>(&mut self, name: N) -> &mut Self
	where
		N: IntoIden,
	{
		self.database_name = Some(name.into_iden());
		self
	}
}

impl Default for AttachDatabaseStatement {
	fn default() -> Self {
		Self::new()
	}
}

impl QueryStatementBuilder for AttachDatabaseStatement {
	fn build_any(&self, query_builder: &dyn QueryBuilderTrait) -> (String, crate::value::Values) {
		use std::any::Any;
		if (query_builder as &dyn Any)
			.downcast_ref::<crate::backend::PostgresQueryBuilder>()
			.is_some()
		{
			unimplemented!("ATTACH DATABASE is SQLite-specific and not supported in PostgreSQL");
		}
		if (query_builder as &dyn Any)
			.downcast_ref::<crate::backend::MySqlQueryBuilder>()
			.is_some()
		{
			unimplemented!("ATTACH DATABASE is SQLite-specific and not supported in MySQL");
		}
		if let Some(sqlite_builder) =
			(query_builder as &dyn Any).downcast_ref::<crate::backend::SqliteQueryBuilder>()
		{
			use crate::backend::QueryBuilder as _;
			let file_path = self
				.file_path
				.as_deref()
				.expect("ATTACH DATABASE requires a file path");
			let db_name = self
				.database_name
				.as_ref()
				.expect("ATTACH DATABASE requires a schema name (AS clause)");
			// Reuse Value::to_sql_literal for proper string literal escaping
			let escaped_file_path =
				Value::String(Some(Box::new(file_path.to_string()))).to_sql_literal();
			// Reuse escape_identifier for proper identifier escaping
			let escaped_db_name = sqlite_builder.escape_identifier(&db_name.to_string());
			let sql = format!(
				"ATTACH DATABASE {} AS {}",
				escaped_file_path, escaped_db_name,
			);
			return (sql, crate::value::Values::new());
		}
		if (query_builder as &dyn Any)
			.downcast_ref::<crate::backend::CockroachDBQueryBuilder>()
			.is_some()
		{
			unimplemented!("ATTACH DATABASE is SQLite-specific and not supported in CockroachDB");
		}
		unreachable!("Unsupported query builder type: expected PostgresQueryBuilder, MySqlQueryBuilder, or SqliteQueryBuilder");
	}
}

impl QueryStatementWriter for AttachDatabaseStatement {}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::*;

	#[rstest]
	fn test_attach_database_new() {
		let stmt = AttachDatabaseStatement::new();
		assert!(stmt.file_path.is_none());
		assert!(stmt.database_name.is_none());
	}

	#[rstest]
	fn test_attach_database_with_file_path() {
		let mut stmt = AttachDatabaseStatement::new();
		stmt.file_path("path/to/db.sqlite");
		assert_eq!(stmt.file_path.as_ref().unwrap(), "path/to/db.sqlite");
	}

	#[rstest]
	fn test_attach_database_with_as_name() {
		let mut stmt = AttachDatabaseStatement::new();
		stmt.as_name("auxiliary");
		assert_eq!(
			stmt.database_name.as_ref().unwrap().to_string(),
			"auxiliary"
		);
	}

	#[rstest]
	fn test_attach_database_full() {
		let mut stmt = AttachDatabaseStatement::new();
		stmt.file_path("path/to/db.sqlite").as_name("auxiliary");
		assert_eq!(stmt.file_path.as_ref().unwrap(), "path/to/db.sqlite");
		assert_eq!(
			stmt.database_name.as_ref().unwrap().to_string(),
			"auxiliary"
		);
	}

	#[rstest]
	fn test_attach_database_take() {
		let mut stmt = AttachDatabaseStatement::new();
		stmt.file_path("path/to/db.sqlite").as_name("auxiliary");
		let taken = stmt.take();
		assert!(stmt.file_path.is_none());
		assert!(stmt.database_name.is_none());
		assert_eq!(taken.file_path.as_ref().unwrap(), "path/to/db.sqlite");
		assert_eq!(
			taken.database_name.as_ref().unwrap().to_string(),
			"auxiliary"
		);
	}

	#[rstest]
	fn test_attach_database_default() {
		let stmt = AttachDatabaseStatement::default();
		assert!(stmt.file_path.is_none());
		assert!(stmt.database_name.is_none());
	}

	#[rstest]
	fn test_attach_database_build_sql() {
		// Arrange
		let mut stmt = AttachDatabaseStatement::new();
		stmt.file_path("path/to/db.sqlite").as_name("auxiliary");

		// Act
		let (sql, values) = stmt.build_any(&crate::backend::SqliteQueryBuilder);

		// Assert
		assert_eq!(sql, r#"ATTACH DATABASE 'path/to/db.sqlite' AS "auxiliary""#);
		assert!(values.0.is_empty());
	}

	#[rstest]
	fn test_attach_database_file_path_with_single_quotes() {
		// Arrange
		let mut stmt = AttachDatabaseStatement::new();
		stmt.file_path("/path/to/file's.db").as_name("auxiliary");

		// Act
		let (sql, _) = stmt.build_any(&crate::backend::SqliteQueryBuilder);

		// Assert
		assert_eq!(
			sql,
			r#"ATTACH DATABASE '/path/to/file''s.db' AS "auxiliary""#
		);
	}

	#[rstest]
	fn test_attach_database_db_name_with_double_quotes() {
		// Arrange
		let mut stmt = AttachDatabaseStatement::new();
		stmt.file_path("path/to/db.sqlite").as_name(r#"my"db"#);

		// Act
		let (sql, _) = stmt.build_any(&crate::backend::SqliteQueryBuilder);

		// Assert
		assert_eq!(sql, r#"ATTACH DATABASE 'path/to/db.sqlite' AS "my""db""#);
	}

	#[rstest]
	fn test_attach_database_both_special_chars() {
		// Arrange
		let mut stmt = AttachDatabaseStatement::new();
		stmt.file_path("/tmp/user's data/test.db")
			.as_name(r#"special"name"#);

		// Act
		let (sql, _) = stmt.build_any(&crate::backend::SqliteQueryBuilder);

		// Assert
		assert_eq!(
			sql,
			r#"ATTACH DATABASE '/tmp/user''s data/test.db' AS "special""name""#
		);
	}
}
