//! URL pattern matching and parameter extraction
//!
//! This module provides Django-style URL pattern matching with support for
//! parameterized URLs using `<param>` syntax.

use std::collections::HashMap;

/// A URL pattern with parameter extraction and URL building capabilities
#[derive(Debug, Clone)]
pub struct UrlPattern {
	/// Pattern name (e.g., "user-detail")
	name: String,
	/// URL template (e.g., "/users/`<id>`/")
	template: String,
	/// Optional namespace (e.g., "admin", "api")
	namespace: Option<String>,
}

impl UrlPattern {
	/// Creates a new URL pattern
	///
	/// # Arguments
	///
	/// * `name` - Pattern name for reverse resolution
	/// * `template` - URL template with `<param>` placeholders
	/// * `namespace` - Optional namespace for pattern grouping
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_urls::proxy::UrlPattern;
	///
	/// let pattern = UrlPattern::new("user-detail", "/users/<id>/", None);
	/// assert_eq!(pattern.name(), "user-detail");
	/// assert_eq!(pattern.template(), "/users/<id>/");
	/// ```
	pub fn new(
		name: impl Into<String>,
		template: impl Into<String>,
		namespace: Option<&str>,
	) -> Self {
		Self {
			name: name.into(),
			template: template.into(),
			namespace: namespace.map(|ns| ns.to_string()),
		}
	}

	/// Returns the pattern name
	pub fn name(&self) -> &str {
		&self.name
	}

	/// Returns the URL template
	pub fn template(&self) -> &str {
		&self.template
	}

	/// Returns the namespace if any
	pub fn namespace(&self) -> Option<&str> {
		self.namespace.as_deref()
	}

	/// Extracts parameter names from the template
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_urls::proxy::UrlPattern;
	///
	/// let pattern = UrlPattern::new("article", "/articles/<year>/<month>/<slug>/", None);
	/// let params = pattern.extract_parameters();
	/// assert_eq!(params, vec!["year", "month", "slug"]);
	/// ```
	pub fn extract_parameters(&self) -> Vec<String> {
		let mut params = Vec::new();
		let mut chars = self.template.chars().peekable();

		while let Some(ch) = chars.next() {
			if ch == '<' {
				let mut param_name = String::new();
				while let Some(&next_ch) = chars.peek() {
					if next_ch == '>' {
						chars.next(); // consume '>'
						break;
					}
					param_name.push(chars.next().unwrap());
				}
				if !param_name.is_empty() {
					params.push(param_name);
				}
			}
		}

		params
	}

	/// Builds a URL by replacing parameters in the template
	///
	/// # Arguments
	///
	/// * `kwargs` - HashMap of parameter name-value pairs
	///
	/// # Errors
	///
	/// Returns an error if a required parameter is missing
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_urls::proxy::UrlPattern;
	/// use std::collections::HashMap;
	///
	/// let pattern = UrlPattern::new("user-detail", "/users/<id>/", None);
	/// let mut kwargs = HashMap::new();
	/// kwargs.insert("id".to_string(), "123".to_string());
	///
	/// let url = pattern.build_url(&kwargs).unwrap();
	/// assert_eq!(url, "/users/123/");
	/// ```
	pub fn build_url(&self, kwargs: &HashMap<String, String>) -> Result<String, String> {
		// Single-pass scan to avoid double substitution when a parameter value
		// contains another parameter's placeholder pattern (e.g. "<id>").
		let mut result = String::with_capacity(self.template.len());
		let mut chars = self.template.chars().peekable();

		while let Some(ch) = chars.next() {
			if ch == '<' {
				let mut param_name = String::new();
				while let Some(&next_ch) = chars.peek() {
					if next_ch == '>' {
						chars.next(); // consume '>'
						break;
					}
					param_name.push(chars.next().unwrap());
				}
				if param_name.is_empty() {
					// Bare '<>' — write it through unchanged
					result.push('<');
					result.push('>');
				} else {
					match kwargs.get(&param_name) {
						Some(value) => result.push_str(value),
						None => {
							return Err(format!("Missing required parameter: {}", param_name));
						}
					}
				}
			} else {
				result.push(ch);
			}
		}

		Ok(result)
	}

	/// Checks if a URL matches this pattern
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_urls::proxy::UrlPattern;
	///
	/// let pattern = UrlPattern::new("user-detail", "/users/<id>/", None);
	/// assert!(pattern.matches("/users/123/"));
	/// assert!(!pattern.matches("/posts/456/"));
	/// ```
	pub fn matches(&self, url: &str) -> bool {
		let pattern_parts: Vec<&str> = self.template.split('/').collect();
		let url_parts: Vec<&str> = url.split('/').collect();

		if pattern_parts.len() != url_parts.len() {
			return false;
		}

		for (pattern_part, url_part) in pattern_parts.iter().zip(url_parts.iter()) {
			if pattern_part.starts_with('<') && pattern_part.ends_with('>') {
				// This is a parameter, any non-empty value matches
				if url_part.is_empty() {
					return false;
				}
			} else if pattern_part != url_part {
				// Static part must match exactly
				return false;
			}
		}

		true
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_url_pattern_creation() {
		let pattern = UrlPattern::new("home", "/", None);
		assert_eq!(pattern.name(), "home");
		assert_eq!(pattern.template(), "/");
		assert_eq!(pattern.namespace(), None);
	}

	#[test]
	fn test_url_pattern_with_namespace() {
		let pattern = UrlPattern::new("admin:index", "/admin/", Some("admin"));
		assert_eq!(pattern.name(), "admin:index");
		assert_eq!(pattern.namespace(), Some("admin"));
	}

	#[test]
	fn test_extract_parameters() {
		let pattern = UrlPattern::new("article", "/articles/<year>/<month>/<slug>/", None);
		let params = pattern.extract_parameters();
		assert_eq!(params, vec!["year", "month", "slug"]);
	}

	#[test]
	fn test_extract_no_parameters() {
		let pattern = UrlPattern::new("home", "/", None);
		let params = pattern.extract_parameters();
		assert!(params.is_empty());
	}

	#[test]
	fn test_build_url() {
		let pattern = UrlPattern::new("user-detail", "/users/<id>/", None);
		let mut kwargs = HashMap::new();
		kwargs.insert("id".to_string(), "123".to_string());

		let url = pattern.build_url(&kwargs).unwrap();
		assert_eq!(url, "/users/123/");
	}

	#[test]
	fn test_build_url_multiple_params() {
		let pattern = UrlPattern::new("article", "/articles/<year>/<month>/<slug>/", None);
		let mut kwargs = HashMap::new();
		kwargs.insert("year".to_string(), "2024".to_string());
		kwargs.insert("month".to_string(), "12".to_string());
		kwargs.insert("slug".to_string(), "hello-world".to_string());

		let url = pattern.build_url(&kwargs).unwrap();
		assert_eq!(url, "/articles/2024/12/hello-world/");
	}

	#[test]
	fn test_build_url_missing_param() {
		let pattern = UrlPattern::new("user-detail", "/users/<id>/", None);
		let kwargs = HashMap::new();

		let result = pattern.build_url(&kwargs);
		assert!(result.is_err());
		assert_eq!(result.unwrap_err(), "Missing required parameter: id");
	}

	#[test]
	fn test_matches() {
		let pattern = UrlPattern::new("user-detail", "/users/<id>/", None);
		assert!(pattern.matches("/users/123/"));
		assert!(pattern.matches("/users/abc/"));
		assert!(!pattern.matches("/posts/123/"));
		assert!(!pattern.matches("/users/"));
		assert!(!pattern.matches("/users/123/edit/"));
	}

	#[test]
	fn test_matches_static_url() {
		let pattern = UrlPattern::new("home", "/", None);
		assert!(pattern.matches("/"));
		assert!(!pattern.matches("/about/"));
	}
}
