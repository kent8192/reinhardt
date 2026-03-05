//! Materialized view type definitions
//!
//! This module provides types for materialized view-related DDL operations:
//!
//! - [`MaterializedViewDef`]: Materialized view definition for CREATE MATERIALIZED VIEW
//! - `MaterializedViewOption`: Options for ALTER MATERIALIZED VIEW operations
//!
//! Note: Materialized views are PostgreSQL and CockroachDB specific features.
//! Other databases will panic with appropriate error messages.

use crate::types::{DynIden, IntoIden};

/// Materialized view definition for CREATE MATERIALIZED VIEW
///
/// This struct represents a materialized view definition, including its name,
/// query, and various storage options.
///
/// # Examples
///
/// ```rust
/// use reinhardt_query::types::materialized_view::MaterializedViewDef;
///
/// // CREATE MATERIALIZED VIEW my_mv AS SELECT * FROM users
/// let mv = MaterializedViewDef::new("my_mv");
///
/// // CREATE MATERIALIZED VIEW my_mv AS SELECT * FROM users WITH DATA
/// let mv = MaterializedViewDef::new("my_mv")
///     .with_data(true);
///
/// // CREATE MATERIALIZED VIEW my_mv AS SELECT * FROM users WITH NO DATA
/// let mv = MaterializedViewDef::new("my_mv")
///     .with_data(false);
/// ```
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct MaterializedViewDef {
	pub(crate) name: DynIden,
	pub(crate) if_not_exists: bool,
	pub(crate) columns: Vec<DynIden>,
	pub(crate) tablespace: Option<DynIden>,
	pub(crate) with_data: Option<bool>,
}

impl MaterializedViewDef {
	/// Create a new materialized view definition
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::types::materialized_view::MaterializedViewDef;
	///
	/// let mv = MaterializedViewDef::new("my_mv");
	/// ```
	pub fn new<N: IntoIden>(name: N) -> Self {
		Self {
			name: name.into_iden(),
			if_not_exists: false,
			columns: Vec::new(),
			tablespace: None,
			with_data: None,
		}
	}

	/// Set IF NOT EXISTS clause
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::types::materialized_view::MaterializedViewDef;
	///
	/// let mv = MaterializedViewDef::new("my_mv")
	///     .if_not_exists(true);
	/// ```
	pub fn if_not_exists(mut self, if_not_exists: bool) -> Self {
		self.if_not_exists = if_not_exists;
		self
	}

	/// Set column names for the materialized view
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::types::materialized_view::MaterializedViewDef;
	///
	/// let mv = MaterializedViewDef::new("my_mv")
	///     .columns(vec!["id", "name", "email"]);
	/// ```
	pub fn columns<I, C>(mut self, cols: I) -> Self
	where
		I: IntoIterator<Item = C>,
		C: IntoIden,
	{
		for col in cols {
			self.columns.push(col.into_iden());
		}
		self
	}

	/// Set TABLESPACE for the materialized view
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::types::materialized_view::MaterializedViewDef;
	///
	/// let mv = MaterializedViewDef::new("my_mv")
	///     .tablespace("pg_default");
	/// ```
	pub fn tablespace<T: IntoIden>(mut self, tablespace: T) -> Self {
		self.tablespace = Some(tablespace.into_iden());
		self
	}

	/// Set WITH DATA or WITH NO DATA clause
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::types::materialized_view::MaterializedViewDef;
	///
	/// // WITH DATA
	/// let mv = MaterializedViewDef::new("my_mv")
	///     .with_data(true);
	///
	/// // WITH NO DATA
	/// let mv = MaterializedViewDef::new("my_mv")
	///     .with_data(false);
	/// ```
	pub fn with_data(mut self, with_data: bool) -> Self {
		self.with_data = Some(with_data);
		self
	}
}

/// Materialized view operation for ALTER MATERIALIZED VIEW statement
///
/// This enum represents the different operations that can be performed on a materialized view.
///
/// # Examples
///
/// Basic usage (typically used via [`AlterMaterializedViewStatement`](crate::query::AlterMaterializedViewStatement)):
///
/// ```rust,ignore
/// use reinhardt_query::prelude::*;
///
/// // Rename materialized view
/// let mut stmt = Query::alter_materialized_view();
/// stmt.name("old_mv").rename_to("new_mv");
///
/// // Change owner
/// let mut stmt = Query::alter_materialized_view();
/// stmt.name("my_mv").owner_to("new_owner");
/// ```
#[derive(Debug, Clone)]
pub enum MaterializedViewOperation {
	/// RENAME TO new_name
	Rename(DynIden),
	/// OWNER TO new_owner
	OwnerTo(DynIden),
	/// SET SCHEMA schema_name
	SetSchema(DynIden),
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::*;

	#[rstest]
	fn test_materialized_view_def_basic() {
		let mv = MaterializedViewDef::new("my_mv");
		assert_eq!(mv.name.to_string(), "my_mv");
		assert!(!mv.if_not_exists);
		assert!(mv.columns.is_empty());
		assert!(mv.tablespace.is_none());
		assert!(mv.with_data.is_none());
	}

	#[rstest]
	fn test_materialized_view_def_if_not_exists() {
		let mv = MaterializedViewDef::new("my_mv").if_not_exists(true);
		assert_eq!(mv.name.to_string(), "my_mv");
		assert!(mv.if_not_exists);
	}

	#[rstest]
	fn test_materialized_view_def_columns() {
		let mv = MaterializedViewDef::new("my_mv").columns(vec!["id", "name", "email"]);
		assert_eq!(mv.columns.len(), 3);
		assert_eq!(mv.columns[0].to_string(), "id");
		assert_eq!(mv.columns[1].to_string(), "name");
		assert_eq!(mv.columns[2].to_string(), "email");
	}

	#[rstest]
	fn test_materialized_view_def_tablespace() {
		let mv = MaterializedViewDef::new("my_mv").tablespace("pg_default");
		assert_eq!(mv.tablespace.as_ref().unwrap().to_string(), "pg_default");
	}

	#[rstest]
	fn test_materialized_view_def_with_data() {
		let mv = MaterializedViewDef::new("my_mv").with_data(true);
		assert_eq!(mv.with_data, Some(true));
	}

	#[rstest]
	fn test_materialized_view_def_with_no_data() {
		let mv = MaterializedViewDef::new("my_mv").with_data(false);
		assert_eq!(mv.with_data, Some(false));
	}

	#[rstest]
	fn test_materialized_view_def_all_options() {
		let mv = MaterializedViewDef::new("my_mv")
			.if_not_exists(true)
			.columns(vec!["id", "name"])
			.tablespace("pg_default")
			.with_data(true);

		assert_eq!(mv.name.to_string(), "my_mv");
		assert!(mv.if_not_exists);
		assert_eq!(mv.columns.len(), 2);
		assert_eq!(mv.tablespace.as_ref().unwrap().to_string(), "pg_default");
		assert_eq!(mv.with_data, Some(true));
	}

	#[rstest]
	fn test_materialized_view_operation_rename() {
		let op = MaterializedViewOperation::Rename("new_mv".into_iden());
		assert!(matches!(op, MaterializedViewOperation::Rename(_)));
	}

	#[rstest]
	fn test_materialized_view_operation_owner_to() {
		let op = MaterializedViewOperation::OwnerTo("new_owner".into_iden());
		assert!(matches!(op, MaterializedViewOperation::OwnerTo(_)));
	}

	#[rstest]
	fn test_materialized_view_operation_set_schema() {
		let op = MaterializedViewOperation::SetSchema("new_schema".into_iden());
		assert!(matches!(op, MaterializedViewOperation::SetSchema(_)));
	}
}
