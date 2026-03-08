//! Validators for ModelSerializer
//!
//! This module provides validators for enforcing database constraints
//! such as uniqueness of fields.
//!
//! # Examples
//!
//! ```no_run
//! use reinhardt_rest::serializers::validators::{UniqueValidator, UniqueTogetherValidator};
//! use reinhardt_db::orm::Model;
//! use reinhardt_db::backends::DatabaseConnection;
//! use serde::{Serialize, Deserialize};
//!
//! #[derive(Debug, Clone, Serialize, Deserialize)]
//! struct User {
//!     id: Option<i64>,
//!     username: String,
//!     email: String,
//! }
//!
//! impl Model for User {
//!     type PrimaryKey = i64;
//!     type Fields = UserFields;
//!     fn table_name() -> &'static str { "users" }
//!     fn primary_key(&self) -> Option<Self::PrimaryKey> { self.id }
//!     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
//!     fn new_fields() -> Self::Fields { UserFields }
//! }
//! #[derive(Clone)]
//! struct UserFields;
//! impl reinhardt_db::orm::FieldSelector for UserFields {
//!     fn with_alias(self, _alias: &str) -> Self { self }
//! }
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let connection = DatabaseConnection::connect_postgres("postgres://localhost/test").await?;
//!
//! // Validate that username is unique
//! let validator = UniqueValidator::<User>::new("username");
//! validator.validate(&connection, "alice", None).await?;
//!
//! // Validate that (username, email) combination is unique
//! let mut values = std::collections::HashMap::new();
//! values.insert("username".to_string(), "alice".to_string());
//! values.insert("email".to_string(), "alice@example.com".to_string());
//!
//! let validator = UniqueTogetherValidator::<User>::new(vec!["username", "email"]);
//! validator.validate(&connection, &values, None).await?;
//! # Ok(())
//! # }
//! ```

use super::SerializerError;
use reinhardt_db::backends::DatabaseConnection;
use reinhardt_db::orm::{Filter, FilterOperator, FilterValue, Model};
use std::marker::PhantomData;
use thiserror::Error;

/// Errors that can occur during database validation
#[derive(Debug, Error, Clone, PartialEq)]
pub enum DatabaseValidatorError {
	/// A unique constraint was violated for a single field
	#[error("Unique constraint violated: {field} = '{value}' already exists in table {table}")]
	UniqueConstraintViolation {
		/// The field name that violated the constraint
		field: String,
		/// The value that caused the violation
		value: String,
		/// The table name
		table: String,
		/// Optional custom message
		message: Option<String>,
	},

	/// A unique together constraint was violated for multiple fields
	#[error(
		"Unique together constraint violated: fields ({fields:?}) with values ({values:?}) already exist in table {table}"
	)]
	UniqueTogetherViolation {
		/// The field names that violated the constraint
		fields: Vec<String>,
		/// The values that caused the violation
		values: Vec<String>,
		/// The table name
		table: String,
		/// Optional custom message
		message: Option<String>,
	},

	/// A database error occurred during validation
	#[error("Database error during validation: {message}")]
	DatabaseError {
		/// The error message from the database
		message: String,
		/// The SQL query that failed (optional, for debugging)
		query: Option<String>,
	},

	/// A required field was not found in the data
	#[error("Required field '{field}' not found in validation data")]
	FieldNotFound {
		/// The field name that was missing
		field: String,
	},
}

impl From<DatabaseValidatorError> for SerializerError {
	fn from(err: DatabaseValidatorError) -> Self {
		SerializerError::Other {
			message: err.to_string(),
		}
	}
}

impl From<DatabaseValidatorError> for reinhardt_core::exception::Error {
	fn from(err: DatabaseValidatorError) -> Self {
		match err {
			DatabaseValidatorError::UniqueConstraintViolation {
				field,
				value,
				table,
				message,
			} => {
				let msg = message.unwrap_or_else(|| {
					format!(
						"Field '{}' with value '{}' already exists in {}",
						field, value, table
					)
				});
				reinhardt_core::exception::Error::Conflict(msg)
			}
			DatabaseValidatorError::UniqueTogetherViolation {
				fields,
				values,
				table,
				message,
			} => {
				let msg = message.unwrap_or_else(|| {
					format!(
						"Combination of fields {:?} with values {:?} already exists in {}",
						fields, values, table
					)
				});
				reinhardt_core::exception::Error::Conflict(msg)
			}
			DatabaseValidatorError::FieldNotFound { field } => {
				reinhardt_core::exception::Error::Validation(format!(
					"Required field '{}' not found",
					field
				))
			}
			DatabaseValidatorError::DatabaseError { message, .. } => {
				reinhardt_core::exception::Error::Database(message)
			}
		}
	}
}

/// UniqueValidator ensures that a field value is unique in the database
///
/// This validator checks that a given field value doesn't already exist
/// in the database table, with optional support for excluding the current
/// instance during updates.
///
/// # Examples
///
/// ```no_run
/// # use reinhardt_rest::serializers::validators::UniqueValidator;
/// # use reinhardt_db::orm::Model;
/// # use reinhardt_db::backends::DatabaseConnection;
/// # use serde::{Serialize, Deserialize};
/// #
/// # #[derive(Debug, Clone, Serialize, Deserialize)]
/// # struct User {
/// #     id: Option<i64>,
/// #     username: String,
/// # }
/// #
/// # impl Model for User {
/// #     type PrimaryKey = i64;
/// #     type Fields = UserFields;
/// #     fn table_name() -> &'static str { "users" }
/// #     fn primary_key(&self) -> Option<Self::PrimaryKey> { self.id }
/// #     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
/// #     fn new_fields() -> Self::Fields { UserFields }
/// # }
/// # #[derive(Clone)]
/// # struct UserFields;
/// # impl reinhardt_db::orm::FieldSelector for UserFields {
/// #     fn with_alias(self, _alias: &str) -> Self { self }
/// # }
/// #
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let connection = DatabaseConnection::connect_postgres("postgres://localhost/test").await?;
/// let validator = UniqueValidator::<User>::new("username");
///
/// // Check if "alice" is unique
/// validator.validate(&connection, "alice", None).await?;
///
/// // Check if "alice" is unique, excluding user with id=1
/// let user_id = 1i64;
/// validator.validate(&connection, "alice", Some(&user_id)).await?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct UniqueValidator<M: Model> {
	field_name: String,
	message: Option<String>,
	_phantom: PhantomData<M>,
}

impl<M: Model> UniqueValidator<M> {
	/// Create a new UniqueValidator for the specified field
	///
	/// # Examples
	///
	/// ```
	/// # use reinhardt_rest::serializers::validators::UniqueValidator;
	/// # use reinhardt_db::orm::Model;
	/// # use serde::{Serialize, Deserialize};
	/// #
	/// # #[derive(Debug, Clone, Serialize, Deserialize)]
	/// # struct User { id: Option<i64>, username: String }
	/// #
	/// # impl Model for User {
	/// #     type PrimaryKey = i64;
	/// #     type Fields = UserFields;
	/// #     fn table_name() -> &'static str { "users" }
	/// #     fn primary_key(&self) -> Option<Self::PrimaryKey> { self.id }
	/// #     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
	/// #     fn new_fields() -> Self::Fields { UserFields }
	/// # }
	/// # #[derive(Clone)]
	/// # struct UserFields;
	/// # impl reinhardt_db::orm::FieldSelector for UserFields {
	/// #     fn with_alias(self, _alias: &str) -> Self { self }
	/// # }
	/// let validator = UniqueValidator::<User>::new("username");
	/// // Verify the validator is created successfully
	/// let _: UniqueValidator<User> = validator;
	/// ```
	pub fn new(field_name: impl Into<String>) -> Self {
		Self {
			field_name: field_name.into(),
			message: None,
			_phantom: PhantomData,
		}
	}

	/// Set a custom error message
	///
	/// # Examples
	///
	/// ```
	/// # use reinhardt_rest::serializers::validators::UniqueValidator;
	/// # use reinhardt_db::orm::Model;
	/// # use serde::{Serialize, Deserialize};
	/// #
	/// # #[derive(Debug, Clone, Serialize, Deserialize)]
	/// # struct User { id: Option<i64>, username: String }
	/// #
	/// # impl Model for User {
	/// #     type PrimaryKey = i64;
	/// #     type Fields = UserFields;
	/// #     fn table_name() -> &'static str { "users" }
	/// #     fn primary_key(&self) -> Option<Self::PrimaryKey> { self.id }
	/// #     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
	/// #     fn new_fields() -> Self::Fields { UserFields }
	/// # }
	/// # #[derive(Clone)]
	/// # struct UserFields;
	/// # impl reinhardt_db::orm::FieldSelector for UserFields {
	/// #     fn with_alias(self, _alias: &str) -> Self { self }
	/// # }
	/// let validator = UniqueValidator::<User>::new("username")
	///     .with_message("Username must be unique");
	/// // Verify the validator is configured with custom message
	/// let _: UniqueValidator<User> = validator;
	/// ```
	pub fn with_message(mut self, message: impl Into<String>) -> Self {
		self.message = Some(message.into());
		self
	}

	/// Get the field name being validated
	pub fn field_name(&self) -> &str {
		&self.field_name
	}

	/// Validates that the given value is unique for this field in the database.
	pub async fn validate(
		&self,
		_connection: &DatabaseConnection,
		value: &str,
		instance_pk: Option<&M::PrimaryKey>,
	) -> Result<(), DatabaseValidatorError>
	where
		M::PrimaryKey: std::fmt::Display,
	{
		let table_name = M::table_name();

		// Build QuerySet with filter
		let mut qs = M::objects().all();
		qs = qs.filter(Filter::new(
			self.field_name.clone(),
			FilterOperator::Eq,
			FilterValue::String(value.to_string()),
		));

		// Exclude current instance if updating
		if let Some(pk) = instance_pk {
			qs = qs.filter(Filter::new(
				M::primary_key_field().to_string(),
				FilterOperator::Ne,
				FilterValue::String(pk.to_string()),
			));
		}

		// Execute count query
		let count = qs
			.count()
			.await
			.map_err(|e| DatabaseValidatorError::DatabaseError {
				message: e.to_string(),
				query: None,
			})?;

		if count > 0 {
			Err(DatabaseValidatorError::UniqueConstraintViolation {
				field: self.field_name.clone(),
				value: value.to_string(),
				table: table_name.to_string(),
				message: self.message.clone(),
			})
		} else {
			Ok(())
		}
	}
}

/// UniqueTogetherValidator ensures that a combination of fields is unique
///
/// This validator checks that a combination of field values doesn't already exist
/// in the database table, with optional support for excluding the current
/// instance during updates.
///
/// # Examples
///
/// ```no_run
/// # use reinhardt_rest::serializers::validators::UniqueTogetherValidator;
/// # use reinhardt_db::orm::Model;
/// # use reinhardt_db::backends::DatabaseConnection;
/// # use serde::{Serialize, Deserialize};
/// # use std::collections::HashMap;
/// #
/// # #[derive(Debug, Clone, Serialize, Deserialize)]
/// # struct User {
/// #     id: Option<i64>,
/// #     username: String,
/// #     email: String,
/// # }
/// #
/// # impl Model for User {
/// #     type PrimaryKey = i64;
/// #     type Fields = UserFields;
/// #     fn table_name() -> &'static str { "users" }
/// #     fn primary_key(&self) -> Option<Self::PrimaryKey> { self.id }
/// #     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
/// #     fn new_fields() -> Self::Fields { UserFields }
/// # }
/// # #[derive(Clone)]
/// # struct UserFields;
/// # impl reinhardt_db::orm::FieldSelector for UserFields {
/// #     fn with_alias(self, _alias: &str) -> Self { self }
/// # }
/// #
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let connection = DatabaseConnection::connect_postgres("postgres://localhost/test").await?;
/// let validator = UniqueTogetherValidator::<User>::new(vec!["username", "email"]);
///
/// let mut values = HashMap::new();
/// values.insert("username".to_string(), "alice".to_string());
/// values.insert("email".to_string(), "alice@example.com".to_string());
///
/// validator.validate(&connection, &values, None).await?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct UniqueTogetherValidator<M: Model> {
	field_names: Vec<String>,
	message: Option<String>,
	_phantom: PhantomData<M>,
}

impl<M: Model> UniqueTogetherValidator<M> {
	/// Create a new UniqueTogetherValidator for the specified fields
	///
	/// # Examples
	///
	/// ```
	/// # use reinhardt_rest::serializers::validators::UniqueTogetherValidator;
	/// # use reinhardt_db::orm::Model;
	/// # use serde::{Serialize, Deserialize};
	/// #
	/// # #[derive(Debug, Clone, Serialize, Deserialize)]
	/// # struct User { id: Option<i64>, username: String, email: String }
	/// #
	/// # impl Model for User {
	/// #     type PrimaryKey = i64;
	/// #     type Fields = UserFields;
	/// #     fn table_name() -> &'static str { "users" }
	/// #     fn primary_key(&self) -> Option<Self::PrimaryKey> { self.id }
	/// #     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
	/// #     fn new_fields() -> Self::Fields { UserFields }
	/// # }
	/// # #[derive(Clone)]
	/// # struct UserFields;
	/// # impl reinhardt_db::orm::FieldSelector for UserFields {
	/// #     fn with_alias(self, _alias: &str) -> Self { self }
	/// # }
	/// let validator = UniqueTogetherValidator::<User>::new(vec!["username", "email"]);
	/// // Verify the validator is created successfully
	/// let _: UniqueTogetherValidator<User> = validator;
	/// ```
	pub fn new(field_names: Vec<impl Into<String>>) -> Self {
		Self {
			field_names: field_names.into_iter().map(|f| f.into()).collect(),
			message: None,
			_phantom: PhantomData,
		}
	}

	/// Set a custom error message
	///
	/// # Examples
	///
	/// ```
	/// # use reinhardt_rest::serializers::validators::UniqueTogetherValidator;
	/// # use reinhardt_db::orm::Model;
	/// # use serde::{Serialize, Deserialize};
	/// #
	/// # #[derive(Debug, Clone, Serialize, Deserialize)]
	/// # struct User { id: Option<i64>, username: String, email: String }
	/// #
	/// # impl Model for User {
	/// #     type PrimaryKey = i64;
	/// #     type Fields = UserFields;
	/// #     fn table_name() -> &'static str { "users" }
	/// #     fn primary_key(&self) -> Option<Self::PrimaryKey> { self.id }
	/// #     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
	/// #     fn new_fields() -> Self::Fields { UserFields }
	/// # }
	/// # #[derive(Clone)]
	/// # struct UserFields;
	/// # impl reinhardt_db::orm::FieldSelector for UserFields {
	/// #     fn with_alias(self, _alias: &str) -> Self { self }
	/// # }
	/// let validator = UniqueTogetherValidator::<User>::new(vec!["username", "email"])
	///     .with_message("Username and email combination must be unique");
	/// // Verify the validator is configured with custom message
	/// let _: UniqueTogetherValidator<User> = validator;
	/// ```
	pub fn with_message(mut self, message: impl Into<String>) -> Self {
		self.message = Some(message.into());
		self
	}

	/// Get the field names being validated
	pub fn field_names(&self) -> &[String] {
		&self.field_names
	}

	/// Validates that the combination of field values is unique together in the database.
	pub async fn validate(
		&self,
		_connection: &DatabaseConnection,
		values: &std::collections::HashMap<String, String>,
		instance_pk: Option<&M::PrimaryKey>,
	) -> Result<(), DatabaseValidatorError>
	where
		M::PrimaryKey: std::fmt::Display,
	{
		let table_name = M::table_name();

		// Build QuerySet with filters for all fields
		let mut qs = M::objects().all();
		let mut field_values = Vec::new();

		for field_name in &self.field_names {
			let value =
				values
					.get(field_name)
					.ok_or_else(|| DatabaseValidatorError::FieldNotFound {
						field: field_name.clone(),
					})?;
			field_values.push(value.clone());

			qs = qs.filter(Filter::new(
				field_name.clone(),
				FilterOperator::Eq,
				FilterValue::String(value.clone()),
			));
		}

		// Exclude current instance if updating
		if let Some(pk) = instance_pk {
			qs = qs.filter(Filter::new(
				M::primary_key_field().to_string(),
				FilterOperator::Ne,
				FilterValue::String(pk.to_string()),
			));
		}

		// Execute count query
		let count = qs
			.count()
			.await
			.map_err(|e| DatabaseValidatorError::DatabaseError {
				message: e.to_string(),
				query: None,
			})?;

		if count > 0 {
			Err(DatabaseValidatorError::UniqueTogetherViolation {
				fields: self.field_names.clone(),
				values: field_values,
				table: table_name.to_string(),
				message: self.message.clone(),
			})
		} else {
			Ok(())
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use reinhardt_db::orm::FieldSelector;

	#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
	struct TestUser {
		id: Option<i64>,
		username: String,
		email: String,
	}

	#[derive(Debug, Clone)]
	struct TestUserFields;

	impl FieldSelector for TestUserFields {
		fn with_alias(self, _alias: &str) -> Self {
			self
		}
	}

	impl Model for TestUser {
		type PrimaryKey = i64;
		type Fields = TestUserFields;

		fn table_name() -> &'static str {
			"test_users"
		}

		fn new_fields() -> Self::Fields {
			TestUserFields
		}

		fn primary_key(&self) -> Option<Self::PrimaryKey> {
			self.id
		}

		fn set_primary_key(&mut self, value: Self::PrimaryKey) {
			self.id = Some(value);
		}
	}

	#[test]
	fn test_unique_validator_new() {
		let validator = UniqueValidator::<TestUser>::new("username");
		assert_eq!(validator.field_name(), "username");
	}

	#[test]
	fn test_unique_validator_with_message() {
		let validator =
			UniqueValidator::<TestUser>::new("username").with_message("Custom error message");
		assert_eq!(validator.field_name(), "username");
		assert!(validator.message.is_some());
	}

	#[test]
	fn test_unique_together_validator_new() {
		let validator = UniqueTogetherValidator::<TestUser>::new(vec!["username", "email"]);
		assert_eq!(validator.field_names().len(), 2);
		assert_eq!(validator.field_names()[0], "username");
		assert_eq!(validator.field_names()[1], "email");
	}

	#[test]
	fn test_unique_together_validator_with_message() {
		let validator = UniqueTogetherValidator::<TestUser>::new(vec!["username", "email"])
			.with_message("Custom combination message");
		assert_eq!(validator.field_names().len(), 2);
		assert!(validator.message.is_some());
	}
}
