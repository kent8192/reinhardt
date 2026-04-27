//! ModelSerializer - Django REST Framework inspired model serialization
//!
//! This module provides ModelSerializer that automatically generates
//! serialization logic from ORM models.

use super::introspection::{FieldInfo, FieldIntrospector};
use super::meta::MetaConfig;
use super::nested_config::{NestedFieldConfig, NestedSerializerConfig};
use super::validator_config::{ModelLevelValidator, ValidatorConfig};
use super::validators::{UniqueTogetherValidator, UniqueValidator};
use super::{Serializer, SerializerError, ValidatorError};
use reinhardt_db::backends::DatabaseConnection;
use reinhardt_db::orm::Model;
use std::collections::HashMap;
use std::marker::PhantomData;
use std::sync::Arc;

/// ModelSerializer provides automatic serialization for ORM models
///
/// Inspired by Django REST Framework's ModelSerializer, this automatically
/// handles serialization, deserialization, validation, and database operations
/// for models that implement the Model trait.
///
/// # Examples
///
/// ```no_run
/// # use reinhardt_rest::serializers::{ModelSerializer, Serializer};
/// # use reinhardt_db::orm::Engine;
/// # use reinhardt_auth::DefaultUser;
/// # use uuid::Uuid;
/// #
/// # fn example() {
/// let serializer = ModelSerializer::<DefaultUser>::new();
///
/// // With Meta configuration
/// let serializer = ModelSerializer::<DefaultUser>::new()
///     .with_fields(vec!["id".to_string(), "username".to_string()])
///     .with_read_only_fields(vec!["id".to_string()]);
///
/// // Serialize a user
/// let user = DefaultUser {
///     id: Uuid::now_v7(),
///     username: "alice".to_string(),
///     email: "alice@example.com".to_string(),
///     ..Default::default()
/// };
///
/// // Validate and serialize
/// assert!(serializer.validate(&user).is_ok());
/// let json = serializer.serialize(&user).unwrap();
/// # }
/// ```
pub struct ModelSerializer<M>
where
	M: Model,
{
	meta: MetaConfig,
	introspector: Option<FieldIntrospector>,
	nested_config: NestedSerializerConfig,
	validator_config: ValidatorConfig<M>,
	_phantom: PhantomData<M>,
}

impl<M> ModelSerializer<M>
where
	M: Model,
{
	/// Create a new ModelSerializer instance
	///
	/// # Examples
	///
	/// ```
	/// # use reinhardt_rest::serializers::ModelSerializer;
	/// # use reinhardt_auth::DefaultUser;
	/// #
	/// let serializer = ModelSerializer::<DefaultUser>::new();
	/// ```
	pub fn new() -> Self {
		Self {
			meta: MetaConfig::new(),
			introspector: None,
			nested_config: NestedSerializerConfig::new(),
			validator_config: ValidatorConfig::new(),
			_phantom: PhantomData,
		}
	}

	/// Specify which fields to include in serialization
	///
	/// # Examples
	///
	/// ```
	/// # use reinhardt_rest::serializers::ModelSerializer;
	/// # use reinhardt_auth::DefaultUser;
	/// #
	/// let serializer = ModelSerializer::<DefaultUser>::new()
	///     .with_fields(vec!["id".to_string(), "username".to_string()]);
	/// ```
	pub fn with_fields(mut self, fields: Vec<String>) -> Self {
		self.meta = self.meta.with_fields(fields);
		self
	}

	/// Specify which fields to exclude from serialization
	///
	/// # Examples
	///
	/// ```
	/// # use reinhardt_rest::serializers::ModelSerializer;
	/// # use reinhardt_auth::DefaultUser;
	/// #
	/// let serializer = ModelSerializer::<DefaultUser>::new()
	///     .with_exclude(vec!["password_hash".to_string()]);
	/// ```
	pub fn with_exclude(mut self, exclude: Vec<String>) -> Self {
		self.meta = self.meta.with_exclude(exclude);
		self
	}

	/// Specify which fields are read-only
	///
	/// # Examples
	///
	/// ```
	/// # use reinhardt_rest::serializers::ModelSerializer;
	/// # use reinhardt_auth::DefaultUser;
	/// #
	/// let serializer = ModelSerializer::<DefaultUser>::new()
	///     .with_read_only_fields(vec!["id".to_string()]);
	/// ```
	pub fn with_read_only_fields(mut self, fields: Vec<String>) -> Self {
		self.meta = self.meta.with_read_only_fields(fields);
		self
	}

	/// Specify which fields are write-only
	///
	/// # Examples
	///
	/// ```
	/// # use reinhardt_rest::serializers::ModelSerializer;
	/// # use reinhardt_auth::DefaultUser;
	/// #
	/// let serializer = ModelSerializer::<DefaultUser>::new()
	///     .with_write_only_fields(vec!["password_hash".to_string()]);
	/// ```
	pub fn with_write_only_fields(mut self, fields: Vec<String>) -> Self {
		self.meta = self.meta.with_write_only_fields(fields);
		self
	}

	/// Get the meta configuration
	pub fn meta(&self) -> &MetaConfig {
		&self.meta
	}

	/// Add a nested field configuration
	///
	/// # Examples
	///
	/// ```
	/// # use reinhardt_rest::serializers::{ModelSerializer, nested_config::NestedFieldConfig};
	/// # use reinhardt_db::orm::Model;
	/// # use serde::{Serialize, Deserialize};
	/// #
	/// # #[derive(Debug, Clone, Serialize, Deserialize)]
	/// # struct Post {
	/// #     id: Option<i64>,
	/// #     title: String,
	/// #     author_id: i64,
	/// # }
	/// #
	/// # impl Model for Post {
	/// #     type PrimaryKey = i64;
	/// #     type Fields = PostFields;
	/// #     fn table_name() -> &'static str { "posts" }
	/// #     fn primary_key(&self) -> Option<Self::PrimaryKey> { self.id }
	/// #     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
	/// #     fn new_fields() -> Self::Fields { PostFields }
	/// # }
	/// # #[derive(Clone)]
	/// # struct PostFields;
	/// # impl reinhardt_db::orm::FieldSelector for PostFields {
	/// #     fn with_alias(self, _alias: &str) -> Self { self }
	/// # }
	/// let serializer = ModelSerializer::<Post>::new()
	///     .with_nested_field(NestedFieldConfig::new("author").depth(2));
	/// ```
	pub fn with_nested_field(mut self, field_config: NestedFieldConfig) -> Self {
		self.nested_config.add_nested_field(field_config);
		self
	}

	/// Get the nested serializer configuration
	pub fn nested_config(&self) -> &NestedSerializerConfig {
		&self.nested_config
	}

	/// Check if a field is configured as nested
	///
	/// # Examples
	///
	/// ```
	/// # use reinhardt_rest::serializers::{ModelSerializer, nested_config::NestedFieldConfig};
	/// # use reinhardt_db::orm::Model;
	/// # use serde::{Serialize, Deserialize};
	/// #
	/// # #[derive(Debug, Clone, Serialize, Deserialize)]
	/// # struct Post {
	/// #     id: Option<i64>,
	/// #     title: String,
	/// # }
	/// #
	/// # impl Model for Post {
	/// #     type PrimaryKey = i64;
	/// #     type Fields = PostFields;
	/// #     fn table_name() -> &'static str { "posts" }
	/// #     fn primary_key(&self) -> Option<Self::PrimaryKey> { self.id }
	/// #     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
	/// #     fn new_fields() -> Self::Fields { PostFields }
	/// # }
	/// # #[derive(Clone)]
	/// # struct PostFields;
	/// # impl reinhardt_db::orm::FieldSelector for PostFields {
	/// #     fn with_alias(self, _alias: &str) -> Self { self }
	/// # }
	/// let serializer = ModelSerializer::<Post>::new()
	///     .with_nested_field(NestedFieldConfig::new("author"));
	///
	/// assert!(serializer.is_nested_field("author"));
	/// assert!(!serializer.is_nested_field("title"));
	/// ```
	pub fn is_nested_field(&self, field_name: &str) -> bool {
		self.nested_config.is_nested_field(field_name)
	}

	/// Set a field introspector for automatic field generation
	///
	/// # Examples
	///
	/// ```
	/// # use reinhardt_rest::serializers::{ModelSerializer, introspection::{FieldIntrospector, FieldInfo}};
	/// # use reinhardt_auth::DefaultUser;
	/// #
	/// let mut introspector = FieldIntrospector::new();
	/// introspector.register_field(FieldInfo::new("id", "Uuid").primary_key());
	/// introspector.register_field(FieldInfo::new("username", "String"));
	///
	/// let serializer = ModelSerializer::<DefaultUser>::new()
	///     .with_introspector(introspector);
	/// ```
	pub fn with_introspector(mut self, introspector: FieldIntrospector) -> Self {
		self.introspector = Some(introspector);
		self
	}

	/// Get the field introspector
	pub fn introspector(&self) -> Option<&FieldIntrospector> {
		self.introspector.as_ref()
	}

	/// Get all field names from introspector or meta configuration
	///
	/// Returns field names from the introspector if available,
	/// otherwise returns field names from meta configuration.
	///
	/// # Examples
	///
	/// ```
	/// # use reinhardt_rest::serializers::{ModelSerializer, introspection::{FieldIntrospector, FieldInfo}};
	/// # use reinhardt_auth::DefaultUser;
	/// #
	/// let mut introspector = FieldIntrospector::new();
	/// introspector.register_field(FieldInfo::new("id", "Uuid"));
	/// introspector.register_field(FieldInfo::new("username", "String"));
	///
	/// let serializer = ModelSerializer::<DefaultUser>::new()
	///     .with_introspector(introspector);
	///
	/// let fields = serializer.field_names();
	/// assert_eq!(fields.len(), 2);
	/// ```
	pub fn field_names(&self) -> Vec<String> {
		if let Some(introspector) = &self.introspector {
			introspector.field_names()
		} else if let Some(fields) = self.meta.fields() {
			fields.clone()
		} else {
			vec![]
		}
	}

	/// Get required fields from introspector
	///
	/// Returns fields that are not optional according to the introspector.
	///
	/// # Examples
	///
	/// ```
	/// # use reinhardt_rest::serializers::{ModelSerializer, introspection::{FieldIntrospector, FieldInfo}};
	/// # use reinhardt_auth::DefaultUser;
	/// #
	/// let mut introspector = FieldIntrospector::new();
	/// introspector.register_field(FieldInfo::new("id", "Uuid"));
	/// introspector.register_field(FieldInfo::new("username", "String"));
	///
	/// let serializer = ModelSerializer::<DefaultUser>::new()
	///     .with_introspector(introspector);
	///
	/// let required = serializer.required_fields();
	/// assert_eq!(required.len(), 2);
	/// ```
	pub fn required_fields(&self) -> Vec<&FieldInfo> {
		if let Some(introspector) = &self.introspector {
			introspector.required_fields()
		} else {
			vec![]
		}
	}

	/// Get optional fields from introspector
	///
	/// Returns fields that are optional according to the introspector.
	///
	/// # Examples
	///
	/// ```
	/// # use reinhardt_rest::serializers::{ModelSerializer, introspection::{FieldIntrospector, FieldInfo}};
	/// # use reinhardt_auth::DefaultUser;
	/// #
	/// let mut introspector = FieldIntrospector::new();
	/// introspector.register_field(FieldInfo::new("email", "String").optional());
	/// introspector.register_field(FieldInfo::new("username", "String"));
	///
	/// let serializer = ModelSerializer::<DefaultUser>::new()
	///     .with_introspector(introspector);
	///
	/// let optional = serializer.optional_fields();
	/// assert_eq!(optional.len(), 1);
	/// assert_eq!(optional[0].name, "email");
	/// ```
	pub fn optional_fields(&self) -> Vec<&FieldInfo> {
		if let Some(introspector) = &self.introspector {
			introspector.optional_fields()
		} else {
			vec![]
		}
	}

	/// Get primary key field from introspector
	///
	/// # Examples
	///
	/// ```
	/// # use reinhardt_rest::serializers::{ModelSerializer, introspection::{FieldIntrospector, FieldInfo}};
	/// # use reinhardt_auth::DefaultUser;
	/// #
	/// let mut introspector = FieldIntrospector::new();
	/// introspector.register_field(FieldInfo::new("id", "Uuid").primary_key());
	/// introspector.register_field(FieldInfo::new("username", "String"));
	///
	/// let serializer = ModelSerializer::<DefaultUser>::new()
	///     .with_introspector(introspector);
	///
	/// let pk = serializer.primary_key_field();
	/// assert!(pk.is_some());
	/// assert_eq!(pk.unwrap().name, "id");
	/// ```
	pub fn primary_key_field(&self) -> Option<&FieldInfo> {
		self.introspector
			.as_ref()
			.and_then(|i| i.primary_key_field())
	}

	/// Add a unique field validator
	///
	/// # Examples
	///
	/// ```
	/// # use reinhardt_rest::serializers::{ModelSerializer, validators::UniqueValidator};
	/// # use reinhardt_auth::DefaultUser;
	/// #
	/// let serializer = ModelSerializer::<DefaultUser>::new()
	///     .with_unique_validator(UniqueValidator::new("username"));
	/// ```
	pub fn with_unique_validator(mut self, validator: UniqueValidator<M>) -> Self {
		self.validator_config.add_unique_validator(validator);
		self
	}

	/// Add a unique together validator
	///
	/// # Examples
	///
	/// ```
	/// # use reinhardt_rest::serializers::{ModelSerializer, validators::UniqueTogetherValidator};
	/// # use reinhardt_auth::DefaultUser;
	/// #
	/// let serializer = ModelSerializer::<DefaultUser>::new()
	///     .with_unique_together_validator(
	///         UniqueTogetherValidator::new(vec!["username", "email"])
	///     );
	/// ```
	pub fn with_unique_together_validator(mut self, validator: UniqueTogetherValidator<M>) -> Self {
		self.validator_config
			.add_unique_together_validator(validator);
		self
	}

	/// Add an object-level synchronous validator.
	///
	/// Synchronous validators run inside [`Self::validate`] and at the start
	/// of [`Self::validate_async`]. They never touch the database — pair
	/// them with [`Self::with_unique_validator`] for DB-backed checks.
	pub fn with_model_validator<V>(mut self, validator: V) -> Self
	where
		V: ModelLevelValidator<M> + 'static,
	{
		self.validator_config
			.add_sync_model_validator(Arc::new(validator));
		self
	}

	/// Get the validator configuration
	///
	/// # Examples
	///
	/// ```
	/// # use reinhardt_rest::serializers::{ModelSerializer, validators::UniqueValidator};
	/// # use reinhardt_auth::DefaultUser;
	/// #
	/// let serializer = ModelSerializer::<DefaultUser>::new()
	///     .with_unique_validator(UniqueValidator::new("username"));
	///
	/// let validators = serializer.validators();
	/// assert!(validators.has_validators());
	/// ```
	pub fn validators(&self) -> &ValidatorConfig<M> {
		&self.validator_config
	}

	/// Run synchronous validators against `instance`.
	///
	/// Executes every [`ModelLevelValidator`] registered via
	/// [`Self::with_model_validator`] in registration order, returning the
	/// first failure as `SerializerError::Validation`. With no synchronous
	/// validators registered the call is a cheap `Ok(())`, preserving the
	/// behavior of the prior placeholder implementation.
	///
	/// Database-backed checks (unique constraints) belong in
	/// [`Self::validate_async`].
	///
	/// # Examples
	///
	/// ```
	/// # use reinhardt_rest::serializers::ModelSerializer;
	/// # use reinhardt_auth::DefaultUser;
	/// # use uuid::Uuid;
	/// #
	/// let serializer = ModelSerializer::<DefaultUser>::new();
	/// let user = DefaultUser {
	///     id: Uuid::now_v7(),
	///     username: "alice".to_string(),
	///     ..Default::default()
	/// };
	/// assert!(serializer.validate(&user).is_ok());
	/// ```
	pub fn validate(&self, instance: &M) -> Result<(), SerializerError> {
		self.validator_config
			.validate(instance)
			.map_err(SerializerError::Validation)
	}

	/// Validate a model instance asynchronously with database checks
	///
	/// This method executes all configured validators including those that
	/// require database access (e.g., UniqueValidator, UniqueTogetherValidator).
	///
	/// # Arguments
	///
	/// * `connection` - Database connection
	/// * `instance` - The model instance to validate
	///
	/// # Returns
	///
	/// Returns `Ok(())` if all validations pass, or `Err(SerializerError)` with
	/// details about the first validation failure.
	///
	/// # Examples
	///
	/// ```no_run
	/// # use reinhardt_rest::serializers::ModelSerializer;
	/// # use reinhardt_auth::DefaultUser;
	/// # use reinhardt_db::backends::DatabaseConnection;
	/// # use uuid::Uuid;
	/// #
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// # let connection = DatabaseConnection::connect_postgres("postgres://localhost/test").await?;
	/// let serializer = ModelSerializer::<DefaultUser>::new();
	/// let user = DefaultUser {
	///     id: Uuid::now_v7(),
	///     username: "alice".to_string(),
	///     ..Default::default()
	/// };
	///
	/// match serializer.validate_async(&connection, &user).await {
	///     Ok(()) => println!("Validation passed"),
	///     Err(e) => println!("Validation failed: {}", e),
	/// }
	/// # Ok(())
	/// # }
	/// ```
	pub async fn validate_async(
		&self,
		connection: &DatabaseConnection,
		instance: &M,
	) -> Result<(), SerializerError>
	where
		M::PrimaryKey: std::fmt::Display,
	{
		// Run synchronous validators first to fail-early without touching
		// the database when the issue is detectable in-process.
		self.validate(instance)?;

		// Convert instance to JSON for field extraction
		let json_value = serde_json::to_value(instance).map_err(|e| SerializerError::Serde {
			message: format!("Failed to serialize instance for validation: {}", e),
		})?;

		// Execute unique validators
		for validator in self.validator_config.unique_validators() {
			let field_name = validator.field_name();

			// Extract field value from JSON
			if let Some(field_value) = json_value.get(field_name) {
				let value_str = match field_value {
					serde_json::Value::String(s) => s.clone(),
					other => other.to_string().trim_matches('"').to_string(),
				};

				validator
					.validate(connection, &value_str, instance.primary_key().as_ref())
					.await
					.map_err(|e| {
						SerializerError::Validation(ValidatorError::UniqueViolation {
							field_name: field_name.to_string(),
							value: value_str.clone(),
							message: e.to_string(),
						})
					})?;
			}
		}

		// Execute unique together validators
		for validator in self.validator_config.unique_together_validators() {
			let field_names = validator.field_names();
			let mut values: HashMap<String, String> = HashMap::new();

			for field_name in field_names {
				if let Some(field_value) = json_value.get(field_name) {
					let value_str = match field_value {
						serde_json::Value::String(s) => s.clone(),
						other => other.to_string().trim_matches('"').to_string(),
					};
					values.insert(field_name.to_string(), value_str);
				}
			}

			validator
				.validate(connection, &values, instance.primary_key().as_ref())
				.await
				.map_err(|e| {
					SerializerError::Validation(ValidatorError::UniqueTogetherViolation {
						field_names: field_names.iter().map(|s| s.to_string()).collect(),
						values: values.clone(),
						message: e.to_string(),
					})
				})?;
		}

		Ok(())
	}
}

impl<M> Default for ModelSerializer<M>
where
	M: Model,
{
	fn default() -> Self {
		Self::new()
	}
}

// Direction of a meta-driven field filter pass.
//
// `Output` is used during `serialize` (model -> JSON): excluded keys and
// write-only keys are stripped, because write-only fields must never leak
// to API responses.
//
// `Input` is used during `deserialize` (JSON -> model): excluded keys and
// read-only keys are stripped, because read-only fields are server-controlled
// and must not be accepted from clients.
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
enum FilterDirection {
	Output,
	Input,
}

// Mutate `value` in place, removing every key that is filtered out by `meta`
// for the given direction. No-op when `value` is not a JSON object (defensive;
// `Model` derive-`Serialize` always produces an object).
//
// The introspector field-name slice acts as the implicit allowlist when
// `meta.fields()` is `None` AND the slice is non-empty. An explicit empty
// `Vec` from the user via `with_fields(vec![])` still means "include nothing".
fn apply_meta_filter(
	value: &mut serde_json::Value,
	meta: &MetaConfig,
	introspector_field_names: Option<&[String]>,
	direction: FilterDirection,
) {
	let serde_json::Value::Object(map) = value else {
		return;
	};

	let implicit_allowlist: Option<&[String]> = match (meta.fields(), introspector_field_names) {
		(Some(_), _) => None,
		(None, Some(names)) if !names.is_empty() => Some(names),
		_ => None,
	};

	map.retain(|key, _| {
		if !meta.is_field_included(key) {
			return false;
		}
		if let Some(allowed) = implicit_allowlist
			&& !allowed.iter().any(|n| n == key)
		{
			return false;
		}
		match direction {
			FilterDirection::Output => !meta.is_write_only(key),
			FilterDirection::Input => !meta.is_read_only(key),
		}
	});
}

impl<M> Serializer for ModelSerializer<M>
where
	M: Model,
{
	type Input = M;
	type Output = String;

	fn serialize(&self, input: &Self::Input) -> Result<Self::Output, SerializerError> {
		// Convert to a JSON Value first so `MetaConfig` filters (fields /
		// exclude / write_only) can be applied before emitting the final
		// string. Backward compatible: an unconfigured serializer retains
		// every key because `MetaConfig::default()` excludes nothing.
		let mut value = serde_json::to_value(input).map_err(|e| SerializerError::Serde {
			message: format!("Serialization error: {}", e),
		})?;

		let introspector_names = self.introspector.as_ref().map(|i| i.field_names());
		apply_meta_filter(
			&mut value,
			&self.meta,
			introspector_names.as_deref(),
			FilterDirection::Output,
		);

		serde_json::to_string(&value).map_err(|e| SerializerError::Serde {
			message: format!("Serialization error: {}", e),
		})
	}

	fn deserialize(&self, output: &Self::Output) -> Result<Self::Input, SerializerError> {
		// Parse to a JSON Value, strip excluded / read_only keys, then
		// reconstruct `M`. Stripping a read-only field that `M` requires
		// surfaces as `SerializerError::Serde` (missing-field), matching
		// the behavior callers already see for malformed input.
		let mut value: serde_json::Value =
			serde_json::from_str(output).map_err(|e| SerializerError::Serde {
				message: format!("Deserialization error: {}", e),
			})?;

		let introspector_names = self.introspector.as_ref().map(|i| i.field_names());
		apply_meta_filter(
			&mut value,
			&self.meta,
			introspector_names.as_deref(),
			FilterDirection::Input,
		);

		serde_json::from_value(value).map_err(|e| SerializerError::Serde {
			message: format!("Deserialization error: {}", e),
		})
	}
}
