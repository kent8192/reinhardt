//! ALTER PROCEDURE statement builder
//!
//! This module provides the `AlterProcedureStatement` type for building SQL ALTER PROCEDURE queries.

use crate::{
	backend::QueryBuilder,
	types::{
		DynIden, IntoIden,
		function::{FunctionBehavior, FunctionSecurity},
		procedure::{ProcedureOperation, ProcedureParameter},
	},
};

use crate::query::traits::{QueryBuilderTrait, QueryStatementBuilder, QueryStatementWriter};

/// ALTER PROCEDURE statement builder
///
/// This struct provides a fluent API for constructing ALTER PROCEDURE queries.
///
/// # Examples
///
/// ```rust
/// use reinhardt_query::prelude::*;
///
/// // ALTER PROCEDURE my_proc RENAME TO new_proc
/// let query = Query::alter_procedure()
///     .name("my_proc")
///     .rename_to("new_proc");
///
/// // ALTER PROCEDURE my_proc OWNER TO new_owner
/// let query = Query::alter_procedure()
///     .name("my_proc")
///     .owner_to("new_owner");
/// ```
#[derive(Debug, Clone)]
pub struct AlterProcedureStatement {
	pub(crate) name: Option<DynIden>,
	pub(crate) parameters: Vec<ProcedureParameter>,
	pub(crate) operation: Option<ProcedureOperation>,
}

impl AlterProcedureStatement {
	/// Create a new ALTER PROCEDURE statement
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::alter_procedure();
	/// ```
	pub fn new() -> Self {
		Self {
			name: None,
			parameters: Vec::new(),
			operation: None,
		}
	}

	/// Take the ownership of data in the current [`AlterProcedureStatement`]
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

	/// Set the procedure name
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::alter_procedure()
	///     .name("my_proc");
	/// ```
	pub fn name<N>(&mut self, name: N) -> &mut Self
	where
		N: IntoIden,
	{
		self.name = Some(name.into_iden());
		self
	}

	/// Add a procedure parameter (for identifying which overloaded procedure to alter)
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::alter_procedure()
	///     .name("my_proc")
	///     .add_parameter("a", "integer")
	///     .rename_to("new_proc");
	/// ```
	pub fn add_parameter<N: IntoIden, T: Into<String>>(
		&mut self,
		name: N,
		param_type: T,
	) -> &mut Self {
		self.parameters.push(ProcedureParameter {
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
	/// let query = Query::alter_procedure()
	///     .name("my_proc")
	///     .rename_to("new_proc");
	/// ```
	pub fn rename_to<N: IntoIden>(&mut self, new_name: N) -> &mut Self {
		self.operation = Some(ProcedureOperation::RenameTo(new_name.into_iden()));
		self
	}

	/// OWNER TO new_owner
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::alter_procedure()
	///     .name("my_proc")
	///     .owner_to("new_owner");
	/// ```
	pub fn owner_to<N: IntoIden>(&mut self, new_owner: N) -> &mut Self {
		self.operation = Some(ProcedureOperation::OwnerTo(new_owner.into_iden()));
		self
	}

	/// SET SCHEMA new_schema
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::alter_procedure()
	///     .name("my_proc")
	///     .set_schema("new_schema");
	/// ```
	pub fn set_schema<N: IntoIden>(&mut self, new_schema: N) -> &mut Self {
		self.operation = Some(ProcedureOperation::SetSchema(new_schema.into_iden()));
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
	/// let query = Query::alter_procedure()
	///     .name("my_proc")
	///     .set_behavior(FunctionBehavior::Immutable);
	/// ```
	pub fn set_behavior(&mut self, behavior: FunctionBehavior) -> &mut Self {
		self.operation = Some(ProcedureOperation::SetBehavior(behavior));
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
	/// let query = Query::alter_procedure()
	///     .name("my_proc")
	///     .set_security(FunctionSecurity::Definer);
	/// ```
	pub fn set_security(&mut self, security: FunctionSecurity) -> &mut Self {
		self.operation = Some(ProcedureOperation::SetSecurity(security));
		self
	}
}

impl Default for AlterProcedureStatement {
	fn default() -> Self {
		Self::new()
	}
}

impl QueryStatementBuilder for AlterProcedureStatement {
	fn build_any(&self, query_builder: &dyn QueryBuilderTrait) -> (String, crate::value::Values) {
		// Downcast to concrete QueryBuilder type
		use std::any::Any;
		if let Some(builder) =
			(query_builder as &dyn Any).downcast_ref::<crate::backend::PostgresQueryBuilder>()
		{
			return builder.build_alter_procedure(self);
		}
		if let Some(builder) =
			(query_builder as &dyn Any).downcast_ref::<crate::backend::MySqlQueryBuilder>()
		{
			return builder.build_alter_procedure(self);
		}
		if let Some(builder) =
			(query_builder as &dyn Any).downcast_ref::<crate::backend::SqliteQueryBuilder>()
		{
			return builder.build_alter_procedure(self);
		}
		if let Some(builder) =
			(query_builder as &dyn Any).downcast_ref::<crate::backend::CockroachDBQueryBuilder>()
		{
			return builder.build_alter_procedure(self);
		}
		panic!("Unsupported query builder type");
	}
}

impl QueryStatementWriter for AlterProcedureStatement {}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::*;

	#[rstest]
	fn test_alter_procedure_new() {
		let stmt = AlterProcedureStatement::new();
		assert!(stmt.name.is_none());
		assert!(stmt.parameters.is_empty());
		assert!(stmt.operation.is_none());
	}

	#[rstest]
	fn test_alter_procedure_with_name() {
		let mut stmt = AlterProcedureStatement::new();
		stmt.name("my_proc");
		assert_eq!(stmt.name.as_ref().unwrap().to_string(), "my_proc");
	}

	#[rstest]
	fn test_alter_procedure_rename_to() {
		let mut stmt = AlterProcedureStatement::new();
		stmt.name("my_proc").rename_to("new_proc");
		assert_eq!(stmt.name.as_ref().unwrap().to_string(), "my_proc");
		assert!(matches!(
			stmt.operation,
			Some(ProcedureOperation::RenameTo(_))
		));
	}

	#[rstest]
	fn test_alter_procedure_owner_to() {
		let mut stmt = AlterProcedureStatement::new();
		stmt.name("my_proc").owner_to("new_owner");
		assert!(matches!(
			stmt.operation,
			Some(ProcedureOperation::OwnerTo(_))
		));
	}

	#[rstest]
	fn test_alter_procedure_set_schema() {
		let mut stmt = AlterProcedureStatement::new();
		stmt.name("my_proc").set_schema("new_schema");
		assert!(matches!(
			stmt.operation,
			Some(ProcedureOperation::SetSchema(_))
		));
	}

	#[rstest]
	fn test_alter_procedure_set_behavior() {
		use crate::types::function::FunctionBehavior;

		let mut stmt = AlterProcedureStatement::new();
		stmt.name("my_proc")
			.set_behavior(FunctionBehavior::Immutable);
		assert!(matches!(
			stmt.operation,
			Some(ProcedureOperation::SetBehavior(FunctionBehavior::Immutable))
		));
	}

	#[rstest]
	fn test_alter_procedure_set_security() {
		use crate::types::function::FunctionSecurity;

		let mut stmt = AlterProcedureStatement::new();
		stmt.name("my_proc").set_security(FunctionSecurity::Definer);
		assert!(matches!(
			stmt.operation,
			Some(ProcedureOperation::SetSecurity(FunctionSecurity::Definer))
		));
	}

	#[rstest]
	fn test_alter_procedure_add_parameter() {
		let mut stmt = AlterProcedureStatement::new();
		stmt.name("my_proc")
			.add_parameter("a", "integer")
			.rename_to("new_proc");
		assert_eq!(stmt.parameters.len(), 1);
		assert_eq!(stmt.parameters[0].name.as_ref().unwrap().to_string(), "a");
		assert_eq!(stmt.parameters[0].param_type.as_ref().unwrap(), "integer");
	}

	#[rstest]
	fn test_alter_procedure_take() {
		let mut stmt = AlterProcedureStatement::new();
		stmt.name("my_proc").rename_to("new_proc");
		let taken = stmt.take();
		assert!(stmt.name.is_none());
		assert!(stmt.operation.is_none());
		assert_eq!(taken.name.as_ref().unwrap().to_string(), "my_proc");
	}
}
