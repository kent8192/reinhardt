//! DROP PROCEDURE statement builder
//!
//! This module provides the `DropProcedureStatement` type for building SQL DROP PROCEDURE queries.

use crate::{
	backend::QueryBuilder,
	types::{DynIden, IntoIden, procedure::ProcedureParameter},
};

use crate::query::traits::{QueryBuilderTrait, QueryStatementBuilder, QueryStatementWriter};

/// DROP PROCEDURE statement builder
///
/// This struct provides a fluent API for constructing DROP PROCEDURE queries.
///
/// # Examples
///
/// ```rust
/// use reinhardt_query::prelude::*;
///
/// // DROP PROCEDURE my_proc
/// let query = Query::drop_procedure()
///     .name("my_proc");
///
/// // DROP PROCEDURE IF EXISTS my_proc(integer) CASCADE
/// let query = Query::drop_procedure()
///     .name("my_proc")
///     .if_exists()
///     .add_parameter("", "integer")
///     .cascade();
/// ```
#[derive(Debug, Clone)]
pub struct DropProcedureStatement {
	pub(crate) name: Option<DynIden>,
	pub(crate) parameters: Vec<ProcedureParameter>,
	pub(crate) if_exists: bool,
	pub(crate) cascade: bool,
}

impl DropProcedureStatement {
	/// Create a new DROP PROCEDURE statement
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::drop_procedure();
	/// ```
	pub fn new() -> Self {
		Self {
			name: None,
			parameters: Vec::new(),
			if_exists: false,
			cascade: false,
		}
	}

	/// Take the ownership of data in the current [`DropProcedureStatement`]
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

	/// Set the procedure name
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::drop_procedure()
	///     .name("my_proc");
	/// ```
	pub fn name<N>(&mut self, name: N) -> &mut Self
	where
		N: IntoIden,
	{
		self.name = Some(name.into_iden());
		self
	}

	/// Add a procedure parameter (for identifying which overloaded procedure to drop)
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::prelude::*;
	///
	/// // DROP PROCEDURE my_proc(integer, text)
	/// let query = Query::drop_procedure()
	///     .name("my_proc")
	///     .add_parameter("", "integer")
	///     .add_parameter("", "text");
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

	/// Add IF EXISTS clause
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::drop_procedure()
	///     .name("my_proc")
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
	/// let query = Query::drop_procedure()
	///     .name("my_proc")
	///     .cascade();
	/// ```
	pub fn cascade(&mut self) -> &mut Self {
		self.cascade = true;
		self
	}
}

impl Default for DropProcedureStatement {
	fn default() -> Self {
		Self::new()
	}
}

impl QueryStatementBuilder for DropProcedureStatement {
	fn build_any(&self, query_builder: &dyn QueryBuilderTrait) -> (String, crate::value::Values) {
		// Downcast to concrete QueryBuilder type
		use std::any::Any;
		if let Some(builder) =
			(query_builder as &dyn Any).downcast_ref::<crate::backend::PostgresQueryBuilder>()
		{
			return builder.build_drop_procedure(self);
		}
		if let Some(builder) =
			(query_builder as &dyn Any).downcast_ref::<crate::backend::MySqlQueryBuilder>()
		{
			return builder.build_drop_procedure(self);
		}
		if let Some(builder) =
			(query_builder as &dyn Any).downcast_ref::<crate::backend::SqliteQueryBuilder>()
		{
			return builder.build_drop_procedure(self);
		}
		if let Some(builder) =
			(query_builder as &dyn Any).downcast_ref::<crate::backend::CockroachDBQueryBuilder>()
		{
			return builder.build_drop_procedure(self);
		}
		panic!("Unsupported query builder type");
	}
}

impl QueryStatementWriter for DropProcedureStatement {}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::*;

	#[rstest]
	fn test_drop_procedure_new() {
		let stmt = DropProcedureStatement::new();
		assert!(stmt.name.is_none());
		assert!(stmt.parameters.is_empty());
		assert!(!stmt.if_exists);
		assert!(!stmt.cascade);
	}

	#[rstest]
	fn test_drop_procedure_with_name() {
		let mut stmt = DropProcedureStatement::new();
		stmt.name("my_proc");
		assert_eq!(stmt.name.as_ref().unwrap().to_string(), "my_proc");
	}

	#[rstest]
	fn test_drop_procedure_if_exists() {
		let mut stmt = DropProcedureStatement::new();
		stmt.name("my_proc").if_exists();
		assert!(stmt.if_exists);
	}

	#[rstest]
	fn test_drop_procedure_cascade() {
		let mut stmt = DropProcedureStatement::new();
		stmt.name("my_proc").cascade();
		assert!(stmt.cascade);
	}

	#[rstest]
	fn test_drop_procedure_with_parameters() {
		let mut stmt = DropProcedureStatement::new();
		stmt.name("my_proc")
			.add_parameter("", "integer")
			.add_parameter("", "text");
		assert_eq!(stmt.parameters.len(), 2);
		assert_eq!(stmt.parameters[0].param_type.as_ref().unwrap(), "integer");
		assert_eq!(stmt.parameters[1].param_type.as_ref().unwrap(), "text");
	}

	#[rstest]
	fn test_drop_procedure_all_options() {
		let mut stmt = DropProcedureStatement::new();
		stmt.name("my_proc")
			.if_exists()
			.add_parameter("", "integer")
			.cascade();
		assert_eq!(stmt.name.as_ref().unwrap().to_string(), "my_proc");
		assert!(stmt.if_exists);
		assert!(stmt.cascade);
		assert_eq!(stmt.parameters.len(), 1);
	}

	#[rstest]
	fn test_drop_procedure_take() {
		let mut stmt = DropProcedureStatement::new();
		stmt.name("my_proc").cascade();
		let taken = stmt.take();
		assert!(stmt.name.is_none());
		assert!(!stmt.cascade);
		assert_eq!(taken.name.as_ref().unwrap().to_string(), "my_proc");
		assert!(taken.cascade);
	}
}
