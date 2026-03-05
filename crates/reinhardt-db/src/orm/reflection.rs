//! Runtime reflection for ORM models
//!
//! This module provides functionality to inspect and manipulate model instances
//! at runtime, allowing dynamic access to fields, relationships, and metadata.
//!
//! # Examples
//!
//! ```rust
//! use reinhardt_db::orm::reflection::{ModelReflector, FieldInfo, FieldValue};
//! use reinhardt_db::orm::Model;
//! use serde::{Deserialize, Serialize};
//!
//! #[derive(Debug, Clone, Serialize, Deserialize)]
//! struct User {
//!     id: Option<i64>,
//!     name: String,
//!     age: i32,
//! }
//!
//! # #[derive(Clone)]
//! # struct UserFields;
//! # impl reinhardt_db::orm::model::FieldSelector for UserFields {
//! #     fn with_alias(self, _alias: &str) -> Self { self }
//! # }
//! #
//! impl Model for User {
//!     type PrimaryKey = i64;
//!     type Fields = UserFields;
//!
//!     fn table_name() -> &'static str {
//!         "users"
//!     }
//!
//!     fn new_fields() -> Self::Fields {
//!         UserFields
//!     }
//!
//!     fn primary_key(&self) -> Option<Self::PrimaryKey> {
//!         self.id
//!     }
//!
//!     fn set_primary_key(&mut self, value: Self::PrimaryKey) {
//!         self.id = Some(value);
//!     }
//! }
//!
//! // Create a reflector for the User type
//! let reflector = ModelReflector::for_model::<User>();
//! assert_eq!(reflector.table_name(), "users");
//!
//! // Serialize a user instance to inspect field values
//! let user = User {
//!     id: Some(1),
//!     name: "Alice".to_string(),
//!     age: 30,
//! };
//! let name_value = reflector.get_field_value(&user, "name").unwrap();
//! assert_eq!(name_value.as_str(), Some("Alice"));
//! ```

use crate::orm::model::Model;
use crate::orm::registry::registry;
use serde::{Deserialize, Serialize};
use std::any::TypeId;
use std::collections::HashMap;
use std::fmt;

/// Error types for reflection operations
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub enum ReflectionError {
	/// Model not found in registry
	ModelNotFound(String),
	/// Field not found in model
	FieldNotFound(String),
	/// Type mismatch when accessing field
	TypeMismatch { expected: String, actual: String },
	/// Invalid operation
	InvalidOperation(String),
	/// Serialization/deserialization error
	SerializationError(String),
}

impl fmt::Display for ReflectionError {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			ReflectionError::ModelNotFound(name) => write!(f, "Model not found: {}", name),
			ReflectionError::FieldNotFound(name) => write!(f, "Field not found: {}", name),
			ReflectionError::TypeMismatch { expected, actual } => {
				write!(f, "Type mismatch: expected {}, got {}", expected, actual)
			}
			ReflectionError::InvalidOperation(msg) => write!(f, "Invalid operation: {}", msg),
			ReflectionError::SerializationError(msg) => {
				write!(f, "Serialization error: {}", msg)
			}
		}
	}
}

impl std::error::Error for ReflectionError {}

/// Field value wrapper that can hold different types
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum FieldValue {
	/// Null value
	Null,
	/// Boolean value
	Bool(bool),
	/// Integer value
	Int(i64),
	/// Floating point value
	Float(f64),
	/// String value
	String(String),
	/// Bytes value
	Bytes(Vec<u8>),
	/// Array of values
	Array(Vec<FieldValue>),
	/// Object (map of field name to value)
	Object(HashMap<String, FieldValue>),
}

impl FieldValue {
	/// Check if the value is null
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::reflection::FieldValue;
	///
	/// let value = FieldValue::Null;
	/// assert!(value.is_null());
	///
	/// let value = FieldValue::Int(42);
	/// assert!(!value.is_null());
	/// ```
	pub fn is_null(&self) -> bool {
		matches!(self, FieldValue::Null)
	}

	/// Try to convert to i64
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::reflection::FieldValue;
	///
	/// let value = FieldValue::Int(42);
	/// assert_eq!(value.as_i64(), Some(42));
	///
	/// let value = FieldValue::String("hello".to_string());
	/// assert_eq!(value.as_i64(), None);
	/// ```
	pub fn as_i64(&self) -> Option<i64> {
		match self {
			FieldValue::Int(v) => Some(*v),
			_ => None,
		}
	}

	/// Try to convert to f64
	pub fn as_f64(&self) -> Option<f64> {
		match self {
			FieldValue::Float(v) => Some(*v),
			FieldValue::Int(v) => Some(*v as f64),
			_ => None,
		}
	}

	/// Try to convert to string
	pub fn as_str(&self) -> Option<&str> {
		match self {
			FieldValue::String(s) => Some(s),
			_ => None,
		}
	}

	/// Try to convert to bool
	pub fn as_bool(&self) -> Option<bool> {
		match self {
			FieldValue::Bool(b) => Some(*b),
			_ => None,
		}
	}
}

impl From<i64> for FieldValue {
	fn from(value: i64) -> Self {
		FieldValue::Int(value)
	}
}

impl From<i32> for FieldValue {
	fn from(value: i32) -> Self {
		FieldValue::Int(value as i64)
	}
}

impl From<f64> for FieldValue {
	fn from(value: f64) -> Self {
		FieldValue::Float(value)
	}
}

impl From<bool> for FieldValue {
	fn from(value: bool) -> Self {
		FieldValue::Bool(value)
	}
}

impl From<String> for FieldValue {
	fn from(value: String) -> Self {
		FieldValue::String(value)
	}
}

impl From<&str> for FieldValue {
	fn from(value: &str) -> Self {
		FieldValue::String(value.to_owned())
	}
}

/// Information about a field in a model
#[derive(Debug, Clone, PartialEq)]
pub struct FieldInfo {
	/// Field name in the model
	pub name: String,
	/// Column name in the database
	pub column_name: String,
	/// SQL type of the field
	pub field_type: String,
	/// Whether the field can be null
	pub nullable: bool,
}

impl FieldInfo {
	/// Create a new FieldInfo
	pub fn new(
		name: impl Into<String>,
		column_name: impl Into<String>,
		field_type: impl Into<String>,
		nullable: bool,
	) -> Self {
		Self {
			name: name.into(),
			column_name: column_name.into(),
			field_type: field_type.into(),
			nullable,
		}
	}
}

/// Reflector for a specific model type
///
/// Provides runtime introspection capabilities for model instances.
///
/// # Examples
///
/// ```rust
/// use reinhardt_db::orm::reflection::ModelReflector;
/// use reinhardt_db::orm::Model;
/// use serde::{Deserialize, Serialize};
///
/// #[derive(Debug, Clone, Serialize, Deserialize)]
/// struct Article {
///     id: Option<i64>,
///     title: String,
///     content: String,
/// }
///
/// # #[derive(Clone)]
/// # struct ArticleFields;
/// # impl reinhardt_db::orm::model::FieldSelector for ArticleFields {
/// #     fn with_alias(self, _alias: &str) -> Self { self }
/// # }
/// #
/// impl Model for Article {
///     type PrimaryKey = i64;
///     type Fields = ArticleFields;
///
///     fn table_name() -> &'static str {
///         "articles"
///     }
///
///     fn new_fields() -> Self::Fields {
///         ArticleFields
///     }
///
///     fn primary_key(&self) -> Option<Self::PrimaryKey> {
///         self.id
///     }
///
///     fn set_primary_key(&mut self, value: Self::PrimaryKey) {
///         self.id = Some(value);
///     }
/// }
///
/// let reflector = ModelReflector::for_model::<Article>();
/// let table_name = reflector.table_name();
/// assert_eq!(table_name, "articles");
/// ```
#[derive(Clone)]
pub struct ModelReflector {
	/// Model name
	model_name: String,
	/// Table name
	table_name: String,
	/// Type ID for runtime type checking
	#[allow(dead_code)]
	type_id: TypeId,
}

impl ModelReflector {
	/// Create a reflector for a specific model type
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_db::orm::reflection::ModelReflector;
	/// use reinhardt_db::orm::Model;
	/// use serde::{Deserialize, Serialize};
	///
	/// #[derive(Debug, Clone, Serialize, Deserialize)]
	/// struct Product {
	///     id: Option<i64>,
	///     name: String,
	/// }
	///
	/// # #[derive(Clone)]
	/// # struct ProductFields;
	/// # impl reinhardt_db::orm::model::FieldSelector for ProductFields {
	/// #     fn with_alias(self, _alias: &str) -> Self { self }
	/// # }
	/// #
	/// impl Model for Product {
	///     type PrimaryKey = i64;
	///     type Fields = ProductFields;
	///
	///     fn table_name() -> &'static str {
	///         "products"
	///     }
	///
	///     fn new_fields() -> Self::Fields {
	///         ProductFields
	///     }
	///
	///     fn primary_key(&self) -> Option<Self::PrimaryKey> {
	///         self.id
	///     }
	///
	///     fn set_primary_key(&mut self, value: Self::PrimaryKey) {
	///         self.id = Some(value);
	///     }
	/// }
	///
	/// let reflector = ModelReflector::for_model::<Product>();
	/// assert_eq!(reflector.model_name(), "Product");
	/// ```
	pub fn for_model<M: Model + 'static>() -> Self {
		Self {
			model_name: std::any::type_name::<M>()
				.split("::")
				.last()
				.unwrap_or("Unknown")
				.to_owned(),
			table_name: M::table_name().to_owned(),
			type_id: TypeId::of::<M>(),
		}
	}

	/// Create a reflector by model name
	///
	/// This looks up the model in the registry.
	pub fn new(model_name: impl Into<String>) -> Result<Self, ReflectionError> {
		let model_name = model_name.into();
		let mapper = registry()
			.get(&model_name)
			.ok_or_else(|| ReflectionError::ModelNotFound(model_name.clone()))?;

		Ok(Self {
			table_name: mapper.table_name,
			model_name,
			type_id: TypeId::of::<()>(), // Default type ID when created from string
		})
	}

	/// Get the model name
	pub fn model_name(&self) -> &str {
		&self.model_name
	}

	/// Get the table name
	pub fn table_name(&self) -> &str {
		&self.table_name
	}

	/// Get field names
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_db::orm::reflection::ModelReflector;
	/// use reinhardt_db::orm::registry::{registry, EntityMapper, ColumnMapping};
	///
	/// // Register a model in the registry
	/// let mut mapper = EntityMapper::new("users");
	/// mapper.add_column(ColumnMapping::new("id", "id", "INTEGER"));
	/// mapper.add_column(ColumnMapping::new("name", "name", "VARCHAR"));
	/// registry().register("User", mapper);
	///
	/// let reflector = ModelReflector::new("User").unwrap();
	/// let field_names = reflector.field_names();
	/// assert_eq!(field_names.len(), 2);
	/// assert!(field_names.contains(&"id".to_string()));
	/// assert!(field_names.contains(&"name".to_string()));
	///
	/// // Cleanup
	/// registry().clear();
	/// ```
	pub fn field_names(&self) -> Vec<String> {
		registry()
			.get(&self.model_name)
			.map(|mapper| {
				mapper
					.columns
					.iter()
					.map(|col| col.property_name.clone())
					.collect()
			})
			.unwrap_or_default()
	}

	/// Get field information
	pub fn fields(&self) -> Vec<FieldInfo> {
		registry()
			.get(&self.model_name)
			.map(|mapper| {
				mapper
					.columns
					.iter()
					.map(|col| {
						FieldInfo::new(
							&col.property_name,
							&col.column_name,
							&col.column_type,
							col.nullable,
						)
					})
					.collect()
			})
			.unwrap_or_default()
	}

	/// Get information about a specific field
	pub fn field_info(&self, field_name: &str) -> Option<FieldInfo> {
		registry().get(&self.model_name).and_then(|mapper| {
			mapper
				.columns
				.iter()
				.find(|col| col.property_name == field_name)
				.map(|col| {
					FieldInfo::new(
						&col.property_name,
						&col.column_name,
						&col.column_type,
						col.nullable,
					)
				})
		})
	}

	/// Get primary key field names
	pub fn primary_key_fields(&self) -> Vec<String> {
		registry()
			.get(&self.model_name)
			.map(|mapper| mapper.primary_key.clone())
			.unwrap_or_default()
	}

	/// Serialize a model instance to a map of field values
	pub fn serialize_to_map<M: Model + Serialize>(
		&self,
		instance: &M,
	) -> Result<HashMap<String, FieldValue>, ReflectionError> {
		let json_value = serde_json::to_value(instance)
			.map_err(|e| ReflectionError::SerializationError(e.to_string()))?;

		match json_value {
			serde_json::Value::Object(map) => {
				let mut result = HashMap::new();
				for (key, value) in map {
					result.insert(key, json_value_to_field_value(value));
				}
				Ok(result)
			}
			_ => Err(ReflectionError::SerializationError(
				"Expected object".to_owned(),
			)),
		}
	}

	/// Get field value from a serializable model
	pub fn get_field_value<M: Model + Serialize>(
		&self,
		instance: &M,
		field_name: &str,
	) -> Result<FieldValue, ReflectionError> {
		let map = self.serialize_to_map(instance)?;
		map.get(field_name)
			.cloned()
			.ok_or_else(|| ReflectionError::FieldNotFound(field_name.to_owned()))
	}
}

impl fmt::Debug for ModelReflector {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_struct("ModelReflector")
			.field("model_name", &self.model_name)
			.field("table_name", &self.table_name)
			.finish()
	}
}

/// Convert serde_json::Value to FieldValue
fn json_value_to_field_value(value: serde_json::Value) -> FieldValue {
	match value {
		serde_json::Value::Null => FieldValue::Null,
		serde_json::Value::Bool(b) => FieldValue::Bool(b),
		serde_json::Value::Number(n) => {
			if let Some(i) = n.as_i64() {
				FieldValue::Int(i)
			} else if let Some(f) = n.as_f64() {
				FieldValue::Float(f)
			} else {
				FieldValue::Null
			}
		}
		serde_json::Value::String(s) => FieldValue::String(s),
		serde_json::Value::Array(arr) => {
			FieldValue::Array(arr.into_iter().map(json_value_to_field_value).collect())
		}
		serde_json::Value::Object(obj) => FieldValue::Object(
			obj.into_iter()
				.map(|(k, v)| (k, json_value_to_field_value(v)))
				.collect(),
		),
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::orm::model::Model;
	use crate::orm::registry::{ColumnMapping, EntityMapper, registry};
	use serial_test::serial;

	#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
	struct TestUser {
		id: Option<i64>,
		name: String,
		age: i32,
		email: Option<String>,
	}

	#[derive(Debug, Clone)]
	struct TestUserFields;

	impl crate::orm::model::FieldSelector for TestUserFields {
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

		fn primary_key(&self) -> Option<Self::PrimaryKey> {
			self.id
		}

		fn set_primary_key(&mut self, value: Self::PrimaryKey) {
			self.id = Some(value);
		}

		fn primary_key_field() -> &'static str {
			"id"
		}

		fn new_fields() -> Self::Fields {
			TestUserFields
		}
	}

	fn setup_test_registry() {
		registry().clear();
		let mut mapper = EntityMapper::new("test_users");
		mapper.add_column(ColumnMapping::new("id", "id", "INTEGER").not_null());
		mapper.add_column(ColumnMapping::new("name", "name", "VARCHAR").not_null());
		mapper.add_column(ColumnMapping::new("age", "age", "INTEGER").not_null());
		mapper.add_column(ColumnMapping::new("email", "email", "VARCHAR"));
		mapper.set_primary_key(vec!["id".to_owned()]);
		registry().register("TestUser", mapper);
	}

	#[test]
	fn test_model_reflector_for_model() {
		let reflector = ModelReflector::for_model::<TestUser>();
		assert_eq!(reflector.table_name(), "test_users");
		assert!(reflector.model_name().contains("TestUser"));
	}

	#[test]
	#[serial]
	fn test_model_reflector_new() {
		setup_test_registry();
		let reflector = ModelReflector::new("TestUser").unwrap();
		assert_eq!(reflector.model_name(), "TestUser");
		assert_eq!(reflector.table_name(), "test_users");
	}

	#[test]
	#[serial]
	fn test_model_reflector_new_not_found() {
		registry().clear();
		let result = ModelReflector::new("NonExistent");
		assert!(result.is_err());
		assert!(matches!(
			result.unwrap_err(),
			ReflectionError::ModelNotFound(_)
		));
	}

	#[test]
	#[serial]
	fn test_model_reflector_field_names() {
		setup_test_registry();
		let reflector = ModelReflector::new("TestUser").unwrap();
		let field_names = reflector.field_names();
		assert_eq!(field_names.len(), 4);
		assert!(field_names.contains(&"id".to_owned()));
		assert!(field_names.contains(&"name".to_owned()));
		assert!(field_names.contains(&"age".to_owned()));
		assert!(field_names.contains(&"email".to_owned()));
	}

	#[test]
	#[serial]
	fn test_model_reflector_fields() {
		setup_test_registry();
		let reflector = ModelReflector::new("TestUser").unwrap();
		let fields = reflector.fields();
		assert_eq!(fields.len(), 4);

		let id_field = fields.iter().find(|f| f.name == "id").unwrap();
		assert_eq!(id_field.column_name, "id");
		assert_eq!(id_field.field_type, "INTEGER");
		assert!(!id_field.nullable);
	}

	#[test]
	#[serial]
	fn test_model_reflector_field_info() {
		setup_test_registry();
		let reflector = ModelReflector::new("TestUser").unwrap();

		let name_field = reflector.field_info("name").unwrap();
		assert_eq!(name_field.name, "name");
		assert_eq!(name_field.column_name, "name");
		assert_eq!(name_field.field_type, "VARCHAR");
		assert!(!name_field.nullable);

		let email_field = reflector.field_info("email").unwrap();
		assert!(email_field.nullable);

		let nonexistent = reflector.field_info("nonexistent");
		assert!(nonexistent.is_none());
	}

	#[test]
	#[serial]
	fn test_model_reflector_primary_key_fields() {
		setup_test_registry();
		let reflector = ModelReflector::new("TestUser").unwrap();
		let pk_fields = reflector.primary_key_fields();
		assert_eq!(pk_fields.len(), 1);
		assert_eq!(pk_fields[0], "id");
	}

	#[test]
	fn test_serialize_to_map() {
		let reflector = ModelReflector::for_model::<TestUser>();
		let user = TestUser {
			id: Some(1),
			name: "Alice".to_owned(),
			age: 30,
			email: Some("alice@example.com".to_owned()),
		};

		let map = reflector.serialize_to_map(&user).unwrap();
		assert_eq!(map.get("id").unwrap().as_i64(), Some(1));
		assert_eq!(map.get("name").unwrap().as_str(), Some("Alice"));
		assert_eq!(map.get("age").unwrap().as_i64(), Some(30));
		assert_eq!(
			map.get("email").unwrap().as_str(),
			Some("alice@example.com")
		);
	}

	#[test]
	fn test_get_field_value() {
		let reflector = ModelReflector::for_model::<TestUser>();
		let user = TestUser {
			id: Some(42),
			name: "Bob".to_owned(),
			age: 25,
			email: None,
		};

		let id_value = reflector.get_field_value(&user, "id").unwrap();
		assert_eq!(id_value.as_i64(), Some(42));

		let name_value = reflector.get_field_value(&user, "name").unwrap();
		assert_eq!(name_value.as_str(), Some("Bob"));

		let age_value = reflector.get_field_value(&user, "age").unwrap();
		assert_eq!(age_value.as_i64(), Some(25));

		let email_value = reflector.get_field_value(&user, "email").unwrap();
		assert!(email_value.is_null());
	}

	#[test]
	fn test_field_value_conversions() {
		let int_value = FieldValue::Int(42);
		assert_eq!(int_value.as_i64(), Some(42));
		assert_eq!(int_value.as_f64(), Some(42.0));
		assert_eq!(int_value.as_str(), None);
		assert_eq!(int_value.as_bool(), None);

		let float_value = FieldValue::Float(3.15);
		assert_eq!(float_value.as_i64(), None);
		assert_eq!(float_value.as_f64(), Some(3.15));

		let string_value = FieldValue::String("hello".to_owned());
		assert_eq!(string_value.as_str(), Some("hello"));
		assert_eq!(string_value.as_i64(), None);

		let bool_value = FieldValue::Bool(true);
		assert_eq!(bool_value.as_bool(), Some(true));

		let null_value = FieldValue::Null;
		assert!(null_value.is_null());
	}

	#[test]
	fn test_field_value_from_conversions() {
		let int_value: FieldValue = 42i32.into();
		assert_eq!(int_value.as_i64(), Some(42));

		let i64_value: FieldValue = 100i64.into();
		assert_eq!(i64_value.as_i64(), Some(100));

		let float_value: FieldValue = 3.15f64.into();
		assert_eq!(float_value.as_f64(), Some(3.15));

		let bool_value: FieldValue = true.into();
		assert_eq!(bool_value.as_bool(), Some(true));

		let string_value: FieldValue = "test".into();
		assert_eq!(string_value.as_str(), Some("test"));

		let owned_string_value: FieldValue = "test".to_owned().into();
		assert_eq!(owned_string_value.as_str(), Some("test"));
	}

	#[test]
	fn test_field_info_new() {
		let field = FieldInfo::new("username", "user_name", "VARCHAR", false);
		assert_eq!(field.name, "username");
		assert_eq!(field.column_name, "user_name");
		assert_eq!(field.field_type, "VARCHAR");
		assert!(!field.nullable);
	}

	#[test]
	fn test_reflection_error_display() {
		let err1 = ReflectionError::ModelNotFound("User".to_owned());
		assert_eq!(err1.to_string(), "Model not found: User");

		let err2 = ReflectionError::FieldNotFound("name".to_owned());
		assert_eq!(err2.to_string(), "Field not found: name");

		let err3 = ReflectionError::TypeMismatch {
			expected: "String".to_owned(),
			actual: "Int".to_owned(),
		};
		assert_eq!(err3.to_string(), "Type mismatch: expected String, got Int");

		let err4 = ReflectionError::InvalidOperation("test".to_owned());
		assert_eq!(err4.to_string(), "Invalid operation: test");

		let err5 = ReflectionError::SerializationError("test".to_owned());
		assert_eq!(err5.to_string(), "Serialization error: test");
	}
}
