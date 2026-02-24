//! DROP FUNCTION statement builder
//!
//! This module provides the `DropFunctionStatement` type for building SQL DROP FUNCTION queries.

use crate::{
	backend::QueryBuilder,
	types::{DynIden, IntoIden, function::FunctionParameter},
};

use crate::query::traits::{QueryBuilderTrait, QueryStatementBuilder, QueryStatementWriter};

/// DROP FUNCTION statement builder
///
/// This struct provides a fluent API for constructing DROP FUNCTION queries.
///
/// # Examples
///
/// ```rust
/// use reinhardt_query::prelude::*;
///
/// // DROP FUNCTION my_func
/// let query = Query::drop_function()
///     .name("my_func");
///
/// // DROP FUNCTION IF EXISTS my_func(integer) CASCADE
/// let query = Query::drop_function()
///     .name("my_func")
///     .if_exists()
///     .add_parameter("", "integer")
///     .cascade();
/// ```
#[derive(Debug, Clone)]
pub struct DropFunctionStatement {
	pub(crate) name: Option<DynIden>,
	pub(crate) parameters: Vec<FunctionParameter>,
	pub(crate) if_exists: bool,
	pub(crate) cascade: bool,
}

impl DropFunctionStatement {
	/// Create a new DROP FUNCTION statement
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::drop_function();
	/// ```
	pub fn new() -> Self {
		Self {
			name: None,
			parameters: Vec::new(),
			if_exists: false,
			cascade: false,
		}
	}

	/// Take the ownership of data in the current [`DropFunctionStatement`]
	pub fn take(&mut self) -> Self {
		let taken = Self {
			name: self.name.take(),
			parameters: self.parameters.clone(),
			if_exists: self.if_exists,
			cascade: self.cascade,
		};
		// Reset self to empty state
		self.name = None;
		self.parameters.clear();
		self.if_exists = false;
		self.cascade = false;
		taken
	}

	/// Set the function name
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::drop_function()
	///     .name("my_func");
	/// ```
	pub fn name<N>(&mut self, name: N) -> &mut Self
	where
		N: IntoIden,
	{
		self.name = Some(name.into_iden());
		self
	}

	/// Add a function parameter (for identifying which overloaded function to drop)
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::prelude::*;
	///
	/// // DROP FUNCTION my_func(integer, text)
	/// let query = Query::drop_function()
	///     .name("my_func")
	///     .add_parameter("", "integer")
	///     .add_parameter("", "text");
	/// ```
	pub fn add_parameter<N: IntoIden, T: Into<String>>(
		&mut self,
		name: N,
		param_type: T,
	) -> &mut Self {
		self.parameters.push(FunctionParameter {
			name: Some(name.into_iden()),
			param_type: Some(param_type.into()),
			mode: None,
			default_value: None,
		});
		self
	}

	/// Add IF EXISTS clause
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::drop_function()
	///     .name("my_func")
	///     .if_exists();
	/// ```
	pub fn if_exists(&mut self) -> &mut Self {
		self.if_exists = true;
		self
	}

	/// Add CASCADE clause
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::drop_function()
	///     .name("my_func")
	///     .cascade();
	/// ```
	pub fn cascade(&mut self) -> &mut Self {
		self.cascade = true;
		self
	}
}

impl Default for DropFunctionStatement {
	fn default() -> Self {
		Self::new()
	}
}

impl QueryStatementBuilder for DropFunctionStatement {
	fn build_any(&self, query_builder: &dyn QueryBuilderTrait) -> (String, crate::value::Values) {
		// Downcast to concrete QueryBuilder type
		use std::any::Any;
		if let Some(builder) =
			(query_builder as &dyn Any).downcast_ref::<crate::backend::PostgresQueryBuilder>()
		{
			return builder.build_drop_function(self);
		}
		if let Some(builder) =
			(query_builder as &dyn Any).downcast_ref::<crate::backend::MySqlQueryBuilder>()
		{
			return builder.build_drop_function(self);
		}
		if let Some(builder) =
			(query_builder as &dyn Any).downcast_ref::<crate::backend::SqliteQueryBuilder>()
		{
			return builder.build_drop_function(self);
		}
		if let Some(builder) =
			(query_builder as &dyn Any).downcast_ref::<crate::backend::CockroachDBQueryBuilder>()
		{
			return builder.build_drop_function(self);
		}
		panic!("Unsupported query builder type");
	}
}

impl QueryStatementWriter for DropFunctionStatement {}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::*;

	#[rstest]
	fn test_drop_function_new() {
		let stmt = DropFunctionStatement::new();
		assert!(stmt.name.is_none());
		assert!(stmt.parameters.is_empty());
		assert!(!stmt.if_exists);
		assert!(!stmt.cascade);
	}

	#[rstest]
	fn test_drop_function_with_name() {
		let mut stmt = DropFunctionStatement::new();
		stmt.name("my_func");
		assert_eq!(stmt.name.as_ref().unwrap().to_string(), "my_func");
	}

	#[rstest]
	fn test_drop_function_if_exists() {
		let mut stmt = DropFunctionStatement::new();
		stmt.name("my_func").if_exists();
		assert!(stmt.if_exists);
	}

	#[rstest]
	fn test_drop_function_cascade() {
		let mut stmt = DropFunctionStatement::new();
		stmt.name("my_func").cascade();
		assert!(stmt.cascade);
	}

	#[rstest]
	fn test_drop_function_with_parameters() {
		let mut stmt = DropFunctionStatement::new();
		stmt.name("my_func")
			.add_parameter("", "integer")
			.add_parameter("", "text");
		assert_eq!(stmt.parameters.len(), 2);
		assert_eq!(stmt.parameters[0].param_type.as_ref().unwrap(), "integer");
		assert_eq!(stmt.parameters[1].param_type.as_ref().unwrap(), "text");
	}

	#[rstest]
	fn test_drop_function_all_options() {
		let mut stmt = DropFunctionStatement::new();
		stmt.name("my_func")
			.if_exists()
			.add_parameter("", "integer")
			.cascade();
		assert_eq!(stmt.name.as_ref().unwrap().to_string(), "my_func");
		assert!(stmt.if_exists);
		assert!(stmt.cascade);
		assert_eq!(stmt.parameters.len(), 1);
	}

	#[rstest]
	fn test_drop_function_take() {
		let mut stmt = DropFunctionStatement::new();
		stmt.name("my_func").cascade();
		let taken = stmt.take();
		assert!(stmt.name.is_none());
		assert!(!stmt.cascade);
		assert_eq!(taken.name.as_ref().unwrap().to_string(), "my_func");
		assert!(taken.cascade);
	}
}
