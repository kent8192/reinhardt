//! ContentType migration support
//!
//! This module provides utilities for migrating content types when apps or models
//! are renamed, similar to Django's ContentType migration support.
//!
//! # Example
//!
//! ```rust
//! use reinhardt_db::contenttypes::migration::{ContentTypeMigration, MigrationRecord};
//! use reinhardt_db::contenttypes::{ContentType, ContentTypeRegistry};
//!
//! let registry = ContentTypeRegistry::new();
//! registry.register(ContentType::new("old_app", "old_model"));
//!
//! let mut migration = ContentTypeMigration::new();
//!
//! // Rename an app
//! let result = migration.rename_app(&registry, "old_app", "new_app");
//! assert!(result.is_ok());
//!
//! // Verify the change
//! assert!(registry.get("new_app", "old_model").is_some());
//! assert!(registry.get("old_app", "old_model").is_none());
//! ```

use super::{ContentType, ContentTypeRegistry};
use std::collections::HashMap;

/// Error type for migration operations
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MigrationError {
	/// The source content type was not found
	SourceNotFound { app_label: String, model: String },
	/// The target content type already exists
	TargetExists { app_label: String, model: String },
	/// Invalid migration parameters
	InvalidParameters(String),
}

impl std::fmt::Display for MigrationError {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			Self::SourceNotFound { app_label, model } => {
				write!(f, "Source content type not found: {}.{}", app_label, model)
			}
			Self::TargetExists { app_label, model } => {
				write!(
					f,
					"Target content type already exists: {}.{}",
					app_label, model
				)
			}
			Self::InvalidParameters(msg) => write!(f, "Invalid migration parameters: {}", msg),
		}
	}
}

impl std::error::Error for MigrationError {}

/// A record of a content type migration
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MigrationRecord {
	/// The original app_label
	pub old_app_label: String,
	/// The original model name
	pub old_model: String,
	/// The new app_label
	pub new_app_label: String,
	/// The new model name
	pub new_model: String,
	/// Timestamp of the migration (Unix timestamp)
	pub timestamp: u64,
}

impl MigrationRecord {
	/// Creates a new migration record
	#[must_use]
	pub fn new(
		old_app_label: impl Into<String>,
		old_model: impl Into<String>,
		new_app_label: impl Into<String>,
		new_model: impl Into<String>,
	) -> Self {
		Self {
			old_app_label: old_app_label.into(),
			old_model: old_model.into(),
			new_app_label: new_app_label.into(),
			new_model: new_model.into(),
			timestamp: std::time::SystemTime::now()
				.duration_since(std::time::UNIX_EPOCH)
				.map(|d| d.as_secs())
				.unwrap_or(0),
		}
	}

	/// Returns the old qualified name (app_label.model)
	#[must_use]
	pub fn old_qualified_name(&self) -> String {
		format!("{}.{}", self.old_app_label, self.old_model)
	}

	/// Returns the new qualified name (app_label.model)
	#[must_use]
	pub fn new_qualified_name(&self) -> String {
		format!("{}.{}", self.new_app_label, self.new_model)
	}

	/// Checks if this migration involves an app rename
	#[must_use]
	pub fn is_app_rename(&self) -> bool {
		self.old_app_label != self.new_app_label
	}

	/// Checks if this migration involves a model rename
	#[must_use]
	pub fn is_model_rename(&self) -> bool {
		self.old_model != self.new_model
	}
}

/// Result of a migration operation
#[derive(Debug, Clone)]
pub struct MigrationResult {
	/// Records of all migrations performed
	pub records: Vec<MigrationRecord>,
	/// Number of content types migrated
	pub migrated_count: usize,
	/// Any warnings generated during migration
	pub warnings: Vec<String>,
}

impl MigrationResult {
	/// Creates a new empty migration result
	#[must_use]
	pub fn new() -> Self {
		Self {
			records: Vec::new(),
			migrated_count: 0,
			warnings: Vec::new(),
		}
	}

	/// Returns true if any content types were migrated
	#[must_use]
	pub fn has_migrations(&self) -> bool {
		self.migrated_count > 0
	}

	/// Returns true if there are warnings
	#[must_use]
	pub fn has_warnings(&self) -> bool {
		!self.warnings.is_empty()
	}
}

impl Default for MigrationResult {
	fn default() -> Self {
		Self::new()
	}
}

/// Manages content type migrations
///
/// This manager handles renaming of apps and models in the content type registry,
/// keeping track of all migrations performed.
#[derive(Debug, Default)]
pub struct ContentTypeMigration {
	/// History of all migrations performed
	history: Vec<MigrationRecord>,
	/// Mapping of old qualified names to new qualified names for reverse lookups
	rename_map: HashMap<String, String>,
}

impl ContentTypeMigration {
	/// Creates a new migration manager
	#[must_use]
	pub fn new() -> Self {
		Self::default()
	}

	/// Returns the migration history
	#[must_use]
	pub fn history(&self) -> &[MigrationRecord] {
		&self.history
	}

	/// Clears the migration history
	pub fn clear_history(&mut self) {
		self.history.clear();
		self.rename_map.clear();
	}

	/// Renames a model within the same app
	///
	/// # Errors
	///
	/// Returns an error if the source model doesn't exist or the target already exists.
	pub fn rename_model(
		&mut self,
		registry: &ContentTypeRegistry,
		app_label: &str,
		old_model: &str,
		new_model: &str,
	) -> Result<MigrationRecord, MigrationError> {
		self.rename(registry, app_label, old_model, app_label, new_model)
	}

	/// Renames an entire app (moves all models to a new app_label)
	///
	/// # Errors
	///
	/// Returns an error if no content types exist for the app or if any target already exists.
	pub fn rename_app(
		&mut self,
		registry: &ContentTypeRegistry,
		old_app_label: &str,
		new_app_label: &str,
	) -> Result<MigrationResult, MigrationError> {
		if old_app_label == new_app_label {
			return Err(MigrationError::InvalidParameters(
				"Old and new app labels are the same".to_string(),
			));
		}

		// Find all content types for the old app
		let types_to_migrate: Vec<ContentType> = registry
			.all()
			.into_iter()
			.filter(|ct| ct.app_label == old_app_label)
			.collect();

		if types_to_migrate.is_empty() {
			return Err(MigrationError::SourceNotFound {
				app_label: old_app_label.to_string(),
				model: "*".to_string(),
			});
		}

		// Check if any target already exists
		for ct in &types_to_migrate {
			if registry.get(new_app_label, &ct.model).is_some() {
				return Err(MigrationError::TargetExists {
					app_label: new_app_label.to_string(),
					model: ct.model.clone(),
				});
			}
		}

		let mut result = MigrationResult::new();

		// Perform the migrations
		for ct in types_to_migrate {
			let record =
				self.rename(registry, &ct.app_label, &ct.model, new_app_label, &ct.model)?;
			result.records.push(record);
			result.migrated_count += 1;
		}

		Ok(result)
	}

	/// Moves a model from one app to another
	///
	/// # Errors
	///
	/// Returns an error if the source doesn't exist or the target already exists.
	pub fn move_model(
		&mut self,
		registry: &ContentTypeRegistry,
		old_app_label: &str,
		model: &str,
		new_app_label: &str,
	) -> Result<MigrationRecord, MigrationError> {
		self.rename(registry, old_app_label, model, new_app_label, model)
	}

	/// Performs a full rename (app_label and/or model)
	///
	/// # Errors
	///
	/// Returns an error if the source doesn't exist or the target already exists.
	pub fn rename(
		&mut self,
		registry: &ContentTypeRegistry,
		old_app_label: &str,
		old_model: &str,
		new_app_label: &str,
		new_model: &str,
	) -> Result<MigrationRecord, MigrationError> {
		// Validate input
		if old_app_label == new_app_label && old_model == new_model {
			return Err(MigrationError::InvalidParameters(
				"Source and target are the same".to_string(),
			));
		}

		// Check source exists
		if registry.get(old_app_label, old_model).is_none() {
			return Err(MigrationError::SourceNotFound {
				app_label: old_app_label.to_string(),
				model: old_model.to_string(),
			});
		}

		// Check target doesn't exist
		if registry.get(new_app_label, new_model).is_some() {
			return Err(MigrationError::TargetExists {
				app_label: new_app_label.to_string(),
				model: new_model.to_string(),
			});
		}

		// Get all types except the one being renamed
		let remaining: Vec<ContentType> = registry
			.all()
			.into_iter()
			.filter(|ct| !(ct.app_label == old_app_label && ct.model == old_model))
			.collect();

		// Clear and re-register with the renamed type
		registry.clear();

		for ct in remaining {
			registry.register(ct);
		}

		// Register the renamed type
		registry.register(ContentType::new(new_app_label, new_model));

		// Create and store the migration record
		let record = MigrationRecord::new(old_app_label, old_model, new_app_label, new_model);

		let old_key = record.old_qualified_name();
		let new_key = record.new_qualified_name();
		self.rename_map.insert(old_key, new_key);
		self.history.push(record.clone());

		Ok(record)
	}

	/// Looks up the current qualified name for an old qualified name
	///
	/// This follows the chain of renames to find the final name.
	#[must_use]
	pub fn resolve_old_name(&self, old_qualified_name: &str) -> Option<String> {
		let mut current = old_qualified_name.to_string();

		// Follow the chain of renames (with cycle detection)
		let mut seen = std::collections::HashSet::new();
		while let Some(new_name) = self.rename_map.get(&current) {
			if !seen.insert(current.clone()) {
				// Cycle detected
				break;
			}
			current = new_name.clone();
		}

		if current == old_qualified_name && !self.rename_map.contains_key(&current) {
			None
		} else {
			Some(current)
		}
	}

	/// Exports the migration history as a list of records
	#[must_use]
	pub fn export_history(&self) -> Vec<MigrationRecord> {
		self.history.clone()
	}

	/// Imports migration history from a list of records
	pub fn import_history(&mut self, records: Vec<MigrationRecord>) {
		for record in records {
			let old_key = record.old_qualified_name();
			let new_key = record.new_qualified_name();
			self.rename_map.insert(old_key, new_key);
			self.history.push(record);
		}
	}
}

/// Helper function to create a model rename migration
pub fn create_model_rename(
	registry: &ContentTypeRegistry,
	app_label: &str,
	old_model: &str,
	new_model: &str,
) -> Result<MigrationRecord, MigrationError> {
	let mut migration = ContentTypeMigration::new();
	migration.rename_model(registry, app_label, old_model, new_model)
}

/// Helper function to create an app rename migration
pub fn create_app_rename(
	registry: &ContentTypeRegistry,
	old_app_label: &str,
	new_app_label: &str,
) -> Result<MigrationResult, MigrationError> {
	let mut migration = ContentTypeMigration::new();
	migration.rename_app(registry, old_app_label, new_app_label)
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_migration_record_creation() {
		let record = MigrationRecord::new("old_app", "old_model", "new_app", "new_model");

		assert_eq!(record.old_app_label, "old_app");
		assert_eq!(record.old_model, "old_model");
		assert_eq!(record.new_app_label, "new_app");
		assert_eq!(record.new_model, "new_model");
		assert!(record.timestamp > 0);
	}

	#[test]
	fn test_migration_record_qualified_names() {
		let record = MigrationRecord::new("blog", "article", "posts", "post");

		assert_eq!(record.old_qualified_name(), "blog.article");
		assert_eq!(record.new_qualified_name(), "posts.post");
	}

	#[test]
	fn test_migration_record_rename_checks() {
		let app_rename = MigrationRecord::new("old_app", "model", "new_app", "model");
		assert!(app_rename.is_app_rename());
		assert!(!app_rename.is_model_rename());

		let model_rename = MigrationRecord::new("app", "old_model", "app", "new_model");
		assert!(!model_rename.is_app_rename());
		assert!(model_rename.is_model_rename());

		let both_rename = MigrationRecord::new("old_app", "old_model", "new_app", "new_model");
		assert!(both_rename.is_app_rename());
		assert!(both_rename.is_model_rename());
	}

	#[test]
	fn test_rename_model() {
		let registry = ContentTypeRegistry::new();
		registry.register(ContentType::new("blog", "old_article"));

		let mut migration = ContentTypeMigration::new();
		let record = migration
			.rename_model(&registry, "blog", "old_article", "new_article")
			.unwrap();

		assert_eq!(record.old_model, "old_article");
		assert_eq!(record.new_model, "new_article");
		assert!(registry.get("blog", "new_article").is_some());
		assert!(registry.get("blog", "old_article").is_none());
	}

	#[test]
	fn test_rename_app() {
		let registry = ContentTypeRegistry::new();
		registry.register(ContentType::new("old_app", "model1"));
		registry.register(ContentType::new("old_app", "model2"));

		let mut migration = ContentTypeMigration::new();
		let result = migration
			.rename_app(&registry, "old_app", "new_app")
			.unwrap();

		assert_eq!(result.migrated_count, 2);
		assert!(registry.get("new_app", "model1").is_some());
		assert!(registry.get("new_app", "model2").is_some());
		assert!(registry.get("old_app", "model1").is_none());
		assert!(registry.get("old_app", "model2").is_none());
	}

	#[test]
	fn test_move_model() {
		let registry = ContentTypeRegistry::new();
		registry.register(ContentType::new("app1", "model"));

		let mut migration = ContentTypeMigration::new();
		let record = migration
			.move_model(&registry, "app1", "model", "app2")
			.unwrap();

		assert_eq!(record.old_app_label, "app1");
		assert_eq!(record.new_app_label, "app2");
		assert!(registry.get("app2", "model").is_some());
		assert!(registry.get("app1", "model").is_none());
	}

	#[test]
	fn test_rename_source_not_found() {
		let registry = ContentTypeRegistry::new();

		let mut migration = ContentTypeMigration::new();
		let result = migration.rename_model(&registry, "blog", "nonexistent", "new_name");

		assert!(matches!(result, Err(MigrationError::SourceNotFound { .. })));
	}

	#[test]
	fn test_rename_target_exists() {
		let registry = ContentTypeRegistry::new();
		registry.register(ContentType::new("blog", "article"));
		registry.register(ContentType::new("blog", "existing"));

		let mut migration = ContentTypeMigration::new();
		let result = migration.rename_model(&registry, "blog", "article", "existing");

		assert!(matches!(result, Err(MigrationError::TargetExists { .. })));
	}

	#[test]
	fn test_rename_same_source_and_target() {
		let registry = ContentTypeRegistry::new();
		registry.register(ContentType::new("blog", "article"));

		let mut migration = ContentTypeMigration::new();
		let result = migration.rename_model(&registry, "blog", "article", "article");

		assert!(matches!(result, Err(MigrationError::InvalidParameters(_))));
	}

	#[test]
	fn test_migration_history() {
		let registry = ContentTypeRegistry::new();
		registry.register(ContentType::new("app", "model1"));
		registry.register(ContentType::new("app", "model2"));

		let mut migration = ContentTypeMigration::new();
		migration
			.rename_model(&registry, "app", "model1", "new_model1")
			.unwrap();
		migration
			.rename_model(&registry, "app", "model2", "new_model2")
			.unwrap();

		let history = migration.history();
		assert_eq!(history.len(), 2);
	}

	#[test]
	fn test_clear_history() {
		let registry = ContentTypeRegistry::new();
		registry.register(ContentType::new("app", "model"));

		let mut migration = ContentTypeMigration::new();
		migration
			.rename_model(&registry, "app", "model", "new_model")
			.unwrap();

		assert_eq!(migration.history().len(), 1);

		migration.clear_history();
		assert_eq!(migration.history().len(), 0);
	}

	#[test]
	fn test_resolve_old_name() {
		let registry = ContentTypeRegistry::new();
		registry.register(ContentType::new("app", "model"));

		let mut migration = ContentTypeMigration::new();
		migration
			.rename_model(&registry, "app", "model", "renamed_model")
			.unwrap();

		let resolved = migration.resolve_old_name("app.model");
		assert_eq!(resolved, Some("app.renamed_model".to_string()));

		let not_found = migration.resolve_old_name("nonexistent.model");
		assert!(not_found.is_none());
	}

	#[test]
	fn test_resolve_chained_renames() {
		let registry = ContentTypeRegistry::new();
		registry.register(ContentType::new("app", "model"));

		let mut migration = ContentTypeMigration::new();

		// First rename: model -> model2
		migration
			.rename_model(&registry, "app", "model", "model2")
			.unwrap();

		// Second rename: model2 -> model3
		migration
			.rename_model(&registry, "app", "model2", "model3")
			.unwrap();

		// Should resolve the original name to the final name
		let resolved = migration.resolve_old_name("app.model");
		assert_eq!(resolved, Some("app.model3".to_string()));
	}

	#[test]
	fn test_export_import_history() {
		let registry1 = ContentTypeRegistry::new();
		registry1.register(ContentType::new("app", "model"));

		let mut migration1 = ContentTypeMigration::new();
		migration1
			.rename_model(&registry1, "app", "model", "new_model")
			.unwrap();

		let exported = migration1.export_history();

		let mut migration2 = ContentTypeMigration::new();
		migration2.import_history(exported);

		assert_eq!(migration2.history().len(), 1);
		assert_eq!(
			migration2.resolve_old_name("app.model"),
			Some("app.new_model".to_string())
		);
	}

	#[test]
	fn test_migration_result() {
		let mut result = MigrationResult::new();

		assert!(!result.has_migrations());
		assert!(!result.has_warnings());

		result.migrated_count = 2;
		result.warnings.push("Warning 1".to_string());

		assert!(result.has_migrations());
		assert!(result.has_warnings());
	}

	#[test]
	fn test_helper_create_model_rename() {
		let registry = ContentTypeRegistry::new();
		registry.register(ContentType::new("blog", "article"));

		let record = create_model_rename(&registry, "blog", "article", "post").unwrap();

		assert_eq!(record.old_model, "article");
		assert_eq!(record.new_model, "post");
	}

	#[test]
	fn test_helper_create_app_rename() {
		let registry = ContentTypeRegistry::new();
		registry.register(ContentType::new("old_blog", "article"));

		let result = create_app_rename(&registry, "old_blog", "blog").unwrap();

		assert_eq!(result.migrated_count, 1);
		assert!(registry.get("blog", "article").is_some());
	}

	#[test]
	fn test_migration_error_display() {
		let source_not_found = MigrationError::SourceNotFound {
			app_label: "app".to_string(),
			model: "model".to_string(),
		};
		assert!(source_not_found.to_string().contains("not found"));

		let target_exists = MigrationError::TargetExists {
			app_label: "app".to_string(),
			model: "model".to_string(),
		};
		assert!(target_exists.to_string().contains("already exists"));

		let invalid = MigrationError::InvalidParameters("test".to_string());
		assert!(invalid.to_string().contains("Invalid"));
	}
}
