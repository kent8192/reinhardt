//! ALTER SCHEMA statement builder
//!
//! This module provides the `AlterSchemaStatement` type for building SQL ALTER SCHEMA queries.

use crate::{
	backend::QueryBuilder,
	types::{DynIden, IntoIden},
};

use crate::query::traits::{QueryBuilderTrait, QueryStatementBuilder, QueryStatementWriter};

/// ALTER SCHEMA operation types
///
/// This enum represents the different operations that can be performed on a schema.
///
/// # Examples
///
/// ```rust
/// use reinhardt_query::query::AlterSchemaOperation;
/// use reinhardt_query::types::IntoIden;
///
/// // Rename schema
/// let op = AlterSchemaOperation::RenameTo("new_schema".into_iden());
///
/// // Change owner
/// let op = AlterSchemaOperation::OwnerTo("new_owner".into_iden());
/// ```
#[derive(Debug, Clone)]
pub enum AlterSchemaOperation {
	/// RENAME TO new_name
	RenameTo(DynIden),
	/// OWNER TO new_owner
	OwnerTo(DynIden),
}

/// ALTER SCHEMA statement builder
///
/// This struct provides a fluent API for constructing ALTER SCHEMA queries.
///
/// # Examples
///
/// ```rust
/// use reinhardt_query::prelude::*;
///
/// // ALTER SCHEMA old_schema RENAME TO new_schema
/// let query = Query::alter_schema()
///     .name("old_schema")
///     .rename_to("new_schema");
///
/// // ALTER SCHEMA my_schema OWNER TO new_owner
/// let query = Query::alter_schema()
///     .name("my_schema")
///     .owner_to("new_owner");
/// ```
#[derive(Debug, Clone)]
pub struct AlterSchemaStatement {
	pub(crate) schema_name: Option<DynIden>,
	pub(crate) operation: Option<AlterSchemaOperation>,
}

impl AlterSchemaStatement {
	/// Create a new ALTER SCHEMA statement
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::alter_schema();
	/// ```
	pub fn new() -> Self {
		Self {
			schema_name: None,
			operation: None,
		}
	}

	/// Take the ownership of data in the current [`AlterSchemaStatement`]
	pub fn take(&mut self) -> Self {
		Self {
			schema_name: self.schema_name.take(),
			operation: self.operation.take(),
		}
	}

	/// Set the schema name
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::alter_schema()
	///     .name("my_schema");
	/// ```
	pub fn name<N>(&mut self, name: N) -> &mut Self
	where
		N: IntoIden,
	{
		self.schema_name = Some(name.into_iden());
		self
	}

	/// Rename the schema to a new name
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::alter_schema()
	///     .name("old_schema")
	///     .rename_to("new_schema");
	/// ```
	pub fn rename_to<N>(&mut self, new_name: N) -> &mut Self
	where
		N: IntoIden,
	{
		self.operation = Some(AlterSchemaOperation::RenameTo(new_name.into_iden()));
		self
	}

	/// Change the owner of the schema
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::alter_schema()
	///     .name("my_schema")
	///     .owner_to("new_owner");
	/// ```
	pub fn owner_to<O>(&mut self, new_owner: O) -> &mut Self
	where
		O: IntoIden,
	{
		self.operation = Some(AlterSchemaOperation::OwnerTo(new_owner.into_iden()));
		self
	}
}

impl Default for AlterSchemaStatement {
	fn default() -> Self {
		Self::new()
	}
}

impl QueryStatementBuilder for AlterSchemaStatement {
	fn build_any(&self, query_builder: &dyn QueryBuilderTrait) -> (String, crate::value::Values) {
		// Downcast to concrete QueryBuilder type
		use std::any::Any;
		if let Some(builder) =
			(query_builder as &dyn Any).downcast_ref::<crate::backend::PostgresQueryBuilder>()
		{
			return builder.build_alter_schema(self);
		}
		if let Some(builder) =
			(query_builder as &dyn Any).downcast_ref::<crate::backend::MySqlQueryBuilder>()
		{
			return builder.build_alter_schema(self);
		}
		if let Some(builder) =
			(query_builder as &dyn Any).downcast_ref::<crate::backend::SqliteQueryBuilder>()
		{
			return builder.build_alter_schema(self);
		}
		panic!("Unsupported query builder type");
	}
}

impl QueryStatementWriter for AlterSchemaStatement {}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::*;

	#[rstest]
	fn test_alter_schema_new() {
		let stmt = AlterSchemaStatement::new();
		assert!(stmt.schema_name.is_none());
		assert!(stmt.operation.is_none());
	}

	#[rstest]
	fn test_alter_schema_with_name() {
		let mut stmt = AlterSchemaStatement::new();
		stmt.name("my_schema");
		assert_eq!(stmt.schema_name.as_ref().unwrap().to_string(), "my_schema");
	}

	#[rstest]
	fn test_alter_schema_rename_to() {
		let mut stmt = AlterSchemaStatement::new();
		stmt.name("old_schema").rename_to("new_schema");
		assert_eq!(stmt.schema_name.as_ref().unwrap().to_string(), "old_schema");
		match &stmt.operation {
			Some(AlterSchemaOperation::RenameTo(name)) => {
				assert_eq!(name.to_string(), "new_schema");
			}
			_ => panic!("Expected RenameTo operation"),
		}
	}

	#[rstest]
	fn test_alter_schema_owner_to() {
		let mut stmt = AlterSchemaStatement::new();
		stmt.name("my_schema").owner_to("new_owner");
		assert_eq!(stmt.schema_name.as_ref().unwrap().to_string(), "my_schema");
		match &stmt.operation {
			Some(AlterSchemaOperation::OwnerTo(owner)) => {
				assert_eq!(owner.to_string(), "new_owner");
			}
			_ => panic!("Expected OwnerTo operation"),
		}
	}

	#[rstest]
	fn test_alter_schema_take() {
		let mut stmt = AlterSchemaStatement::new();
		stmt.name("my_schema").rename_to("new_schema");
		let taken = stmt.take();
		assert!(stmt.schema_name.is_none());
		assert!(stmt.operation.is_none());
		assert_eq!(taken.schema_name.as_ref().unwrap().to_string(), "my_schema");
		match &taken.operation {
			Some(AlterSchemaOperation::RenameTo(name)) => {
				assert_eq!(name.to_string(), "new_schema");
			}
			_ => panic!("Expected RenameTo operation"),
		}
	}
}
