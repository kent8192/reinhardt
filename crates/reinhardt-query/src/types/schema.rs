//! Schema type definitions
//!
//! This module provides types for schema-related DDL operations:
//!
//! - [`SchemaDef`]: Schema definition for CREATE SCHEMA

use crate::types::{DynIden, IntoIden};

/// Schema definition for CREATE SCHEMA
///
/// This struct represents a schema definition, including its name,
/// IF NOT EXISTS clause, and authorization owner.
///
/// # Examples
///
/// ```rust
/// use reinhardt_query::types::schema::SchemaDef;
///
/// // CREATE SCHEMA my_schema
/// let schema = SchemaDef::new("my_schema");
///
/// // CREATE SCHEMA IF NOT EXISTS my_schema
/// let schema = SchemaDef::new("my_schema")
///     .if_not_exists(true);
///
/// // CREATE SCHEMA my_schema AUTHORIZATION owner_user
/// let schema = SchemaDef::new("my_schema")
///     .authorization("owner_user");
/// ```
#[derive(Debug, Clone)]
// Fields will be used by query builders (CreateSchemaStatement, etc.)
#[allow(dead_code)]
pub struct SchemaDef {
	pub(crate) name: DynIden,
	pub(crate) if_not_exists: bool,
	pub(crate) authorization: Option<DynIden>,
}

impl SchemaDef {
	/// Create a new schema definition
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::types::schema::SchemaDef;
	///
	/// let schema = SchemaDef::new("my_schema");
	/// ```
	pub fn new<N: IntoIden>(name: N) -> Self {
		Self {
			name: name.into_iden(),
			if_not_exists: false,
			authorization: None,
		}
	}

	/// Set IF NOT EXISTS clause
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::types::schema::SchemaDef;
	///
	/// let schema = SchemaDef::new("my_schema")
	///     .if_not_exists(true);
	/// ```
	pub fn if_not_exists(mut self, if_not_exists: bool) -> Self {
		self.if_not_exists = if_not_exists;
		self
	}

	/// Set AUTHORIZATION owner
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::types::schema::SchemaDef;
	///
	/// let schema = SchemaDef::new("my_schema")
	///     .authorization("owner_user");
	/// ```
	pub fn authorization<O: IntoIden>(mut self, owner: O) -> Self {
		self.authorization = Some(owner.into_iden());
		self
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::*;

	#[rstest]
	fn test_schema_def_basic() {
		let schema = SchemaDef::new("my_schema");
		assert_eq!(schema.name.to_string(), "my_schema");
		assert!(!schema.if_not_exists);
		assert!(schema.authorization.is_none());
	}

	#[rstest]
	fn test_schema_def_if_not_exists() {
		let schema = SchemaDef::new("my_schema").if_not_exists(true);
		assert_eq!(schema.name.to_string(), "my_schema");
		assert!(schema.if_not_exists);
	}

	#[rstest]
	fn test_schema_def_with_authorization() {
		let schema = SchemaDef::new("my_schema").authorization("owner_user");
		assert_eq!(schema.name.to_string(), "my_schema");
		assert!(!schema.if_not_exists);
		assert_eq!(
			schema.authorization.as_ref().unwrap().to_string(),
			"owner_user"
		);
	}

	#[rstest]
	fn test_schema_def_all_options() {
		let schema = SchemaDef::new("my_schema")
			.if_not_exists(true)
			.authorization("owner_user");
		assert_eq!(schema.name.to_string(), "my_schema");
		assert!(schema.if_not_exists);
		assert_eq!(
			schema.authorization.as_ref().unwrap().to_string(),
			"owner_user"
		);
	}
}
