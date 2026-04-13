//! URL pattern matching for request handlers.

use std::collections::HashMap;

use regex::Regex;

/// URL pattern matching for request interception.
#[derive(Debug)]
pub enum UrlMatcher {
	/// Exact path match (query string is stripped before comparison).
	Exact(String),
	/// Parameterized path segments (e.g., `/api/users/:id`).
	Parameterized {
		/// The parsed segments of the parameterized URL pattern.
		segments: Vec<Segment>,
	},
	/// Regular expression match.
	Regex(Regex),
}

/// A single segment of a parameterized URL pattern.
#[derive(Debug, Clone)]
pub enum Segment {
	/// Literal text (e.g., `"api"`, `"users"`).
	Literal(String),
	/// Named parameter (e.g., `"id"` from `:id`).
	Param(String),
}

/// Extract the path portion from a URL string.
///
/// Handles both relative paths (`/api/users`) and full URLs
/// (`http://localhost:8080/api/users?page=1`), stripping scheme, host, and query.
pub(crate) fn extract_path(url: &str) -> &str {
	// Strip query string first
	let without_query = url.split('?').next().unwrap_or(url);
	// If it starts with http:// or https://, extract path after host
	if let Some(rest) = without_query.strip_prefix("http://") {
		rest.find('/').map_or("", |i| &rest[i..])
	} else if let Some(rest) = without_query.strip_prefix("https://") {
		rest.find('/').map_or("", |i| &rest[i..])
	} else {
		without_query
	}
}

impl UrlMatcher {
	/// Returns true if the given URL matches this pattern.
	///
	/// Handles both relative paths (`/api/users`) and full URLs
	/// (`http://localhost:8080/api/users?page=1`).
	pub fn matches(&self, url: &str) -> bool {
		let path = extract_path(url);
		match self {
			UrlMatcher::Exact(expected) => path == expected,
			UrlMatcher::Parameterized { segments } => {
				let url_segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
				if url_segments.len() != segments.len() {
					return false;
				}
				url_segments
					.iter()
					.zip(segments.iter())
					.all(|(url_seg, pat_seg)| match pat_seg {
						Segment::Literal(lit) => url_seg == lit,
						Segment::Param(_) => true,
					})
			}
			UrlMatcher::Regex(re) => re.is_match(path),
		}
	}

	/// Extracts named parameters from a URL. Returns empty map if not parameterized.
	pub fn extract_params(&self, url: &str) -> HashMap<String, String> {
		let path = extract_path(url);
		let mut params = HashMap::new();
		if let UrlMatcher::Parameterized { segments } = self {
			let url_segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
			for (url_seg, pat_seg) in url_segments.iter().zip(segments.iter()) {
				if let Segment::Param(name) = pat_seg {
					params.insert(name.clone(), url_seg.to_string());
				}
			}
		}
		params
	}
}

impl From<&str> for UrlMatcher {
	fn from(pattern: &str) -> Self {
		// Normalize the pattern using the same path-extraction logic used during matching,
		// then check for parameterized segments (`:name`) in path segments only.
		let path = extract_path(pattern);
		let has_params = path
			.split('/')
			.filter(|s| !s.is_empty())
			.any(|s| s.starts_with(':'));
		if has_params {
			let segments = path
				.split('/')
				.filter(|s| !s.is_empty())
				.map(|s| {
					if let Some(name) = s.strip_prefix(':') {
						Segment::Param(name.to_string())
					} else {
						Segment::Literal(s.to_string())
					}
				})
				.collect();
			UrlMatcher::Parameterized { segments }
		} else {
			UrlMatcher::Exact(path.to_string())
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::*;

	#[rstest]
	fn exact_match_succeeds() {
		let matcher = UrlMatcher::Exact("/api/users".to_string());
		assert!(matcher.matches("/api/users"));
	}

	#[rstest]
	fn exact_match_fails_on_mismatch() {
		let matcher = UrlMatcher::Exact("/api/users".to_string());
		assert!(!matcher.matches("/api/posts"));
	}

	#[rstest]
	fn exact_match_strips_query_string() {
		let matcher = UrlMatcher::Exact("/api/users".to_string());
		assert!(matcher.matches("/api/users?page=1"));
	}

	#[rstest]
	fn parameterized_match_succeeds() {
		let matcher: UrlMatcher = "/api/users/:id".into();
		assert!(matcher.matches("/api/users/42"));
	}

	#[rstest]
	fn parameterized_match_extracts_params() {
		let matcher: UrlMatcher = "/api/users/:id/posts/:post_id".into();
		let params = matcher.extract_params("/api/users/42/posts/7");
		assert_eq!(params.get("id"), Some(&"42".to_string()));
		assert_eq!(params.get("post_id"), Some(&"7".to_string()));
	}

	#[rstest]
	fn parameterized_match_fails_on_segment_count_mismatch() {
		let matcher: UrlMatcher = "/api/users/:id".into();
		assert!(!matcher.matches("/api/users"));
		assert!(!matcher.matches("/api/users/42/extra"));
	}

	#[rstest]
	fn regex_match_succeeds() {
		let matcher = UrlMatcher::Regex(Regex::new(r"/api/users/\d+").unwrap());
		assert!(matcher.matches("/api/users/42"));
		assert!(!matcher.matches("/api/users/abc"));
	}

	#[rstest]
	fn full_url_matches_path_pattern() {
		let matcher = UrlMatcher::Exact("/api/users".to_string());
		assert!(matcher.matches("http://127.0.0.1:8080/api/users"));
		assert!(matcher.matches("https://example.com/api/users?page=1"));
		assert!(!matcher.matches("http://localhost/api/posts"));
	}

	#[rstest]
	fn from_str_exact() {
		let matcher: UrlMatcher = "/api/users".into();
		assert!(matches!(matcher, UrlMatcher::Exact(_)));
	}

	#[rstest]
	fn from_str_parameterized() {
		let matcher: UrlMatcher = "/api/users/:id".into();
		assert!(matches!(matcher, UrlMatcher::Parameterized { .. }));
	}

	#[rstest]
	fn from_str_full_url_not_misclassified_as_parameterized() {
		// The colon in `https:` must not trigger Parameterized classification.
		let matcher: UrlMatcher = "https://example.com/api/users".into();
		assert!(matches!(matcher, UrlMatcher::Exact(_)));
		assert!(matcher.matches("/api/users"));
	}

	#[rstest]
	fn from_str_full_url_with_params() {
		let matcher: UrlMatcher = "https://example.com/api/users/:id".into();
		assert!(matches!(matcher, UrlMatcher::Parameterized { .. }));
		assert!(matcher.matches("/api/users/42"));
	}
}
