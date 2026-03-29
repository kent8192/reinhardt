//! Nested resources support for ViewSets
//!
//! Enables hierarchical resource relationships like:
//! - /users/{user_id}/posts/
//! - /posts/{post_id}/comments/
//! - /organizations/{org_id}/teams/{team_id}/members/

use reinhardt_http::Request;
use std::collections::HashMap;
use std::sync::Arc;

/// Nested resource configuration
#[derive(Debug, Clone)]
pub struct NestedResource {
	/// Parent resource name (e.g., "user")
	pub parent: String,
	/// Parent ID parameter name (e.g., "user_id")
	pub parent_id_param: String,
	/// Lookup field in child resource to filter by parent (e.g., "user_id")
	pub lookup_field: String,
}

impl NestedResource {
	/// Create a new nested resource configuration
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_views::viewsets::NestedResource;
	///
	/// let nested = NestedResource::new("user", "user_id", "user_id");
	/// assert_eq!(nested.parent, "user");
	/// assert_eq!(nested.parent_id_param, "user_id");
	/// ```
	pub fn new(
		parent: impl Into<String>,
		parent_id_param: impl Into<String>,
		lookup_field: impl Into<String>,
	) -> Self {
		Self {
			parent: parent.into(),
			parent_id_param: parent_id_param.into(),
			lookup_field: lookup_field.into(),
		}
	}
}

/// Nested resource path builder
#[derive(Debug, Clone)]
pub struct NestedResourcePath {
	/// Path segments (resource_name, id_param_name)
	segments: Vec<(String, String)>,
}

impl NestedResourcePath {
	/// Create a new nested resource path
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_views::viewsets::NestedResourcePath;
	///
	/// let path = NestedResourcePath::new()
	///     .add_segment("users", "user_id")
	///     .add_segment("posts", "post_id");
	/// let url = path.build_url();
	/// assert_eq!(url, "users/{user_id}/posts/{post_id}/");
	/// ```
	pub fn new() -> Self {
		Self {
			segments: Vec::new(),
		}
	}

	/// Add a path segment
	pub fn add_segment(mut self, resource: impl Into<String>, id_param: impl Into<String>) -> Self {
		self.segments.push((resource.into(), id_param.into()));
		self
	}

	/// Build the URL pattern
	pub fn build_url(&self) -> String {
		let mut parts = Vec::new();
		for (resource, id_param) in &self.segments {
			parts.push(resource.clone());
			parts.push(format!("{{{}}}", id_param));
		}
		format!("{}/", parts.join("/"))
	}

	/// Build the list URL pattern (resource names only, no ID placeholders)
	///
	/// This returns a URL pattern with only the resource names, without any
	/// ID placeholders. For routing purposes where IDs are extracted separately.
	///
	/// # Examples
	///
	/// - Single segment: `users` -> `"users/"`
	/// - Two segments: `users/posts` -> `"users/posts/"`
	pub fn build_list_url(&self) -> String {
		if self.segments.is_empty() {
			return "/".to_string();
		}

		let parts: Vec<&str> = self
			.segments
			.iter()
			.map(|(resource, _)| resource.as_str())
			.collect();
		format!("{}/", parts.join("/"))
	}

	/// Extract parent IDs from request path
	pub fn extract_parent_ids(&self, request: &Request) -> HashMap<String, String> {
		let mut ids = HashMap::new();
		let path = request.uri.path();

		// Simple path parsing (assumes URL follows pattern)
		let parts: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();

		let mut segment_index = 0;
		for i in (0..parts.len()).step_by(2) {
			if segment_index >= self.segments.len() {
				break;
			}

			if i + 1 < parts.len() {
				let (_resource, id_param) = &self.segments[segment_index];
				ids.insert(id_param.clone(), parts[i + 1].to_string());
				segment_index += 1;
			}
		}

		ids
	}
}

impl Default for NestedResourcePath {
	fn default() -> Self {
		Self::new()
	}
}

/// Nested ViewSet wrapper
pub struct NestedViewSet<V> {
	/// Inner ViewSet
	inner: Arc<V>,
	/// Nested resource configuration
	nested: NestedResource,
}

impl<V> NestedViewSet<V> {
	/// Create a new nested ViewSet
	///
	/// # Examples
	///
	/// ```no_run
	/// use reinhardt_views::viewsets::{NestedViewSet, NestedResource, ModelViewSet};
	///
	/// let inner_viewset = ModelViewSet::new("comments");
	/// let nested = NestedResource::new("post", "post_id", "post_id");
	/// let nested_viewset = NestedViewSet::new(inner_viewset, nested);
	/// ```
	pub fn new(inner: V, nested: NestedResource) -> Self {
		Self {
			inner: Arc::new(inner),
			nested,
		}
	}

	/// Get the parent ID from request
	pub fn get_parent_id(&self, request: &Request) -> Option<String> {
		// Extract parent ID from path parameters
		let path = request.uri.path();
		let parts: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();

		// Simple extraction: look for the pattern /parent/{parent_id}/child/
		for (i, part) in parts.iter().enumerate() {
			if *part == self.nested.parent && i + 1 < parts.len() {
				return Some(parts[i + 1].to_string());
			}
		}

		None
	}

	/// Get the nested resource configuration
	pub fn nested_config(&self) -> &NestedResource {
		&self.nested
	}

	/// Get the inner ViewSet
	pub fn inner(&self) -> Arc<V> {
		self.inner.clone()
	}
}

/// Helper to create nested URLs
pub fn nested_url(parent: &str, parent_id: &str, child: &str) -> String {
	format!("{}/{}/{}/", parent, parent_id, child)
}

/// Helper to create detail nested URLs
pub fn nested_detail_url(parent: &str, parent_id: &str, child: &str, child_id: &str) -> String {
	format!("{}/{}/{}/{}/", parent, parent_id, child, child_id)
}

#[cfg(test)]
mod tests {
	use super::*;
	use bytes::Bytes;
	use hyper::{HeaderMap, Method, Version};

	#[test]
	fn test_nested_resource_new() {
		let nested = NestedResource::new("user", "user_id", "user_id");
		assert_eq!(nested.parent, "user");
		assert_eq!(nested.parent_id_param, "user_id");
		assert_eq!(nested.lookup_field, "user_id");
	}

	#[test]
	fn test_nested_resource_path_single_level() {
		let path = NestedResourcePath::new().add_segment("users", "user_id");
		assert_eq!(path.build_url(), "users/{user_id}/");
		assert_eq!(path.build_list_url(), "users/");
	}

	#[test]
	fn test_nested_resource_path_two_levels() {
		let path = NestedResourcePath::new()
			.add_segment("users", "user_id")
			.add_segment("posts", "post_id");

		assert_eq!(path.build_url(), "users/{user_id}/posts/{post_id}/");
		assert_eq!(path.build_list_url(), "users/posts/");
	}

	#[test]
	fn test_nested_resource_path_three_levels() {
		let path = NestedResourcePath::new()
			.add_segment("organizations", "org_id")
			.add_segment("teams", "team_id")
			.add_segment("members", "member_id");

		assert_eq!(
			path.build_url(),
			"organizations/{org_id}/teams/{team_id}/members/{member_id}/"
		);
		assert_eq!(path.build_list_url(), "organizations/teams/members/");
	}

	#[test]
	fn test_extract_parent_ids() {
		let path = NestedResourcePath::new()
			.add_segment("users", "user_id")
			.add_segment("posts", "post_id");

		let request = Request::builder()
			.method(Method::GET)
			.uri("/users/123/posts/456/")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();

		let ids = path.extract_parent_ids(&request);
		assert_eq!(ids.get("user_id"), Some(&"123".to_string()));
		assert_eq!(ids.get("post_id"), Some(&"456".to_string()));
	}

	#[test]
	fn test_extract_parent_ids_three_levels() {
		let path = NestedResourcePath::new()
			.add_segment("organizations", "org_id")
			.add_segment("teams", "team_id")
			.add_segment("members", "member_id");

		let request = Request::builder()
			.method(Method::GET)
			.uri("/organizations/1/teams/2/members/3/")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();

		let ids = path.extract_parent_ids(&request);
		assert_eq!(ids.get("org_id"), Some(&"1".to_string()));
		assert_eq!(ids.get("team_id"), Some(&"2".to_string()));
		assert_eq!(ids.get("member_id"), Some(&"3".to_string()));
	}

	#[test]
	fn test_nested_viewset_creation() {
		#[derive(Debug, Clone)]
		struct TestViewSet {
			// Allow dead_code: field set during construction to simulate a named viewset in test
			#[allow(dead_code)]
			name: String,
		}

		let inner = TestViewSet {
			name: "comments".to_string(),
		};
		let nested = NestedResource::new("post", "post_id", "post_id");
		let viewset = NestedViewSet::new(inner, nested);

		assert_eq!(viewset.nested_config().parent, "post");
		assert_eq!(viewset.nested_config().parent_id_param, "post_id");
	}

	#[test]
	fn test_nested_viewset_get_parent_id() {
		#[derive(Debug, Clone)]
		struct TestViewSet;

		let inner = TestViewSet;
		let nested = NestedResource::new("posts", "post_id", "post_id");
		let viewset = NestedViewSet::new(inner, nested);

		let request = Request::builder()
			.method(Method::GET)
			.uri("/posts/123/comments/")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();

		let parent_id = viewset.get_parent_id(&request);
		assert_eq!(parent_id, Some("123".to_string()));
	}

	#[test]
	fn test_nested_url_helpers() {
		assert_eq!(nested_url("users", "123", "posts"), "users/123/posts/");
		assert_eq!(
			nested_detail_url("users", "123", "posts", "456"),
			"users/123/posts/456/"
		);
	}

	#[test]
	fn test_nested_resource_path_empty() {
		let path = NestedResourcePath::new();
		assert_eq!(path.build_list_url(), "/");
	}

	#[test]
	fn test_extract_parent_ids_empty_path() {
		let path = NestedResourcePath::new();
		let request = Request::builder()
			.method(Method::GET)
			.uri("/")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();

		let ids = path.extract_parent_ids(&request);
		assert_eq!(ids.len(), 0);
	}
}
