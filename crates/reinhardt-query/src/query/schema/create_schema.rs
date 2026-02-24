//! CREATE SCHEMA statement builder
//!
//! This module provides the `CreateSchemaStatement` type for building SQL CREATE SCHEMA queries.

use crate::{
	backend::QueryBuilder,
	types::{DynIden, IntoIden},
};

use crate::query::traits::{QueryBuilderTrait, QueryStatementBuilder, QueryStatementWriter};

/// CREATE SCHEMA statement builder
///
/// This struct provides a fluent API for constructing CREATE SCHEMA queries.
///
/// # Examples
///
/// ```rust
/// use reinhardt_query::prelude::*;
///
/// // CREATE SCHEMA my_schema
/// let query = Query::create_schema()
///     .name("my_schema");
///
/// // CREATE SCHEMA IF NOT EXISTS my_schema
/// let query = Query::create_schema()
///     .name("my_schema")
///     .if_not_exists();
///
/// // CREATE SCHEMA my_schema AUTHORIZATION owner_user
/// let query = Query::create_schema()
///     .name("my_schema")
///     .authorization("owner_user");
/// ```
#[derive(Debug, Clone)]
pub struct CreateSchemaStatement {
	pub(crate) schema_name: Option<DynIden>,
	pub(crate) if_not_exists: bool,
	pub(crate) authorization: Option<DynIden>,
}

impl CreateSchemaStatement {
	/// Create a new CREATE SCHEMA statement
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::create_schema();
	/// ```
	pub fn new() -> Self {
		Self {
			schema_name: None,
			if_not_exists: false,
			authorization: None,
		}
	}

	/// Take the ownership of data in the current [`CreateSchemaStatement`]
	pub fn take(&mut self) -> Self {
		Self {
			schema_name: self.schema_name.take(),
			if_not_exists: self.if_not_exists,
			authorization: self.authorization.take(),
		}
	}

	/// Set the schema name
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::create_schema()
	///     .name("my_schema");
	/// ```
	pub fn name<N>(&mut self, name: N) -> &mut Self
	where
		N: IntoIden,
	{
		self.schema_name = Some(name.into_iden());
		self
	}

	/// Add IF NOT EXISTS clause
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::create_schema()
	///     .name("my_schema")
	///     .if_not_exists();
	/// ```
	pub fn if_not_exists(&mut self) -> &mut Self {
		self.if_not_exists = true;
		self
	}

	/// Set AUTHORIZATION owner
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::create_schema()
	///     .name("my_schema")
	///     .authorization("owner_user");
	/// ```
	pub fn authorization<O>(&mut self, owner: O) -> &mut Self
	where
		O: IntoIden,
	{
		self.authorization = Some(owner.into_iden());
		self
	}
}

impl Default for CreateSchemaStatement {
	fn default() -> Self {
		Self::new()
	}
}

impl QueryStatementBuilder for CreateSchemaStatement {
	fn build_any(&self, query_builder: &dyn QueryBuilderTrait) -> (String, crate::value::Values) {
		// Downcast to concrete QueryBuilder type
		use std::any::Any;
		if let Some(builder) =
			(query_builder as &dyn Any).downcast_ref::<crate::backend::PostgresQueryBuilder>()
		{
			return builder.build_create_schema(self);
		}
		if let Some(builder) =
			(query_builder as &dyn Any).downcast_ref::<crate::backend::MySqlQueryBuilder>()
		{
			return builder.build_create_schema(self);
		}
		if let Some(builder) =
			(query_builder as &dyn Any).downcast_ref::<crate::backend::SqliteQueryBuilder>()
		{
			return builder.build_create_schema(self);
		}
		panic!("Unsupported query builder type");
	}
}

impl QueryStatementWriter for CreateSchemaStatement {}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::*;

	#[rstest]
	fn test_create_schema_new() {
		let stmt = CreateSchemaStatement::new();
		assert!(stmt.schema_name.is_none());
		assert!(!stmt.if_not_exists);
		assert!(stmt.authorization.is_none());
	}

	#[rstest]
	fn test_create_schema_with_name() {
		let mut stmt = CreateSchemaStatement::new();
		stmt.name("my_schema");
		assert_eq!(stmt.schema_name.as_ref().unwrap().to_string(), "my_schema");
	}

	#[rstest]
	fn test_create_schema_if_not_exists() {
		let mut stmt = CreateSchemaStatement::new();
		stmt.name("my_schema").if_not_exists();
		assert!(stmt.if_not_exists);
	}

	#[rstest]
	fn test_create_schema_with_authorization() {
		let mut stmt = CreateSchemaStatement::new();
		stmt.name("my_schema").authorization("owner_user");
		assert_eq!(
			stmt.authorization.as_ref().unwrap().to_string(),
			"owner_user"
		);
	}

	#[rstest]
	fn test_create_schema_all_options() {
		let mut stmt = CreateSchemaStatement::new();
		stmt.name("my_schema")
			.if_not_exists()
			.authorization("owner_user");
		assert_eq!(stmt.schema_name.as_ref().unwrap().to_string(), "my_schema");
		assert!(stmt.if_not_exists);
		assert_eq!(
			stmt.authorization.as_ref().unwrap().to_string(),
			"owner_user"
		);
	}

	#[rstest]
	fn test_create_schema_take() {
		let mut stmt = CreateSchemaStatement::new();
		stmt.name("my_schema");
		let taken = stmt.take();
		assert!(stmt.schema_name.is_none());
		assert_eq!(taken.schema_name.as_ref().unwrap().to_string(), "my_schema");
	}
}
