//! CREATE FUNCTION statement builder
//!
//! This module provides the `CreateFunctionStatement` type for building SQL CREATE FUNCTION queries.

use crate::{
	backend::QueryBuilder,
	types::{
		IntoIden,
		function::{FunctionBehavior, FunctionDef, FunctionLanguage, FunctionSecurity},
	},
};

use crate::query::traits::{QueryBuilderTrait, QueryStatementBuilder, QueryStatementWriter};

/// CREATE FUNCTION statement builder
///
/// This struct provides a fluent API for constructing CREATE FUNCTION queries.
///
/// # Examples
///
/// ```rust
/// use reinhardt_query::prelude::*;
/// use reinhardt_query::types::function::{FunctionLanguage, FunctionBehavior};
///
/// // CREATE FUNCTION my_func() RETURNS integer LANGUAGE SQL AS 'SELECT 1'
/// let query = Query::create_function()
///     .name("my_func")
///     .returns("integer")
///     .language(FunctionLanguage::Sql)
///     .body("SELECT 1");
///
/// // CREATE OR REPLACE FUNCTION my_func(a integer) RETURNS integer
/// // LANGUAGE PLPGSQL IMMUTABLE AS 'BEGIN RETURN a + 1; END;'
/// let query = Query::create_function()
///     .name("my_func")
///     .or_replace()
///     .add_parameter("a", "integer")
///     .returns("integer")
///     .language(FunctionLanguage::PlPgSql)
///     .behavior(FunctionBehavior::Immutable)
///     .body("BEGIN RETURN a + 1; END;");
/// ```
#[derive(Debug, Clone)]
pub struct CreateFunctionStatement {
	pub(crate) function_def: FunctionDef,
}

impl CreateFunctionStatement {
	/// Create a new CREATE FUNCTION statement
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::create_function();
	/// ```
	pub fn new() -> Self {
		// Start with empty name - will be set via .name()
		Self {
			function_def: FunctionDef::new(""),
		}
	}

	/// Take the ownership of data in the current [`CreateFunctionStatement`]
	pub fn take(&mut self) -> Self {
		let taken = Self {
			function_def: self.function_def.clone(),
		};
		// Reset self to empty state
		self.function_def = FunctionDef::new("");
		taken
	}

	/// Set the function name
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::create_function()
	///     .name("my_func");
	/// ```
	pub fn name<N>(&mut self, name: N) -> &mut Self
	where
		N: IntoIden,
	{
		self.function_def.name = name.into_iden();
		self
	}

	/// Add OR REPLACE clause
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::create_function()
	///     .name("my_func")
	///     .or_replace();
	/// ```
	pub fn or_replace(&mut self) -> &mut Self {
		self.function_def.or_replace = true;
		self
	}

	/// Add a function parameter
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::create_function()
	///     .name("my_func")
	///     .add_parameter("param1", "integer")
	///     .add_parameter("param2", "text");
	/// ```
	pub fn add_parameter<N: IntoIden, T: Into<String>>(
		&mut self,
		name: N,
		param_type: T,
	) -> &mut Self {
		self.function_def = self.function_def.clone().add_parameter(name, param_type);
		self
	}

	/// Set RETURNS type
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::create_function()
	///     .name("my_func")
	///     .returns("integer");
	/// ```
	pub fn returns<T: Into<String>>(&mut self, returns: T) -> &mut Self {
		self.function_def.returns = Some(returns.into());
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
	/// let query = Query::create_function()
	///     .name("my_func")
	///     .language(FunctionLanguage::PlPgSql);
	/// ```
	pub fn language(&mut self, language: FunctionLanguage) -> &mut Self {
		self.function_def.language = Some(language);
		self
	}

	/// Set function behavior (IMMUTABLE/STABLE/VOLATILE)
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::prelude::*;
	/// use reinhardt_query::types::function::FunctionBehavior;
	///
	/// let query = Query::create_function()
	///     .name("my_func")
	///     .behavior(FunctionBehavior::Immutable);
	/// ```
	pub fn behavior(&mut self, behavior: FunctionBehavior) -> &mut Self {
		self.function_def.behavior = Some(behavior);
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
	/// let query = Query::create_function()
	///     .name("my_func")
	///     .security(FunctionSecurity::Definer);
	/// ```
	pub fn security(&mut self, security: FunctionSecurity) -> &mut Self {
		self.function_def.security = Some(security);
		self
	}

	/// Set function body (AS clause)
	///
	/// # Security Warning
	///
	/// The function body is embedded directly into the CREATE FUNCTION statement
	/// and will be stored in the database. **DO NOT** pass user input directly to
	/// this method, as it could lead to arbitrary code execution.
	///
	/// Only use with trusted, validated code.
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::prelude::*;
	/// use reinhardt_query::types::function::{FunctionLanguage, FunctionBehavior};
	///
	/// // ✅ SAFE: Static code
	/// let query = Query::create_function()
	///     .name("my_func")
	///     .returns("integer")
	///     .language(FunctionLanguage::Sql)
	///     .body("SELECT 1");
	///
	/// // ❌ UNSAFE: User input
	/// // let query = Query::create_function()
	/// //     .name("user_func")
	/// //     .body(&user_code);
	/// ```
	pub fn body<B: Into<String>>(&mut self, body: B) -> &mut Self {
		self.function_def.body = Some(body.into());
		self
	}
}

impl Default for CreateFunctionStatement {
	fn default() -> Self {
		Self::new()
	}
}

impl QueryStatementBuilder for CreateFunctionStatement {
	fn build_any(&self, query_builder: &dyn QueryBuilderTrait) -> (String, crate::value::Values) {
		// Downcast to concrete QueryBuilder type
		use std::any::Any;
		if let Some(builder) =
			(query_builder as &dyn Any).downcast_ref::<crate::backend::PostgresQueryBuilder>()
		{
			return builder.build_create_function(self);
		}
		if let Some(builder) =
			(query_builder as &dyn Any).downcast_ref::<crate::backend::MySqlQueryBuilder>()
		{
			return builder.build_create_function(self);
		}
		if let Some(builder) =
			(query_builder as &dyn Any).downcast_ref::<crate::backend::SqliteQueryBuilder>()
		{
			return builder.build_create_function(self);
		}
		if let Some(builder) =
			(query_builder as &dyn Any).downcast_ref::<crate::backend::CockroachDBQueryBuilder>()
		{
			return builder.build_create_function(self);
		}
		panic!("Unsupported query builder type");
	}
}

impl QueryStatementWriter for CreateFunctionStatement {}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::*;

	#[rstest]
	fn test_create_function_new() {
		let stmt = CreateFunctionStatement::new();
		assert!(stmt.function_def.name.to_string().is_empty());
		assert!(!stmt.function_def.or_replace);
		assert!(stmt.function_def.parameters.is_empty());
		assert!(stmt.function_def.returns.is_none());
		assert!(stmt.function_def.language.is_none());
		assert!(stmt.function_def.behavior.is_none());
		assert!(stmt.function_def.security.is_none());
		assert!(stmt.function_def.body.is_none());
	}

	#[rstest]
	fn test_create_function_with_name() {
		let mut stmt = CreateFunctionStatement::new();
		stmt.name("my_func");
		assert_eq!(stmt.function_def.name.to_string(), "my_func");
	}

	#[rstest]
	fn test_create_function_or_replace() {
		let mut stmt = CreateFunctionStatement::new();
		stmt.name("my_func").or_replace();
		assert!(stmt.function_def.or_replace);
	}

	#[rstest]
	fn test_create_function_add_parameter() {
		let mut stmt = CreateFunctionStatement::new();
		stmt.name("my_func").add_parameter("param1", "integer");
		assert_eq!(stmt.function_def.parameters.len(), 1);
		assert_eq!(
			stmt.function_def.parameters[0]
				.name
				.as_ref()
				.unwrap()
				.to_string(),
			"param1"
		);
		assert_eq!(
			stmt.function_def.parameters[0].param_type.as_ref().unwrap(),
			"integer"
		);
	}

	#[rstest]
	fn test_create_function_multiple_parameters() {
		let mut stmt = CreateFunctionStatement::new();
		stmt.name("my_func")
			.add_parameter("param1", "integer")
			.add_parameter("param2", "text");
		assert_eq!(stmt.function_def.parameters.len(), 2);
		assert_eq!(
			stmt.function_def.parameters[0]
				.name
				.as_ref()
				.unwrap()
				.to_string(),
			"param1"
		);
		assert_eq!(
			stmt.function_def.parameters[1]
				.name
				.as_ref()
				.unwrap()
				.to_string(),
			"param2"
		);
	}

	#[rstest]
	fn test_create_function_returns() {
		let mut stmt = CreateFunctionStatement::new();
		stmt.name("my_func").returns("integer");
		assert_eq!(stmt.function_def.returns.as_ref().unwrap(), "integer");
	}

	#[rstest]
	fn test_create_function_language() {
		let mut stmt = CreateFunctionStatement::new();
		stmt.name("my_func").language(FunctionLanguage::PlPgSql);
		assert_eq!(stmt.function_def.language, Some(FunctionLanguage::PlPgSql));
	}

	#[rstest]
	fn test_create_function_behavior() {
		let mut stmt = CreateFunctionStatement::new();
		stmt.name("my_func").behavior(FunctionBehavior::Immutable);
		assert_eq!(
			stmt.function_def.behavior,
			Some(FunctionBehavior::Immutable)
		);
	}

	#[rstest]
	fn test_create_function_security() {
		let mut stmt = CreateFunctionStatement::new();
		stmt.name("my_func").security(FunctionSecurity::Definer);
		assert_eq!(stmt.function_def.security, Some(FunctionSecurity::Definer));
	}

	#[rstest]
	fn test_create_function_body() {
		let mut stmt = CreateFunctionStatement::new();
		stmt.name("my_func").body("SELECT 1");
		assert_eq!(stmt.function_def.body.as_ref().unwrap(), "SELECT 1");
	}

	#[rstest]
	fn test_create_function_all_options() {
		let mut stmt = CreateFunctionStatement::new();
		stmt.name("my_func")
			.or_replace()
			.add_parameter("a", "integer")
			.add_parameter("b", "text")
			.returns("integer")
			.language(FunctionLanguage::PlPgSql)
			.behavior(FunctionBehavior::Immutable)
			.security(FunctionSecurity::Definer)
			.body("BEGIN RETURN a + LENGTH(b); END;");

		assert_eq!(stmt.function_def.name.to_string(), "my_func");
		assert!(stmt.function_def.or_replace);
		assert_eq!(stmt.function_def.parameters.len(), 2);
		assert_eq!(stmt.function_def.returns.as_ref().unwrap(), "integer");
		assert_eq!(stmt.function_def.language, Some(FunctionLanguage::PlPgSql));
		assert_eq!(
			stmt.function_def.behavior,
			Some(FunctionBehavior::Immutable)
		);
		assert_eq!(stmt.function_def.security, Some(FunctionSecurity::Definer));
		assert_eq!(
			stmt.function_def.body.as_ref().unwrap(),
			"BEGIN RETURN a + LENGTH(b); END;"
		);
	}

	#[rstest]
	fn test_create_function_take() {
		let mut stmt = CreateFunctionStatement::new();
		stmt.name("my_func");
		let taken = stmt.take();
		assert!(stmt.function_def.name.to_string().is_empty());
		assert_eq!(taken.function_def.name.to_string(), "my_func");
	}
}
