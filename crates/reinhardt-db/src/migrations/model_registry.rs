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

use super::autodetector::{FieldState, IndexDefinition, ModelState, to_snake_case};
use super::{ConstraintDefinition, GeneratedColumnDefinition};
use crate::field_domain::FieldDomain;
use std::collections::{BTreeSet, HashMap};
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
	/// Model-declared indexes.
	///
	/// Kept private so adding structured index metadata does not break external
	/// struct construction. Read and write through the methods below.
	indexes: Vec<IndexDefinition>,
}

impl ModelMetadata {
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
			indexes: Vec::new(),
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

	/// Adds a model-declared index.
	pub fn add_index(&mut self, index: IndexDefinition) {
		self.indexes.push(index);
	}

	/// Returns model-declared indexes.
	pub fn indexes(&self) -> &[IndexDefinition] {
		&self.indexes
	}

	/// Adds the typed enum-domain constraint for a database column.
	pub fn add_enum_domain_constraint(&mut self, column: &str, domain: FieldDomain) {
		let name = super::operations::truncate_identifier_with_hash(&format!(
			"{}_{}_model_enum_check",
			self.table_name, column
		));
		self.constraints
			.push(ConstraintDefinition::enum_domain(name, column, domain));
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
		self.to_model_state_with_registry(global_registry())
	}

	fn to_model_state_with_registry(&self, registry: &ModelRegistry) -> ModelState {
		let mut model_state = ModelState::new(&self.app_label, &self.model_name);

		// Set the correct table name from metadata
		// This overrides the default snake_case conversion in ModelState::new
		model_state.table_name = self.table_name.clone();

		// Convert fields
		for (name, field_meta) in &self.fields {
			let column_name = field_meta
				.params
				.get("db_column")
				.cloned()
				.unwrap_or_else(|| name.clone());
			let mut field_state = FieldState::new(
				column_name,
				field_meta.field_type.clone(),
				field_meta.nullable,
			);
			for (key, value) in &field_meta.params {
				if key == "null" {
					continue;
				}
				field_state.params.insert(key.clone(), value.clone());
			}
			field_state.generated = field_meta.generated.clone();
			field_state.domain = field_meta.domain.clone();
			// Resolve qualified string FK targets after all model metadata has been
			// registered so explicit table-name overrides are preserved (#5673).
			let mut foreign_key = field_meta.foreign_key.clone();
			if let Some(fk_info) = &mut foreign_key
				&& let (Some(target_app), Some(target_model)) = (
					field_meta.params.get("fk_target_app"),
					field_meta.params.get("fk_target_model"),
				) {
				fk_info.referenced_table = registry
					.get_model(target_app, target_model)
					.map(|metadata| metadata.table_name)
					.unwrap_or_else(|| format!("{}_{}", target_app, to_snake_case(target_model)));
			}
			field_state.foreign_key = foreign_key;
			model_state.add_field(field_state);
		}

		// Copy options
		model_state.options = self.options.clone();

		// Generate ForeignKey constraints from fields
		for (field_name, field_meta) in &self.fields {
			if field_meta.foreign_key.is_some() {
				let column_name = field_meta
					.params
					.get("db_column")
					.map_or(field_name, |name| name);
				model_state.add_foreign_key_constraint_from_field(column_name);
			}
		}

		// Copy ManyToMany relationship metadata
		model_state.many_to_many_fields = self.many_to_many_fields.clone();

		// Generate Unique constraints from field params
		for (field_name, field_meta) in &self.fields {
			if field_meta.params.get("unique").map(String::as_str) == Some("true") {
				let column_name = field_meta
					.params
					.get("db_column")
					.map_or(field_name, |name| name);
				let constraint = ConstraintDefinition {
					name: format!(
						"{}_{}_{}_uniq",
						self.app_label,
						self.model_name.to_lowercase(),
						column_name
					),
					constraint_type: "unique".to_string(),
					fields: vec![column_name.clone()],
					expression: None,
					foreign_key_info: None,
				};
				model_state.constraints.push(constraint);
			}
		}

		// Copy model-level constraints declared via #[model(unique_together = ...)]
		// (and other peer-constraint attributes). These are populated by the
		// derive macro at registration time. See reinhardt-web#4022.
		model_state
			.constraints
			.extend(self.constraints.iter().cloned());
		model_state.indexes.extend(self.indexes.iter().cloned());

		for (name, field_meta) in &self.fields {
			let Some(domain) = &field_meta.domain else {
				continue;
			};
			let column = field_meta
				.params
				.get("db_column")
				.map(String::as_str)
				.unwrap_or(name);
			let constraint_name = super::operations::truncate_identifier_with_hash(&format!(
				"{}_{}_model_enum_check",
				self.table_name, column
			));
			if !model_state
				.constraints
				.iter()
				.any(|constraint| constraint.name == constraint_name)
			{
				model_state
					.constraints
					.push(ConstraintDefinition::enum_domain(
						constraint_name,
						column,
						domain.clone(),
					));
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
	/// Whether this field is nullable (`NULL` is allowed).
	///
	/// This is the canonical source of truth. [`Self::is_nullable`]
	/// returns this value directly. [`Self::with_nullable`] sets both
	/// this field and syncs `params["null"]` for backward compatibility.
	pub nullable: bool,
	/// Field parameters (max_length, blank, default, etc.)
	pub params: HashMap<String, String>,
	/// Generated-column metadata.
	pub generated: Option<GeneratedColumnDefinition>,
	/// Structured database value domain.
	pub domain: Option<FieldDomain>,
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
			generated: None,
			domain: None,
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

	/// Sets generated-column metadata and returns self for chaining.
	pub fn with_generated(mut self, generated: GeneratedColumnDefinition) -> Self {
		self.generated = Some(generated);
		self
	}

	/// Sets optional structured field-domain metadata and returns self for chaining.
	pub fn with_domain_opt(mut self, domain: Option<FieldDomain>) -> Self {
		self.domain = domain.map(FieldDomain::canonicalized);
		self
	}

	/// Sets structured database value domain metadata.
	pub fn with_domain(mut self, domain: FieldDomain) -> Self {
		self.domain = Some(domain.canonicalized());
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

	/// Validates physical index names across all registered models.
	///
	/// PostgreSQL index names share a schema-level namespace, so two models
	/// cannot safely declare the same physical name even when their tables
	/// differ.
	pub fn validate_physical_index_names(&self) -> super::Result<()> {
		let models = self.models.read().map_err(|_| {
			super::MigrationError::InvalidMigration("model registry lock is poisoned".to_string())
		})?;
		let mut owners = HashMap::new();
		for metadata in models.values() {
			if let Some(previous_table) =
				owners.insert(metadata.table_name.clone(), metadata.table_name.clone())
			{
				return Err(super::MigrationError::InvalidMigration(format!(
					"physical table name `{}` is registered by both `{}` and `{}`",
					metadata.table_name, previous_table, metadata.table_name
				)));
			}
		}
		for metadata in models.values() {
			for index in metadata.indexes() {
				if index.name.is_empty() {
					return Err(super::MigrationError::InvalidMigration(format!(
						"physical index name on table `{}` must not be empty",
						metadata.table_name
					)));
				}
				if index.name.contains('\0') {
					return Err(super::MigrationError::InvalidMigration(format!(
						"physical index name on table `{}` must not contain NUL",
						metadata.table_name
					)));
				}
				if let Some(previous_table) =
					owners.insert(index.name.clone(), metadata.table_name.clone())
				{
					return Err(super::MigrationError::InvalidMigration(format!(
						"physical index name `{}` on table `{}` conflicts with relation name owned by table `{}`",
						index.name, metadata.table_name, previous_table
					)));
				}
			}
		}
		Ok(())
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

	/// Get all registered models or report a poisoned registry lock.
	pub fn try_get_models(&self) -> super::Result<Vec<ModelMetadata>> {
		self.models
			.read()
			.map(|models| models.values().cloned().collect())
			.map_err(|_| {
				super::MigrationError::InvalidMigration(
					"model registry lock is poisoned".to_string(),
				)
			})
	}

	/// Collect storage aliases referenced by semantic file fields.
	///
	/// Migration metadata records the semantic field type and storage alias in
	/// [`FieldMetadata::params`]. Keeping this collector in the database crate
	/// avoids introducing a dependency from storage construction back into the
	/// model registry.
	pub fn file_storage_aliases(&self) -> BTreeSet<String> {
		self.get_models()
			.into_iter()
			.flat_map(|model| model.fields.into_values())
			.filter(|field| {
				field.params.get("model_field_type").map(String::as_str) == Some("file")
			})
			.map(|field| {
				field
					.params
					.get("file_storage")
					.cloned()
					.unwrap_or_else(|| "default".to_string())
			})
			.collect()
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
	use crate::migrations::{FieldType, GeneratedStorage, SchemaExpr};
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
	#[cfg(feature = "pgvector")]
	fn duplicate_physical_index_names_are_rejected_by_registry_validation() {
		// Arrange
		let registry = ModelRegistry::new();
		let mut document = ModelMetadata::new("search", "Document", "search_document");
		document.add_index(IndexDefinition {
			name: "shared_embedding_ann".to_string(),
			fields: vec!["embedding".to_string()],
			unique: false,
			where_clause: None,
			index_type: Some(super::super::operations::IndexType::Hnsw {
				m: Some(16),
				ef_construction: Some(64),
			}),
			operator_class: Some("vector_cosine_ops".to_string()),
			expressions: None,
		});
		let mut invoice = ModelMetadata::new("billing", "Invoice", "billing_invoice");
		invoice.add_index(IndexDefinition {
			name: "shared_embedding_ann".to_string(),
			fields: vec!["embedding".to_string()],
			unique: false,
			where_clause: None,
			index_type: Some(super::super::operations::IndexType::Ivfflat { lists: Some(100) }),
			operator_class: Some("vector_l2_ops".to_string()),
			expressions: None,
		});
		registry.register_model(document);
		registry.register_model(invoice);

		// Act
		let error = registry
			.validate_physical_index_names()
			.expect_err("duplicate physical index names must fail validation");

		// Assert
		assert!(matches!(
			error,
			super::super::MigrationError::InvalidMigration(message)
				if message.contains("shared_embedding_ann")
					&& message.contains("search_document")
					&& message.contains("billing_invoice")
		));
	}

	#[test]
	#[cfg(feature = "pgvector")]
	fn physical_index_names_colliding_with_table_names_are_rejected() {
		let registry = ModelRegistry::new();
		let document = ModelMetadata::new("search", "Document", "search_document");
		let mut invoice = ModelMetadata::new("billing", "Invoice", "billing_invoice");
		invoice.add_index(IndexDefinition {
			name: "search_document".to_string(),
			fields: vec!["embedding".to_string()],
			unique: false,
			where_clause: None,
			index_type: Some(super::super::operations::IndexType::Hnsw {
				m: Some(16),
				ef_construction: Some(64),
			}),
			operator_class: Some("vector_cosine_ops".to_string()),
			expressions: None,
		});
		registry.register_model(document);
		registry.register_model(invoice);

		let error = registry
			.validate_physical_index_names()
			.expect_err("an index may not reuse a physical table relation name");

		assert!(matches!(
			error,
			super::super::MigrationError::InvalidMigration(message)
				if message.contains("search_document") && message.contains("billing_invoice")
		));
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
	fn try_get_models_reports_a_poisoned_lock() {
		let registry = ModelRegistry::new();
		let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
			let _guard = registry.models.write().unwrap();
			panic!("poison the registry lock");
		}));
		assert!(result.is_err());

		let error = registry.try_get_models().unwrap_err();
		assert!(matches!(
			error,
			super::super::MigrationError::InvalidMigration(message)
				if message == "model registry lock is poisoned"
		));
	}

	#[test]
	fn file_storage_aliases_collect_only_file_fields_and_default_missing_aliases() {
		// Arrange
		let registry = ModelRegistry::new();
		let mut metadata = ModelMetadata::new("media", "Asset", "media_asset");
		metadata.add_field(
			"title".to_string(),
			FieldMetadata::new(FieldType::VarChar(255)).with_param("model_field_type", "string"),
		);
		metadata.add_field(
			"private_file".to_string(),
			FieldMetadata::new(FieldType::VarChar(255))
				.with_param("model_field_type", "file")
				.with_param("file_storage", "private_uploads"),
		);
		metadata.add_field(
			"public_file".to_string(),
			FieldMetadata::new(FieldType::VarChar(255)).with_param("model_field_type", "file"),
		);
		registry.register_model(metadata);

		// Act
		let aliases = registry.file_storage_aliases();

		// Assert
		assert_eq!(
			aliases,
			["default".to_string(), "private_uploads".to_string()]
				.into_iter()
				.collect()
		);
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
	fn file_field_model_state_preserves_semantic_and_physical_storage_metadata() {
		let mut metadata = ModelMetadata::new("media", "Asset", "media_asset");
		let file_field = FieldMetadata::new(FieldType::VarChar(255))
			.with_param("model_field_type", "file")
			.with_param("upload_to", "avatars/%Y/%m/%d")
			.with_param("file_storage", "private_uploads")
			.with_param("max_length", "255")
			.with_param("storage", "external");
		metadata.add_field("avatar".to_string(), file_field);

		let model_state = metadata.to_model_state();
		let field_state = model_state
			.fields
			.get("avatar")
			.expect("file field should be present in migration state");

		assert_eq!(field_state.field_type, FieldType::VarChar(255));
		for (key, value) in [
			("model_field_type", "file"),
			("upload_to", "avatars/%Y/%m/%d"),
			("file_storage", "private_uploads"),
			("max_length", "255"),
		] {
			assert_eq!(
				field_state.params.get(key).map(String::as_str),
				Some(value),
				"migration state must preserve `{key}`"
			);
		}
		assert_eq!(
			field_state.params.get("storage").map(String::as_str),
			Some("external"),
			"PostgreSQL physical storage must remain separate from file_storage"
		);
	}

	#[test]
	fn test_to_model_state_resolves_qualified_fk_target_table() {
		// Arrange
		let registry = ModelRegistry::new();
		registry.register_model(ModelMetadata::new("blog", "Post", "articles"));

		let mut metadata = ModelMetadata::new("comments", "Comment", "comments");
		let foreign_key = crate::migrations::autodetector::ForeignKeyInfo {
			referenced_table: "post".to_string(),
			referenced_column: "id".to_string(),
			on_delete: crate::migrations::autodetector::ForeignKeyAction::Cascade,
			on_update: crate::migrations::autodetector::ForeignKeyAction::Cascade,
		};
		let field = FieldMetadata::new(FieldType::Uuid)
			.with_param("fk_target_app", "blog")
			.with_param("fk_target_model", "Post")
			.with_foreign_key(foreign_key);
		metadata.add_field("post".to_string(), field);

		// Act
		let model_state = metadata.to_model_state_with_registry(&registry);

		// Assert
		let field_state = model_state
			.fields
			.get("post")
			.expect("qualified FK field should be present");
		assert_eq!(
			field_state
				.foreign_key
				.as_ref()
				.expect("FK metadata should be preserved")
				.referenced_table,
			"articles"
		);
		let constraint = model_state
			.constraints
			.iter()
			.find(|constraint| constraint.constraint_type == "foreign_key")
			.expect("FK constraint should be generated");
		assert_eq!(
			constraint
				.foreign_key_info
				.as_ref()
				.expect("FK constraint metadata should be present")
				.referenced_table,
			"articles"
		);
	}

	#[test]
	fn test_to_model_state_uses_app_prefixed_fallback_for_unregistered_qualified_fk() {
		let registry = ModelRegistry::new();
		let mut metadata = ModelMetadata::new("comments", "Comment", "comments_comment");
		let foreign_key = crate::migrations::autodetector::ForeignKeyInfo {
			referenced_table: "user".to_string(),
			referenced_column: "id".to_string(),
			on_delete: crate::migrations::autodetector::ForeignKeyAction::Cascade,
			on_update: crate::migrations::autodetector::ForeignKeyAction::Cascade,
		};
		let field = FieldMetadata::new(FieldType::Uuid)
			.with_param("fk_target_app", "auth")
			.with_param("fk_target_model", "User")
			.with_foreign_key(foreign_key);
		metadata.add_field("user".to_string(), field);

		let model_state = metadata.to_model_state_with_registry(&registry);

		assert_eq!(
			model_state.fields["user"]
				.foreign_key
				.as_ref()
				.expect("foreign key metadata should be preserved")
				.referenced_table,
			"auth_user"
		);
	}

	#[test]
	fn test_model_metadata_to_model_state_preserves_generated_metadata() {
		let mut metadata = ModelMetadata::new("blog", "Post", "blog_post");
		let generated = GeneratedColumnDefinition::typed(
			SchemaExpr::col("title"),
			"SchemaExpr::col(\"title\")",
			GeneratedStorage::Stored,
		);
		let field = FieldMetadata::new(FieldType::VarChar(255)).with_generated(generated.clone());
		metadata.add_field("title_slug".to_string(), field);

		let model_state = metadata.to_model_state();

		let field_state = model_state
			.fields
			.get("title_slug")
			.expect("generated field should be present");
		assert_eq!(field_state.generated, Some(generated));
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
