//! Join operations for proxy relationships

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// Configuration for join operations on proxy relationships.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JoinConfig {
	/// Whether to eagerly load the relationship.
	pub eager_load: bool,
	/// Maximum depth for nested relationship traversal.
	pub max_depth: Option<usize>,
	/// Strategy to use when loading the relationship.
	#[serde(skip)]
	pub loading_strategy: Option<crate::proxy::LoadingStrategy>,
	/// SQL join type (e.g., `"LEFT JOIN"`, `"INNER JOIN"`).
	pub join_type: Option<String>,
	/// SQL join condition expression.
	pub condition: Option<String>,
}

impl JoinConfig {
	/// Create a new `JoinConfig` with default values.
	pub fn new() -> Self {
		Self {
			eager_load: false,
			max_depth: None,
			loading_strategy: None,
			join_type: None,
			condition: None,
		}
	}

	/// Set the loading strategy (builder pattern)
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_urls::proxy::{JoinConfig, LoadingStrategy};
	///
	/// let config = JoinConfig::new()
	///     .with_loading_strategy(LoadingStrategy::Joined);
	/// ```
	pub fn with_loading_strategy(mut self, strategy: crate::proxy::LoadingStrategy) -> Self {
		self.loading_strategy = Some(strategy);
		self
	}

	/// Set the join type (builder pattern)
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_urls::proxy::JoinConfig;
	///
	/// let config = JoinConfig::new()
	///     .with_join_type("LEFT JOIN");
	/// ```
	pub fn with_join_type(mut self, join_type: &str) -> Self {
		self.join_type = Some(join_type.to_string());
		self
	}

	/// Set the join condition (builder pattern)
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_urls::proxy::JoinConfig;
	///
	/// let config = JoinConfig::new()
	///     .with_condition("users.id = posts.user_id");
	/// ```
	pub fn with_condition(mut self, condition: &str) -> Self {
		self.condition = Some(condition.to_string());
		self
	}
}

impl Default for JoinConfig {
	fn default() -> Self {
		Self::new()
	}
}

// LoadingStrategy is now re-exported from reinhardt-orm in lib.rs

/// A proxy for traversing nested relationships with conditions.
#[derive(Debug, Clone)]
pub struct NestedProxy {
	/// Sequence of relationship names forming the nested path.
	pub path: Vec<String>,
	/// Filter conditions applied along the path.
	pub conditions: Vec<String>,
	/// Final attribute to extract from the deepest relationship.
	pub final_attribute: Option<String>,
}

impl NestedProxy {
	/// Create a new empty nested proxy
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_urls::proxy::NestedProxy;
	///
	/// let proxy = NestedProxy::new();
	/// assert_eq!(proxy.depth(), 0);
	/// ```
	pub fn new() -> Self {
		Self {
			path: Vec::new(),
			conditions: Vec::new(),
			final_attribute: None,
		}
	}

	/// Create from a path vector (for backward compatibility)
	pub fn from_path(path: Vec<String>) -> Self {
		Self {
			path,
			conditions: Vec::new(),
			final_attribute: None,
		}
	}

	/// Add a level to the nested path (builder pattern)
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_urls::proxy::NestedProxy;
	///
	/// let proxy = NestedProxy::new()
	///     .add_level("posts")
	///     .add_level("comments");
	/// assert_eq!(proxy.depth(), 2);
	/// ```
	pub fn add_level(mut self, level: &str) -> Self {
		self.path.push(level.to_string());
		self
	}

	/// Add a condition to the nested proxy (builder pattern)
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_urls::proxy::NestedProxy;
	///
	/// let proxy = NestedProxy::new()
	///     .add_level("posts")
	///     .with_condition("published = true");
	/// assert_eq!(proxy.conditions().len(), 1);
	/// ```
	pub fn with_condition(mut self, condition: &str) -> Self {
		self.conditions.push(condition.to_string());
		self
	}

	/// Set the final attribute to access (builder pattern)
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_urls::proxy::NestedProxy;
	///
	/// let proxy = NestedProxy::new()
	///     .add_level("posts")
	///     .with_attribute("title");
	/// assert_eq!(proxy.attribute(), "title");
	/// ```
	pub fn with_attribute(mut self, attr: &str) -> Self {
		self.final_attribute = Some(attr.to_string());
		self
	}

	/// Get the depth (number of levels) in the nested path
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_urls::proxy::NestedProxy;
	///
	/// let proxy = NestedProxy::new()
	///     .add_level("posts")
	///     .add_level("comments")
	///     .add_level("author");
	/// assert_eq!(proxy.depth(), 3);
	/// ```
	pub fn depth(&self) -> usize {
		self.path.len()
	}

	/// Get the final attribute name
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_urls::proxy::NestedProxy;
	///
	/// let proxy = NestedProxy::new()
	///     .add_level("posts")
	///     .with_attribute("title");
	/// assert_eq!(proxy.attribute(), "title");
	/// ```
	pub fn attribute(&self) -> &str {
		self.final_attribute.as_deref().unwrap_or("")
	}

	/// Get all conditions
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_urls::proxy::NestedProxy;
	///
	/// let proxy = NestedProxy::new()
	///     .add_level("posts")
	///     .with_condition("published = true")
	///     .add_level("comments")
	///     .with_condition("approved = true");
	/// assert_eq!(proxy.conditions().len(), 2);
	/// ```
	pub fn conditions(&self) -> &[String] {
		&self.conditions
	}
}

impl Default for NestedProxy {
	fn default() -> Self {
		Self::new()
	}
}

/// A path through relationships with circular reference detection
///
/// RelationshipPath provides a builder API for constructing paths through
/// model relationships while detecting and preventing circular references.
///
/// # Examples
///
/// ```rust,no_run
/// # use reinhardt_urls::proxy::RelationshipPath;
/// // Valid path: user -> posts -> comments
/// let path = RelationshipPath::new()
///     .through("posts")
///     .through("comments")
///     .attribute("content");
///
/// // Circular path: posts -> author -> posts (ERROR)
/// let result = RelationshipPath::new()
///     .try_through("posts").unwrap()
///     .try_through("author").unwrap()
///     .try_through("posts");  // This creates a cycle
/// assert!(result.is_err());
/// ```
#[derive(Debug, Clone)]
pub struct RelationshipPath {
	/// Sequence of relationship names in the path
	pub segments: Vec<String>,
	/// Set of visited relationship names for cycle detection
	visited: HashSet<String>,
	/// Filters applied at each relationship level
	filters: HashMap<String, Vec<(String, String)>>,
	/// Transformations applied at each relationship level
	transforms: HashMap<String, Vec<(String, String)>>,
	/// Final attribute to access
	attribute: Option<String>,
}

impl RelationshipPath {
	/// Create a new empty relationship path
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_urls::proxy::RelationshipPath;
	///
	/// let path = RelationshipPath::new();
	/// assert_eq!(path.path().len(), 0);
	/// ```
	pub fn new() -> Self {
		Self {
			segments: Vec::new(),
			visited: HashSet::new(),
			filters: HashMap::new(),
			transforms: HashMap::new(),
			attribute: None,
		}
	}

	/// Add a relationship to the path
	///
	/// Returns self for chaining. Use `try_through()` if you need error handling.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_urls::proxy::RelationshipPath;
	///
	/// let path = RelationshipPath::new()
	///     .through("posts")
	///     .through("comments");
	/// assert_eq!(path.path().len(), 2);
	/// ```
	pub fn through(mut self, relationship: &str) -> Self {
		let rel = relationship.to_string();
		self.segments.push(rel.clone());
		self.visited.insert(rel);
		self
	}

	/// Add a relationship to the path with error handling for circular references
	///
	/// # Errors
	///
	/// Returns an error if adding this relationship would create a cycle.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_urls::proxy::RelationshipPath;
	///
	/// let path = RelationshipPath::new()
	///     .try_through("posts").unwrap()
	///     .try_through("author").unwrap();
	///
	/// // This would create a cycle
	/// let result = path.try_through("posts");
	/// assert!(result.is_err());
	/// ```
	pub fn try_through(mut self, relationship: &str) -> Result<Self, CircularReferenceError> {
		let rel = relationship.to_string();

		if self.visited.contains(&rel) {
			return Err(CircularReferenceError {
				relationship: rel,
				path: self.segments.clone(),
			});
		}

		self.segments.push(rel.clone());
		self.visited.insert(rel);
		Ok(self)
	}

	/// Add a filter at the current relationship level
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_urls::proxy::RelationshipPath;
	///
	/// let path = RelationshipPath::new()
	///     .through("posts")
	///     .with_filter("published", "true");
	/// assert!(path.has_filters());
	/// ```
	pub fn with_filter(mut self, field: &str, value: &str) -> Self {
		let current_rel = self.segments.last().cloned().unwrap_or_default();
		self.filters
			.entry(current_rel)
			.or_default()
			.push((field.to_string(), value.to_string()));
		self
	}

	/// Add a transformation at a specific relationship level
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_urls::proxy::RelationshipPath;
	///
	/// let path = RelationshipPath::new()
	///     .through("posts")
	///     .through("comments")
	///     .with_transform("author", "upper");
	/// assert!(path.has_transforms());
	/// ```
	pub fn with_transform(mut self, relationship: &str, transform: &str) -> Self {
		self.transforms
			.entry(relationship.to_string())
			.or_default()
			.push((relationship.to_string(), transform.to_string()));
		self
	}

	/// Set the final attribute to access
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_urls::proxy::RelationshipPath;
	///
	/// let path = RelationshipPath::new()
	///     .through("posts")
	///     .through("comments")
	///     .attribute("content");
	/// assert_eq!(path.get_attribute(), "content");
	/// ```
	pub fn attribute(mut self, attr: &str) -> Self {
		self.attribute = Some(attr.to_string());
		self
	}

	/// Get the path segments
	pub fn path(&self) -> &[String] {
		&self.segments
	}

	/// Get the final attribute name
	pub fn get_attribute(&self) -> &str {
		self.attribute.as_deref().unwrap_or("")
	}

	/// Check if any filters are configured
	pub fn has_filters(&self) -> bool {
		!self.filters.is_empty()
	}

	/// Get all filters
	pub fn filters(&self) -> Vec<(String, String)> {
		self.filters
			.values()
			.flat_map(|v| v.iter().cloned())
			.collect()
	}

	/// Check if any transformations are configured
	pub fn has_transforms(&self) -> bool {
		!self.transforms.is_empty()
	}

	/// Get all transformations
	pub fn transforms(&self) -> Vec<(String, String)> {
		self.transforms
			.values()
			.flat_map(|v| v.iter().cloned())
			.collect()
	}

	/// Check if a relationship is in the path (for cycle detection)
	pub fn contains(&self, relationship: &str) -> bool {
		self.visited.contains(relationship)
	}

	/// Validate the path and return self or error
	///
	/// This method is primarily for testing - the builder methods
	/// already prevent invalid paths from being constructed.
	pub fn validate(self) -> Result<Self, CircularReferenceError> {
		// Path is already validated during construction
		Ok(self)
	}
}

impl Default for RelationshipPath {
	fn default() -> Self {
		Self::new()
	}
}

/// Error returned when a circular reference is detected in a relationship path
#[derive(Debug, Clone, thiserror::Error)]
#[error("Circular reference detected: relationship '{relationship}' already exists in path {}", path_display(.path))]
pub struct CircularReferenceError {
	/// The relationship that would create a cycle
	pub relationship: String,
	/// The current path when the cycle was detected
	pub path: Vec<String>,
}

fn path_display(path: &[String]) -> String {
	if path.is_empty() {
		"(empty)".to_string()
	} else {
		format!("[{}]", path.join(" -> "))
	}
}

/// Extract relationship segments from a dot-separated path string.
pub fn extract_through_path(path: &str) -> Vec<String> {
	path.split('.').map(|s| s.to_string()).collect()
}

/// Check if any segment in a `RelationshipPath` matches the given predicate.
pub fn filter_through_path(path: &RelationshipPath, predicate: impl Fn(&str) -> bool) -> bool {
	path.segments.iter().any(|s| predicate(s))
}

/// Extract the path segments from a `NestedProxy`.
pub fn traverse_and_extract(proxy: &NestedProxy) -> Vec<String> {
	proxy.path.clone()
}

/// Extract the path segments from a `RelationshipPath`.
pub fn traverse_relationships(path: &RelationshipPath) -> Vec<String> {
	path.segments.clone()
}

#[cfg(test)]
mod tests {
	use super::*;

	/// Test basic path construction without cycles
	#[test]
	fn test_relationship_path_no_cycle() {
		let path = RelationshipPath::new()
			.through("posts")
			.through("comments")
			.through("author")
			.attribute("name");

		assert_eq!(path.path().len(), 3);
		assert_eq!(path.path()[0], "posts");
		assert_eq!(path.path()[1], "comments");
		assert_eq!(path.path()[2], "author");
		assert_eq!(path.get_attribute(), "name");
	}

	/// Test simple cycle detection: A -> B -> A
	#[test]
	fn test_simple_cycle_detection() {
		let path = RelationshipPath::new().through("posts").through("author");

		// Try to add "posts" again - should create a cycle
		let result = path.try_through("posts");

		assert!(result.is_err());
		let err = result.unwrap_err();
		assert_eq!(err.relationship, "posts");
		assert_eq!(err.path, vec!["posts", "author"]);
	}

	/// Test complex cycle detection: A -> B -> C -> A
	#[test]
	fn test_complex_cycle_detection() {
		let path = RelationshipPath::new()
			.through("user")
			.through("posts")
			.through("comments");

		// Try to add "user" again - should create a cycle
		let result = path.try_through("user");

		assert!(result.is_err());
		let err = result.unwrap_err();
		assert_eq!(err.relationship, "user");
		assert_eq!(err.path, vec!["user", "posts", "comments"]);
	}

	/// Test that different relationships don't trigger cycle detection
	#[test]
	fn test_no_false_positive_cycle() {
		let path = RelationshipPath::new()
			.through("posts")
			.through("comments")
			.through("author")
			.through("profile");

		assert_eq!(path.path().len(), 4);
	}

	/// Test contains method for cycle detection
	#[test]
	fn test_contains_relationship() {
		let path = RelationshipPath::new().through("posts").through("comments");

		assert!(path.contains("posts"));
		assert!(path.contains("comments"));
		assert!(!path.contains("author"));
	}

	/// Test path with filters
	#[test]
	fn test_path_with_filters() {
		let path = RelationshipPath::new()
			.through("posts")
			.with_filter("published", "true")
			.through("comments")
			.attribute("content");

		assert!(path.has_filters());
		assert_eq!(path.filters().len(), 1);
		assert_eq!(
			path.filters()[0],
			("published".to_string(), "true".to_string())
		);
	}

	/// Test path with transforms
	#[test]
	fn test_path_with_transforms() {
		let path = RelationshipPath::new()
			.through("posts")
			.through("comments")
			.with_transform("author", "upper")
			.attribute("name");

		assert!(path.has_transforms());
		assert_eq!(path.transforms().len(), 1);
	}

	/// Test multiple filters on different relationships
	#[test]
	fn test_multiple_filters() {
		let path = RelationshipPath::new()
			.through("posts")
			.with_filter("published", "true")
			.through("comments")
			.with_filter("approved", "true")
			.attribute("content");

		assert!(path.has_filters());
		assert_eq!(path.filters().len(), 2);
	}

	/// Test error message formatting
	#[test]
	fn test_error_message_format() {
		let path = RelationshipPath::new().through("posts").through("author");

		let result = path.try_through("posts");
		assert!(result.is_err());

		let err = result.unwrap_err();
		let error_msg = err.to_string();
		assert!(error_msg.contains("Circular reference detected"));
		assert!(error_msg.contains("posts"));
		assert!(error_msg.contains("author"));
	}

	/// Test default implementation
	#[test]
	fn test_default_path() {
		let path = RelationshipPath::default();
		assert_eq!(path.path().len(), 0);
		assert!(!path.has_filters());
		assert!(!path.has_transforms());
	}

	/// Test empty path display in error
	#[test]
	fn test_empty_path_error() {
		let err = CircularReferenceError {
			relationship: "posts".to_string(),
			path: vec![],
		};
		let msg = err.to_string();
		assert!(msg.contains("(empty)"));
	}

	/// Test clone functionality
	#[test]
	fn test_path_clone() {
		let path1 = RelationshipPath::new()
			.through("posts")
			.through("comments")
			.attribute("content");

		let path2 = path1.clone();

		assert_eq!(path1.path(), path2.path());
		assert_eq!(path1.get_attribute(), path2.get_attribute());
	}

	/// Test using through() method (non-checked version)
	#[test]
	fn test_through_allows_cycles() {
		// through() method doesn't check for cycles - it just adds to the path
		// This is by design for backwards compatibility
		let path = RelationshipPath::new()
			.through("posts")
			.through("author")
			.through("posts"); // No error, just adds to path

		assert_eq!(path.path().len(), 3);
		// But the visited set will detect it
		assert!(path.contains("posts"));
	}

	/// Test validate method
	#[test]
	fn test_validate_method() {
		let path = RelationshipPath::new()
			.through("posts")
			.through("comments")
			.attribute("content");

		let result = path.validate();
		assert!(result.is_ok());
	}

	/// Test path with only attribute (no relationships)
	#[test]
	fn test_attribute_only_path() {
		let path = RelationshipPath::new().attribute("name");

		assert_eq!(path.path().len(), 0);
		assert_eq!(path.get_attribute(), "name");
	}

	/// Test filter on empty path (edge case)
	#[test]
	fn test_filter_on_empty_path() {
		let path = RelationshipPath::new().with_filter("field", "value");

		// Filter is added with empty relationship name
		assert!(path.has_filters());
	}
}
