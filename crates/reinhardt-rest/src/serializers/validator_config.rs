//! Validator configuration for ModelSerializer
//!
//! This module provides configuration structures for managing validators
//! in ModelSerializer instances.

use super::validators::{DatabaseValidatorError, UniqueTogetherValidator, UniqueValidator};
use reinhardt_db::backends::DatabaseConnection;
use reinhardt_db::orm::Model;
use serde::Serialize;
use std::marker::PhantomData;

/// Configuration for field validators
#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct ValidatorConfig<M: Model> {
	unique_validators: Vec<UniqueValidator<M>>,
	unique_together_validators: Vec<UniqueTogetherValidator<M>>,
	_phantom: PhantomData<M>,
}

impl<M: Model> ValidatorConfig<M> {
	/// Create a new empty validator configuration
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::serializers::validator_config::ValidatorConfig;
	/// use reinhardt_db::orm::Model;
	/// use serde::{Serialize, Deserialize};
	///
	/// #[derive(Debug, Clone, Serialize, Deserialize)]
	/// struct User {
	///     id: Option<i64>,
	///     username: String,
	/// }
	///
	/// impl Model for User {
	///     type PrimaryKey = i64;
	///     type Fields = UserFields;
	///     fn table_name() -> &'static str { "users" }
	///     fn primary_key(&self) -> Option<Self::PrimaryKey> { self.id }
	///     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
	///     fn new_fields() -> Self::Fields { UserFields }
	/// }
	/// #[derive(Clone)]
	/// struct UserFields;
	/// impl reinhardt_db::orm::FieldSelector for UserFields {
	///     fn with_alias(self, _alias: &str) -> Self { self }
	/// }
	///
	/// let config = ValidatorConfig::<User>::new();
	/// ```
	pub fn new() -> Self {
		Self {
			unique_validators: Vec::new(),
			unique_together_validators: Vec::new(),
			_phantom: PhantomData,
		}
	}

	/// Add a unique field validator
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::serializers::validator_config::ValidatorConfig;
	/// use reinhardt_rest::serializers::validators::UniqueValidator;
	/// use reinhardt_db::orm::Model;
	/// use serde::{Serialize, Deserialize};
	///
	/// #[derive(Debug, Clone, Serialize, Deserialize)]
	/// struct User {
	///     id: Option<i64>,
	///     username: String,
	/// }
	///
	/// impl Model for User {
	///     type PrimaryKey = i64;
	///     type Fields = UserFields;
	///     fn table_name() -> &'static str { "users" }
	///     fn primary_key(&self) -> Option<Self::PrimaryKey> { self.id }
	///     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
	///     fn new_fields() -> Self::Fields { UserFields }
	/// }
	/// #[derive(Clone)]
	/// struct UserFields;
	/// impl reinhardt_db::orm::FieldSelector for UserFields {
	///     fn with_alias(self, _alias: &str) -> Self { self }
	/// }
	///
	/// let mut config = ValidatorConfig::<User>::new();
	/// config.add_unique_validator(UniqueValidator::new("username"));
	/// ```
	pub fn add_unique_validator(&mut self, validator: UniqueValidator<M>) {
		self.unique_validators.push(validator);
	}

	/// Add a unique together validator
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::serializers::validator_config::ValidatorConfig;
	/// use reinhardt_rest::serializers::validators::UniqueTogetherValidator;
	/// use reinhardt_db::orm::Model;
	/// use serde::{Serialize, Deserialize};
	///
	/// #[derive(Debug, Clone, Serialize, Deserialize)]
	/// struct User {
	///     id: Option<i64>,
	///     username: String,
	///     email: String,
	/// }
	///
	/// impl Model for User {
	///     type PrimaryKey = i64;
	///     type Fields = UserFields;
	///     fn table_name() -> &'static str { "users" }
	///     fn primary_key(&self) -> Option<Self::PrimaryKey> { self.id }
	///     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
	///     fn new_fields() -> Self::Fields { UserFields }
	/// }
	/// #[derive(Clone)]
	/// struct UserFields;
	/// impl reinhardt_db::orm::FieldSelector for UserFields {
	///     fn with_alias(self, _alias: &str) -> Self { self }
	/// }
	///
	/// let mut config = ValidatorConfig::<User>::new();
	/// config.add_unique_together_validator(
	///     UniqueTogetherValidator::new(vec!["username", "email"])
	/// );
	/// ```
	pub fn add_unique_together_validator(&mut self, validator: UniqueTogetherValidator<M>) {
		self.unique_together_validators.push(validator);
	}

	/// Get all unique validators
	pub fn unique_validators(&self) -> &[UniqueValidator<M>] {
		&self.unique_validators
	}

	/// Get all unique together validators
	pub fn unique_together_validators(&self) -> &[UniqueTogetherValidator<M>] {
		&self.unique_together_validators
	}

	/// Check if any validators are configured
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::serializers::validator_config::ValidatorConfig;
	/// use reinhardt_rest::serializers::validators::UniqueValidator;
	/// use reinhardt_db::orm::Model;
	/// use serde::{Serialize, Deserialize};
	///
	/// #[derive(Debug, Clone, Serialize, Deserialize)]
	/// struct User {
	///     id: Option<i64>,
	///     username: String,
	/// }
	///
	/// impl Model for User {
	///     type PrimaryKey = i64;
	///     type Fields = UserFields;
	///     fn table_name() -> &'static str { "users" }
	///     fn primary_key(&self) -> Option<Self::PrimaryKey> { self.id }
	///     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
	///     fn new_fields() -> Self::Fields { UserFields }
	/// }
	/// #[derive(Clone)]
	/// struct UserFields;
	/// impl reinhardt_db::orm::FieldSelector for UserFields {
	///     fn with_alias(self, _alias: &str) -> Self { self }
	/// }
	///
	/// let mut config = ValidatorConfig::<User>::new();
	/// assert!(!config.has_validators());
	///
	/// config.add_unique_validator(UniqueValidator::new("username"));
	/// assert!(config.has_validators());
	/// ```
	pub fn has_validators(&self) -> bool {
		!self.unique_validators.is_empty() || !self.unique_together_validators.is_empty()
	}

	/// Validate model instance asynchronously against configured validators
	///
	/// Performs database-backed validation checks (uniqueness constraints).
	/// Converts the model instance to JSON for field extraction.
	///
	/// # Arguments
	///
	/// * `connection` - Database connection for validation queries
	/// * `instance` - Model instance to validate
	/// * `instance_pk` - Optional primary key (for update operations, excludes current record)
	///
	/// # Errors
	///
	/// Returns `DatabaseValidatorError` if:
	/// - Serialization fails
	/// - Field not found in serialized data
	/// - Unique constraint violated
	/// - Unique together constraint violated
	/// - Database query fails
	///
	/// # Examples
	///
	/// ```ignore
	/// use reinhardt_rest::serializers::validator_config::ValidatorConfig;
	/// use reinhardt_db::connection::DatabaseConnection;
	///
	/// let config = ValidatorConfig::new();
	/// let user = User { id: None, username: "alice".into() };
	/// config.validate_async(&connection, &user, None).await?;
	/// ```
	pub async fn validate_async(
		&self,
		connection: &DatabaseConnection,
		instance: &M,
		instance_pk: Option<&M::PrimaryKey>,
	) -> Result<(), DatabaseValidatorError>
	where
		M: Serialize,
		M::PrimaryKey: std::fmt::Display,
	{
		// Convert model instance to JSON for field extraction
		let value =
			serde_json::to_value(instance).map_err(|e| DatabaseValidatorError::DatabaseError {
				message: format!("Failed to serialize model: {}", e),
				query: None,
			})?;

		let obj = value
			.as_object()
			.ok_or_else(|| DatabaseValidatorError::DatabaseError {
				message: "Model must serialize to an object".to_string(),
				query: None,
			})?;

		// Validate unique constraints
		for validator in &self.unique_validators {
			let field_value = obj
				.get(validator.field_name())
				.and_then(|v| v.as_str())
				.ok_or_else(|| DatabaseValidatorError::FieldNotFound {
					field: validator.field_name().to_string(),
				})?;

			validator
				.validate(connection, field_value, instance_pk)
				.await?;
		}

		// Validate unique together constraints
		for validator in &self.unique_together_validators {
			let mut values = std::collections::HashMap::new();
			for field in validator.field_names() {
				let value = obj.get(field).and_then(|v| v.as_str()).ok_or_else(|| {
					DatabaseValidatorError::FieldNotFound {
						field: field.clone(),
					}
				})?;
				values.insert(field.clone(), value.to_string());
			}

			validator.validate(connection, &values, instance_pk).await?;
		}

		Ok(())
	}
}

impl<M: Model> Default for ValidatorConfig<M> {
	fn default() -> Self {
		Self::new()
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::serializers::validators::{UniqueTogetherValidator, UniqueValidator};
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
	fn test_validator_config_new() {
		let config = ValidatorConfig::<TestUser>::new();
		assert_eq!(config.unique_validators().len(), 0);
		assert_eq!(config.unique_together_validators().len(), 0);
		assert!(!config.has_validators());
	}

	#[test]
	fn test_add_unique_validator() {
		let mut config = ValidatorConfig::<TestUser>::new();
		config.add_unique_validator(UniqueValidator::new("username"));

		assert_eq!(config.unique_validators().len(), 1);
		assert!(config.has_validators());
	}

	#[test]
	fn test_add_unique_together_validator() {
		let mut config = ValidatorConfig::<TestUser>::new();
		config
			.add_unique_together_validator(UniqueTogetherValidator::new(vec!["username", "email"]));

		assert_eq!(config.unique_together_validators().len(), 1);
		assert!(config.has_validators());
	}

	#[test]
	fn test_multiple_validators() {
		let mut config = ValidatorConfig::<TestUser>::new();
		config.add_unique_validator(UniqueValidator::new("username"));
		config.add_unique_validator(UniqueValidator::new("email"));
		config
			.add_unique_together_validator(UniqueTogetherValidator::new(vec!["username", "email"]));

		assert_eq!(config.unique_validators().len(), 2);
		assert_eq!(config.unique_together_validators().len(), 1);
		assert!(config.has_validators());
	}
}
