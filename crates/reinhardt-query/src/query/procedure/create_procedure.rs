//! CREATE PROCEDURE statement builder
//!
//! This module provides the `CreateProcedureStatement` type for building SQL CREATE PROCEDURE queries.

use crate::{
	backend::QueryBuilder,
	types::{
		IntoIden,
		function::{FunctionBehavior, FunctionLanguage, FunctionSecurity},
		procedure::ProcedureDef,
	},
};

use crate::query::traits::{QueryBuilderTrait, QueryStatementBuilder, QueryStatementWriter};

/// CREATE PROCEDURE statement builder
///
/// This struct provides a fluent API for constructing CREATE PROCEDURE queries.
///
/// # Examples
///
/// ```rust
/// use reinhardt_query::prelude::*;
/// use reinhardt_query::types::function::{FunctionLanguage, FunctionBehavior};
///
/// // CREATE PROCEDURE my_proc() LANGUAGE SQL AS 'SELECT 1'
/// let query = Query::create_procedure()
///     .name("my_proc")
///     .language(FunctionLanguage::Sql)
///     .body("SELECT 1");
///
/// // CREATE OR REPLACE PROCEDURE my_proc(a integer)
/// // LANGUAGE PLPGSQL IMMUTABLE AS 'BEGIN INSERT INTO log VALUES (a); END;'
/// let query = Query::create_procedure()
///     .name("my_proc")
///     .or_replace()
///     .add_parameter("a", "integer")
///     .language(FunctionLanguage::PlPgSql)
///     .behavior(FunctionBehavior::Immutable)
///     .body("BEGIN INSERT INTO log VALUES (a); END;");
/// ```
#[derive(Debug, Clone)]
pub struct CreateProcedureStatement {
	pub(crate) procedure_def: ProcedureDef,
}

impl CreateProcedureStatement {
	/// Create a new CREATE PROCEDURE statement
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::create_procedure();
	/// ```
	pub fn new() -> Self {
		// Start with empty name - will be set via .name()
		Self {
			procedure_def: ProcedureDef::new(""),
		}
	}

	/// Take the ownership of data in the current [`CreateProcedureStatement`]
	pub fn take(&mut self) -> Self {
		let taken = Self {
			procedure_def: self.procedure_def.clone(),
		};
		// Reset self to empty state
		self.procedure_def = ProcedureDef::new("");
		taken
	}

	/// Set the procedure name
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::create_procedure()
	///     .name("my_proc");
	/// ```
	pub fn name<N>(&mut self, name: N) -> &mut Self
	where
		N: IntoIden,
	{
		self.procedure_def.name = name.into_iden();
		self
	}

	/// Add OR REPLACE clause
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::create_procedure()
	///     .name("my_proc")
	///     .or_replace();
	/// ```
	pub fn or_replace(&mut self) -> &mut Self {
		self.procedure_def.or_replace = true;
		self
	}

	/// Add a procedure parameter
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::create_procedure()
	///     .name("my_proc")
	///     .add_parameter("param1", "integer")
	///     .add_parameter("param2", "text");
	/// ```
	pub fn add_parameter<N: IntoIden, T: Into<String>>(
		&mut self,
		name: N,
		param_type: T,
	) -> &mut Self {
		self.procedure_def = self.procedure_def.clone().add_parameter(name, param_type);
		self
	}

	/// Set LANGUAGE
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::prelude::*;
	/// use reinhardt_query::types::function::FunctionLanguage;
	///
	/// let query = Query::create_procedure()
	///     .name("my_proc")
	///     .language(FunctionLanguage::PlPgSql);
	/// ```
	pub fn language(&mut self, language: FunctionLanguage) -> &mut Self {
		self.procedure_def.language = Some(language);
		self
	}

	/// Set procedure behavior (IMMUTABLE/STABLE/VOLATILE)
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::prelude::*;
	/// use reinhardt_query::types::function::FunctionBehavior;
	///
	/// let query = Query::create_procedure()
	///     .name("my_proc")
	///     .behavior(FunctionBehavior::Immutable);
	/// ```
	pub fn behavior(&mut self, behavior: FunctionBehavior) -> &mut Self {
		self.procedure_def.behavior = Some(behavior);
		self
	}

	/// Set security context (DEFINER/INVOKER)
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::prelude::*;
	/// use reinhardt_query::types::function::FunctionSecurity;
	///
	/// let query = Query::create_procedure()
	///     .name("my_proc")
	///     .security(FunctionSecurity::Definer);
	/// ```
	pub fn security(&mut self, security: FunctionSecurity) -> &mut Self {
		self.procedure_def.security = Some(security);
		self
	}

	/// Set procedure body (AS clause)
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::create_procedure()
	///     .name("my_proc")
	///     .body("SELECT 1");
	/// ```
	pub fn body<B: Into<String>>(&mut self, body: B) -> &mut Self {
		self.procedure_def.body = Some(body.into());
		self
	}
}

impl Default for CreateProcedureStatement {
	fn default() -> Self {
		Self::new()
	}
}

impl QueryStatementBuilder for CreateProcedureStatement {
	fn build_any(&self, query_builder: &dyn QueryBuilderTrait) -> (String, crate::value::Values) {
		// Downcast to concrete QueryBuilder type
		use std::any::Any;
		if let Some(builder) =
			(query_builder as &dyn Any).downcast_ref::<crate::backend::PostgresQueryBuilder>()
		{
			return builder.build_create_procedure(self);
		}
		if let Some(builder) =
			(query_builder as &dyn Any).downcast_ref::<crate::backend::MySqlQueryBuilder>()
		{
			return builder.build_create_procedure(self);
		}
		if let Some(builder) =
			(query_builder as &dyn Any).downcast_ref::<crate::backend::SqliteQueryBuilder>()
		{
			return builder.build_create_procedure(self);
		}
		if let Some(builder) =
			(query_builder as &dyn Any).downcast_ref::<crate::backend::CockroachDBQueryBuilder>()
		{
			return builder.build_create_procedure(self);
		}
		panic!("Unsupported query builder type");
	}
}

impl QueryStatementWriter for CreateProcedureStatement {}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::*;

	#[rstest]
	fn test_create_procedure_new() {
		let stmt = CreateProcedureStatement::new();
		assert!(stmt.procedure_def.name.to_string().is_empty());
		assert!(!stmt.procedure_def.or_replace);
		assert!(stmt.procedure_def.parameters.is_empty());
		assert!(stmt.procedure_def.language.is_none());
		assert!(stmt.procedure_def.behavior.is_none());
		assert!(stmt.procedure_def.security.is_none());
		assert!(stmt.procedure_def.body.is_none());
	}

	#[rstest]
	fn test_create_procedure_with_name() {
		let mut stmt = CreateProcedureStatement::new();
		stmt.name("my_proc");
		assert_eq!(stmt.procedure_def.name.to_string(), "my_proc");
	}

	#[rstest]
	fn test_create_procedure_or_replace() {
		let mut stmt = CreateProcedureStatement::new();
		stmt.name("my_proc").or_replace();
		assert!(stmt.procedure_def.or_replace);
	}

	#[rstest]
	fn test_create_procedure_add_parameter() {
		let mut stmt = CreateProcedureStatement::new();
		stmt.name("my_proc").add_parameter("param1", "integer");
		assert_eq!(stmt.procedure_def.parameters.len(), 1);
		assert_eq!(
			stmt.procedure_def.parameters[0]
				.name
				.as_ref()
				.unwrap()
				.to_string(),
			"param1"
		);
		assert_eq!(
			stmt.procedure_def.parameters[0]
				.param_type
				.as_ref()
				.unwrap(),
			"integer"
		);
	}

	#[rstest]
	fn test_create_procedure_multiple_parameters() {
		let mut stmt = CreateProcedureStatement::new();
		stmt.name("my_proc")
			.add_parameter("param1", "integer")
			.add_parameter("param2", "text");
		assert_eq!(stmt.procedure_def.parameters.len(), 2);
		assert_eq!(
			stmt.procedure_def.parameters[0]
				.name
				.as_ref()
				.unwrap()
				.to_string(),
			"param1"
		);
		assert_eq!(
			stmt.procedure_def.parameters[1]
				.name
				.as_ref()
				.unwrap()
				.to_string(),
			"param2"
		);
	}

	#[rstest]
	fn test_create_procedure_language() {
		let mut stmt = CreateProcedureStatement::new();
		stmt.name("my_proc").language(FunctionLanguage::PlPgSql);
		assert_eq!(stmt.procedure_def.language, Some(FunctionLanguage::PlPgSql));
	}

	#[rstest]
	fn test_create_procedure_behavior() {
		let mut stmt = CreateProcedureStatement::new();
		stmt.name("my_proc").behavior(FunctionBehavior::Immutable);
		assert_eq!(
			stmt.procedure_def.behavior,
			Some(FunctionBehavior::Immutable)
		);
	}

	#[rstest]
	fn test_create_procedure_security() {
		let mut stmt = CreateProcedureStatement::new();
		stmt.name("my_proc").security(FunctionSecurity::Definer);
		assert_eq!(stmt.procedure_def.security, Some(FunctionSecurity::Definer));
	}

	#[rstest]
	fn test_create_procedure_body() {
		let mut stmt = CreateProcedureStatement::new();
		stmt.name("my_proc").body("SELECT 1");
		assert_eq!(stmt.procedure_def.body.as_ref().unwrap(), "SELECT 1");
	}

	#[rstest]
	fn test_create_procedure_all_options() {
		let mut stmt = CreateProcedureStatement::new();
		stmt.name("my_proc")
			.or_replace()
			.add_parameter("a", "integer")
			.add_parameter("b", "text")
			.language(FunctionLanguage::PlPgSql)
			.behavior(FunctionBehavior::Immutable)
			.security(FunctionSecurity::Definer)
			.body("BEGIN INSERT INTO log VALUES (a, b); END;");

		assert_eq!(stmt.procedure_def.name.to_string(), "my_proc");
		assert!(stmt.procedure_def.or_replace);
		assert_eq!(stmt.procedure_def.parameters.len(), 2);
		assert_eq!(stmt.procedure_def.language, Some(FunctionLanguage::PlPgSql));
		assert_eq!(
			stmt.procedure_def.behavior,
			Some(FunctionBehavior::Immutable)
		);
		assert_eq!(stmt.procedure_def.security, Some(FunctionSecurity::Definer));
		assert_eq!(
			stmt.procedure_def.body.as_ref().unwrap(),
			"BEGIN INSERT INTO log VALUES (a, b); END;"
		);
	}

	#[rstest]
	fn test_create_procedure_take() {
		let mut stmt = CreateProcedureStatement::new();
		stmt.name("my_proc");
		let taken = stmt.take();
		assert!(stmt.procedure_def.name.to_string().is_empty());
		assert_eq!(taken.procedure_def.name.to_string(), "my_proc");
	}
}
