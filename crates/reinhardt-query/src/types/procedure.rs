//! Procedure type definitions
//!
//! This module provides types for procedure-related DDL operations:
//!
//! - [`ProcedureDef`]: Procedure definition for CREATE PROCEDURE
//! - [`ProcedureParameter`]: Procedure parameter definition (name, type, mode)
//! - [`ProcedureOperation`]: Operations for ALTER PROCEDURE
//!
//! Procedures are similar to functions but have NO return type.
//!
//! # Examples
//!
//! ```rust
//! use reinhardt_query::types::procedure::{ProcedureDef, ProcedureOperation};
//! use reinhardt_query::types::function::{FunctionLanguage, FunctionBehavior};
//!
//! // CREATE PROCEDURE my_proc() LANGUAGE SQL AS 'SELECT 1'
//! let proc = ProcedureDef::new("my_proc")
//!     .language(FunctionLanguage::Sql)
//!     .body("SELECT 1");
//!
//! // CREATE OR REPLACE PROCEDURE my_proc(a integer, b text)
//! // LANGUAGE PLPGSQL AS '...'
//! let proc = ProcedureDef::new("my_proc")
//!     .or_replace(true)
//!     .add_parameter("a", "integer")
//!     .add_parameter("b", "text")
//!     .language(FunctionLanguage::PlPgSql)
//!     .behavior(FunctionBehavior::Immutable)
//!     .body("BEGIN INSERT INTO log VALUES (a, b); END;");
//! ```

use crate::types::{
	DynIden, IntoIden,
	function::{FunctionBehavior, FunctionLanguage, FunctionSecurity, ParameterMode},
};

/// Procedure definition for CREATE PROCEDURE
///
/// This struct represents a procedure definition, including its name,
/// parameters, language, behavior, security, and body.
///
/// Note: Unlike functions, procedures do NOT have a return type.
///
/// # Examples
///
/// ```rust
/// use reinhardt_query::types::procedure::ProcedureDef;
/// use reinhardt_query::types::function::FunctionLanguage;
///
/// // CREATE PROCEDURE my_proc() LANGUAGE SQL
/// let proc = ProcedureDef::new("my_proc")
///     .language(FunctionLanguage::Sql)
///     .body("SELECT 1");
/// ```
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct ProcedureDef {
	pub(crate) name: DynIden,
	pub(crate) or_replace: bool,
	pub(crate) parameters: Vec<ProcedureParameter>,
	pub(crate) language: Option<FunctionLanguage>,
	pub(crate) behavior: Option<FunctionBehavior>,
	pub(crate) security: Option<FunctionSecurity>,
	pub(crate) body: Option<String>,
}

/// Procedure parameter definition
///
/// Represents a parameter in a procedure signature with optional name, type, and mode.
///
/// # Examples
///
/// ```rust
/// use reinhardt_query::types::procedure::ProcedureParameter;
/// use reinhardt_query::types::function::ParameterMode;
///
/// // IN parameter with name
/// let param = ProcedureParameter::new()
///     .name("my_param")
///     .param_type("integer")
///     .mode(ParameterMode::In);
///
/// // OUT parameter without name
/// let param = ProcedureParameter::new()
///     .param_type("text")
///     .mode(ParameterMode::Out);
/// ```
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct ProcedureParameter {
	pub(crate) name: Option<DynIden>,
	pub(crate) param_type: Option<String>,
	pub(crate) mode: Option<ParameterMode>,
	pub(crate) default_value: Option<String>,
}

/// ALTER PROCEDURE operation types
///
/// Specifies operations that can be performed on procedures.
///
/// # Examples
///
/// ```rust
/// use reinhardt_query::types::procedure::ProcedureOperation;
/// use reinhardt_query::types::{Alias, IntoIden};
///
/// let op = ProcedureOperation::RenameTo(Alias::new("new_proc").into_iden());
/// ```
#[derive(Debug, Clone)]
pub enum ProcedureOperation {
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

impl ProcedureDef {
	/// Create a new procedure definition
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::types::procedure::ProcedureDef;
	///
	/// let proc = ProcedureDef::new("my_proc");
	/// ```
	pub fn new<N: IntoIden>(name: N) -> Self {
		Self {
			name: name.into_iden(),
			or_replace: false,
			parameters: Vec::new(),
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
	/// use reinhardt_query::types::procedure::ProcedureDef;
	///
	/// let proc = ProcedureDef::new("my_proc")
	///     .or_replace(true);
	/// ```
	pub fn or_replace(mut self, or_replace: bool) -> Self {
		self.or_replace = or_replace;
		self
	}

	/// Add a procedure parameter
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::types::procedure::ProcedureDef;
	///
	/// let proc = ProcedureDef::new("my_proc")
	///     .add_parameter("param1", "integer")
	///     .add_parameter("param2", "text");
	/// ```
	pub fn add_parameter<N: IntoIden, T: Into<String>>(mut self, name: N, param_type: T) -> Self {
		self.parameters.push(ProcedureParameter {
			name: Some(name.into_iden()),
			param_type: Some(param_type.into()),
			mode: None,
			default_value: None,
		});
		self
	}

	/// Add a procedure parameter with full specification
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::types::procedure::{ProcedureDef, ProcedureParameter};
	/// use reinhardt_query::types::function::ParameterMode;
	///
	/// let param = ProcedureParameter::new()
	///     .name("my_param")
	///     .param_type("integer")
	///     .mode(ParameterMode::InOut);
	///
	/// let proc = ProcedureDef::new("my_proc")
	///     .add_parameter_spec(param);
	/// ```
	pub fn add_parameter_spec(mut self, param: ProcedureParameter) -> Self {
		self.parameters.push(param);
		self
	}

	/// Set LANGUAGE
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::types::procedure::ProcedureDef;
	/// use reinhardt_query::types::function::FunctionLanguage;
	///
	/// let proc = ProcedureDef::new("my_proc")
	///     .language(FunctionLanguage::PlPgSql);
	/// ```
	pub fn language(mut self, language: FunctionLanguage) -> Self {
		self.language = Some(language);
		self
	}

	/// Set procedure behavior (IMMUTABLE/STABLE/VOLATILE)
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::types::procedure::ProcedureDef;
	/// use reinhardt_query::types::function::FunctionBehavior;
	///
	/// let proc = ProcedureDef::new("my_proc")
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
	/// use reinhardt_query::types::procedure::ProcedureDef;
	/// use reinhardt_query::types::function::FunctionSecurity;
	///
	/// let proc = ProcedureDef::new("my_proc")
	///     .security(FunctionSecurity::Definer);
	/// ```
	pub fn security(mut self, security: FunctionSecurity) -> Self {
		self.security = Some(security);
		self
	}

	/// Set procedure body (AS clause)
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::types::procedure::ProcedureDef;
	///
	/// let proc = ProcedureDef::new("my_proc")
	///     .body("SELECT 1");
	/// ```
	pub fn body<B: Into<String>>(mut self, body: B) -> Self {
		self.body = Some(body.into());
		self
	}
}

impl ProcedureParameter {
	/// Create a new procedure parameter
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::types::procedure::ProcedureParameter;
	///
	/// let param = ProcedureParameter::new();
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
	/// use reinhardt_query::types::procedure::ProcedureParameter;
	///
	/// let param = ProcedureParameter::new()
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
	/// use reinhardt_query::types::procedure::ProcedureParameter;
	///
	/// let param = ProcedureParameter::new()
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
	/// use reinhardt_query::types::procedure::ProcedureParameter;
	/// use reinhardt_query::types::function::ParameterMode;
	///
	/// let param = ProcedureParameter::new()
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
	/// use reinhardt_query::types::procedure::ProcedureParameter;
	///
	/// let param = ProcedureParameter::new()
	///     .default_value("42");
	/// ```
	pub fn default_value<V: Into<String>>(mut self, value: V) -> Self {
		self.default_value = Some(value.into());
		self
	}
}

impl Default for ProcedureParameter {
	fn default() -> Self {
		Self::new()
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::*;

	// ProcedureDef tests
	#[rstest]
	fn test_procedure_def_basic() {
		let proc = ProcedureDef::new("my_proc");
		assert_eq!(proc.name.to_string(), "my_proc");
		assert!(!proc.or_replace);
		assert!(proc.parameters.is_empty());
		assert!(proc.language.is_none());
		assert!(proc.behavior.is_none());
		assert!(proc.security.is_none());
		assert!(proc.body.is_none());
	}

	#[rstest]
	fn test_procedure_def_or_replace() {
		let proc = ProcedureDef::new("my_proc").or_replace(true);
		assert_eq!(proc.name.to_string(), "my_proc");
		assert!(proc.or_replace);
	}

	#[rstest]
	fn test_procedure_def_add_parameter() {
		let proc = ProcedureDef::new("my_proc").add_parameter("param1", "integer");
		assert_eq!(proc.parameters.len(), 1);
		assert_eq!(
			proc.parameters[0].name.as_ref().unwrap().to_string(),
			"param1"
		);
		assert_eq!(proc.parameters[0].param_type.as_ref().unwrap(), "integer");
	}

	#[rstest]
	fn test_procedure_def_multiple_parameters() {
		let proc = ProcedureDef::new("my_proc")
			.add_parameter("param1", "integer")
			.add_parameter("param2", "text");
		assert_eq!(proc.parameters.len(), 2);
		assert_eq!(
			proc.parameters[0].name.as_ref().unwrap().to_string(),
			"param1"
		);
		assert_eq!(proc.parameters[0].param_type.as_ref().unwrap(), "integer");
		assert_eq!(
			proc.parameters[1].name.as_ref().unwrap().to_string(),
			"param2"
		);
		assert_eq!(proc.parameters[1].param_type.as_ref().unwrap(), "text");
	}

	#[rstest]
	fn test_procedure_def_language_sql() {
		let proc = ProcedureDef::new("my_proc").language(FunctionLanguage::Sql);
		assert_eq!(proc.language, Some(FunctionLanguage::Sql));
	}

	#[rstest]
	fn test_procedure_def_language_plpgsql() {
		let proc = ProcedureDef::new("my_proc").language(FunctionLanguage::PlPgSql);
		assert_eq!(proc.language, Some(FunctionLanguage::PlPgSql));
	}

	#[rstest]
	fn test_procedure_def_behavior_immutable() {
		let proc = ProcedureDef::new("my_proc").behavior(FunctionBehavior::Immutable);
		assert_eq!(proc.behavior, Some(FunctionBehavior::Immutable));
	}

	#[rstest]
	fn test_procedure_def_behavior_stable() {
		let proc = ProcedureDef::new("my_proc").behavior(FunctionBehavior::Stable);
		assert_eq!(proc.behavior, Some(FunctionBehavior::Stable));
	}

	#[rstest]
	fn test_procedure_def_behavior_volatile() {
		let proc = ProcedureDef::new("my_proc").behavior(FunctionBehavior::Volatile);
		assert_eq!(proc.behavior, Some(FunctionBehavior::Volatile));
	}

	#[rstest]
	fn test_procedure_def_security_definer() {
		let proc = ProcedureDef::new("my_proc").security(FunctionSecurity::Definer);
		assert_eq!(proc.security, Some(FunctionSecurity::Definer));
	}

	#[rstest]
	fn test_procedure_def_security_invoker() {
		let proc = ProcedureDef::new("my_proc").security(FunctionSecurity::Invoker);
		assert_eq!(proc.security, Some(FunctionSecurity::Invoker));
	}

	#[rstest]
	fn test_procedure_def_body() {
		let proc = ProcedureDef::new("my_proc").body("SELECT 1");
		assert_eq!(proc.body.as_ref().unwrap(), "SELECT 1");
	}

	#[rstest]
	fn test_procedure_def_all_options() {
		let proc = ProcedureDef::new("my_proc")
			.or_replace(true)
			.add_parameter("a", "integer")
			.add_parameter("b", "text")
			.language(FunctionLanguage::PlPgSql)
			.behavior(FunctionBehavior::Immutable)
			.security(FunctionSecurity::Definer)
			.body("BEGIN INSERT INTO log VALUES (a, b); END;");

		assert_eq!(proc.name.to_string(), "my_proc");
		assert!(proc.or_replace);
		assert_eq!(proc.parameters.len(), 2);
		assert_eq!(proc.language, Some(FunctionLanguage::PlPgSql));
		assert_eq!(proc.behavior, Some(FunctionBehavior::Immutable));
		assert_eq!(proc.security, Some(FunctionSecurity::Definer));
		assert_eq!(
			proc.body.as_ref().unwrap(),
			"BEGIN INSERT INTO log VALUES (a, b); END;"
		);
	}

	// ProcedureParameter tests
	#[rstest]
	fn test_procedure_parameter_basic() {
		let param = ProcedureParameter::new();
		assert!(param.name.is_none());
		assert!(param.param_type.is_none());
		assert!(param.mode.is_none());
		assert!(param.default_value.is_none());
	}

	#[rstest]
	fn test_procedure_parameter_name() {
		let param = ProcedureParameter::new().name("my_param");
		assert_eq!(param.name.as_ref().unwrap().to_string(), "my_param");
	}

	#[rstest]
	fn test_procedure_parameter_type() {
		let param = ProcedureParameter::new().param_type("integer");
		assert_eq!(param.param_type.as_ref().unwrap(), "integer");
	}

	#[rstest]
	fn test_procedure_parameter_mode_in() {
		let param = ProcedureParameter::new().mode(ParameterMode::In);
		assert_eq!(param.mode, Some(ParameterMode::In));
	}

	#[rstest]
	fn test_procedure_parameter_mode_out() {
		let param = ProcedureParameter::new().mode(ParameterMode::Out);
		assert_eq!(param.mode, Some(ParameterMode::Out));
	}

	#[rstest]
	fn test_procedure_parameter_mode_inout() {
		let param = ProcedureParameter::new().mode(ParameterMode::InOut);
		assert_eq!(param.mode, Some(ParameterMode::InOut));
	}

	#[rstest]
	fn test_procedure_parameter_mode_variadic() {
		let param = ProcedureParameter::new().mode(ParameterMode::Variadic);
		assert_eq!(param.mode, Some(ParameterMode::Variadic));
	}

	#[rstest]
	fn test_procedure_parameter_default_value() {
		let param = ProcedureParameter::new().default_value("42");
		assert_eq!(param.default_value.as_ref().unwrap(), "42");
	}

	#[rstest]
	fn test_procedure_parameter_all_options() {
		let param = ProcedureParameter::new()
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
	fn test_procedure_def_add_parameter_spec() {
		let param = ProcedureParameter::new()
			.name("my_param")
			.param_type("integer")
			.mode(ParameterMode::Out);

		let proc = ProcedureDef::new("my_proc").add_parameter_spec(param);

		assert_eq!(proc.parameters.len(), 1);
		assert_eq!(
			proc.parameters[0].name.as_ref().unwrap().to_string(),
			"my_param"
		);
		assert_eq!(proc.parameters[0].param_type.as_ref().unwrap(), "integer");
		assert_eq!(proc.parameters[0].mode, Some(ParameterMode::Out));
	}

	// ProcedureOperation tests
	#[rstest]
	fn test_procedure_operation_rename_to() {
		use crate::types::Alias;
		let op = ProcedureOperation::RenameTo(Alias::new("new_proc").into_iden());
		match op {
			ProcedureOperation::RenameTo(name) => {
				assert_eq!(name.to_string(), "new_proc");
			}
			_ => panic!("Expected RenameTo operation"),
		}
	}

	#[rstest]
	fn test_procedure_operation_owner_to() {
		use crate::types::Alias;
		let op = ProcedureOperation::OwnerTo(Alias::new("new_owner").into_iden());
		match op {
			ProcedureOperation::OwnerTo(owner) => {
				assert_eq!(owner.to_string(), "new_owner");
			}
			_ => panic!("Expected OwnerTo operation"),
		}
	}

	#[rstest]
	fn test_procedure_operation_set_schema() {
		use crate::types::Alias;
		let op = ProcedureOperation::SetSchema(Alias::new("new_schema").into_iden());
		match op {
			ProcedureOperation::SetSchema(schema) => {
				assert_eq!(schema.to_string(), "new_schema");
			}
			_ => panic!("Expected SetSchema operation"),
		}
	}

	#[rstest]
	fn test_procedure_operation_set_behavior() {
		let op = ProcedureOperation::SetBehavior(FunctionBehavior::Immutable);
		match op {
			ProcedureOperation::SetBehavior(behavior) => {
				assert_eq!(behavior, FunctionBehavior::Immutable);
			}
			_ => panic!("Expected SetBehavior operation"),
		}
	}

	#[rstest]
	fn test_procedure_operation_set_security() {
		let op = ProcedureOperation::SetSecurity(FunctionSecurity::Definer);
		match op {
			ProcedureOperation::SetSecurity(security) => {
				assert_eq!(security, FunctionSecurity::Definer);
			}
			_ => panic!("Expected SetSecurity operation"),
		}
	}
}
