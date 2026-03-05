//! ContentType inspection utilities
//!
//! This module provides utilities for inspecting and querying content types,
//! useful for debugging, admin interfaces, and management commands.
//!
//! # Example
//!
//! ```rust
//! use reinhardt_db::contenttypes::inspect::{ContentTypeInspector, InspectOptions};
//! use reinhardt_db::contenttypes::{ContentType, ContentTypeRegistry};
//!
//! let registry = ContentTypeRegistry::new();
//! registry.register(ContentType::new("blog", "article"));
//! registry.register(ContentType::new("blog", "comment"));
//! registry.register(ContentType::new("auth", "user"));
//!
//! let inspector = ContentTypeInspector::new();
//!
//! // List all content types
//! let all = inspector.list(&registry, None);
//! assert_eq!(all.len(), 3);
//!
//! // List content types for specific app
//! let blog_types = inspector.list(&registry, Some("blog"));
//! assert_eq!(blog_types.len(), 2);
//!
//! // Get detailed info about a content type
//! if let Some(info) = inspector.inspect(&registry, "blog", "article") {
//!     println!("{:?}", info);
//! }
//! ```

use super::{ContentType, ContentTypeRegistry};
use std::collections::{HashMap, HashSet};

/// Options for inspection operations
#[non_exhaustive]
#[derive(Debug, Clone, Default)]
pub struct InspectOptions {
	/// Whether to include detailed information
	pub detailed: bool,
	/// Whether to sort results
	pub sorted: bool,
	/// Optional app_label filter
	pub filter_app_label: Option<String>,
	/// Optional model name pattern filter (substring match)
	pub filter_model_pattern: Option<String>,
}

impl InspectOptions {
	/// Creates new default options
	#[must_use]
	pub fn new() -> Self {
		Self::default()
	}

	/// Sets detailed mode
	#[must_use]
	pub fn detailed(mut self, detailed: bool) -> Self {
		self.detailed = detailed;
		self
	}

	/// Sets sorted mode
	#[must_use]
	pub fn sorted(mut self, sorted: bool) -> Self {
		self.sorted = sorted;
		self
	}

	/// Sets app_label filter
	#[must_use]
	pub fn filter_app_label(mut self, app_label: impl Into<String>) -> Self {
		self.filter_app_label = Some(app_label.into());
		self
	}

	/// Sets model pattern filter
	#[must_use]
	pub fn filter_model_pattern(mut self, pattern: impl Into<String>) -> Self {
		self.filter_model_pattern = Some(pattern.into());
		self
	}
}

/// Summary information about a content type
#[derive(Debug, Clone)]
pub struct ContentTypeInfo {
	/// The content type
	pub content_type: ContentType,
	/// Qualified name (app_label.model)
	pub qualified_name: String,
	/// Whether this type has any references (if known)
	pub reference_count: Option<usize>,
}

impl ContentTypeInfo {
	/// Creates new info from a content type
	#[must_use]
	pub fn from_content_type(ct: ContentType) -> Self {
		let qualified_name = format!("{}.{}", ct.app_label, ct.model);
		Self {
			content_type: ct,
			qualified_name,
			reference_count: None,
		}
	}

	/// Sets the reference count
	#[must_use]
	pub fn with_reference_count(mut self, count: usize) -> Self {
		self.reference_count = Some(count);
		self
	}
}

/// Detailed information about a content type
#[derive(Debug, Clone)]
pub struct ContentTypeDetails {
	/// Basic info
	pub info: ContentTypeInfo,
	/// App label
	pub app_label: String,
	/// Model name
	pub model: String,
	/// Internal ID
	pub id: i64,
	/// Related permissions (if permissions feature is enabled)
	pub permissions: Vec<String>,
	/// Metadata
	pub metadata: HashMap<String, String>,
}

impl ContentTypeDetails {
	/// Creates detailed info from a content type
	#[must_use]
	pub fn from_content_type(ct: ContentType) -> Self {
		let app_label = ct.app_label.clone();
		let model = ct.model.clone();
		let id = ct.id.unwrap_or(0);

		// Generate default permissions
		let permissions = vec![
			format!("{}.add_{}", app_label, model),
			format!("{}.change_{}", app_label, model),
			format!("{}.delete_{}", app_label, model),
			format!("{}.view_{}", app_label, model),
		];

		Self {
			info: ContentTypeInfo::from_content_type(ct),
			app_label,
			model,
			id,
			permissions,
			metadata: HashMap::new(),
		}
	}

	/// Adds metadata
	#[must_use]
	pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
		self.metadata.insert(key.into(), value.into());
		self
	}
}

/// Statistics about the registry
#[derive(Debug, Clone, Default)]
pub struct RegistryStats {
	/// Total number of content types
	pub total_count: usize,
	/// Number of unique apps
	pub app_count: usize,
	/// Content types per app
	pub types_per_app: HashMap<String, usize>,
	/// List of all app labels
	pub app_labels: Vec<String>,
}

impl RegistryStats {
	/// Returns the average number of types per app
	#[must_use]
	pub fn average_types_per_app(&self) -> f64 {
		if self.app_count == 0 {
			0.0
		} else {
			self.total_count as f64 / self.app_count as f64
		}
	}

	/// Returns the app with the most content types
	#[must_use]
	pub fn largest_app(&self) -> Option<(&String, &usize)> {
		self.types_per_app.iter().max_by_key(|(_, count)| *count)
	}
}

/// Handles inspection of content types
#[derive(Debug, Clone, Default)]
pub struct ContentTypeInspector {
	/// Inspection options
	options: InspectOptions,
}

impl ContentTypeInspector {
	/// Creates a new inspector with default options
	#[must_use]
	pub fn new() -> Self {
		Self::default()
	}

	/// Creates an inspector with specified options
	#[must_use]
	pub fn with_options(options: InspectOptions) -> Self {
		Self { options }
	}

	/// Lists all content types, optionally filtered by app_label
	#[must_use]
	pub fn list(
		&self,
		registry: &ContentTypeRegistry,
		app_label: Option<&str>,
	) -> Vec<ContentTypeInfo> {
		let mut results: Vec<ContentTypeInfo> = registry
			.all()
			.into_iter()
			.filter(|ct| match app_label {
				Some(label) => ct.app_label == label,
				None => true,
			})
			.filter(|ct| match &self.options.filter_model_pattern {
				Some(pattern) => ct.model.contains(pattern),
				None => true,
			})
			.map(ContentTypeInfo::from_content_type)
			.collect();

		if self.options.sorted {
			results.sort_by(|a, b| a.qualified_name.cmp(&b.qualified_name));
		}

		results
	}

	/// Gets detailed information about a specific content type
	#[must_use]
	pub fn inspect(
		&self,
		registry: &ContentTypeRegistry,
		app_label: &str,
		model: &str,
	) -> Option<ContentTypeDetails> {
		registry
			.get(app_label, model)
			.map(ContentTypeDetails::from_content_type)
	}

	/// Gets detailed information by qualified name
	#[must_use]
	pub fn inspect_by_name(
		&self,
		registry: &ContentTypeRegistry,
		qualified_name: &str,
	) -> Option<ContentTypeDetails> {
		let parts: Vec<&str> = qualified_name.splitn(2, '.').collect();
		if parts.len() == 2 {
			self.inspect(registry, parts[0], parts[1])
		} else {
			None
		}
	}

	/// Lists all unique app labels
	#[must_use]
	pub fn list_apps(&self, registry: &ContentTypeRegistry) -> Vec<String> {
		let mut apps: Vec<String> = registry
			.all()
			.into_iter()
			.map(|ct| ct.app_label)
			.collect::<HashSet<_>>()
			.into_iter()
			.collect();

		if self.options.sorted {
			apps.sort();
		}

		apps
	}

	/// Lists all models for a specific app
	#[must_use]
	pub fn list_models(&self, registry: &ContentTypeRegistry, app_label: &str) -> Vec<String> {
		let mut models: Vec<String> = registry
			.all()
			.into_iter()
			.filter(|ct| ct.app_label == app_label)
			.map(|ct| ct.model)
			.collect();

		if self.options.sorted {
			models.sort();
		}

		models
	}

	/// Gets statistics about the registry
	#[must_use]
	pub fn stats(&self, registry: &ContentTypeRegistry) -> RegistryStats {
		let all = registry.all();
		let mut types_per_app: HashMap<String, usize> = HashMap::new();

		for ct in &all {
			*types_per_app.entry(ct.app_label.clone()).or_insert(0) += 1;
		}

		let mut app_labels: Vec<String> = types_per_app.keys().cloned().collect();
		app_labels.sort();

		RegistryStats {
			total_count: all.len(),
			app_count: types_per_app.len(),
			types_per_app,
			app_labels,
		}
	}

	/// Searches for content types matching a pattern
	#[must_use]
	pub fn search(&self, registry: &ContentTypeRegistry, pattern: &str) -> Vec<ContentTypeInfo> {
		let pattern_lower = pattern.to_lowercase();

		let mut results: Vec<ContentTypeInfo> = registry
			.all()
			.into_iter()
			.filter(|ct| {
				ct.app_label.to_lowercase().contains(&pattern_lower)
					|| ct.model.to_lowercase().contains(&pattern_lower)
			})
			.map(ContentTypeInfo::from_content_type)
			.collect();

		if self.options.sorted {
			results.sort_by(|a, b| a.qualified_name.cmp(&b.qualified_name));
		}

		results
	}

	/// Finds orphaned content types (types with no corresponding model)
	///
	/// This is useful for identifying stale content types that should be cleaned up.
	/// The `known_models` parameter should contain a list of qualified names
	/// (app_label.model) of models that are currently defined.
	#[must_use]
	pub fn find_orphaned(
		&self,
		registry: &ContentTypeRegistry,
		known_models: &[String],
	) -> Vec<ContentTypeInfo> {
		let known_set: HashSet<&String> = known_models.iter().collect();

		registry
			.all()
			.into_iter()
			.filter(|ct| {
				let qualified = format!("{}.{}", ct.app_label, ct.model);
				!known_set.contains(&qualified)
			})
			.map(ContentTypeInfo::from_content_type)
			.collect()
	}

	/// Validates all content types in the registry
	///
	/// Returns a list of validation errors (if any).
	#[must_use]
	pub fn validate(&self, registry: &ContentTypeRegistry) -> Vec<String> {
		let mut errors = Vec::new();
		let all = registry.all();

		// Check for empty app_labels or models
		for ct in &all {
			if ct.app_label.is_empty() {
				errors.push(format!(
					"Content type with ID {:?} has empty app_label",
					ct.id
				));
			}
			if ct.model.is_empty() {
				errors.push(format!(
					"Content type {}.{:?} has empty model",
					ct.app_label, ct.id
				));
			}

			// Check for invalid characters
			if ct.app_label.contains('.') {
				errors.push(format!(
					"Content type {}.{} has app_label containing '.'",
					ct.app_label, ct.model
				));
			}
			if ct.model.contains('.') {
				errors.push(format!(
					"Content type {}.{} has model containing '.'",
					ct.app_label, ct.model
				));
			}
		}

		// Check for duplicates (shouldn't happen, but good to verify)
		let mut seen: HashSet<String> = HashSet::new();
		for ct in &all {
			let key = format!("{}.{}", ct.app_label, ct.model);
			if !seen.insert(key.clone()) {
				errors.push(format!("Duplicate content type: {}", key));
			}
		}

		errors
	}

	/// Generates a summary report of the registry
	#[must_use]
	pub fn generate_report(&self, registry: &ContentTypeRegistry) -> String {
		let stats = self.stats(registry);
		let validation_errors = self.validate(registry);

		let mut report = String::new();
		report.push_str("=== ContentType Registry Report ===\n\n");

		report.push_str(&format!("Total content types: {}\n", stats.total_count));
		report.push_str(&format!("Number of apps: {}\n", stats.app_count));
		report.push_str(&format!(
			"Average types per app: {:.2}\n",
			stats.average_types_per_app()
		));

		if let Some((app, count)) = stats.largest_app() {
			report.push_str(&format!("Largest app: {} ({} types)\n", app, count));
		}

		report.push_str("\n--- Apps ---\n");
		for (app, count) in &stats.types_per_app {
			report.push_str(&format!("  {}: {} types\n", app, count));
		}

		if !validation_errors.is_empty() {
			report.push_str("\n--- Validation Errors ---\n");
			for error in &validation_errors {
				report.push_str(&format!("  - {}\n", error));
			}
		} else {
			report.push_str("\n--- Validation: OK ---\n");
		}

		report
	}
}

/// Convenience function to list all content types
#[must_use]
pub fn list(registry: &ContentTypeRegistry) -> Vec<ContentTypeInfo> {
	ContentTypeInspector::with_options(InspectOptions::new().sorted(true)).list(registry, None)
}

/// Convenience function to list content types for an app
#[must_use]
pub fn list_for_app(registry: &ContentTypeRegistry, app_label: &str) -> Vec<ContentTypeInfo> {
	ContentTypeInspector::with_options(InspectOptions::new().sorted(true))
		.list(registry, Some(app_label))
}

/// Convenience function to inspect a content type
#[must_use]
pub fn inspect(
	registry: &ContentTypeRegistry,
	app_label: &str,
	model: &str,
) -> Option<ContentTypeDetails> {
	ContentTypeInspector::new().inspect(registry, app_label, model)
}

/// Convenience function to find orphaned content types
#[must_use]
pub fn find_orphaned(
	registry: &ContentTypeRegistry,
	known_models: &[String],
) -> Vec<ContentTypeInfo> {
	ContentTypeInspector::new().find_orphaned(registry, known_models)
}

/// Convenience function to get registry stats
#[must_use]
pub fn stats(registry: &ContentTypeRegistry) -> RegistryStats {
	ContentTypeInspector::new().stats(registry)
}

/// Convenience function to validate the registry
#[must_use]
pub fn validate(registry: &ContentTypeRegistry) -> Vec<String> {
	ContentTypeInspector::new().validate(registry)
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_inspect_options_builder() {
		let options = InspectOptions::new()
			.detailed(true)
			.sorted(true)
			.filter_app_label("blog")
			.filter_model_pattern("article");

		assert!(options.detailed);
		assert!(options.sorted);
		assert_eq!(options.filter_app_label, Some("blog".to_string()));
		assert_eq!(options.filter_model_pattern, Some("article".to_string()));
	}

	#[test]
	fn test_content_type_info() {
		let ct = ContentType::new("blog", "article");
		let info = ContentTypeInfo::from_content_type(ct);

		assert_eq!(info.qualified_name, "blog.article");
		assert!(info.reference_count.is_none());
	}

	#[test]
	fn test_content_type_info_with_reference_count() {
		let ct = ContentType::new("blog", "article");
		let info = ContentTypeInfo::from_content_type(ct).with_reference_count(10);

		assert_eq!(info.reference_count, Some(10));
	}

	#[test]
	fn test_content_type_details() {
		let ct = ContentType::new("blog", "article");
		let details = ContentTypeDetails::from_content_type(ct);

		assert_eq!(details.app_label, "blog");
		assert_eq!(details.model, "article");
		assert!(
			details
				.permissions
				.contains(&"blog.add_article".to_string())
		);
		assert!(
			details
				.permissions
				.contains(&"blog.view_article".to_string())
		);
	}

	#[test]
	fn test_content_type_details_with_metadata() {
		let ct = ContentType::new("blog", "article");
		let details =
			ContentTypeDetails::from_content_type(ct).with_metadata("source", "migration");

		assert_eq!(
			details.metadata.get("source"),
			Some(&"migration".to_string())
		);
	}

	#[test]
	fn test_registry_stats() {
		let registry = ContentTypeRegistry::new();
		registry.register(ContentType::new("blog", "article"));
		registry.register(ContentType::new("blog", "comment"));
		registry.register(ContentType::new("auth", "user"));

		let inspector = ContentTypeInspector::new();
		let stats = inspector.stats(&registry);

		assert_eq!(stats.total_count, 3);
		assert_eq!(stats.app_count, 2);
		assert_eq!(stats.types_per_app.get("blog"), Some(&2));
		assert_eq!(stats.types_per_app.get("auth"), Some(&1));
	}

	#[test]
	fn test_registry_stats_average() {
		let registry = ContentTypeRegistry::new();
		registry.register(ContentType::new("blog", "article"));
		registry.register(ContentType::new("blog", "comment"));
		registry.register(ContentType::new("auth", "user"));
		registry.register(ContentType::new("auth", "group"));

		let stats = stats(&registry);

		assert!((stats.average_types_per_app() - 2.0).abs() < 0.001);
	}

	#[test]
	fn test_list_all() {
		let registry = ContentTypeRegistry::new();
		registry.register(ContentType::new("blog", "article"));
		registry.register(ContentType::new("auth", "user"));

		let inspector = ContentTypeInspector::new();
		let results = inspector.list(&registry, None);

		assert_eq!(results.len(), 2);
	}

	#[test]
	fn test_list_by_app() {
		let registry = ContentTypeRegistry::new();
		registry.register(ContentType::new("blog", "article"));
		registry.register(ContentType::new("blog", "comment"));
		registry.register(ContentType::new("auth", "user"));

		let inspector = ContentTypeInspector::new();
		let results = inspector.list(&registry, Some("blog"));

		assert_eq!(results.len(), 2);
	}

	#[test]
	fn test_list_sorted() {
		let registry = ContentTypeRegistry::new();
		registry.register(ContentType::new("blog", "comment"));
		registry.register(ContentType::new("auth", "user"));
		registry.register(ContentType::new("blog", "article"));

		let inspector = ContentTypeInspector::with_options(InspectOptions::new().sorted(true));
		let results = inspector.list(&registry, None);

		assert_eq!(results[0].qualified_name, "auth.user");
		assert_eq!(results[1].qualified_name, "blog.article");
		assert_eq!(results[2].qualified_name, "blog.comment");
	}

	#[test]
	fn test_inspect() {
		let registry = ContentTypeRegistry::new();
		registry.register(ContentType::new("blog", "article"));

		let inspector = ContentTypeInspector::new();
		let details = inspector.inspect(&registry, "blog", "article");

		assert!(details.is_some());
		let details = details.unwrap();
		assert_eq!(details.app_label, "blog");
		assert_eq!(details.model, "article");
	}

	#[test]
	fn test_inspect_not_found() {
		let registry = ContentTypeRegistry::new();

		let inspector = ContentTypeInspector::new();
		let details = inspector.inspect(&registry, "blog", "nonexistent");

		assert!(details.is_none());
	}

	#[test]
	fn test_inspect_by_name() {
		let registry = ContentTypeRegistry::new();
		registry.register(ContentType::new("blog", "article"));

		let inspector = ContentTypeInspector::new();
		let details = inspector.inspect_by_name(&registry, "blog.article");

		assert!(details.is_some());
	}

	#[test]
	fn test_inspect_by_name_invalid() {
		let registry = ContentTypeRegistry::new();

		let inspector = ContentTypeInspector::new();
		let details = inspector.inspect_by_name(&registry, "invalid");

		assert!(details.is_none());
	}

	#[test]
	fn test_list_apps() {
		let registry = ContentTypeRegistry::new();
		registry.register(ContentType::new("blog", "article"));
		registry.register(ContentType::new("auth", "user"));
		registry.register(ContentType::new("admin", "logentry"));

		let inspector = ContentTypeInspector::with_options(InspectOptions::new().sorted(true));
		let apps = inspector.list_apps(&registry);

		assert_eq!(apps.len(), 3);
		assert_eq!(apps[0], "admin");
		assert_eq!(apps[1], "auth");
		assert_eq!(apps[2], "blog");
	}

	#[test]
	fn test_list_models() {
		let registry = ContentTypeRegistry::new();
		registry.register(ContentType::new("blog", "article"));
		registry.register(ContentType::new("blog", "comment"));
		registry.register(ContentType::new("blog", "tag"));

		let inspector = ContentTypeInspector::with_options(InspectOptions::new().sorted(true));
		let models = inspector.list_models(&registry, "blog");

		assert_eq!(models.len(), 3);
		assert_eq!(models[0], "article");
		assert_eq!(models[1], "comment");
		assert_eq!(models[2], "tag");
	}

	#[test]
	fn test_search() {
		let registry = ContentTypeRegistry::new();
		registry.register(ContentType::new("blog", "article"));
		registry.register(ContentType::new("blog", "comment"));
		registry.register(ContentType::new("auth", "user"));
		registry.register(ContentType::new("auth", "article_permission")); // Contains "article"

		let inspector = ContentTypeInspector::new();
		let results = inspector.search(&registry, "article");

		assert_eq!(results.len(), 2);
	}

	#[test]
	fn test_search_case_insensitive() {
		let registry = ContentTypeRegistry::new();
		registry.register(ContentType::new("Blog", "Article"));

		let inspector = ContentTypeInspector::new();
		let results = inspector.search(&registry, "blog");

		assert_eq!(results.len(), 1);
	}

	#[test]
	fn test_find_orphaned() {
		let registry = ContentTypeRegistry::new();
		registry.register(ContentType::new("blog", "article"));
		registry.register(ContentType::new("blog", "old_model"));
		registry.register(ContentType::new("auth", "user"));

		let known_models = vec!["blog.article".to_string(), "auth.user".to_string()];

		let inspector = ContentTypeInspector::new();
		let orphaned = inspector.find_orphaned(&registry, &known_models);

		assert_eq!(orphaned.len(), 1);
		assert_eq!(orphaned[0].qualified_name, "blog.old_model");
	}

	#[test]
	fn test_validate_valid() {
		let registry = ContentTypeRegistry::new();
		registry.register(ContentType::new("blog", "article"));
		registry.register(ContentType::new("auth", "user"));

		let inspector = ContentTypeInspector::new();
		let errors = inspector.validate(&registry);

		assert!(errors.is_empty());
	}

	#[test]
	fn test_validate_with_model_pattern() {
		let registry = ContentTypeRegistry::new();
		registry.register(ContentType::new("blog", "article"));
		registry.register(ContentType::new("blog", "article_comment"));
		registry.register(ContentType::new("auth", "user"));

		let inspector = ContentTypeInspector::with_options(
			InspectOptions::new().filter_model_pattern("article"),
		);
		let results = inspector.list(&registry, None);

		assert_eq!(results.len(), 2);
	}

	#[test]
	fn test_generate_report() {
		let registry = ContentTypeRegistry::new();
		registry.register(ContentType::new("blog", "article"));
		registry.register(ContentType::new("blog", "comment"));
		registry.register(ContentType::new("auth", "user"));

		let inspector = ContentTypeInspector::new();
		let report = inspector.generate_report(&registry);

		assert!(report.contains("ContentType Registry Report"));
		assert!(report.contains("Total content types: 3"));
		assert!(report.contains("Number of apps: 2"));
		assert!(report.contains("blog"));
		assert!(report.contains("auth"));
		assert!(report.contains("Validation: OK"));
	}

	#[test]
	fn test_convenience_functions() {
		let registry = ContentTypeRegistry::new();
		registry.register(ContentType::new("blog", "article"));
		registry.register(ContentType::new("auth", "user"));

		// list
		let all = list(&registry);
		assert_eq!(all.len(), 2);

		// list_for_app
		let blog_types = list_for_app(&registry, "blog");
		assert_eq!(blog_types.len(), 1);

		// inspect
		let details = inspect(&registry, "blog", "article");
		assert!(details.is_some());

		// stats
		let registry_stats = stats(&registry);
		assert_eq!(registry_stats.total_count, 2);

		// validate
		let errors = validate(&registry);
		assert!(errors.is_empty());

		// find_orphaned
		let known = vec!["blog.article".to_string()];
		let orphaned = find_orphaned(&registry, &known);
		assert_eq!(orphaned.len(), 1);
	}
}
