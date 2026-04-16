//! Lightweight, thread-safe URL reverser for client-side named routes.
//!
//! [`ClientUrlReverser`] holds only the route-name-to-pattern mapping extracted
//! from a [`ClientRouter`]. Unlike `ClientRouter`, it is `Send + Sync` and
//! contains no reactive signals or component references.

use std::collections::HashMap;

/// Thread-safe reverse URL resolver for client-side routes.
///
/// Extracted from [`ClientRouter`] via [`ClientRouter::to_reverser()`] to
/// provide URL generation without carrying reactive state or handler
/// references.
///
/// # Example
///
/// ```rust
/// use std::collections::HashMap;
/// use reinhardt_urls::routers::client_router::ClientUrlReverser;
///
/// let mut patterns = HashMap::new();
/// patterns.insert("app:home".to_string(), "/".to_string());
/// patterns.insert("app:user".to_string(), "/users/{id}/".to_string());
///
/// let reverser = ClientUrlReverser::new(patterns);
/// assert_eq!(reverser.reverse("app:home", &[]), Some("/".to_string()));
/// assert_eq!(
///     reverser.reverse("app:user", &[("id", "42")]),
///     Some("/users/42/".to_string()),
/// );
/// ```
///
/// [`ClientRouter`]: super::ClientRouter
/// [`ClientRouter::to_reverser()`]: super::ClientRouter::to_reverser
#[derive(Debug, Clone)]
pub struct ClientUrlReverser {
	named_patterns: HashMap<String, String>,
}

impl ClientUrlReverser {
	/// Create from a map of route names to URL patterns.
	pub fn new(named_patterns: HashMap<String, String>) -> Self {
		Self { named_patterns }
	}

	/// Reverse a named route with the given parameters.
	///
	/// Returns `None` if the route name is not found or if required
	/// parameters are missing (i.e., unreplaced `{param}` placeholders remain).
	///
	/// # Panics
	///
	/// Panics if a parameter value contains path separators or other
	/// injection characters.
	pub fn reverse(&self, name: &str, params: &[(&str, &str)]) -> Option<String> {
		let pattern = self.named_patterns.get(name)?;
		let param_map: HashMap<String, String> = params
			.iter()
			.map(|(k, v)| (k.to_string(), v.to_string()))
			.collect();
		let result = Self::substitute(pattern, &param_map);
		// If any placeholder was not replaced, the params were incomplete
		if result.contains('{') {
			return None;
		}
		Some(result)
	}

	/// Substitute `{param}` placeholders in `pattern` with values from `params`.
	///
	/// Validates parameter values against path-separator injection.
	/// Unmatched placeholders are preserved verbatim.
	fn substitute(pattern: &str, params: &HashMap<String, String>) -> String {
		// Validate parameter values
		for (name, value) in params {
			if value.contains('/')
				|| value.contains('\\')
				|| value.contains('?')
				|| value.contains('#')
				|| value.contains('%')
			{
				panic!(
					"Invalid parameter value for '{}': contains dangerous characters \
                     (path separators, query delimiters, or encoded sequences)",
					name
				);
			}
		}

		let mut result = String::with_capacity(pattern.len());
		let mut chars = pattern.chars().peekable();

		while let Some(ch) = chars.next() {
			if ch == '{' {
				let param_name: String = chars.by_ref().take_while(|&c| c != '}').collect();
				if let Some(value) = params.get(&param_name) {
					result.push_str(value);
				} else {
					result.push('{');
					result.push_str(&param_name);
					result.push('}');
				}
			} else {
				result.push(ch);
			}
		}

		result
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::*;

	#[fixture]
	fn reverser() -> ClientUrlReverser {
		let mut patterns = HashMap::new();
		patterns.insert("auth:login_page".to_string(), "/login/".to_string());
		patterns.insert("auth:user_detail".to_string(), "/users/{id}/".to_string());
		patterns.insert(
			"auth:user_posts".to_string(),
			"/users/{user_id}/posts/{post_id}/".to_string(),
		);
		ClientUrlReverser::new(patterns)
	}

	#[rstest]
	fn test_reverse_no_params(reverser: ClientUrlReverser) {
		// Act
		let url = reverser.reverse("auth:login_page", &[]);

		// Assert
		assert_eq!(url, Some("/login/".to_string()));
	}

	#[rstest]
	fn test_reverse_single_param(reverser: ClientUrlReverser) {
		// Act
		let url = reverser.reverse("auth:user_detail", &[("id", "42")]);

		// Assert
		assert_eq!(url, Some("/users/42/".to_string()));
	}

	#[rstest]
	fn test_reverse_multiple_params(reverser: ClientUrlReverser) {
		// Act
		let url = reverser.reverse("auth:user_posts", &[("user_id", "5"), ("post_id", "10")]);

		// Assert
		assert_eq!(url, Some("/users/5/posts/10/".to_string()));
	}

	#[rstest]
	fn test_reverse_unknown_route(reverser: ClientUrlReverser) {
		// Act
		let url = reverser.reverse("nonexistent", &[]);

		// Assert
		assert_eq!(url, None);
	}

	#[rstest]
	fn test_reverse_missing_params_returns_none(reverser: ClientUrlReverser) {
		// Act — route requires {id} but no params provided
		let url = reverser.reverse("auth:user_detail", &[]);

		// Assert
		assert_eq!(url, None);
	}
}
