//! ALTER FUNCTION statement builder
//!
//! This module provides the `AlterFunctionStatement` type for building SQL ALTER FUNCTION queries.

use crate::{
	backend::QueryBuilder,
	types::{
		DynIden, IntoIden,
		function::{FunctionBehavior, FunctionParameter, FunctionSecurity},
	},
};

use crate::query::traits::{QueryBuilderTrait, QueryStatementBuilder, QueryStatementWriter};

/// ALTER FUNCTION statement builder
///
///This struct provides a fluent API for constructing ALTER FUNCTION queries.
///
/// # Examples
///
/// ```rust
/// use reinhardt_query::prelude::*;
///
/// // ALTER FUNCTION my_func RENAME TO new_func
/// let query = Query::alter_function()
///     .name("my_func")
///     .rename_to("new_func");
///
/// // ALTER FUNCTION my_func OWNER TO new_owner
/// let query = Query::alter_function()
///     .name("my_func")
///     .owner_to("new_owner");
/// ```
#[derive(Debug, Clone)]
pub struct AlterFunctionStatement {
	pub(crate) name: Option<DynIden>,
	pub(crate) parameters: Vec<FunctionParameter>,
	pub(crate) operation: Option<AlterFunctionOperation>,
}

/// ALTER FUNCTION operation types
#[derive(Debug, Clone)]
pub enum AlterFunctionOperation {
	/// RENAME TO new_name
	RenameTo(DynIden),
	/// OWNER TO new_owner
	OwnerTo(DynIden),
	/// SET SCHEMA new_schema
	SetSchema(DynIden),
	/// Change behavior (IMMUTABLE/STABLE/VOLATILE)
	SetBehavior(FunctionBehavior),
	/// Change security (DEFINER/INVOKER)
	SetSecurity(FunctionSecurity),
}

impl AlterFunctionStatement {
	/// Create a new ALTER FUNCTION statement
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::alter_function();
	/// ```
	pub fn new() -> Self {
		Self {
			name: None,
			parameters: Vec::new(),
			operation: None,
		}
	}

	/// Take the ownership of data in the current [`AlterFunctionStatement`]
	pub fn take(&mut self) -> Self {
		let taken = Self {
			name: self.name.take(),
			parameters: self.parameters.clone(),
			operation: self.operation.take(),
		};
		// Reset self to empty state
		self.name = None;
		self.parameters.clear();
		self.operation = None;
		taken
	}

	/// Set the function name
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::alter_function()
	///     .name("my_func");
	/// ```
	pub fn name<N>(&mut self, name: N) -> &mut Self
	where
		N: IntoIden,
	{
		self.name = Some(name.into_iden());
		self
	}

	/// Add a function parameter (for identifying which overloaded function to alter)
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::alter_function()
	///     .name("my_func")
	///     .add_parameter("a", "integer")
	///     .rename_to("new_func");
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

	/// RENAME TO new_name
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::alter_function()
	///     .name("my_func")
	///     .rename_to("new_func");
	/// ```
	pub fn rename_to<N: IntoIden>(&mut self, new_name: N) -> &mut Self {
		self.operation = Some(AlterFunctionOperation::RenameTo(new_name.into_iden()));
		self
	}

	/// OWNER TO new_owner
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::alter_function()
	///     .name("my_func")
	///     .owner_to("new_owner");
	/// ```
	pub fn owner_to<N: IntoIden>(&mut self, new_owner: N) -> &mut Self {
		self.operation = Some(AlterFunctionOperation::OwnerTo(new_owner.into_iden()));
		self
	}

	/// SET SCHEMA new_schema
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::alter_function()
	///     .name("my_func")
	///     .set_schema("new_schema");
	/// ```
	pub fn set_schema<N: IntoIden>(&mut self, new_schema: N) -> &mut Self {
		self.operation = Some(AlterFunctionOperation::SetSchema(new_schema.into_iden()));
		self
	}

	/// Set behavior (IMMUTABLE/STABLE/VOLATILE)
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::prelude::*;
	/// use reinhardt_query::types::function::FunctionBehavior;
	///
	/// let query = Query::alter_function()
	///     .name("my_func")
	///     .set_behavior(FunctionBehavior::Immutable);
	/// ```
	pub fn set_behavior(&mut self, behavior: FunctionBehavior) -> &mut Self {
		self.operation = Some(AlterFunctionOperation::SetBehavior(behavior));
		self
	}

	/// Set security (DEFINER/INVOKER)
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::prelude::*;
	/// use reinhardt_query::types::function::FunctionSecurity;
	///
	/// let query = Query::alter_function()
	///     .name("my_func")
	///     .set_security(FunctionSecurity::Definer);
	/// ```
	pub fn set_security(&mut self, security: FunctionSecurity) -> &mut Self {
		self.operation = Some(AlterFunctionOperation::SetSecurity(security));
		self
	}
}

impl Default for AlterFunctionStatement {
	fn default() -> Self {
		Self::new()
	}
}

impl QueryStatementBuilder for AlterFunctionStatement {
	fn build_any(&self, query_builder: &dyn QueryBuilderTrait) -> (String, crate::value::Values) {
		// Downcast to concrete QueryBuilder type
		use std::any::Any;
		if let Some(builder) =
			(query_builder as &dyn Any).downcast_ref::<crate::backend::PostgresQueryBuilder>()
		{
			return builder.build_alter_function(self);
		}
		if let Some(builder) =
			(query_builder as &dyn Any).downcast_ref::<crate::backend::MySqlQueryBuilder>()
		{
			return builder.build_alter_function(self);
		}
		if let Some(builder) =
			(query_builder as &dyn Any).downcast_ref::<crate::backend::SqliteQueryBuilder>()
		{
			return builder.build_alter_function(self);
		}
		if let Some(builder) =
			(query_builder as &dyn Any).downcast_ref::<crate::backend::CockroachDBQueryBuilder>()
		{
			return builder.build_alter_function(self);
		}
		panic!("Unsupported query builder type");
	}
}

impl QueryStatementWriter for AlterFunctionStatement {}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::*;

	#[rstest]
	fn test_alter_function_new() {
		let stmt = AlterFunctionStatement::new();
		assert!(stmt.name.is_none());
		assert!(stmt.parameters.is_empty());
		assert!(stmt.operation.is_none());
	}

	#[rstest]
	fn test_alter_function_with_name() {
		let mut stmt = AlterFunctionStatement::new();
		stmt.name("my_func");
		assert_eq!(stmt.name.as_ref().unwrap().to_string(), "my_func");
	}

	#[rstest]
	fn test_alter_function_rename_to() {
		let mut stmt = AlterFunctionStatement::new();
		stmt.name("my_func").rename_to("new_func");
		assert_eq!(stmt.name.as_ref().unwrap().to_string(), "my_func");
		assert!(matches!(
			stmt.operation,
			Some(AlterFunctionOperation::RenameTo(_))
		));
	}

	#[rstest]
	fn test_alter_function_owner_to() {
		let mut stmt = AlterFunctionStatement::new();
		stmt.name("my_func").owner_to("new_owner");
		assert!(matches!(
			stmt.operation,
			Some(AlterFunctionOperation::OwnerTo(_))
		));
	}

	#[rstest]
	fn test_alter_function_set_schema() {
		let mut stmt = AlterFunctionStatement::new();
		stmt.name("my_func").set_schema("new_schema");
		assert!(matches!(
			stmt.operation,
			Some(AlterFunctionOperation::SetSchema(_))
		));
	}

	#[rstest]
	fn test_alter_function_set_behavior() {
		use crate::types::function::FunctionBehavior;

		let mut stmt = AlterFunctionStatement::new();
		stmt.name("my_func")
			.set_behavior(FunctionBehavior::Immutable);
		assert!(matches!(
			stmt.operation,
			Some(AlterFunctionOperation::SetBehavior(
				FunctionBehavior::Immutable
			))
		));
	}

	#[rstest]
	fn test_alter_function_set_security() {
		use crate::types::function::FunctionSecurity;

		let mut stmt = AlterFunctionStatement::new();
		stmt.name("my_func").set_security(FunctionSecurity::Definer);
		assert!(matches!(
			stmt.operation,
			Some(AlterFunctionOperation::SetSecurity(
				FunctionSecurity::Definer
			))
		));
	}

	#[rstest]
	fn test_alter_function_add_parameter() {
		let mut stmt = AlterFunctionStatement::new();
		stmt.name("my_func")
			.add_parameter("a", "integer")
			.rename_to("new_func");
		assert_eq!(stmt.parameters.len(), 1);
		assert_eq!(stmt.parameters[0].name.as_ref().unwrap().to_string(), "a");
		assert_eq!(stmt.parameters[0].param_type.as_ref().unwrap(), "integer");
	}

	#[rstest]
	fn test_alter_function_take() {
		let mut stmt = AlterFunctionStatement::new();
		stmt.name("my_func").rename_to("new_func");
		let taken = stmt.take();
		assert!(stmt.name.is_none());
		assert!(stmt.operation.is_none());
		assert_eq!(taken.name.as_ref().unwrap().to_string(), "my_func");
	}
}
