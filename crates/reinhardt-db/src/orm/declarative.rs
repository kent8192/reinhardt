//! Declarative model definition system
//!
//! This module provides a declarative approach to defining database models,
//! similar to SQLAlchemy's declarative base system.

use super::fields::Field;
use crate::orm::registry::{ColumnInfo, EntityMapper, TableInfo, registry};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// Metadata about a model's fields
#[derive(Debug, Clone)]
pub struct FieldMetadata {
	/// The name.
	pub name: String,
	/// The field type.
	pub field_type: String,
	/// The nullable.
	pub nullable: bool,
	/// The primary key.
	pub primary_key: bool,
	/// The unique.
	pub unique: bool,
	/// The default.
	pub default: Option<String>,
	/// The max length.
	pub max_length: Option<u64>,
}

impl FieldMetadata {
	/// Creates a new FieldMetadata
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::declarative::FieldMetadata;
	///
	/// let field = FieldMetadata::new("username", "CharField");
	/// assert_eq!(field.name, "username");
	/// assert_eq!(field.field_type, "CharField");
	/// assert!(!field.nullable);
	/// assert!(!field.primary_key);
	/// ```
	pub fn new(name: impl Into<String>, field_type: impl Into<String>) -> Self {
		Self {
			name: name.into(),
			field_type: field_type.into(),
			nullable: false,
			primary_key: false,
			unique: false,
			default: None,
			max_length: None,
		}
	}

	/// Creates a FieldMetadata from a Field trait object
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::fields::{CharField, Field};
	/// use reinhardt_db::orm::declarative::FieldMetadata;
	///
	/// let mut field = CharField::new(100);
	/// field.set_attributes_from_name("username");
	/// let metadata = FieldMetadata::from_field(&field);
	/// assert_eq!(metadata.name, "username");
	/// ```
	pub fn from_field(field: &dyn Field) -> Self {
		let deconstruction = field.deconstruct();
		let name = deconstruction.name.unwrap_or_default();
		let field_type = deconstruction
			.path
			.split('.')
			.next_back()
			.unwrap_or("Unknown");

		let mut metadata = Self::new(name, field_type);
		metadata.nullable = field.is_null();
		metadata.primary_key = field.is_primary_key();

		metadata
	}

	/// Sets the nullable flag
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::declarative::FieldMetadata;
	///
	/// let field = FieldMetadata::new("email", "EmailField").nullable(true);
	/// assert!(field.nullable);
	/// ```
	pub fn nullable(mut self, nullable: bool) -> Self {
		self.nullable = nullable;
		self
	}

	/// Sets the primary_key flag
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::declarative::FieldMetadata;
	///
	/// let field = FieldMetadata::new("id", "AutoField").primary_key(true);
	/// assert!(field.primary_key);
	/// ```
	pub fn primary_key(mut self, primary_key: bool) -> Self {
		self.primary_key = primary_key;
		self
	}

	/// Sets the unique flag
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::declarative::FieldMetadata;
	///
	/// let field = FieldMetadata::new("username", "CharField").unique(true);
	/// assert!(field.unique);
	/// ```
	pub fn unique(mut self, unique: bool) -> Self {
		self.unique = unique;
		self
	}

	/// Sets the max_length
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::declarative::FieldMetadata;
	///
	/// let field = FieldMetadata::new("username", "CharField").max_length(150);
	/// assert_eq!(field.max_length, Some(150));
	/// ```
	pub fn max_length(mut self, max_length: u64) -> Self {
		self.max_length = Some(max_length);
		self
	}
}

/// Metadata for a declarative model
#[derive(Debug, Clone)]
pub struct ModelMetadata {
	/// The table name.
	pub table_name: String,
	/// The fields.
	pub fields: Vec<FieldMetadata>,
	/// The primary key fields.
	pub primary_key_fields: Vec<String>,
}

impl ModelMetadata {
	/// Creates a new ModelMetadata
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::declarative::ModelMetadata;
	///
	/// let metadata = ModelMetadata::new("users");
	/// assert_eq!(metadata.table_name, "users");
	/// assert_eq!(metadata.fields.len(), 0);
	/// ```
	pub fn new(table_name: impl Into<String>) -> Self {
		Self {
			table_name: table_name.into(),
			fields: Vec::new(),
			primary_key_fields: Vec::new(),
		}
	}

	/// Adds a field to the model metadata
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::declarative::{ModelMetadata, FieldMetadata};
	///
	/// let mut metadata = ModelMetadata::new("users");
	/// let field = FieldMetadata::new("username", "CharField").max_length(150);
	/// metadata.add_field(field);
	/// assert_eq!(metadata.fields.len(), 1);
	/// ```
	pub fn add_field(&mut self, field: FieldMetadata) {
		if field.primary_key {
			self.primary_key_fields.push(field.name.clone());
		}
		self.fields.push(field);
	}

	/// Gets a field by name
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::declarative::{ModelMetadata, FieldMetadata};
	///
	/// let mut metadata = ModelMetadata::new("users");
	/// let field = FieldMetadata::new("username", "CharField");
	/// metadata.add_field(field);
	/// assert!(metadata.get_field("username").is_some());
	/// assert!(metadata.get_field("nonexistent").is_none());
	/// ```
	pub fn get_field(&self, name: &str) -> Option<&FieldMetadata> {
		self.fields.iter().find(|f| f.name == name)
	}

	/// Converts to TableInfo for registry
	pub fn to_table_info(&self) -> TableInfo {
		let mut table_info = TableInfo::new(&self.table_name);

		for field in &self.fields {
			let column = ColumnInfo {
				name: field.name.clone(),
				column_type: field.field_type.clone(),
				nullable: field.nullable,
				default: field.default.clone(),
			};
			table_info.columns.push(column);
		}

		table_info.primary_key = self.primary_key_fields.clone();
		table_info
	}
}

/// Declarative base for model definitions
#[derive(Debug, Clone)]
pub struct DeclarativeBase {
	metadata: Arc<RwLock<HashMap<String, ModelMetadata>>>,
}

impl DeclarativeBase {
	/// Creates a new DeclarativeBase
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::declarative::DeclarativeBase;
	///
	/// let base = DeclarativeBase::new();
	/// assert_eq!(base.count(), 0);
	/// ```
	pub fn new() -> Self {
		Self {
			metadata: Arc::new(RwLock::new(HashMap::new())),
		}
	}

	/// Registers a model with its metadata
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::declarative::{DeclarativeBase, ModelMetadata, FieldMetadata};
	///
	/// let base = DeclarativeBase::new();
	/// let mut metadata = ModelMetadata::new("users");
	/// let id_field = FieldMetadata::new("id", "AutoField").primary_key(true);
	/// metadata.add_field(id_field);
	/// base.register_model("User", metadata);
	/// assert_eq!(base.count(), 1);
	/// ```
	pub fn register_model(&self, model_name: impl Into<String>, metadata: ModelMetadata) {
		let model_name = model_name.into();

		if let Ok(mut models) = self.metadata.write() {
			models.insert(model_name.clone(), metadata.clone());
		}

		let mut mapper = EntityMapper::new(&metadata.table_name);
		for pk in &metadata.primary_key_fields {
			mapper.primary_key.push(pk.clone());
		}

		registry().register(&model_name, mapper);
	}

	/// Gets metadata for a registered model
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::declarative::{DeclarativeBase, ModelMetadata, FieldMetadata};
	///
	/// let base = DeclarativeBase::new();
	/// let mut metadata = ModelMetadata::new("users");
	/// let id_field = FieldMetadata::new("id", "AutoField").primary_key(true);
	/// metadata.add_field(id_field);
	/// base.register_model("User", metadata);
	///
	/// let retrieved = base.get_metadata("User");
	/// assert!(retrieved.is_some());
	/// assert_eq!(retrieved.unwrap().table_name, "users");
	/// ```
	pub fn get_metadata(&self, model_name: &str) -> Option<ModelMetadata> {
		if let Ok(models) = self.metadata.read() {
			models.get(model_name).cloned()
		} else {
			None
		}
	}

	/// Removes a model from the declarative base
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::declarative::{DeclarativeBase, ModelMetadata};
	///
	/// let base = DeclarativeBase::new();
	/// let metadata = ModelMetadata::new("users");
	/// base.register_model("User", metadata);
	/// assert_eq!(base.count(), 1);
	///
	/// assert!(base.remove_model("User"));
	/// assert_eq!(base.count(), 0);
	/// assert!(!base.remove_model("NonExistent"));
	/// ```
	pub fn remove_model(&self, model_name: &str) -> bool {
		let removed = if let Ok(mut models) = self.metadata.write() {
			models.remove(model_name).is_some()
		} else {
			false
		};

		if removed {
			registry().remove(model_name);
		}

		removed
	}

	/// Clears all registered models
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::declarative::{DeclarativeBase, ModelMetadata};
	///
	/// let base = DeclarativeBase::new();
	/// base.register_model("User", ModelMetadata::new("users"));
	/// base.register_model("Post", ModelMetadata::new("posts"));
	/// assert_eq!(base.count(), 2);
	///
	/// base.clear();
	/// assert_eq!(base.count(), 0);
	/// ```
	pub fn clear(&self) {
		if let Ok(mut models) = self.metadata.write() {
			models.clear();
		}
	}

	/// Returns the number of registered models
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::declarative::{DeclarativeBase, ModelMetadata};
	///
	/// let base = DeclarativeBase::new();
	/// assert_eq!(base.count(), 0);
	///
	/// base.register_model("User", ModelMetadata::new("users"));
	/// assert_eq!(base.count(), 1);
	///
	/// base.register_model("Post", ModelMetadata::new("posts"));
	/// assert_eq!(base.count(), 2);
	/// ```
	pub fn count(&self) -> usize {
		if let Ok(models) = self.metadata.read() {
			models.len()
		} else {
			0
		}
	}

	/// Lists all registered model names
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::declarative::{DeclarativeBase, ModelMetadata};
	///
	/// let base = DeclarativeBase::new();
	/// base.register_model("User", ModelMetadata::new("users"));
	/// base.register_model("Post", ModelMetadata::new("posts"));
	///
	/// let names = base.list_models();
	/// assert_eq!(names.len(), 2);
	/// assert!(names.contains(&"User".to_string()));
	/// assert!(names.contains(&"Post".to_string()));
	/// ```
	pub fn list_models(&self) -> Vec<String> {
		if let Ok(models) = self.metadata.read() {
			models.keys().cloned().collect()
		} else {
			Vec::new()
		}
	}
}

impl Default for DeclarativeBase {
	fn default() -> Self {
		Self::new()
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_field_metadata_new() {
		let field = FieldMetadata::new("username", "CharField");
		assert_eq!(field.name, "username");
		assert_eq!(field.field_type, "CharField");
		assert!(!field.nullable);
		assert!(!field.primary_key);
		assert!(!field.unique);
		assert!(field.default.is_none());
		assert!(field.max_length.is_none());
	}

	#[test]
	fn test_field_metadata_builder() {
		let field = FieldMetadata::new("username", "CharField")
			.nullable(true)
			.unique(true)
			.max_length(150);

		assert!(field.nullable);
		assert!(field.unique);
		assert_eq!(field.max_length, Some(150));
	}

	#[test]
	fn test_field_metadata_primary_key() {
		let field = FieldMetadata::new("id", "AutoField").primary_key(true);
		assert!(field.primary_key);
	}

	#[test]
	fn test_field_metadata_chaining() {
		let field = FieldMetadata::new("email", "EmailField")
			.nullable(true)
			.unique(true)
			.max_length(254);

		assert_eq!(field.name, "email");
		assert!(field.nullable);
		assert!(field.unique);
		assert_eq!(field.max_length, Some(254));
	}

	#[test]
	fn test_field_metadata_from_char_field() {
		use crate::orm::fields::{CharField, Field};

		let mut char_field = CharField::new(100);
		char_field.set_attributes_from_name("username");

		let metadata = FieldMetadata::from_field(&char_field);
		assert_eq!(metadata.name, "username");
		assert_eq!(metadata.field_type, "CharField");
	}

	#[test]
	fn test_model_metadata_new() {
		let metadata = ModelMetadata::new("users");
		assert_eq!(metadata.table_name, "users");
		assert_eq!(metadata.fields.len(), 0);
		assert_eq!(metadata.primary_key_fields.len(), 0);
	}

	#[test]
	fn test_model_metadata_add_field() {
		let mut metadata = ModelMetadata::new("users");
		let field = FieldMetadata::new("username", "CharField").max_length(150);

		metadata.add_field(field);
		assert_eq!(metadata.fields.len(), 1);
	}

	#[test]
	fn test_model_metadata_add_primary_key() {
		let mut metadata = ModelMetadata::new("users");
		let id_field = FieldMetadata::new("id", "AutoField").primary_key(true);

		metadata.add_field(id_field);
		assert_eq!(metadata.primary_key_fields.len(), 1);
		assert_eq!(metadata.primary_key_fields[0], "id");
	}

	#[test]
	fn test_model_metadata_get_field() {
		let mut metadata = ModelMetadata::new("users");
		let field = FieldMetadata::new("username", "CharField");
		metadata.add_field(field);

		assert!(metadata.get_field("username").is_some());
		assert!(metadata.get_field("nonexistent").is_none());
	}

	#[test]
	fn test_model_metadata_multiple_fields() {
		let mut metadata = ModelMetadata::new("users");

		let id_field = FieldMetadata::new("id", "AutoField").primary_key(true);
		let username_field = FieldMetadata::new("username", "CharField").max_length(150);
		let email_field = FieldMetadata::new("email", "EmailField").max_length(254);

		metadata.add_field(id_field);
		metadata.add_field(username_field);
		metadata.add_field(email_field);

		assert_eq!(metadata.fields.len(), 3);
		assert_eq!(metadata.primary_key_fields.len(), 1);
	}

	#[test]
	fn test_declarative_base_new() {
		let base = DeclarativeBase::new();
		assert_eq!(base.count(), 0);
	}

	#[test]
	fn test_declarative_base_register_model() {
		let base = DeclarativeBase::new();
		let mut metadata = ModelMetadata::new("users");
		let id_field = FieldMetadata::new("id", "AutoField").primary_key(true);
		metadata.add_field(id_field);

		base.register_model("User", metadata);
		assert_eq!(base.count(), 1);
	}

	#[test]
	fn test_declarative_base_get_metadata() {
		let base = DeclarativeBase::new();
		let mut metadata = ModelMetadata::new("users");
		let id_field = FieldMetadata::new("id", "AutoField").primary_key(true);
		metadata.add_field(id_field);

		base.register_model("User", metadata);

		let retrieved = base.get_metadata("User");
		assert!(retrieved.is_some());
		assert_eq!(retrieved.unwrap().table_name, "users");
	}

	#[test]
	fn test_declarative_base_remove_model() {
		let base = DeclarativeBase::new();
		let metadata = ModelMetadata::new("users");

		base.register_model("User", metadata);
		assert_eq!(base.count(), 1);

		assert!(base.remove_model("User"));
		assert_eq!(base.count(), 0);
	}

	#[test]
	fn test_declarative_base_remove_nonexistent() {
		let base = DeclarativeBase::new();
		assert!(!base.remove_model("NonExistent"));
	}

	#[test]
	fn test_declarative_base_clear() {
		let base = DeclarativeBase::new();

		base.register_model("User", ModelMetadata::new("users"));
		base.register_model("Post", ModelMetadata::new("posts"));
		assert_eq!(base.count(), 2);

		base.clear();
		assert_eq!(base.count(), 0);
	}

	#[test]
	fn test_declarative_base_count() {
		let base = DeclarativeBase::new();
		assert_eq!(base.count(), 0);

		base.register_model("User", ModelMetadata::new("users"));
		assert_eq!(base.count(), 1);

		base.register_model("Post", ModelMetadata::new("posts"));
		assert_eq!(base.count(), 2);
	}

	#[test]
	fn test_declarative_base_list_models() {
		let base = DeclarativeBase::new();

		base.register_model("User", ModelMetadata::new("users"));
		base.register_model("Post", ModelMetadata::new("posts"));

		let names = base.list_models();
		assert_eq!(names.len(), 2);
		assert!(names.contains(&"User".to_string()));
		assert!(names.contains(&"Post".to_string()));
	}

	#[test]
	fn test_declarative_base_multiple_models() {
		let base = DeclarativeBase::new();

		let mut user_metadata = ModelMetadata::new("users");
		user_metadata.add_field(FieldMetadata::new("id", "AutoField").primary_key(true));
		user_metadata.add_field(FieldMetadata::new("username", "CharField").max_length(150));

		let mut post_metadata = ModelMetadata::new("posts");
		post_metadata.add_field(FieldMetadata::new("id", "AutoField").primary_key(true));
		post_metadata.add_field(FieldMetadata::new("title", "CharField").max_length(200));

		base.register_model("User", user_metadata);
		base.register_model("Post", post_metadata);

		assert_eq!(base.count(), 2);

		let user = base.get_metadata("User").unwrap();
		assert_eq!(user.table_name, "users");
		assert_eq!(user.fields.len(), 2);

		let post = base.get_metadata("Post").unwrap();
		assert_eq!(post.table_name, "posts");
		assert_eq!(post.fields.len(), 2);
	}

	#[test]
	fn test_field_metadata_composite_primary_key() {
		let mut metadata = ModelMetadata::new("user_groups");

		let user_id = FieldMetadata::new("user_id", "ForeignKey").primary_key(true);
		let group_id = FieldMetadata::new("group_id", "ForeignKey").primary_key(true);

		metadata.add_field(user_id);
		metadata.add_field(group_id);

		assert_eq!(metadata.primary_key_fields.len(), 2);
		assert!(metadata.primary_key_fields.contains(&"user_id".to_string()));
		assert!(
			metadata
				.primary_key_fields
				.contains(&"group_id".to_string())
		);
	}

	#[test]
	fn test_model_metadata_to_table_info() {
		let mut metadata = ModelMetadata::new("users");

		let id_field = FieldMetadata::new("id", "AutoField").primary_key(true);
		let username_field = FieldMetadata::new("username", "CharField")
			.max_length(150)
			.nullable(false)
			.unique(true);

		metadata.add_field(id_field);
		metadata.add_field(username_field);

		let table_info = metadata.to_table_info();

		assert_eq!(table_info.name, "users");
		assert_eq!(table_info.columns.len(), 2);
		assert_eq!(table_info.primary_key.len(), 1);
		assert_eq!(table_info.primary_key[0], "id");
	}
}
