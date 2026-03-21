//! ContentType synchronization between registry and database
//!
//! This module provides utilities for synchronizing content types between
//! in-memory registries and persistent storage.
//!
//! # Example
//!
//! ```rust
//! use reinhardt_db::contenttypes::sync::{ContentTypeSynchronizer, SyncMode};
//! use reinhardt_db::contenttypes::{ContentType, ContentTypeRegistry};
//!
//! let registry = ContentTypeRegistry::new();
//! registry.register(ContentType::new("blog", "article"));
//!
//! let synchronizer = ContentTypeSynchronizer::new();
//!
//! // Prepare sync plan (dry run)
//! let plan = synchronizer.plan_sync(&registry, &[]);
//! println!("Would create: {:?}", plan.to_create);
//! ```

use super::{ContentType, ContentTypeRegistry};
use std::collections::{HashMap, HashSet};

/// Synchronization mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SyncMode {
	/// Only add new content types (default)
	#[default]
	AddOnly,
	/// Add new and remove stale content types
	Full,
	/// Only remove stale content types
	RemoveOnly,
}

/// Error type for sync operations
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyncError {
	/// Database error
	DatabaseError(String),
	/// Conflict detected during sync
	ConflictError(String),
	/// Invalid state
	InvalidState(String),
}

impl std::fmt::Display for SyncError {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			Self::DatabaseError(msg) => write!(f, "Database error: {}", msg),
			Self::ConflictError(msg) => write!(f, "Conflict error: {}", msg),
			Self::InvalidState(msg) => write!(f, "Invalid state: {}", msg),
		}
	}
}

impl std::error::Error for SyncError {}

/// Represents a content type entry for sync comparison
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SyncEntry {
	/// The application label
	pub app_label: String,
	/// The model name
	pub model: String,
	/// Optional database ID (None if only in registry)
	pub db_id: Option<i64>,
}

impl SyncEntry {
	/// Creates a new sync entry
	#[must_use]
	pub fn new(app_label: impl Into<String>, model: impl Into<String>) -> Self {
		Self {
			app_label: app_label.into(),
			model: model.into(),
			db_id: None,
		}
	}

	/// Creates a sync entry with database ID
	#[must_use]
	pub fn with_db_id(app_label: impl Into<String>, model: impl Into<String>, db_id: i64) -> Self {
		Self {
			app_label: app_label.into(),
			model: model.into(),
			db_id: Some(db_id),
		}
	}

	/// Creates from a ContentType
	#[must_use]
	pub fn from_content_type(ct: &ContentType) -> Self {
		Self {
			app_label: ct.app_label.clone(),
			model: ct.model.clone(),
			db_id: ct.id,
		}
	}

	/// Returns the qualified name (app_label.model)
	#[must_use]
	pub fn qualified_name(&self) -> String {
		format!("{}.{}", self.app_label, self.model)
	}
}

/// Plan for synchronization operation
#[derive(Debug, Clone, Default)]
pub struct SyncPlan {
	/// Content types to create in database
	pub to_create: Vec<SyncEntry>,
	/// Content types to delete from database
	pub to_delete: Vec<SyncEntry>,
	/// Content types that are already in sync
	pub in_sync: Vec<SyncEntry>,
	/// Content types with conflicts (different IDs for same app_label.model)
	pub conflicts: Vec<(SyncEntry, SyncEntry)>,
}

impl SyncPlan {
	/// Creates a new empty sync plan
	#[must_use]
	pub fn new() -> Self {
		Self::default()
	}

	/// Returns true if there are any changes to make
	#[must_use]
	pub fn has_changes(&self) -> bool {
		!self.to_create.is_empty() || !self.to_delete.is_empty()
	}

	/// Returns true if there are conflicts
	#[must_use]
	pub fn has_conflicts(&self) -> bool {
		!self.conflicts.is_empty()
	}

	/// Returns the total number of operations
	#[must_use]
	pub fn operation_count(&self) -> usize {
		self.to_create.len() + self.to_delete.len()
	}

	/// Returns a summary of the plan
	#[must_use]
	pub fn summary(&self) -> String {
		format!(
			"Sync plan: {} to create, {} to delete, {} in sync, {} conflicts",
			self.to_create.len(),
			self.to_delete.len(),
			self.in_sync.len(),
			self.conflicts.len()
		)
	}
}

/// Result of a sync operation
#[derive(Debug, Clone, Default)]
pub struct SyncResult {
	/// Number of content types created
	pub created: usize,
	/// Number of content types updated
	pub updated: usize,
	/// Number of content types deleted
	pub deleted: usize,
	/// Number of entries unchanged
	pub unchanged: usize,
	/// Errors encountered during sync
	pub errors: Vec<String>,
}

impl SyncResult {
	/// Creates a new empty sync result
	#[must_use]
	pub fn new() -> Self {
		Self::default()
	}

	/// Returns true if any changes were made
	#[must_use]
	pub fn has_changes(&self) -> bool {
		self.created > 0 || self.updated > 0 || self.deleted > 0
	}

	/// Returns true if there were any errors
	#[must_use]
	pub fn has_errors(&self) -> bool {
		!self.errors.is_empty()
	}

	/// Returns the total number of operations performed
	#[must_use]
	pub fn total_operations(&self) -> usize {
		self.created + self.updated + self.deleted
	}

	/// Returns a summary of the result
	#[must_use]
	pub fn summary(&self) -> String {
		format!(
			"Sync result: {} created, {} updated, {} deleted, {} unchanged, {} errors",
			self.created,
			self.updated,
			self.deleted,
			self.unchanged,
			self.errors.len()
		)
	}
}

/// Options for sync operation
#[non_exhaustive]
#[derive(Debug, Clone, Default)]
pub struct SyncOptions {
	/// Sync mode
	pub mode: SyncMode,
	/// Whether to perform a dry run (no actual changes)
	pub dry_run: bool,
	/// Optional filter for app_label
	pub filter_app_label: Option<String>,
	/// Whether to delete stale entries
	pub delete_stale: bool,
}

impl SyncOptions {
	/// Creates default sync options
	#[must_use]
	pub fn new() -> Self {
		Self::default()
	}

	/// Sets the sync mode
	#[must_use]
	pub fn mode(mut self, mode: SyncMode) -> Self {
		self.mode = mode;
		self
	}

	/// Sets dry run mode
	#[must_use]
	pub fn dry_run(mut self, dry_run: bool) -> Self {
		self.dry_run = dry_run;
		self
	}

	/// Sets app_label filter
	#[must_use]
	pub fn filter_app_label(mut self, app_label: impl Into<String>) -> Self {
		self.filter_app_label = Some(app_label.into());
		self
	}

	/// Sets delete_stale option
	#[must_use]
	pub fn delete_stale(mut self, delete: bool) -> Self {
		self.delete_stale = delete;
		self
	}
}

/// Handles synchronization of content types
#[derive(Debug, Clone, Default)]
pub struct ContentTypeSynchronizer {
	/// Sync options
	options: SyncOptions,
}

impl ContentTypeSynchronizer {
	/// Creates a new synchronizer with default options
	#[must_use]
	pub fn new() -> Self {
		Self::default()
	}

	/// Creates a new synchronizer with specified options
	#[must_use]
	pub fn with_options(options: SyncOptions) -> Self {
		Self { options }
	}

	/// Plans a sync operation without executing it
	///
	/// Compares the registry with database entries and returns a plan
	/// describing what operations would be performed.
	#[must_use]
	pub fn plan_sync(&self, registry: &ContentTypeRegistry, db_entries: &[SyncEntry]) -> SyncPlan {
		let mut plan = SyncPlan::new();

		// Build sets for comparison
		let registry_set: HashMap<String, SyncEntry> = registry
			.all()
			.into_iter()
			.filter(|ct| {
				if let Some(ref filter) = self.options.filter_app_label {
					&ct.app_label == filter
				} else {
					true
				}
			})
			.map(|ct| {
				(
					format!("{}.{}", ct.app_label, ct.model),
					SyncEntry::from_content_type(&ct),
				)
			})
			.collect();

		let db_set: HashMap<String, SyncEntry> = db_entries
			.iter()
			.filter(|entry| {
				if let Some(ref filter) = self.options.filter_app_label {
					&entry.app_label == filter
				} else {
					true
				}
			})
			.map(|entry| (entry.qualified_name(), entry.clone()))
			.collect();

		// Find entries to create (in registry but not in DB)
		for (key, entry) in &registry_set {
			if !db_set.contains_key(key) {
				plan.to_create.push(entry.clone());
			} else {
				// Check for conflicts (same key but different IDs)
				let db_entry = db_set.get(key).unwrap();
				if entry.db_id != db_entry.db_id {
					plan.conflicts.push((entry.clone(), db_entry.clone()));
				} else {
					plan.in_sync.push(entry.clone());
				}
			}
		}

		// Find entries to delete (in DB but not in registry)
		if self.options.delete_stale
			|| self.options.mode == SyncMode::Full
			|| self.options.mode == SyncMode::RemoveOnly
		{
			for (key, entry) in &db_set {
				if !registry_set.contains_key(key) {
					plan.to_delete.push(entry.clone());
				}
			}
		}

		plan
	}

	/// Synchronizes registry with provided database entries
	///
	/// This method applies the sync based on the options set.
	pub fn sync(&self, registry: &ContentTypeRegistry, db_entries: &[SyncEntry]) -> SyncResult {
		let plan = self.plan_sync(registry, db_entries);
		self.execute_plan(registry, &plan)
	}

	/// Executes a sync plan
	pub fn execute_plan(&self, registry: &ContentTypeRegistry, plan: &SyncPlan) -> SyncResult {
		let mut result = SyncResult::new();

		if self.options.dry_run {
			// In dry run mode, just report what would happen
			result.created = plan.to_create.len();
			result.deleted = plan.to_delete.len();
			result.unchanged = plan.in_sync.len();
			return result;
		}

		// Process creates (add to registry from plan)
		match self.options.mode {
			SyncMode::AddOnly | SyncMode::Full => {
				for _entry in &plan.to_create {
					// Entry is already in registry based on plan creation
					// In a real scenario, this would write to DB
					result.created += 1;
				}
			}
			SyncMode::RemoveOnly => {}
		}

		// Process deletes
		match self.options.mode {
			SyncMode::RemoveOnly | SyncMode::Full => {
				for entry in &plan.to_delete {
					// In-memory removal
					let all: Vec<ContentType> = registry
						.all()
						.into_iter()
						.filter(|ct| !(ct.app_label == entry.app_label && ct.model == entry.model))
						.collect();

					registry.clear();
					for ct in all {
						registry.register(ct);
					}
					result.deleted += 1;
				}
			}
			SyncMode::AddOnly => {}
		}

		result.unchanged = plan.in_sync.len();

		// Report conflicts as errors
		for (registry_entry, db_entry) in &plan.conflicts {
			result.errors.push(format!(
				"Conflict for {}: registry ID {:?} vs DB ID {:?}",
				registry_entry.qualified_name(),
				registry_entry.db_id,
				db_entry.db_id
			));
		}

		result
	}

	/// Compares two registries and returns differences
	#[must_use]
	pub fn compare_registries(
		&self,
		source: &ContentTypeRegistry,
		target: &ContentTypeRegistry,
	) -> SyncPlan {
		let target_entries: Vec<SyncEntry> = target
			.all()
			.into_iter()
			.map(|ct| SyncEntry::from_content_type(&ct))
			.collect();

		self.plan_sync(source, &target_entries)
	}

	/// Identifies stale entries that exist in DB but not in registry
	#[must_use]
	pub fn find_stale(
		&self,
		registry: &ContentTypeRegistry,
		db_entries: &[SyncEntry],
	) -> Vec<SyncEntry> {
		let registry_keys: HashSet<String> = registry
			.all()
			.into_iter()
			.map(|ct| format!("{}.{}", ct.app_label, ct.model))
			.collect();

		db_entries
			.iter()
			.filter(|entry| !registry_keys.contains(&entry.qualified_name()))
			.cloned()
			.collect()
	}

	/// Identifies missing entries that exist in registry but not in DB
	#[must_use]
	pub fn find_missing(
		&self,
		registry: &ContentTypeRegistry,
		db_entries: &[SyncEntry],
	) -> Vec<SyncEntry> {
		let db_keys: HashSet<String> = db_entries.iter().map(|e| e.qualified_name()).collect();

		registry
			.all()
			.into_iter()
			.filter(|ct| !db_keys.contains(&format!("{}.{}", ct.app_label, ct.model)))
			.map(|ct| SyncEntry::from_content_type(&ct))
			.collect()
	}
}

/// Convenience function to create a sync plan
#[must_use]
pub fn plan_sync(registry: &ContentTypeRegistry, db_entries: &[SyncEntry]) -> SyncPlan {
	ContentTypeSynchronizer::new().plan_sync(registry, db_entries)
}

/// Convenience function to sync with default options
pub fn sync(registry: &ContentTypeRegistry, db_entries: &[SyncEntry]) -> SyncResult {
	ContentTypeSynchronizer::new().sync(registry, db_entries)
}

/// Convenience function to find stale entries
#[must_use]
pub fn find_stale(registry: &ContentTypeRegistry, db_entries: &[SyncEntry]) -> Vec<SyncEntry> {
	ContentTypeSynchronizer::new().find_stale(registry, db_entries)
}

/// Convenience function to find missing entries
#[must_use]
pub fn find_missing(registry: &ContentTypeRegistry, db_entries: &[SyncEntry]) -> Vec<SyncEntry> {
	ContentTypeSynchronizer::new().find_missing(registry, db_entries)
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_sync_entry_new() {
		let entry = SyncEntry::new("blog", "article");
		assert_eq!(entry.app_label, "blog");
		assert_eq!(entry.model, "article");
		assert!(entry.db_id.is_none());
	}

	#[test]
	fn test_sync_entry_with_db_id() {
		let entry = SyncEntry::with_db_id("blog", "article", 42);
		assert_eq!(entry.db_id, Some(42));
	}

	#[test]
	fn test_sync_entry_from_content_type() {
		// Unregistered ContentType has no ID
		let ct = ContentType::new("auth", "user");
		let entry = SyncEntry::from_content_type(&ct);
		assert_eq!(entry.app_label, "auth");
		assert_eq!(entry.model, "user");
		assert!(entry.db_id.is_none()); // ID is None until registered
	}

	#[test]
	fn test_sync_entry_from_registered_content_type() {
		// Registered ContentType has an ID
		let registry = ContentTypeRegistry::new();
		let ct = registry.register(ContentType::new("auth", "user"));
		let entry = SyncEntry::from_content_type(&ct);
		assert_eq!(entry.app_label, "auth");
		assert_eq!(entry.model, "user");
		assert!(entry.db_id.is_some()); // ID is assigned after registration
	}

	#[test]
	fn test_sync_entry_qualified_name() {
		let entry = SyncEntry::new("blog", "article");
		assert_eq!(entry.qualified_name(), "blog.article");
	}

	#[test]
	fn test_sync_plan_empty() {
		let plan = SyncPlan::new();
		assert!(!plan.has_changes());
		assert!(!plan.has_conflicts());
		assert_eq!(plan.operation_count(), 0);
	}

	#[test]
	fn test_sync_plan_with_changes() {
		let mut plan = SyncPlan::new();
		plan.to_create.push(SyncEntry::new("blog", "article"));
		plan.to_delete.push(SyncEntry::new("auth", "user"));

		assert!(plan.has_changes());
		assert_eq!(plan.operation_count(), 2);
	}

	#[test]
	fn test_sync_plan_summary() {
		let mut plan = SyncPlan::new();
		plan.to_create.push(SyncEntry::new("blog", "article"));
		plan.in_sync.push(SyncEntry::new("auth", "user"));

		let summary = plan.summary();
		assert!(summary.contains("1 to create"));
		assert!(summary.contains("1 in sync"));
	}

	#[test]
	fn test_sync_result_empty() {
		let result = SyncResult::new();
		assert!(!result.has_changes());
		assert!(!result.has_errors());
		assert_eq!(result.total_operations(), 0);
	}

	#[test]
	fn test_sync_result_with_changes() {
		let mut result = SyncResult::new();
		result.created = 2;
		result.deleted = 1;

		assert!(result.has_changes());
		assert_eq!(result.total_operations(), 3);
	}

	#[test]
	fn test_sync_options_builder() {
		let options = SyncOptions::new()
			.mode(SyncMode::Full)
			.dry_run(true)
			.filter_app_label("blog")
			.delete_stale(true);

		assert_eq!(options.mode, SyncMode::Full);
		assert!(options.dry_run);
		assert_eq!(options.filter_app_label, Some("blog".to_string()));
		assert!(options.delete_stale);
	}

	#[test]
	fn test_plan_sync_creates() {
		let registry = ContentTypeRegistry::new();
		registry.register(ContentType::new("blog", "article"));
		registry.register(ContentType::new("auth", "user"));

		let synchronizer = ContentTypeSynchronizer::new();
		let plan = synchronizer.plan_sync(&registry, &[]);

		assert_eq!(plan.to_create.len(), 2);
		assert!(plan.to_delete.is_empty());
	}

	#[test]
	fn test_plan_sync_deletes() {
		let registry = ContentTypeRegistry::new();
		registry.register(ContentType::new("blog", "article"));

		let db_entries = vec![
			SyncEntry::new("blog", "article"),
			SyncEntry::new("auth", "user"),
		];

		let synchronizer =
			ContentTypeSynchronizer::with_options(SyncOptions::new().delete_stale(true));
		let plan = synchronizer.plan_sync(&registry, &db_entries);

		assert!(plan.to_create.is_empty());
		assert_eq!(plan.to_delete.len(), 1);
		assert_eq!(plan.to_delete[0].qualified_name(), "auth.user");
	}

	#[test]
	fn test_plan_sync_in_sync() {
		let registry = ContentTypeRegistry::new();
		// Register first, then get the assigned ID
		let registered_ct = registry.register(ContentType::new("blog", "article"));
		let ct_id = registered_ct.id.unwrap();

		let db_entries = vec![SyncEntry::with_db_id("blog", "article", ct_id)];

		let synchronizer = ContentTypeSynchronizer::new();
		let plan = synchronizer.plan_sync(&registry, &db_entries);

		assert!(plan.to_create.is_empty());
		assert!(plan.to_delete.is_empty());
		assert_eq!(plan.in_sync.len(), 1);
	}

	#[test]
	fn test_plan_sync_with_filter() {
		let registry = ContentTypeRegistry::new();
		registry.register(ContentType::new("blog", "article"));
		registry.register(ContentType::new("auth", "user"));

		let synchronizer =
			ContentTypeSynchronizer::with_options(SyncOptions::new().filter_app_label("blog"));
		let plan = synchronizer.plan_sync(&registry, &[]);

		assert_eq!(plan.to_create.len(), 1);
		assert_eq!(plan.to_create[0].qualified_name(), "blog.article");
	}

	#[test]
	fn test_sync_dry_run() {
		let registry = ContentTypeRegistry::new();
		registry.register(ContentType::new("blog", "article"));

		let synchronizer = ContentTypeSynchronizer::with_options(SyncOptions::new().dry_run(true));
		let result = synchronizer.sync(&registry, &[]);

		assert_eq!(result.created, 1);
		// Registry should be unchanged in dry run
		assert!(registry.get("blog", "article").is_some());
	}

	#[test]
	fn test_sync_add_only() {
		let registry = ContentTypeRegistry::new();
		registry.register(ContentType::new("blog", "article"));

		let db_entries = vec![SyncEntry::new("auth", "user")];

		let synchronizer =
			ContentTypeSynchronizer::with_options(SyncOptions::new().mode(SyncMode::AddOnly));
		let result = synchronizer.sync(&registry, &db_entries);

		assert_eq!(result.created, 1);
		assert_eq!(result.deleted, 0); // AddOnly doesn't delete
	}

	#[test]
	fn test_sync_remove_only() {
		let registry = ContentTypeRegistry::new();
		registry.register(ContentType::new("blog", "article"));

		let db_entries = vec![
			SyncEntry::new("blog", "article"),
			SyncEntry::new("auth", "user"),
		];

		let synchronizer =
			ContentTypeSynchronizer::with_options(SyncOptions::new().mode(SyncMode::RemoveOnly));
		let result = synchronizer.sync(&registry, &db_entries);

		assert_eq!(result.created, 0);
		assert_eq!(result.deleted, 1);
	}

	#[test]
	fn test_sync_full() {
		let registry = ContentTypeRegistry::new();
		registry.register(ContentType::new("blog", "article"));
		registry.register(ContentType::new("blog", "comment"));

		let db_entries = vec![SyncEntry::new("auth", "user")];

		let synchronizer =
			ContentTypeSynchronizer::with_options(SyncOptions::new().mode(SyncMode::Full));
		let result = synchronizer.sync(&registry, &db_entries);

		assert_eq!(result.created, 2);
		assert_eq!(result.deleted, 1);
	}

	#[test]
	fn test_find_stale() {
		let registry = ContentTypeRegistry::new();
		registry.register(ContentType::new("blog", "article"));

		let db_entries = vec![
			SyncEntry::new("blog", "article"),
			SyncEntry::new("auth", "user"),
			SyncEntry::new("auth", "group"),
		];

		let stale = find_stale(&registry, &db_entries);

		assert_eq!(stale.len(), 2);
		assert!(stale.iter().any(|e| e.qualified_name() == "auth.user"));
		assert!(stale.iter().any(|e| e.qualified_name() == "auth.group"));
	}

	#[test]
	fn test_find_missing() {
		let registry = ContentTypeRegistry::new();
		registry.register(ContentType::new("blog", "article"));
		registry.register(ContentType::new("blog", "comment"));
		registry.register(ContentType::new("auth", "user"));

		let db_entries = vec![SyncEntry::new("blog", "article")];

		let missing = find_missing(&registry, &db_entries);

		assert_eq!(missing.len(), 2);
	}

	#[test]
	fn test_compare_registries() {
		let source = ContentTypeRegistry::new();
		source.register(ContentType::new("blog", "article"));
		source.register(ContentType::new("blog", "comment"));

		let target = ContentTypeRegistry::new();
		target.register(ContentType::new("blog", "article"));
		target.register(ContentType::new("auth", "user"));

		let synchronizer = ContentTypeSynchronizer::with_options(
			SyncOptions::new().mode(SyncMode::Full).delete_stale(true),
		);
		let plan = synchronizer.compare_registries(&source, &target);

		assert_eq!(plan.to_create.len(), 1); // blog.comment
		assert_eq!(plan.to_delete.len(), 1); // auth.user
	}

	#[test]
	fn test_sync_error_display() {
		let db_error = SyncError::DatabaseError("connection failed".to_string());
		assert!(db_error.to_string().contains("Database error"));

		let conflict_error = SyncError::ConflictError("ID mismatch".to_string());
		assert!(conflict_error.to_string().contains("Conflict error"));

		let invalid_error = SyncError::InvalidState("bad state".to_string());
		assert!(invalid_error.to_string().contains("Invalid state"));
	}

	#[test]
	fn test_convenience_functions() {
		let registry = ContentTypeRegistry::new();
		registry.register(ContentType::new("blog", "article"));

		let db_entries = vec![SyncEntry::new("auth", "user")];

		// plan_sync
		let plan = plan_sync(&registry, &db_entries);
		assert!(plan.has_changes());

		// sync
		let result = sync(&registry, &[]);
		assert!(!result.has_errors());

		// find_stale
		let stale = find_stale(&registry, &db_entries);
		assert_eq!(stale.len(), 1);

		// find_missing
		let missing = find_missing(&registry, &db_entries);
		assert_eq!(missing.len(), 1);
	}
}
