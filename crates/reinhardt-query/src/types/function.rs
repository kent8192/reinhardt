//! Function type definitions
//!
//! This module provides types for function-related DDL operations:
//!
//! - [`FunctionDef`]: Function definition for CREATE FUNCTION
//! - [`FunctionParameter`]: Function parameter definition (name, type, mode)
//! - [`FunctionLanguage`]: Programming language for function body
//! - [`FunctionBehavior`]: Function volatility category
//! - [`FunctionSecurity`]: Security context for function execution
//!
//! # Examples
//!
//! ```rust
//! use reinhardt_query::types::function::{FunctionDef, FunctionLanguage, FunctionBehavior};
//!
//! // CREATE FUNCTION my_func() RETURNS integer LANGUAGE SQL AS 'SELECT 1'
//! let func = FunctionDef::new("my_func")
//!     .returns("integer")
//!     .language(FunctionLanguage::Sql)
//!     .body("SELECT 1");
//!
//! // CREATE OR REPLACE FUNCTION my_func(a integer, b text)
//! // RETURNS integer LANGUAGE PLPGSQL AS '...'
//! let func = FunctionDef::new("my_func")
//!     .or_replace(true)
//!     .add_parameter("a", "integer")
//!     .add_parameter("b", "text")
//!     .returns("integer")
//!     .language(FunctionLanguage::PlPgSql)
//!     .behavior(FunctionBehavior::Immutable)
//!     .body("BEGIN RETURN a + LENGTH(b); END;");
//! ```

use crate::types::{DynIden, IntoIden};

/// Function definition for CREATE FUNCTION
///
/// This struct represents a function definition, including its name,
/// parameters, return type, language, behavior, security, and body.
///
/// # Examples
///
/// ```rust
/// use reinhardt_query::types::function::{FunctionDef, FunctionLanguage};
///
/// // CREATE FUNCTION my_func() RETURNS integer
/// let func = FunctionDef::new("my_func")
///     .returns("integer")
///     .language(FunctionLanguage::Sql)
///     .body("SELECT 1");
/// ```
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct FunctionDef {
	pub(crate) name: DynIden,
	pub(crate) or_replace: bool,
	pub(crate) parameters: Vec<FunctionParameter>,
	pub(crate) returns: Option<String>,
	pub(crate) language: Option<FunctionLanguage>,
	pub(crate) behavior: Option<FunctionBehavior>,
	pub(crate) security: Option<FunctionSecurity>,
	pub(crate) body: Option<String>,
}

/// Function parameter definition
///
/// Represents a parameter in a function signature with optional name, type, and mode.
///
/// # Examples
///
/// ```rust
/// use reinhardt_query::types::function::{FunctionParameter, ParameterMode};
///
/// // IN parameter with name
/// let param = FunctionParameter::new()
///     .name("my_param")
///     .param_type("integer")
///     .mode(ParameterMode::In);
///
/// // OUT parameter without name
/// let param = FunctionParameter::new()
///     .param_type("text")
///     .mode(ParameterMode::Out);
/// ```
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct FunctionParameter {
	pub(crate) name: Option<DynIden>,
	pub(crate) param_type: Option<String>,
	pub(crate) mode: Option<ParameterMode>,
	pub(crate) default_value: Option<String>,
}

/// Parameter mode (IN, OUT, INOUT, VARIADIC)
///
/// Specifies the direction and behavior of a function parameter.
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code)]
pub enum ParameterMode {
	/// IN - Input parameter (default)
	In,
	/// OUT - Output parameter
	Out,
	/// INOUT - Input/output parameter
	InOut,
	/// VARIADIC - Variable number of arguments
	Variadic,
}

/// Programming language for function body
///
/// Specifies the language in which the function is written.
///
/// # Examples
///
/// ```rust
/// use reinhardt_query::types::function::FunctionLanguage;
///
/// let lang = FunctionLanguage::Sql;
/// let lang = FunctionLanguage::PlPgSql;
/// let lang = FunctionLanguage::C;
/// let lang = FunctionLanguage::Custom("plpython3u".to_string());
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code)]
pub enum FunctionLanguage {
	/// SQL language
	Sql,
	/// PL/pgSQL (PostgreSQL)
	PlPgSql,
	/// C language
	C,
	/// Custom language (extension)
	///
	/// # Security Note
	///
	/// Only use with trusted language names. Do not use with user input.
	Custom(String),
}

/// Function volatility category
///
/// Specifies whether the function modifies the database or depends on database state.
///
/// # Examples
///
/// ```rust
/// use reinhardt_query::types::function::FunctionBehavior;
///
/// let behavior = FunctionBehavior::Immutable; // Result depends only on arguments
/// let behavior = FunctionBehavior::Stable;     // Result depends on DB state (same within transaction)
/// let behavior = FunctionBehavior::Volatile;   // Result may change (default)
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code)]
pub enum FunctionBehavior {
	/// IMMUTABLE - Result depends only on arguments, never changes
	Immutable,
	/// STABLE - Result depends on database state, but stable within a transaction
	Stable,
	/// VOLATILE - Result may change even within a transaction (default)
	Volatile,
}

/// Security context for function execution
///
/// Specifies whether the function executes with the privileges of the definer or invoker.
///
/// # Examples
///
/// ```rust
/// use reinhardt_query::types::function::FunctionSecurity;
///
/// let security = FunctionSecurity::Definer; // Execute with definer's privileges
/// let security = FunctionSecurity::Invoker; // Execute with invoker's privileges (default)
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code)]
pub enum FunctionSecurity {
	/// SECURITY DEFINER - Execute with privileges of function definer
	Definer,
	/// SECURITY INVOKER - Execute with privileges of function caller (default)
	Invoker,
}

impl FunctionDef {
	/// Create a new function definition
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::types::function::FunctionDef;
	///
	/// let func = FunctionDef::new("my_func");
	/// ```
	pub fn new<N: IntoIden>(name: N) -> Self {
		Self {
			name: name.into_iden(),
			or_replace: false,
			parameters: Vec::new(),
			returns: None,
			language: None,
			behavior: None,
			security: None,
			body: None,
		}
	}

	/// Set OR REPLACE clause
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::types::function::FunctionDef;
	///
	/// let func = FunctionDef::new("my_func")
	///     .or_replace(true);
	/// ```
	pub fn or_replace(mut self, or_replace: bool) -> Self {
		self.or_replace = or_replace;
		self
	}

	/// Add a function parameter
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::types::function::FunctionDef;
	///
	/// let func = FunctionDef::new("my_func")
	///     .add_parameter("param1", "integer")
	///     .add_parameter("param2", "text");
	/// ```
	pub fn add_parameter<N: IntoIden, T: Into<String>>(mut self, name: N, param_type: T) -> Self {
		self.parameters.push(FunctionParameter {
			name: Some(name.into_iden()),
			param_type: Some(param_type.into()),
			mode: None,
			default_value: None,
		});
		self
	}

	/// Add a function parameter with full specification
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::types::function::{FunctionDef, FunctionParameter, ParameterMode};
	///
	/// let param = FunctionParameter::new()
	///     .name("my_param")
	///     .param_type("integer")
	///     .mode(ParameterMode::InOut);
	///
	/// let func = FunctionDef::new("my_func")
	///     .add_parameter_spec(param);
	/// ```
	pub fn add_parameter_spec(mut self, param: FunctionParameter) -> Self {
		self.parameters.push(param);
		self
	}

	/// Set RETURNS type
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::types::function::FunctionDef;
	///
	/// let func = FunctionDef::new("my_func")
	///     .returns("integer");
	/// ```
	pub fn returns<T: Into<String>>(mut self, returns: T) -> Self {
		self.returns = Some(returns.into());
		self
	}

	/// Set LANGUAGE
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::types::function::{FunctionDef, FunctionLanguage};
	///
	/// let func = FunctionDef::new("my_func")
	///     .language(FunctionLanguage::PlPgSql);
	/// ```
	pub fn language(mut self, language: FunctionLanguage) -> Self {
		self.language = Some(language);
		self
	}

	/// Set function behavior (IMMUTABLE/STABLE/VOLATILE)
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::types::function::{FunctionDef, FunctionBehavior};
	///
	/// let func = FunctionDef::new("my_func")
	///     .behavior(FunctionBehavior::Immutable);
	/// ```
	pub fn behavior(mut self, behavior: FunctionBehavior) -> Self {
		self.behavior = Some(behavior);
		self
	}

	/// Set security context (DEFINER/INVOKER)
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::types::function::{FunctionDef, FunctionSecurity};
	///
	/// let func = FunctionDef::new("my_func")
	///     .security(FunctionSecurity::Definer);
	/// ```
	pub fn security(mut self, security: FunctionSecurity) -> Self {
		self.security = Some(security);
		self
	}

	/// Set function body (AS clause)
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::types::function::FunctionDef;
	///
	/// let func = FunctionDef::new("my_func")
	///     .body("SELECT 1");
	/// ```
	pub fn body<B: Into<String>>(mut self, body: B) -> Self {
		self.body = Some(body.into());
		self
	}
}

impl FunctionParameter {
	/// Create a new function parameter
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::types::function::FunctionParameter;
	///
	/// let param = FunctionParameter::new();
	/// ```
	pub fn new() -> Self {
		Self {
			name: None,
			param_type: None,
			mode: None,
			default_value: None,
		}
	}

	/// Set parameter name
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::types::function::FunctionParameter;
	///
	/// let param = FunctionParameter::new()
	///     .name("my_param");
	/// ```
	pub fn name<N: IntoIden>(mut self, name: N) -> Self {
		self.name = Some(name.into_iden());
		self
	}

	/// Set parameter type
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::types::function::FunctionParameter;
	///
	/// let param = FunctionParameter::new()
	///     .param_type("integer");
	/// ```
	pub fn param_type<T: Into<String>>(mut self, param_type: T) -> Self {
		self.param_type = Some(param_type.into());
		self
	}

	/// Set parameter mode
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::types::function::{FunctionParameter, ParameterMode};
	///
	/// let param = FunctionParameter::new()
	///     .mode(ParameterMode::InOut);
	/// ```
	pub fn mode(mut self, mode: ParameterMode) -> Self {
		self.mode = Some(mode);
		self
	}

	/// Set parameter default value
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::types::function::FunctionParameter;
	///
	/// let param = FunctionParameter::new()
	///     .default_value("42");
	/// ```
	pub fn default_value<V: Into<String>>(mut self, value: V) -> Self {
		self.default_value = Some(value.into());
		self
	}
}

impl Default for FunctionParameter {
	fn default() -> Self {
		Self::new()
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::*;

	// FunctionDef tests
	#[rstest]
	fn test_function_def_basic() {
		let func = FunctionDef::new("my_func");
		assert_eq!(func.name.to_string(), "my_func");
		assert!(!func.or_replace);
		assert!(func.parameters.is_empty());
		assert!(func.returns.is_none());
		assert!(func.language.is_none());
		assert!(func.behavior.is_none());
		assert!(func.security.is_none());
		assert!(func.body.is_none());
	}

	#[rstest]
	fn test_function_def_or_replace() {
		let func = FunctionDef::new("my_func").or_replace(true);
		assert_eq!(func.name.to_string(), "my_func");
		assert!(func.or_replace);
	}

	#[rstest]
	fn test_function_def_add_parameter() {
		let func = FunctionDef::new("my_func").add_parameter("param1", "integer");
		assert_eq!(func.parameters.len(), 1);
		assert_eq!(
			func.parameters[0].name.as_ref().unwrap().to_string(),
			"param1"
		);
		assert_eq!(func.parameters[0].param_type.as_ref().unwrap(), "integer");
	}

	#[rstest]
	fn test_function_def_multiple_parameters() {
		let func = FunctionDef::new("my_func")
			.add_parameter("param1", "integer")
			.add_parameter("param2", "text");
		assert_eq!(func.parameters.len(), 2);
		assert_eq!(
			func.parameters[0].name.as_ref().unwrap().to_string(),
			"param1"
		);
		assert_eq!(func.parameters[0].param_type.as_ref().unwrap(), "integer");
		assert_eq!(
			func.parameters[1].name.as_ref().unwrap().to_string(),
			"param2"
		);
		assert_eq!(func.parameters[1].param_type.as_ref().unwrap(), "text");
	}

	#[rstest]
	fn test_function_def_returns() {
		let func = FunctionDef::new("my_func").returns("integer");
		assert_eq!(func.returns.as_ref().unwrap(), "integer");
	}

	#[rstest]
	fn test_function_def_language_sql() {
		let func = FunctionDef::new("my_func").language(FunctionLanguage::Sql);
		assert_eq!(func.language, Some(FunctionLanguage::Sql));
	}

	#[rstest]
	fn test_function_def_language_plpgsql() {
		let func = FunctionDef::new("my_func").language(FunctionLanguage::PlPgSql);
		assert_eq!(func.language, Some(FunctionLanguage::PlPgSql));
	}

	#[rstest]
	fn test_function_def_behavior_immutable() {
		let func = FunctionDef::new("my_func").behavior(FunctionBehavior::Immutable);
		assert_eq!(func.behavior, Some(FunctionBehavior::Immutable));
	}

	#[rstest]
	fn test_function_def_behavior_stable() {
		let func = FunctionDef::new("my_func").behavior(FunctionBehavior::Stable);
		assert_eq!(func.behavior, Some(FunctionBehavior::Stable));
	}

	#[rstest]
	fn test_function_def_behavior_volatile() {
		let func = FunctionDef::new("my_func").behavior(FunctionBehavior::Volatile);
		assert_eq!(func.behavior, Some(FunctionBehavior::Volatile));
	}

	#[rstest]
	fn test_function_def_security_definer() {
		let func = FunctionDef::new("my_func").security(FunctionSecurity::Definer);
		assert_eq!(func.security, Some(FunctionSecurity::Definer));
	}

	#[rstest]
	fn test_function_def_security_invoker() {
		let func = FunctionDef::new("my_func").security(FunctionSecurity::Invoker);
		assert_eq!(func.security, Some(FunctionSecurity::Invoker));
	}

	#[rstest]
	fn test_function_def_body() {
		let func = FunctionDef::new("my_func").body("SELECT 1");
		assert_eq!(func.body.as_ref().unwrap(), "SELECT 1");
	}

	#[rstest]
	fn test_function_def_all_options() {
		let func = FunctionDef::new("my_func")
			.or_replace(true)
			.add_parameter("a", "integer")
			.add_parameter("b", "text")
			.returns("integer")
			.language(FunctionLanguage::PlPgSql)
			.behavior(FunctionBehavior::Immutable)
			.security(FunctionSecurity::Definer)
			.body("BEGIN RETURN a + LENGTH(b); END;");

		assert_eq!(func.name.to_string(), "my_func");
		assert!(func.or_replace);
		assert_eq!(func.parameters.len(), 2);
		assert_eq!(func.returns.as_ref().unwrap(), "integer");
		assert_eq!(func.language, Some(FunctionLanguage::PlPgSql));
		assert_eq!(func.behavior, Some(FunctionBehavior::Immutable));
		assert_eq!(func.security, Some(FunctionSecurity::Definer));
		assert_eq!(
			func.body.as_ref().unwrap(),
			"BEGIN RETURN a + LENGTH(b); END;"
		);
	}

	// FunctionParameter tests
	#[rstest]
	fn test_function_parameter_basic() {
		let param = FunctionParameter::new();
		assert!(param.name.is_none());
		assert!(param.param_type.is_none());
		assert!(param.mode.is_none());
		assert!(param.default_value.is_none());
	}

	#[rstest]
	fn test_function_parameter_name() {
		let param = FunctionParameter::new().name("my_param");
		assert_eq!(param.name.as_ref().unwrap().to_string(), "my_param");
	}

	#[rstest]
	fn test_function_parameter_type() {
		let param = FunctionParameter::new().param_type("integer");
		assert_eq!(param.param_type.as_ref().unwrap(), "integer");
	}

	#[rstest]
	fn test_function_parameter_mode_in() {
		let param = FunctionParameter::new().mode(ParameterMode::In);
		assert_eq!(param.mode, Some(ParameterMode::In));
	}

	#[rstest]
	fn test_function_parameter_mode_out() {
		let param = FunctionParameter::new().mode(ParameterMode::Out);
		assert_eq!(param.mode, Some(ParameterMode::Out));
	}

	#[rstest]
	fn test_function_parameter_mode_inout() {
		let param = FunctionParameter::new().mode(ParameterMode::InOut);
		assert_eq!(param.mode, Some(ParameterMode::InOut));
	}

	#[rstest]
	fn test_function_parameter_mode_variadic() {
		let param = FunctionParameter::new().mode(ParameterMode::Variadic);
		assert_eq!(param.mode, Some(ParameterMode::Variadic));
	}

	#[rstest]
	fn test_function_parameter_default_value() {
		let param = FunctionParameter::new().default_value("42");
		assert_eq!(param.default_value.as_ref().unwrap(), "42");
	}

	#[rstest]
	fn test_function_parameter_all_options() {
		let param = FunctionParameter::new()
			.name("my_param")
			.param_type("integer")
			.mode(ParameterMode::InOut)
			.default_value("42");

		assert_eq!(param.name.as_ref().unwrap().to_string(), "my_param");
		assert_eq!(param.param_type.as_ref().unwrap(), "integer");
		assert_eq!(param.mode, Some(ParameterMode::InOut));
		assert_eq!(param.default_value.as_ref().unwrap(), "42");
	}

	#[rstest]
	fn test_function_def_add_parameter_spec() {
		let param = FunctionParameter::new()
			.name("my_param")
			.param_type("integer")
			.mode(ParameterMode::Out);

		let func = FunctionDef::new("my_func").add_parameter_spec(param);

		assert_eq!(func.parameters.len(), 1);
		assert_eq!(
			func.parameters[0].name.as_ref().unwrap().to_string(),
			"my_param"
		);
		assert_eq!(func.parameters[0].param_type.as_ref().unwrap(), "integer");
		assert_eq!(func.parameters[0].mode, Some(ParameterMode::Out));
	}

	// FunctionLanguage tests
	#[rstest]
	fn test_function_language_custom() {
		let lang = FunctionLanguage::Custom("plpython3u".to_string());
		assert_eq!(lang, FunctionLanguage::Custom("plpython3u".to_string()));
	}
}
