//! ALTER MATERIALIZED VIEW statement builder
//!
//! This module provides the `AlterMaterializedViewStatement` type for building
//! SQL ALTER MATERIALIZED VIEW queries.

use crate::backend::QueryBuilder;
use crate::types::{DynIden, IntoIden, MaterializedViewOperation};

use crate::query::traits::{QueryBuilderTrait, QueryStatementBuilder, QueryStatementWriter};

/// ALTER MATERIALIZED VIEW statement builder
///
/// This struct provides a fluent API for constructing ALTER MATERIALIZED VIEW queries.
///
/// # Examples
///
/// ```rust,ignore
/// use reinhardt_query::prelude::*;
///
/// // Rename materialized view
/// let query = Query::alter_materialized_view()
///     .name("old_mv")
///     .rename_to("new_mv");
///
/// // Change owner
/// let query = Query::alter_materialized_view()
///     .name("my_mv")
///     .owner_to("new_owner");
/// ```
#[derive(Debug, Clone)]
pub struct AlterMaterializedViewStatement {
	pub(crate) name: Option<DynIden>,
	pub(crate) operations: Vec<MaterializedViewOperation>,
}

impl AlterMaterializedViewStatement {
	/// Create a new ALTER MATERIALIZED VIEW statement
	pub fn new() -> Self {
		Self {
			name: None,
			operations: Vec::new(),
		}
	}

	/// Take the ownership of data in the current statement
	pub fn take(&mut self) -> Self {
		Self {
			name: self.name.take(),
			operations: std::mem::take(&mut self.operations),
		}
	}

	/// Set the materialized view name
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::alter_materialized_view()
	///     .name("my_mv");
	/// ```
	pub fn name<N>(&mut self, name: N) -> &mut Self
	where
		N: IntoIden,
	{
		self.name = Some(name.into_iden());
		self
	}

	/// Add RENAME TO operation
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::alter_materialized_view()
	///     .name("old_mv")
	///     .rename_to("new_mv");
	/// ```
	pub fn rename_to<N>(&mut self, new_name: N) -> &mut Self
	where
		N: IntoIden,
	{
		self.operations
			.push(MaterializedViewOperation::Rename(new_name.into_iden()));
		self
	}

	/// Add OWNER TO operation
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::alter_materialized_view()
	///     .name("my_mv")
	///     .owner_to("new_owner");
	/// ```
	pub fn owner_to<O>(&mut self, owner: O) -> &mut Self
	where
		O: IntoIden,
	{
		self.operations
			.push(MaterializedViewOperation::OwnerTo(owner.into_iden()));
		self
	}

	/// Add SET SCHEMA operation
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::alter_materialized_view()
	///     .name("my_mv")
	///     .set_schema("new_schema");
	/// ```
	pub fn set_schema<S>(&mut self, schema: S) -> &mut Self
	where
		S: IntoIden,
	{
		self.operations
			.push(MaterializedViewOperation::SetSchema(schema.into_iden()));
		self
	}
}

impl Default for AlterMaterializedViewStatement {
	fn default() -> Self {
		Self::new()
	}
}

impl QueryStatementBuilder for AlterMaterializedViewStatement {
	fn build_any(&self, query_builder: &dyn QueryBuilderTrait) -> (String, crate::value::Values) {
		// Downcast to concrete QueryBuilder type
		use std::any::Any;
		if let Some(builder) =
			(query_builder as &dyn Any).downcast_ref::<crate::backend::PostgresQueryBuilder>()
		{
			return builder.build_alter_materialized_view(self);
		}
		if let Some(builder) =
			(query_builder as &dyn Any).downcast_ref::<crate::backend::CockroachDBQueryBuilder>()
		{
			return builder.build_alter_materialized_view(self);
		}
		if let Some(_builder) =
			(query_builder as &dyn Any).downcast_ref::<crate::backend::MySqlQueryBuilder>()
		{
			panic!("MySQL does not support materialized views");
		}
		if let Some(_builder) =
			(query_builder as &dyn Any).downcast_ref::<crate::backend::SqliteQueryBuilder>()
		{
			panic!("SQLite does not support materialized views");
		}
		panic!("Unsupported query builder type");
	}
}

impl QueryStatementWriter for AlterMaterializedViewStatement {}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::*;

	#[rstest]
	fn test_alter_materialized_view_basic() {
		let mut stmt = AlterMaterializedViewStatement::new();
		stmt.name("my_mv");
		assert_eq!(stmt.name.as_ref().unwrap().to_string(), "my_mv");
		assert!(stmt.operations.is_empty());
	}

	#[rstest]
	fn test_alter_materialized_view_rename_to() {
		let mut stmt = AlterMaterializedViewStatement::new();
		stmt.name("old_mv").rename_to("new_mv");
		assert_eq!(stmt.name.as_ref().unwrap().to_string(), "old_mv");
		assert_eq!(stmt.operations.len(), 1);
		assert!(matches!(
			&stmt.operations[0],
			MaterializedViewOperation::Rename(_)
		));
	}

	#[rstest]
	fn test_alter_materialized_view_owner_to() {
		let mut stmt = AlterMaterializedViewStatement::new();
		stmt.name("my_mv").owner_to("new_owner");
		assert_eq!(stmt.operations.len(), 1);
		assert!(matches!(
			&stmt.operations[0],
			MaterializedViewOperation::OwnerTo(_)
		));
	}

	#[rstest]
	fn test_alter_materialized_view_set_schema() {
		let mut stmt = AlterMaterializedViewStatement::new();
		stmt.name("my_mv").set_schema("new_schema");
		assert_eq!(stmt.operations.len(), 1);
		assert!(matches!(
			&stmt.operations[0],
			MaterializedViewOperation::SetSchema(_)
		));
	}

	#[rstest]
	fn test_alter_materialized_view_multiple_operations() {
		let mut stmt = AlterMaterializedViewStatement::new();
		stmt.name("my_mv").owner_to("alice").set_schema("public");
		assert_eq!(stmt.operations.len(), 2);
	}

	#[rstest]
	fn test_alter_materialized_view_default() {
		let stmt = AlterMaterializedViewStatement::default();
		assert!(stmt.name.is_none());
		assert!(stmt.operations.is_empty());
	}

	#[rstest]
	fn test_alter_materialized_view_take() {
		let mut stmt = AlterMaterializedViewStatement::new();
		stmt.name("my_mv").rename_to("new_mv");
		let taken = stmt.take();
		assert_eq!(taken.name.as_ref().unwrap().to_string(), "my_mv");
		assert_eq!(taken.operations.len(), 1);
		assert!(stmt.name.is_none());
		assert!(stmt.operations.is_empty());
	}
}
