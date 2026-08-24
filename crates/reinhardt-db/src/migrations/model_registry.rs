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

use super::ConstraintDefinition;
use super::autodetector::{FieldState, ModelState};
use std::collections::{HashMap, HashSet};
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
	/// Model-level constraints declared via `#[model(unique_together = ...)]`
	/// and other peer-constraint attributes. Field-level `unique = true` is
	/// still synthesized inside `to_model_state()` and not stored here, to
	/// preserve the existing single-field UNIQUE behavior.
	///
	/// Kept private so that adding the field to a previously
	/// externally-constructible struct does not break the public API.
	/// Read via [`Self::constraints`]; write via [`Self::add_constraint`].
	constraints: Vec<ConstraintDefinition>,
}

impl ModelMetadata {
	const MAX_CONSTRAINT_IDENTIFIER_BYTES: usize = 63;

	/// Creates a new instance.
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
			constraints: Vec::new(),
		}
	}

	/// Adds field.
	pub fn add_field(&mut self, name: String, field: FieldMetadata) {
		self.fields.insert(name, field);
	}

	/// Sets the option.
	pub fn set_option(&mut self, key: String, value: String) {
		self.options.insert(key, value);
	}

	/// Adds many to many.
	pub fn add_many_to_many(&mut self, m2m: ManyToManyMetadata) {
		self.many_to_many_fields.push(m2m);
	}

	/// Adds a model-level constraint declared via macro attributes
	/// (e.g., `#[model(unique_together = ...)]`).
	pub fn add_constraint(&mut self, constraint: ConstraintDefinition) {
		self.constraints.push(constraint);
	}

	fn synthesized_unique_constraint_name(
		&self,
		field_name: &str,
		generated_names: &HashSet<String>,
	) -> String {
		let base_name = bounded_constraint_identifier(&format!(
			"{}_{}_uniq",
			safe_constraint_table_fragment(&self.table_name),
			safe_constraint_name_fragment(field_name)
		));
		let is_taken = |candidate: &str| {
			self.constraints
				.iter()
				.any(|constraint| constraint.name.eq_ignore_ascii_case(candidate))
				|| generated_names
					.iter()
					.any(|name| name.eq_ignore_ascii_case(candidate))
		};
		if !is_taken(&base_name) {
			return base_name;
		}

		let mut candidate = bounded_constraint_identifier(&format!("{base_name}_field"));
		let mut suffix = 2;
		while is_taken(&candidate) {
			candidate = bounded_constraint_identifier(&format!("{base_name}_field_{suffix}"));
			suffix += 1;
		}
		candidate
	}

	/// Returns model-level constraints registered by the `#[model(...)]`
	/// macro (currently composite UNIQUE from `unique_together`).
	///
	/// Field-level `unique = true` is not included here; it is synthesized
	/// inside [`Self::to_model_state`] from `FieldMetadata` parameters.
	pub fn constraints(&self) -> &[ConstraintDefinition] {
		&self.constraints
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
			let is_unique = field_meta.params.get("unique").map(String::as_str) == Some("true");
			let mut field_state = FieldState::new(
				name.clone(),
				field_meta.field_type.clone(),
				field_meta.nullable,
			);
			for (key, value) in &field_meta.params {
				if key == "null" || (is_unique && key == "unique") {
					continue;
				}
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

		// Generate named Unique constraints from field params. The field-level
		// `unique` flag is consumed above so the same declaration cannot be
		// emitted both inline and as a table constraint.
		let mut generated_unique_constraint_names = HashSet::new();
		let mut unique_fields = self
			.fields
			.iter()
			.filter(|(_, field_meta)| {
				field_meta.params.get("unique").map(String::as_str) == Some("true")
			})
			.collect::<Vec<_>>();
		unique_fields.sort_unstable_by(|(left, _), (right, _)| left.cmp(right));
		for (field_name, field_meta) in unique_fields {
			if field_meta.params.get("unique").map(String::as_str) == Some("true") {
				// Prefer a model-level declaration when it explicitly names the
				// single-column constraint. This keeps one physical constraint and
				// preserves the declared name.
				if self.constraints.iter().any(|constraint| {
					constraint.constraint_type.eq_ignore_ascii_case("unique")
						&& constraint.fields.len() == 1
						&& constraint.fields[0] == *field_name
				}) {
					continue;
				}
				let constraint = ConstraintDefinition {
					name: self.synthesized_unique_constraint_name(
						field_name,
						&generated_unique_constraint_names,
					),
					constraint_type: "unique".to_string(),
					fields: vec![field_name.clone()],
					expression: None,
					foreign_key_info: None,
				};
				generated_unique_constraint_names.insert(constraint.name.clone());
				model_state.constraints.push(constraint);
			}
		}

		// Copy model-level constraints declared via #[model(unique_together = ...)]
		// (and other peer-constraint attributes). These are populated by the
		// derive macro at registration time. See reinhardt-web#4022.
		model_state
			.constraints
			.extend(self.constraints.iter().cloned());

		model_state
	}
}

fn safe_constraint_name_fragment(value: &str) -> String {
	let mut fragment = String::with_capacity(value.len());
	for character in value.chars() {
		if character.is_ascii_alphanumeric() || character == '_' {
			fragment.push(character.to_ascii_lowercase());
		} else {
			fragment.push('_');
		}
	}

	if fragment.is_empty() {
		fragment.push_str("table");
	} else if fragment
		.as_bytes()
		.first()
		.is_some_and(|character| character.is_ascii_digit())
	{
		fragment.insert_str(0, "table_");
	}
	fragment
}

fn safe_constraint_table_fragment(value: &str) -> String {
	let fragment = safe_constraint_name_fragment(value);
	if fragment == value {
		return fragment;
	}
	format!("{fragment}_{:08x}", stable_constraint_name_hash(value))
}

fn stable_constraint_name_hash(value: &str) -> u32 {
	let mut hash = 0x811c9dc5_u32;
	for byte in value.bytes() {
		hash ^= u32::from(byte);
		hash = hash.wrapping_mul(0x01000193);
	}
	hash
}

fn bounded_constraint_identifier(value: &str) -> String {
	if value.len() <= ModelMetadata::MAX_CONSTRAINT_IDENTIFIER_BYTES {
		return value.to_owned();
	}

	let suffix = format!("_{:08x}", stable_constraint_name_hash(value));
	let prefix_len = ModelMetadata::MAX_CONSTRAINT_IDENTIFIER_BYTES - suffix.len();
	let mut end = prefix_len;
	while !value.is_char_boundary(end) {
		end -= 1;
	}
	format!("{}{}", &value[..end], suffix)
}

/// Field metadata for registration
#[derive(Debug, Clone)]
pub struct FieldMetadata {
	/// Field type (e.g., CharField, IntegerField, ForeignKey)
	pub field_type: super::FieldType,
	/// Whether this field is nullable (`NULL` is allowed).
	///
	/// This is the canonical source of truth. [`Self::is_nullable`]
	/// returns this value directly. [`Self::with_nullable`] sets both
	/// this field and syncs `params["null"]` for backward compatibility.
	pub nullable: bool,
	/// Field parameters (max_length, blank, default, etc.)
	pub params: HashMap<String, String>,
	/// ForeignKey information if this field is a foreign key
	pub foreign_key: Option<super::autodetector::ForeignKeyInfo>,
}

impl FieldMetadata {
	/// Creates a new instance.
	pub fn new(field_type: super::FieldType) -> Self {
		Self {
			field_type,
			nullable: false,
			params: HashMap::new(),
			foreign_key: None,
		}
	}

	/// Sets the param and returns self for chaining.
	///
	/// When `key` is `"null"`, the value is parsed as a bool and
	/// [`Self::nullable`] is synced automatically to prevent silent
	/// divergence between the struct field and `params["null"]`.
	/// Prefer [`Self::with_nullable`] for new code.
	pub fn with_param(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
		let key_s: String = key.into();
		let value_s: String = value.into();
		if key_s == "null" {
			let parsed = value_s.parse::<bool>().unwrap_or(false);
			self.nullable = parsed;
			self.params.insert(key_s, parsed.to_string());
			return self;
		}
		self.params.insert(key_s, value_s);
		self
	}

	/// Sets the nullability and returns self for chaining.
	///
	/// Sets both [`Self::nullable`] (canonical) and `params["null"]`
	/// (backward compatibility).
	pub fn with_nullable(mut self, nullable: bool) -> Self {
		self.nullable = nullable;
		self.params.insert("null".to_string(), nullable.to_string());
		self
	}

	/// Returns whether the column is nullable (i.e., `NULL` is allowed).
	pub fn is_nullable(&self) -> bool {
		self.nullable
	}

	/// Sets the foreign key and returns self for chaining.
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
	/// Creates a new instance.
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
	/// Returns a freshly-cloned `Vec<ModelMetadata>`. For hot paths that
	/// only need to look up a single model, prefer
	/// [`Self::find_model_qualified`] (when the target app is known) or
	/// [`Self::find_model_by_name`] (when only the model name is known)
	/// to avoid materializing the entire registry on each call.
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

	/// Find a model by `(app_label, model_name)` without materializing the
	/// entire registry.
	///
	/// The cost is an O(1) index lookup plus a single clone of the matched
	/// [`ModelMetadata`] (whose size depends on its `fields` vector). This
	/// is the preferred path for hot code (e.g. migration generation, FK
	/// column type resolution) where [`Self::get_models`] would otherwise
	/// clone every registered model on every call.
	///
	/// Semantically equivalent to [`Self::get_model`]; named to make the
	/// "qualified lookup" intent explicit at call sites. See issue #4436.
	pub fn find_model_qualified(&self, app_label: &str, model_name: &str) -> Option<ModelMetadata> {
		self.get_model(app_label, model_name)
	}

	/// Find a model by `model_name` alone, without an app label.
	///
	/// Scans the registry values under the read lock but clones only the
	/// matched entry (not the entire registry), so it avoids the
	/// `Vec<ModelMetadata>` materialization in [`Self::get_models`].
	///
	/// # Ambiguity
	///
	/// If two or more apps have registered a model with the same
	/// `model_name`, this function returns `None` and emits a
	/// `tracing::warn!` (one log line per call — there is no
	/// deduplication, so callers on a hot path should switch to
	/// [`Self::find_model_qualified`]). Callers that need a specific
	/// cross-app FK target must use [`Self::find_model_qualified`]
	/// instead. This conservative behavior prevents the silent
	/// wrong-target resolution flagged on PR #4434 (Copilot review
	/// thread HYL).
	///
	/// See issue #4436.
	pub fn find_model_by_name(&self, model_name: &str) -> Option<ModelMetadata> {
		let models = self.models.read().ok()?;
		let mut matches = models.values().filter(|m| m.model_name == model_name);
		let first = matches.next()?.clone();
		if matches.next().is_some() {
			tracing::warn!(
				model_name,
				"ModelRegistry::find_model_by_name: ambiguous model name registered \
				 under multiple app labels; returning None. Use \
				 ModelRegistry::find_model_qualified(app, name) to disambiguate.",
			);
			return None;
		}
		Some(first)
	}

	/// Count how many registered models have `model_name`, irrespective
	/// of app label.
	///
	/// Used by [`crate::migrations::operations`] FK column-type
	/// resolution to distinguish "model name is genuinely missing" from
	/// "model name is registered under more than one app" when a
	/// by-name lookup returns `None`. The two cases need different
	/// diagnostics: ambiguity is a user error worth a `tracing::warn!`,
	/// while a missing name is normal during partial registry
	/// population at startup.
	///
	/// See issue #4436.
	pub fn count_models_by_name(&self, model_name: &str) -> usize {
		if let Ok(models) = self.models.read() {
			models
				.values()
				.filter(|m| m.model_name == model_name)
				.count()
		} else {
			0
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
	use crate::migrations::autodetector::{MigrationAutodetector, ProjectState};
	use crate::migrations::operations::{Constraint, Operation, SqlDialect};
	use rstest::rstest;

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
	fn test_find_model_qualified_hit() {
		// Arrange
		let registry = ModelRegistry::new();
		registry.register_model(ModelMetadata::new("auth", "User", "auth_user"));
		registry.register_model(ModelMetadata::new("blog", "Post", "blog_post"));

		// Act
		let hit = registry.find_model_qualified("auth", "User");

		// Assert
		assert!(hit.is_some());
		let model = hit.unwrap();
		assert_eq!(model.app_label, "auth");
		assert_eq!(model.model_name, "User");
		assert_eq!(model.table_name, "auth_user");
	}

	#[test]
	fn test_find_model_qualified_miss_wrong_app() {
		// Arrange
		let registry = ModelRegistry::new();
		registry.register_model(ModelMetadata::new("auth", "User", "auth_user"));

		// Act / Assert: same model name registered under a different app
		// must not be returned.
		assert!(registry.find_model_qualified("billing", "User").is_none());
	}

	#[test]
	fn test_find_model_by_name_unique() {
		// Arrange
		let registry = ModelRegistry::new();
		registry.register_model(ModelMetadata::new("auth", "User", "auth_user"));
		registry.register_model(ModelMetadata::new("blog", "Post", "blog_post"));

		// Act
		let hit = registry.find_model_by_name("Post");

		// Assert
		assert!(hit.is_some());
		assert_eq!(hit.unwrap().app_label, "blog");
	}

	#[test]
	fn test_find_model_by_name_missing() {
		// Arrange
		let registry = ModelRegistry::new();
		registry.register_model(ModelMetadata::new("auth", "User", "auth_user"));

		// Act / Assert
		assert!(registry.find_model_by_name("NoSuchModel").is_none());
	}

	#[test]
	fn test_find_model_by_name_ambiguous_returns_none() {
		// Arrange: same model name registered under two different apps.
		// The conservative behavior is to refuse the unqualified lookup
		// rather than silently pick one (issue #4436, PR #4434 thread HYL).
		let registry = ModelRegistry::new();
		registry.register_model(ModelMetadata::new("auth", "User", "auth_user"));
		registry.register_model(ModelMetadata::new("billing", "User", "billing_user"));

		// Act
		let hit = registry.find_model_by_name("User");

		// Assert
		assert!(hit.is_none());
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
	fn test_unique_field_uses_stable_table_constraint_without_inline_duplicate() {
		// Arrange
		let mut metadata = ModelMetadata::new("auth", "RenamedEmailVerificationToken", "auth_evt");
		metadata.add_field(
			"token_hash".to_string(),
			FieldMetadata::new(FieldType::VarChar(255)).with_param("unique", "true"),
		);

		// Act
		let model_state = metadata.to_model_state();
		let mut to_state = ProjectState::new();
		to_state.add_model(model_state);
		let migrations =
			MigrationAutodetector::new(ProjectState::new(), to_state).generate_migrations();

		// Assert
		let model_state = &migrations[0].operations;
		let Operation::CreateTable {
			columns,
			constraints,
			..
		} = &model_state[0]
		else {
			panic!("expected an initial CreateTable operation");
		};
		assert_eq!(
			columns
				.iter()
				.filter(|column| column.name == "token_hash" && column.unique)
				.count(),
			0,
			"single-column uniqueness must not be emitted inline"
		);
		assert_eq!(
			constraints,
			&vec![Constraint::Unique {
				name: "auth_evt_token_hash_uniq".to_string(),
				columns: vec!["token_hash".to_string()],
			}],
			"the physical constraint name must derive from the stable table name"
		);
		assert_eq!(
			model_state[0]
				.to_sql(&SqlDialect::Postgres)
				.matches("UNIQUE")
				.count(),
			1,
			"the generated PostgreSQL DDL must contain one UNIQUE representation"
		);
	}

	#[test]
	fn test_explicit_single_field_unique_constraint_name_is_preserved() {
		// Arrange
		let mut metadata = ModelMetadata::new("auth", "Token", "auth_evt");
		metadata.add_field(
			"token_hash".to_string(),
			FieldMetadata::new(FieldType::VarChar(255)).with_param("unique", "true"),
		);
		metadata.add_constraint(ConstraintDefinition {
			name: "auth_evt_token_hash_uniq".to_string(),
			constraint_type: "unique".to_string(),
			fields: vec!["token_hash".to_string()],
			expression: None,
			foreign_key_info: None,
		});

		// Act
		let model_state = metadata.to_model_state();

		// Assert
		assert!(
			!model_state.fields["token_hash"]
				.params
				.contains_key("unique")
		);
		assert_eq!(model_state.constraints.len(), 1);
		assert_eq!(model_state.constraints[0].name, "auth_evt_token_hash_uniq");
	}

	#[test]
	fn test_synthesized_unique_constraint_avoids_model_constraint_name_collision() {
		// Arrange
		let mut metadata = ModelMetadata::new("accounts", "Account", "accounts");
		metadata.add_field(
			"a_b".to_string(),
			FieldMetadata::new(FieldType::VarChar(255)).with_param("unique", "true"),
		);
		metadata.add_field("a".to_string(), FieldMetadata::new(FieldType::VarChar(255)));
		metadata.add_field("b".to_string(), FieldMetadata::new(FieldType::VarChar(255)));
		metadata.add_constraint(ConstraintDefinition {
			name: "accounts_a_b_uniq".to_string(),
			constraint_type: "unique".to_string(),
			fields: vec!["a".to_string(), "b".to_string()],
			expression: None,
			foreign_key_info: None,
		});

		// Act
		let model_state = metadata.to_model_state();

		// Assert
		let mut names: Vec<_> = model_state
			.constraints
			.iter()
			.map(|constraint| constraint.name.as_str())
			.collect();
		names.sort_unstable();
		assert_eq!(names, vec!["accounts_a_b_uniq", "accounts_a_b_uniq_field"]);
	}

	#[test]
	fn test_synthesized_unique_constraint_names_avoid_normalized_field_collisions() {
		// Arrange
		let mut metadata = ModelMetadata::new("accounts", "Account", "accounts");
		metadata.add_field(
			"é".to_string(),
			FieldMetadata::new(FieldType::VarChar(255)).with_param("unique", "true"),
		);
		metadata.add_field(
			"ü".to_string(),
			FieldMetadata::new(FieldType::VarChar(255)).with_param("unique", "true"),
		);

		// Act
		let model_state = metadata.to_model_state();

		// Assert
		let mut names: Vec<_> = model_state
			.constraints
			.iter()
			.map(|constraint| constraint.name.as_str())
			.collect();
		names.sort_unstable();
		assert_eq!(names, vec!["accounts___uniq", "accounts___uniq_field"]);
	}

	#[test]
	fn test_synthesized_unique_constraint_name_is_safe_for_custom_table_names() {
		// Arrange
		let mut metadata = ModelMetadata::new("accounts", "Account", "User-Events");
		metadata.add_field(
			"token".to_string(),
			FieldMetadata::new(FieldType::VarChar(255)).with_param("unique", "true"),
		);

		// Act
		let model_state = metadata.to_model_state();
		let constraint_name = model_state.constraints[0].name.clone();
		let expected_constraint_name = format!(
			"user_events_{:08x}_token_uniq",
			stable_constraint_name_hash("User-Events")
		);
		let mut to_state = ProjectState::new();
		to_state.add_model(model_state);
		let migrations =
			MigrationAutodetector::new(ProjectState::new(), to_state).generate_migrations();
		let sql = migrations[0].operations[0].to_sql(&SqlDialect::Postgres);

		// Assert
		assert_eq!(constraint_name, expected_constraint_name);
		assert_eq!(
			sql,
			format!(
				"CREATE TABLE \"User-Events\" (\n  token VARCHAR(255) NOT NULL,\n  CONSTRAINT {expected_constraint_name} UNIQUE (token)\n);"
			)
		);
	}

	#[test]
	fn test_synthesized_unique_constraint_names_are_distinct_for_normalized_tables() {
		let mut dashed = ModelMetadata::new("accounts", "Dashed", "User-Events");
		dashed.add_field(
			"token".to_string(),
			FieldMetadata::new(FieldType::VarChar(255)).with_param("unique", "true"),
		);
		let mut underscored = ModelMetadata::new("accounts", "Underscored", "user_events");
		underscored.add_field(
			"token".to_string(),
			FieldMetadata::new(FieldType::VarChar(255)).with_param("unique", "true"),
		);

		assert_ne!(
			dashed.to_model_state().constraints[0].name,
			underscored.to_model_state().constraints[0].name
		);
	}

	#[test]
	fn test_synthesized_unique_constraint_names_are_bounded_and_distinct() {
		let long_table = "t".repeat(40);
		let long_field = "f".repeat(40);
		let other_field = format!("{}g", "f".repeat(39));
		let mut metadata = ModelMetadata::new("accounts", "Account", long_table);
		metadata.add_field(
			long_field,
			FieldMetadata::new(FieldType::VarChar(255)).with_param("unique", "true"),
		);
		metadata.add_field(
			other_field,
			FieldMetadata::new(FieldType::VarChar(255)).with_param("unique", "true"),
		);

		let constraints = metadata.to_model_state().constraints;
		assert_eq!(constraints.len(), 2);
		assert!(constraints.iter().all(|constraint| {
			constraint.name.len() <= ModelMetadata::MAX_CONSTRAINT_IDENTIFIER_BYTES
		}));
		assert_ne!(constraints[0].name, constraints[1].name);
	}

	#[test]
	fn test_field_metadata_builder() {
		let field = FieldMetadata::new(FieldType::Custom("CharField".to_string()))
			.with_param("max_length", "100")
			.with_nullable(false);

		assert_eq!(field.field_type, FieldType::Custom("CharField".to_string()));
		assert_eq!(field.params.get("max_length").unwrap(), "100");
		assert!(!field.nullable);
		assert_eq!(field.params.get("null").unwrap(), "false");

		let field =
			FieldMetadata::new(FieldType::Custom("IntegerField".to_string())).with_nullable(true);
		assert!(field.nullable);
		assert_eq!(field.params.get("null").unwrap(), "true");
	}

	#[rstest]
	#[case(true, true)]
	#[case(false, false)]
	fn test_to_model_state_overrides_nullable_from_params(
		#[case] nullable: bool,
		#[case] expected_nullable: bool,
	) {
		// Arrange
		let mut metadata = ModelMetadata::new("blog", "Post", "blog_post");
		let field = FieldMetadata::new(FieldType::Custom("CharField".to_string()))
			.with_param("max_length", "200")
			.with_nullable(nullable);
		metadata.add_field("description".to_string(), field);

		// Act
		let model_state = metadata.to_model_state();

		// Assert
		let field_state = model_state.fields.get("description").unwrap();
		assert_eq!(field_state.nullable, expected_nullable);
		assert!(
			!field_state.params.contains_key("null"),
			"params must not contain `null` key after to_model_state 			 (it is already carried by FieldState.nullable)"
		);
	}

	#[rstest]
	fn to_model_state_nullable_false_for_primary_key_matches_macro_contract() {
		// Arrange — regression for issue #4052.
		//
		// The `#[model]` macro must emit `null = "false"` for primary key
		// fields regardless of whether the Rust type is `Option<T>`. The
		// `Option<T>` wrapper for PKs is a Rust-side convention to allow
		// `id = None` before the DB assigns the auto-increment value, not
		// a DB-level nullability statement. PK columns are always NOT NULL
		// at the DB level.
		//
		// This test codifies the contract that `to_model_state` consumes
		// from the macro: with the fixed macro params, the resulting
		// `FieldState.nullable` for an `Option<i64>` PK must be `false`,
		// matching the migration-replay path's
		// `column_def_to_field_state(...).nullable = !col.not_null = false`.
		//
		// Pre-fix, the macro emitted `null = "true"` for any Option<T>
		// field including PKs, producing `FieldState.nullable = true` and
		// surfacing as a spurious `AlterColumn` for the unchanged PK in
		// offline `makemigrations` runs.
		let mut metadata = ModelMetadata::new("clusters", "Cluster", "clusters");
		// Mirror the fixed macro params for `id: Option<i64>` with
		// `#[field(primary_key = true)]`: `null = "false"` (forced by the
		// fix), `not_null = "true"`, `primary_key = "true"`,
		// `auto_increment = "true"`.
		let id_field = FieldMetadata::new(FieldType::BigInteger)
			.with_param("primary_key", "true")
			.with_param("auto_increment", "true")
			.with_param("not_null", "true")
			.with_nullable(false);
		metadata.add_field("id".to_string(), id_field);

		// Act
		let model_state = metadata.to_model_state();

		// Assert — nullable=false on the FieldState side, regardless of
		// the underlying Rust Option<T> wrapping.
		let id_state = model_state
			.fields
			.get("id")
			.expect("id field present in to_model_state output");
		assert!(
			!id_state.nullable,
			"PK FieldState.nullable must be false even when the Rust type is \
			 Option<i64>. Did the #[model] macro regress to emitting \
			 null=\"true\" for Option<T> PKs? params={:?}",
			id_state.params
		);
		assert!(
			!id_state.params.contains_key("null"),
			"PK params must not contain `null` after to_model_state \
			 (nullable is already carried by FieldState.nullable). \
			 Got params={:?}",
			id_state.params
		);
	}
}
