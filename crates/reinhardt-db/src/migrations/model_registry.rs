//! Global model registry for Reinhardt migrations
//!
//! This module provides a Django-like model registration system that allows
//! models to be registered globally and accessed during migration generation.
//!
//! # Django Reference
//! Django's app registry is implemented in `django/apps/registry.py` and provides:
//! - Global model registration via `Apps.register_model()`
//! - Model retrieval via `Apps.get_models()`
//! - Thread-safe access with RwLock
//!
//! See [`ModelMetadata`] for the architecture comparison diagram.

use super::autodetector::{FieldState, ModelState};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

#[cfg_attr(doc, aquamarine::aquamarine)]
/// Model metadata for registration
///
/// # Architecture
///
/// This struct mirrors Django's model registration pattern:
///
/// ```mermaid
/// graph LR
///     subgraph Django["Django (Reference)"]
///         Apps["Apps"]
///         Apps --> all_models["all_models"]
///         Apps --> register_model["register_model()"]
///         Apps --> get_models["get_models()"]
///     end
///
///     subgraph Reinhardt["Reinhardt"]
///         ModelRegistry["ModelRegistry"]
///         ModelRegistry --> models["models"]
///         ModelRegistry --> register_model2["register_model()"]
///         ModelRegistry --> get_models2["get_models()"]
///         ModelRegistry --> get_model["get_model()"]
///     end
///
///     Django -.-> Reinhardt
/// ```
#[derive(Debug, Clone)]
pub struct ModelMetadata {
	/// Application label (e.g., "auth", "blog")
	pub app_label: String,
	/// Model name (e.g., "User", "Post")
	pub model_name: String,
	/// Table name (e.g., "auth_user", "blog_post")
	pub table_name: String,
	/// Field definitions
	pub fields: HashMap<String, FieldMetadata>,
	/// Model options (e.g., db_table, ordering)
	pub options: HashMap<String, String>,
	/// ManyToMany relationship definitions
	pub many_to_many_fields: Vec<ManyToManyMetadata>,
}

impl ModelMetadata {
	pub fn new(
		app_label: impl Into<String>,
		model_name: impl Into<String>,
		table_name: impl Into<String>,
	) -> Self {
		Self {
			app_label: app_label.into(),
			model_name: model_name.into(),
			table_name: table_name.into(),
			fields: HashMap::new(),
			options: HashMap::new(),
			many_to_many_fields: Vec::new(),
		}
	}

	pub fn add_field(&mut self, name: String, field: FieldMetadata) {
		self.fields.insert(name, field);
	}

	pub fn set_option(&mut self, key: String, value: String) {
		self.options.insert(key, value);
	}

	pub fn add_many_to_many(&mut self, m2m: ManyToManyMetadata) {
		self.many_to_many_fields.push(m2m);
	}

	/// Convert to ModelState for migrations
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_db::migrations::model_registry::{ModelMetadata, FieldMetadata};
	/// use reinhardt_db::migrations::FieldType;
	///
	/// let mut metadata = ModelMetadata::new("myapp", "User", "myapp_user");
	/// metadata.add_field(
	///     "email".to_string(),
	///     FieldMetadata::new(FieldType::VarChar(255)).with_param("max_length", "255"),
	/// );
	///
	/// let model_state = metadata.to_model_state();
	/// assert_eq!(model_state.app_label, "myapp");
	/// assert_eq!(model_state.name, "User");
	/// assert!(model_state.has_field("email"));
	/// ```
	pub fn to_model_state(&self) -> ModelState {
		let mut model_state = ModelState::new(&self.app_label, &self.model_name);

		// Set the correct table name from metadata
		// This overrides the default snake_case conversion in ModelState::new
		model_state.table_name = self.table_name.clone();

		// Convert fields
		for (name, field_meta) in &self.fields {
			let mut field_state = FieldState::new(
				name.clone(),
				field_meta.field_type.clone(),
				field_meta
					.params
					.get("null")
					.and_then(|v| v.parse::<bool>().ok())
					.unwrap_or(false),
			);
			for (key, value) in &field_meta.params {
				field_state.params.insert(key.clone(), value.clone());
			}
			// Set ForeignKey information if present
			if let Some(ref fk_info) = field_meta.foreign_key {
				field_state.foreign_key = Some(fk_info.clone());
			}
			model_state.add_field(field_state);
		}

		// Copy options
		model_state.options = self.options.clone();

		// Generate ForeignKey constraints from fields
		for (field_name, field_meta) in &self.fields {
			if field_meta.foreign_key.is_some() {
				model_state.add_foreign_key_constraint_from_field(field_name);
			}
		}

		// Copy ManyToMany relationship metadata
		model_state.many_to_many_fields = self.many_to_many_fields.clone();

		// Generate Unique constraints from field params
		for (field_name, field_meta) in &self.fields {
			if field_meta.params.get("unique").map(String::as_str) == Some("true") {
				use super::ConstraintDefinition;
				let constraint = ConstraintDefinition {
					name: format!(
						"{}_{}_{}_uniq",
						self.app_label,
						self.model_name.to_lowercase(),
						field_name
					),
					constraint_type: "unique".to_string(),
					fields: vec![field_name.clone()],
					expression: None,
					foreign_key_info: None,
				};
				model_state.constraints.push(constraint);
			}
		}

		model_state
	}
}

/// Field metadata for registration
#[derive(Debug, Clone)]
pub struct FieldMetadata {
	/// Field type (e.g., CharField, IntegerField, ForeignKey)
	pub field_type: super::FieldType,
	/// Field parameters (max_length, null, blank, default, etc.)
	pub params: HashMap<String, String>,
	/// ForeignKey information if this field is a foreign key
	pub foreign_key: Option<super::autodetector::ForeignKeyInfo>,
}

impl FieldMetadata {
	pub fn new(field_type: super::FieldType) -> Self {
		Self {
			field_type,
			params: HashMap::new(),
			foreign_key: None,
		}
	}

	pub fn with_param(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
		self.params.insert(key.into(), value.into());
		self
	}

	pub fn with_foreign_key(mut self, foreign_key: super::autodetector::ForeignKeyInfo) -> Self {
		self.foreign_key = Some(foreign_key);
		self
	}
}

/// Relationship metadata for `#[rel]` attributes
///
/// This structure holds metadata about relationships defined on model fields
/// using the `#[rel(...)]` attribute.
#[derive(Debug, Clone)]
pub struct RelationshipMetadata {
	/// Field name
	pub field_name: String,
	/// Relationship type (foreign_key, one_to_one, many_to_many, etc.)
	pub rel_type: String,
	/// Target model (e.g., "User", "auth.User")
	pub to_model: Option<String>,
	/// Related name for reverse accessor
	pub related_name: Option<String>,
	/// Through table name (for ManyToMany)
	pub through_table: Option<String>,
	/// Composite struct name (for additional through table fields)
	pub composite: Option<String>,
	/// Source model app label (for generating Through table foreign keys)
	pub source_app_label: Option<String>,
	/// Source model name (for generating Through table foreign keys)
	pub source_model_name: Option<String>,
}

impl RelationshipMetadata {
	/// Create a new RelationshipMetadata
	pub fn new(field_name: impl Into<String>, rel_type: impl Into<String>) -> Self {
		Self {
			field_name: field_name.into(),
			rel_type: rel_type.into(),
			to_model: None,
			related_name: None,
			through_table: None,
			composite: None,
			source_app_label: None,
			source_model_name: None,
		}
	}

	/// Set target model
	pub fn with_to_model(mut self, to_model: impl Into<String>) -> Self {
		self.to_model = Some(to_model.into());
		self
	}

	/// Set related name
	pub fn with_related_name(mut self, related_name: impl Into<String>) -> Self {
		self.related_name = Some(related_name.into());
		self
	}

	/// Set through table name
	pub fn with_through_table(mut self, through_table: impl Into<String>) -> Self {
		self.through_table = Some(through_table.into());
		self
	}

	/// Set composite struct name
	pub fn with_composite(mut self, composite: impl Into<String>) -> Self {
		self.composite = Some(composite.into());
		self
	}

	/// Set source model information
	pub fn with_source_info(
		mut self,
		app_label: impl Into<String>,
		model_name: impl Into<String>,
	) -> Self {
		self.source_app_label = Some(app_label.into());
		self.source_model_name = Some(model_name.into());
		self
	}

	/// Check if this is a ManyToMany relationship
	pub fn is_many_to_many(&self) -> bool {
		self.rel_type == "many_to_many" || self.rel_type == "polymorphic_many_to_many"
	}
}

/// ManyToMany relationship metadata
///
/// This structure holds specific metadata for ManyToMany relationships,
/// including through table information and custom field names.
#[derive(Debug, Clone, PartialEq)]
pub struct ManyToManyMetadata {
	/// Field name
	pub field_name: String,
	/// Target model name (e.g., "Group", "User")
	pub to_model: String,
	/// Related name for reverse accessor
	pub related_name: Option<String>,
	/// Custom through table name (if specified)
	pub through: Option<String>,
	/// Source field name in through table (defaults to "{source_model}_id")
	pub source_field: Option<String>,
	/// Target field name in through table (defaults to "{target_model}_id")
	pub target_field: Option<String>,
	/// Database constraint prefix
	pub db_constraint_prefix: Option<String>,
}

impl ManyToManyMetadata {
	/// Create a new ManyToManyMetadata
	pub fn new(field_name: impl Into<String>, to_model: impl Into<String>) -> Self {
		Self {
			field_name: field_name.into(),
			to_model: to_model.into(),
			related_name: None,
			through: None,
			source_field: None,
			target_field: None,
			db_constraint_prefix: None,
		}
	}

	/// Set related name
	pub fn with_related_name(mut self, related_name: impl Into<String>) -> Self {
		self.related_name = Some(related_name.into());
		self
	}

	/// Set through table name
	pub fn with_through(mut self, through: impl Into<String>) -> Self {
		self.through = Some(through.into());
		self
	}

	/// Set source field name
	pub fn with_source_field(mut self, source_field: impl Into<String>) -> Self {
		self.source_field = Some(source_field.into());
		self
	}

	/// Set target field name
	pub fn with_target_field(mut self, target_field: impl Into<String>) -> Self {
		self.target_field = Some(target_field.into());
		self
	}

	/// Set database constraint prefix
	pub fn with_db_constraint_prefix(mut self, prefix: impl Into<String>) -> Self {
		self.db_constraint_prefix = Some(prefix.into());
		self
	}
}

/// Global model registry
///
/// This registry is thread-safe and can be accessed from anywhere in the application.
/// Models should register themselves during initialization, typically via derive macros.
///
/// # Django Equivalent
/// ```python
/// # Django: django/apps/registry.py
/// class Apps:
///     def __init__(self):
///         self.all_models = defaultdict(dict)  # {app_label: {model_name: model_class}}
///
///     def register_model(self, app_label, model):
///         model_name = model._meta.model_name
///         self.all_models[app_label][model_name] = model
///
///     def get_models(self, include_auto_created=False, include_swapped=False):
///         result = []
///         for app_config in self.app_configs.values():
///             result.extend(app_config.get_models(include_auto_created, include_swapped))
///         return result
/// ```
#[derive(Debug, Clone)]
pub struct ModelRegistry {
	/// Models: (app_label, model_name) -> ModelMetadata
	models: Arc<RwLock<HashMap<(String, String), ModelMetadata>>>,
}

impl ModelRegistry {
	pub fn new() -> Self {
		Self {
			models: Arc::new(RwLock::new(HashMap::new())),
		}
	}

	/// Register a model in the registry
	///
	/// # Django Reference
	/// From: django/apps/registry.py:215-240
	/// ```python
	/// def register_model(self, app_label, model):
	///     model_name = model._meta.model_name
	///     app_models = self.all_models[app_label]
	///     if model_name in app_models:
	///         # Handle conflicts...
	///     app_models[model_name] = model
	/// ```
	pub fn register_model(&self, metadata: ModelMetadata) {
		let key = (metadata.app_label.clone(), metadata.model_name.clone());
		if let Ok(mut models) = self.models.write() {
			models.insert(key, metadata);
		}
	}

	/// Get all registered models
	///
	/// # Django Reference
	/// From: django/apps/registry.py:169-186
	/// ```python
	/// def get_models(self, include_auto_created=False, include_swapped=False):
	///     result = []
	///     for app_config in self.app_configs.values():
	///         result.extend(app_config.get_models(include_auto_created, include_swapped))
	///     return result
	/// ```
	pub fn get_models(&self) -> Vec<ModelMetadata> {
		if let Ok(models) = self.models.read() {
			models.values().cloned().collect()
		} else {
			Vec::new()
		}
	}

	/// Get a specific model by app_label and model_name
	///
	/// # Django Reference
	/// From: django/apps/registry.py:188-213
	/// ```python
	/// def get_model(self, app_label, model_name=None, require_ready=True):
	///     if model_name is None:
	///         app_label, model_name = app_label.split(".")
	///     app_config = self.get_app_config(app_label)
	///     return app_config.get_model(model_name, require_ready=require_ready)
	/// ```
	pub fn get_model(&self, app_label: &str, model_name: &str) -> Option<ModelMetadata> {
		if let Ok(models) = self.models.read() {
			models
				.get(&(app_label.to_string(), model_name.to_string()))
				.cloned()
		} else {
			None
		}
	}

	/// Get all models for a specific app
	pub fn get_app_models(&self, app_label: &str) -> Vec<ModelMetadata> {
		if let Ok(models) = self.models.read() {
			models
				.iter()
				.filter(|((app, _), _)| app == app_label)
				.map(|(_, meta)| meta.clone())
				.collect()
		} else {
			Vec::new()
		}
	}

	/// Remove a model from the registry
	pub fn remove_model(&self, app_label: &str, model_name: &str) -> bool {
		if let Ok(mut models) = self.models.write() {
			models
				.remove(&(app_label.to_string(), model_name.to_string()))
				.is_some()
		} else {
			false
		}
	}

	/// Clear all registered models
	pub fn clear(&self) {
		if let Ok(mut models) = self.models.write() {
			models.clear();
		}
	}

	/// Get the count of registered models
	pub fn count(&self) -> usize {
		if let Ok(models) = self.models.read() {
			models.len()
		} else {
			0
		}
	}
}

impl Default for ModelRegistry {
	fn default() -> Self {
		Self::new()
	}
}

/// Global model registry instance
///
/// This is the primary way to access the model registry from anywhere in the application.
pub fn global_registry() -> &'static ModelRegistry {
	use once_cell::sync::Lazy;
	static REGISTRY: Lazy<ModelRegistry> = Lazy::new(ModelRegistry::new);
	&REGISTRY
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::migrations::FieldType;

	#[test]
	fn test_model_registry_new() {
		let registry = ModelRegistry::new();
		assert_eq!(registry.count(), 0);
	}

	#[test]
	fn test_register_model() {
		let registry = ModelRegistry::new();
		let metadata = ModelMetadata::new("blog", "Post", "blog_post");
		registry.register_model(metadata);
		assert_eq!(registry.count(), 1);
	}

	#[test]
	fn test_get_model() {
		let registry = ModelRegistry::new();
		let metadata = ModelMetadata::new("auth", "User", "auth_user");
		registry.register_model(metadata);

		let retrieved = registry.get_model("auth", "User");
		assert!(retrieved.is_some());
		assert_eq!(retrieved.unwrap().table_name, "auth_user");
	}

	#[test]
	fn test_get_models() {
		let registry = ModelRegistry::new();
		registry.register_model(ModelMetadata::new("auth", "User", "auth_user"));
		registry.register_model(ModelMetadata::new("blog", "Post", "blog_post"));

		let models = registry.get_models();
		assert_eq!(models.len(), 2);
	}

	#[test]
	fn test_get_app_models() {
		let registry = ModelRegistry::new();
		registry.register_model(ModelMetadata::new("auth", "User", "auth_user"));
		registry.register_model(ModelMetadata::new("auth", "Group", "auth_group"));
		registry.register_model(ModelMetadata::new("blog", "Post", "blog_post"));

		let auth_models = registry.get_app_models("auth");
		assert_eq!(auth_models.len(), 2);

		let blog_models = registry.get_app_models("blog");
		assert_eq!(blog_models.len(), 1);
	}

	#[test]
	fn test_remove_model() {
		let registry = ModelRegistry::new();
		registry.register_model(ModelMetadata::new("auth", "User", "auth_user"));

		assert!(registry.remove_model("auth", "User"));
		assert_eq!(registry.count(), 0);
	}

	#[test]
	fn test_migrations_registry_clear() {
		let registry = ModelRegistry::new();
		registry.register_model(ModelMetadata::new("auth", "User", "auth_user"));
		registry.register_model(ModelMetadata::new("blog", "Post", "blog_post"));

		registry.clear();
		assert_eq!(registry.count(), 0);
	}

	#[test]
	fn test_model_metadata_to_model_state() {
		let mut metadata = ModelMetadata::new("blog", "Post", "blog_post");

		let mut title_field = FieldMetadata::new(FieldType::Custom("CharField".to_string()));
		title_field
			.params
			.insert("max_length".to_string(), "200".to_string());
		metadata.add_field("title".to_string(), title_field);

		let model_state = metadata.to_model_state();
		assert_eq!(model_state.name, "Post");
		assert_eq!(model_state.fields.len(), 1);
		assert!(model_state.fields.contains_key("title"));
	}

	#[test]
	fn test_field_metadata_builder() {
		let field = FieldMetadata::new(FieldType::Custom("CharField".to_string()))
			.with_param("max_length", "100")
			.with_param("null", "False");

		assert_eq!(field.field_type, FieldType::Custom("CharField".to_string()));
		assert_eq!(field.params.get("max_length").unwrap(), "100");
		assert_eq!(field.params.get("null").unwrap(), "False");
	}
}
