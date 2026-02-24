/// URL reverse resolution
/// Inspired by Django's django.urls.reverse() function
///
/// This module provides both string-based (runtime) and type-safe (compile-time)
/// URL reversal mechanisms.
// use crate::path;
use super::pattern::validate_reverse_param;
use super::{PathPattern, Route};
use aho_corasick::AhoCorasick;
use reinhardt_core::exception::{Error, Result};
use std::collections::HashMap;
use std::marker::PhantomData;

pub type ReverseError = Error;
pub type ReverseResult<T> = Result<T>;

/// Optimized URL parameter substitution using Aho-Corasick algorithm
///
/// This function uses Aho-Corasick for multi-pattern matching, allowing
/// simultaneous detection of all placeholders in a single pass.
///
/// # Algorithm
///
/// 1. Extract all placeholder names from the pattern
/// 2. Build Aho-Corasick automaton for all placeholders (one-time construction)
/// 3. Find all placeholder positions in O(n+z) where z is number of matches
/// 4. Replace placeholders from right to left to avoid position shifts
///
/// # Performance
///
/// - Time complexity: O(n+m+z) where:
///   - n: pattern length
///   - m: total parameter values length
///   - z: number of placeholder matches
/// - Expected improvement: 3-5x for patterns with 10+ parameters
///
/// # Arguments
///
/// * `pattern` - URL pattern with placeholders like "/users/{id}/posts/{post_id}/"
/// * `params` - HashMap of parameter names to values
///
/// # Examples
///
/// ```
/// use std::collections::HashMap;
/// use reinhardt_urls::routers::reverse::reverse_with_aho_corasick;
///
/// let mut params = HashMap::new();
/// params.insert("id".to_string(), "123".to_string());
/// params.insert("post_id".to_string(), "456".to_string());
///
/// let url = reverse_with_aho_corasick("/users/{id}/posts/{post_id}/", &params);
/// assert_eq!(url, "/users/123/posts/456/");
/// ```
pub fn reverse_with_aho_corasick(pattern: &str, params: &HashMap<String, String>) -> String {
	// Extract all placeholder names
	let param_names = extract_param_names(pattern);

	if param_names.is_empty() {
		return pattern.to_string();
	}

	// Validate parameter values against injection attacks
	for (name, value) in params {
		if !validate_reverse_param(value) {
			panic!(
				"Invalid parameter value for '{}': contains dangerous characters (path separators, query delimiters, or encoded sequences)",
				name
			);
		}
	}

	// Build patterns for Aho-Corasick: ["{id}", "{post_id}", ...]
	let placeholders: Vec<String> = param_names
		.iter()
		.map(|name| format!("{{{}}}", name))
		.collect();

	// Build Aho-Corasick automaton
	let ac = match AhoCorasick::new(&placeholders) {
		Ok(ac) => ac,
		Err(_) => {
			// Fallback to original implementation if AC construction fails
			return reverse_single_pass(pattern, params);
		}
	};

	// Find all matches
	let mut replacements = Vec::new();
	for mat in ac.find_iter(pattern) {
		let param_name = &param_names[mat.pattern()];
		if let Some(value) = params.get(param_name) {
			replacements.push((mat.start(), mat.end(), value.clone()));
		} else {
			// Keep placeholder if parameter not found
			replacements.push((mat.start(), mat.end(), format!("{{{}}}", param_name)));
		}
	}

	// Apply replacements from right to left to avoid position shifts
	let mut result = pattern.to_string();
	for (start, end, value) in replacements.into_iter().rev() {
		result.replace_range(start..end, &value);
	}

	result
}

/// Extract parameter names from a URL pattern
///
/// # Examples
///
/// ```
/// use reinhardt_urls::routers::reverse::extract_param_names;
///
/// let names = extract_param_names("/users/{id}/posts/{post_id}/");
/// assert_eq!(names, vec!["id", "post_id"]);
/// ```
pub fn extract_param_names(pattern: &str) -> Vec<String> {
	let mut names = Vec::new();
	let mut chars = pattern.chars().peekable();

	while let Some(ch) = chars.next() {
		if ch == '{' {
			let name: String = chars.by_ref().take_while(|&c| c != '}').collect();
			if !name.is_empty() {
				names.push(name);
			}
		}
	}

	names
}

/// Single-pass URL parameter substitution algorithm
///
/// This function performs placeholder substitution in O(n+m) time complexity,
/// where n is the length of the pattern and m is the total length of parameter values.
///
/// # Algorithm
///
/// 1. Iterate through pattern characters once (O(n))
/// 2. When encountering '{', extract parameter name until '}'
/// 3. Lookup parameter value in HashMap (O(1) amortized)
/// 4. Append value to result string
///
/// # Performance
///
/// - Old algorithm: O(n×m×p) where p is number of parameters
/// - New algorithm: O(n+m) where m is total length of parameter values
/// - Expected improvement: 10-50x for patterns with multiple parameters
///
/// # Arguments
///
/// * `pattern` - URL pattern with placeholders like "/users/{id}/posts/{post_id}/"
/// * `params` - HashMap of parameter names to values
///
/// # Examples
///
/// ```
/// use std::collections::HashMap;
/// use reinhardt_urls::routers::reverse::reverse_single_pass;
///
/// let mut params = HashMap::new();
/// params.insert("id".to_string(), "123".to_string());
/// params.insert("post_id".to_string(), "456".to_string());
///
/// let url = reverse_single_pass("/users/{id}/posts/{post_id}/", &params);
/// assert_eq!(url, "/users/123/posts/456/");
/// ```
pub fn reverse_single_pass(pattern: &str, params: &HashMap<String, String>) -> String {
	// Validate parameter values against injection attacks
	for (name, value) in params {
		if !validate_reverse_param(value) {
			panic!(
				"Invalid parameter value for '{}': contains dangerous characters (path separators, query delimiters, or encoded sequences)",
				name
			);
		}
	}

	let mut result = String::with_capacity(pattern.len());
	let mut chars = pattern.chars().peekable();

	while let Some(ch) = chars.next() {
		if ch == '{' {
			// Extract parameter name until '}'
			let param_name: String = chars.by_ref().take_while(|&c| c != '}').collect();

			// Lookup parameter value (O(1) amortized)
			if let Some(value) = params.get(&param_name) {
				result.push_str(value);
			} else {
				// Parameter not found - preserve placeholder
				// This should not happen if validation was done beforehand
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

/// URL reverser for resolving names back to URLs
/// Similar to Django's URLResolver reverse functionality
pub struct UrlReverser {
	/// Map of route names (including namespace) to routes
	routes: HashMap<String, Route>,
}

impl UrlReverser {
	pub fn new() -> Self {
		Self {
			routes: HashMap::new(),
		}
	}

	/// Register a route for reverse lookup
	pub fn register(&mut self, route: Route) {
		if let Some(full_name) = route.full_name() {
			self.routes.insert(full_name, route);
		}
	}

	/// Register a route by name and path (without handler)
	///
	/// This is used for hierarchical routers where we only need the name-to-path mapping
	/// for URL reversal, not the actual handler.
	///
	/// # Arguments
	///
	/// * `name` - The fully qualified route name (e.g., "v1:users:detail")
	/// * `path` - The URL path pattern (e.g., "/users/{id}/")
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_urls::routers::UrlReverser;
	///
	/// let mut reverser = UrlReverser::new();
	/// reverser.register_path("v1:users:detail", "/api/v1/users/{id}/");
	///
	/// let url = reverser.reverse_with("v1:users:detail", &[("id", "123")]).unwrap();
	/// assert_eq!(url, "/api/v1/users/123/");
	/// ```
	pub fn register_path(&mut self, name: &str, path: &str) {
		// Create a dummy handler for the route
		// The handler is never used for URL reversal
		use reinhardt_http::Handler;
		use std::sync::Arc;

		#[derive(Clone)]
		struct DummyHandler;

		#[async_trait::async_trait]
		impl Handler for DummyHandler {
			async fn handle(
				&self,
				_req: reinhardt_http::Request,
			) -> reinhardt_core::exception::Result<reinhardt_http::Response> {
				unreachable!("DummyHandler should never be called")
			}
		}

		// Parse the name to extract namespace (if any)
		let parts: Vec<&str> = name.rsplitn(2, ':').collect();
		let (route_name, namespace) = if parts.len() == 2 {
			(parts[0].to_string(), Some(parts[1].to_string()))
		} else {
			(name.to_string(), None)
		};

		let route = Route::new(path, Arc::new(DummyHandler)).with_name(&route_name);

		let route = if let Some(ns) = namespace {
			route.with_namespace(&ns)
		} else {
			route
		};

		self.routes.insert(name.to_string(), route);
	}

	/// Reverse a URL name to a path with parameters
	/// Similar to Django's reverse() function
	///
	/// # Arguments
	///
	/// * `name` - The route name, optionally with namespace (e.g., "users:detail")
	/// * `params` - Map of parameter names to values
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_urls::routers::{UrlReverser, Route};
	/// use reinhardt_http::Handler;
	/// use std::sync::Arc;
	/// use std::collections::HashMap;
	///
	/// # use async_trait::async_trait;
	/// # use reinhardt_http::{Request, Response, Result};
	/// # struct DummyHandler;
	/// # #[async_trait]
	/// # impl Handler for DummyHandler {
	/// #     async fn handle(&self, _req: Request) -> Result<Response> {
	/// #         Ok(Response::ok())
	/// #     }
	/// # }
	/// let handler = Arc::new(DummyHandler);
	/// let mut reverser = UrlReverser::new();
	/// let route = Route::new("/users/{id}/", handler)
	///     .with_name("detail")
	///     .with_namespace("users");
	/// reverser.register(route);
	///
	/// let mut params = HashMap::new();
	/// params.insert("id".to_string(), "123".to_string());
	///
	/// let url = reverser.reverse("users:detail", &params).unwrap();
	/// assert_eq!(url, "/users/123/");
	/// ```
	pub fn reverse(&self, name: &str, params: &HashMap<String, String>) -> ReverseResult<String> {
		let route = self
			.routes
			.get(name)
			.ok_or_else(|| Error::NotFound(name.to_string()))?;

		// Parse the path pattern to find parameters
		let pattern = PathPattern::new(&route.path)
			.map_err(|e| Error::Validation(format!("pattern: {}", e)))?;

		// Validate all required parameters are present before substitution
		for param_name in pattern.param_names() {
			if !params.contains_key(param_name) {
				return Err(Error::Validation(format!("missing param: {}", param_name)));
			}
		}

		// Validate parameter values against injection attacks
		for (name, value) in params {
			if !validate_reverse_param(value) {
				return Err(Error::Validation(format!(
					"invalid param '{}': contains dangerous characters",
					name
				)));
			}
		}

		// Use single-pass substitution algorithm: O(n+m) instead of O(n×m×p)
		Ok(reverse_single_pass(&route.path, params))
	}

	/// Reverse a URL name to a path with positional parameters
	/// Convenience method that takes a slice of key-value pairs
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_urls::routers::{UrlReverser, Route};
	/// use reinhardt_http::Handler;
	/// use std::sync::Arc;
	///
	/// # use async_trait::async_trait;
	/// # use reinhardt_http::{Request, Response, Result};
	/// # struct DummyHandler;
	/// # #[async_trait]
	/// # impl Handler for DummyHandler {
	/// #     async fn handle(&self, _req: Request) -> Result<Response> {
	/// #         Ok(Response::ok())
	/// #     }
	/// # }
	/// let handler = Arc::new(DummyHandler);
	/// let mut reverser = UrlReverser::new();
	/// let route = Route::new("/users/{id}/", handler)
	///     .with_name("detail");
	/// reverser.register(route);
	///
	/// let url = reverser.reverse_with("detail", &[("id", "123")]).unwrap();
	/// assert_eq!(url, "/users/123/");
	/// ```
	pub fn reverse_with<S: AsRef<str>>(
		&self,
		name: &str,
		params: &[(S, S)],
	) -> ReverseResult<String> {
		let params_map: HashMap<String, String> = params
			.iter()
			.map(|(k, v)| (k.as_ref().to_string(), v.as_ref().to_string()))
			.collect();

		self.reverse(name, &params_map)
	}

	/// Check if a route name is registered
	pub fn has_route(&self, name: &str) -> bool {
		self.routes.contains_key(name)
	}

	/// Get all registered route names
	pub fn route_names(&self) -> Vec<String> {
		self.routes.keys().cloned().collect()
	}
}

impl Default for UrlReverser {
	fn default() -> Self {
		Self::new()
	}
}

/// Standalone reverse function for convenience
/// Similar to Django's reverse() function
///
/// This requires routes to be registered with a global reverser.
/// For more control, use UrlReverser directly.
pub fn reverse(
	name: &str,
	params: &HashMap<String, String>,
	reverser: &UrlReverser,
) -> ReverseResult<String> {
	reverser.reverse(name, params)
}

// ============================================================================
// Type-safe URL reversal (compile-time checked)
// ============================================================================

/// Trait for URL patterns that can be reversed at compile time
///
/// Implement this trait for each URL pattern in your application.
/// The compiler will ensure that only valid URL patterns can be reversed.
///
/// # Example
///
/// ```rust
/// use reinhardt_urls::routers::reverse::UrlPattern;
///
/// pub struct UserListUrl;
/// impl UrlPattern for UserListUrl {
///     const NAME: &'static str = "user-list";
///     const PATTERN: &'static str = "/users/";
/// }
/// ```
pub trait UrlPattern {
	/// The unique name for this URL pattern
	const NAME: &'static str;

	/// The URL pattern string
	const PATTERN: &'static str;
}

/// Trait for URL patterns with parameters
///
/// Use this for URLs that require path parameters.
///
/// # Example
///
/// ```rust
/// use reinhardt_urls::routers::reverse::{UrlPattern, UrlPatternWithParams};
///
/// pub struct UserDetailUrl;
/// impl UrlPattern for UserDetailUrl {
///     const NAME: &'static str = "user-detail";
///     const PATTERN: &'static str = "/users/{id}/";
/// }
/// impl UrlPatternWithParams for UserDetailUrl {
///     const PARAMS: &'static [&'static str] = &["id"];
/// }
/// ```
pub trait UrlPatternWithParams: UrlPattern {
	/// The parameter names in order
	const PARAMS: &'static [&'static str];
}

/// Type-safe reverse for simple URL patterns (no parameters)
///
/// This function takes a type parameter implementing `UrlPattern`
/// and returns the URL string. Invalid patterns will fail at compile time.
///
/// # Example
///
/// ```rust
/// use reinhardt_urls::routers::reverse::{reverse_typed, UrlPattern};
///
/// pub struct HomeUrl;
/// impl UrlPattern for HomeUrl {
///     const NAME: &'static str = "home";
///     const PATTERN: &'static str = "/";
/// }
///
/// let url = reverse_typed::<HomeUrl>();
/// assert_eq!(url, "/");
/// ```
pub fn reverse_typed<U: UrlPattern>() -> String {
	U::PATTERN.to_string()
}

/// Type-safe reverse for URL patterns with parameters
///
/// This function takes a type parameter and a HashMap of parameters,
/// substituting them into the URL pattern. Missing parameters will
/// result in a runtime error, but the pattern itself is compile-time checked.
///
/// # Example
///
/// ```rust
/// use reinhardt_urls::routers::reverse::{reverse_typed_with_params, UrlPattern, UrlPatternWithParams};
/// use std::collections::HashMap;
///
/// pub struct UserDetailUrl;
/// impl UrlPattern for UserDetailUrl {
///     const NAME: &'static str = "user-detail";
///     const PATTERN: &'static str = "/users/{id}/";
/// }
/// impl UrlPatternWithParams for UserDetailUrl {
///     const PARAMS: &'static [&'static str] = &["id"];
/// }
///
/// let mut params = HashMap::new();
/// params.insert("id", "123");
/// let url = reverse_typed_with_params::<UserDetailUrl>(&params).unwrap();
/// assert_eq!(url, "/users/123/");
/// ```
pub fn reverse_typed_with_params<U: UrlPatternWithParams>(
	params: &HashMap<&str, &str>,
) -> ReverseResult<String> {
	// Validate that all required parameters are provided
	for param_name in U::PARAMS {
		if !params.contains_key(param_name) {
			return Err(ReverseError::MissingParameter(param_name.to_string()));
		}
	}

	// Validate parameter values against injection attacks
	for (name, value) in params {
		if !validate_reverse_param(value) {
			return Err(ReverseError::Validation(format!(
				"invalid param '{}': contains dangerous characters",
				name
			)));
		}
	}

	// Convert &str HashMap to String HashMap for single-pass algorithm
	let string_params: HashMap<String, String> = params
		.iter()
		.map(|(k, v)| (k.to_string(), v.to_string()))
		.collect();

	Ok(reverse_single_pass(U::PATTERN, &string_params))
}

/// Type-safe URL parameter builder
///
/// Provides a fluent API for building URL parameters with compile-time checking
/// of parameter names.
///
/// # Example
///
/// ```rust
/// use reinhardt_urls::routers::reverse::{UrlParams, UrlPattern, UrlPatternWithParams};
///
/// pub struct UserDetailUrl;
/// impl UrlPattern for UserDetailUrl {
///     const NAME: &'static str = "user-detail";
///     const PATTERN: &'static str = "/users/{id}/";
/// }
/// impl UrlPatternWithParams for UserDetailUrl {
///     const PARAMS: &'static [&'static str] = &["id"];
/// }
///
/// let params = UrlParams::<UserDetailUrl>::new()
///     .param("id", "123")
///     .build()
///     .unwrap();
///
/// assert_eq!(params, "/users/123/");
/// ```
pub struct UrlParams<U: UrlPatternWithParams> {
	_phantom: PhantomData<U>,
	params: HashMap<String, String>,
}

impl<U: UrlPatternWithParams> UrlParams<U> {
	/// Create a new URL parameter builder
	pub fn new() -> Self {
		Self {
			_phantom: PhantomData,
			params: HashMap::new(),
		}
	}

	/// Add a parameter (note: parameter name is not compile-time checked currently,
	/// but the pattern itself is)
	pub fn param(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
		self.params.insert(name.into(), value.into());
		self
	}

	/// Build the URL string, checking that all required parameters are present
	pub fn build(self) -> ReverseResult<String> {
		let params_ref: HashMap<&str, &str> = self
			.params
			.iter()
			.map(|(k, v)| (k.as_str(), v.as_str()))
			.collect();

		reverse_typed_with_params::<U>(&params_ref)
	}
}

impl<U: UrlPatternWithParams> Default for UrlParams<U> {
	fn default() -> Self {
		Self::new()
	}
}

#[cfg(test)]
mod tests {
	use super::Route;
	use super::*;
	use crate::routers_macros::path;
	use async_trait::async_trait;
	use reinhardt_http::{Handler, Request, Response, Result as CoreResult};
	use std::sync::Arc;

	// Simple test handler
	struct TestHandler;

	#[async_trait]
	impl Handler for TestHandler {
		async fn handle(&self, _request: Request) -> CoreResult<Response> {
			Ok(Response::ok())
		}
	}

	#[test]
	fn test_reverse_simple_path() {
		let mut reverser = UrlReverser::new();

		let route = Route::new(path!("/users/"), Arc::new(TestHandler)).with_name("users-list");

		reverser.register(route);

		let url = reverser.reverse("users-list", &HashMap::new()).unwrap();
		assert_eq!(url, path!("/users/"));
	}

	#[test]
	fn test_reverse_with_parameters() {
		let mut reverser = UrlReverser::new();

		let route =
			Route::new(path!("/users/{id}/"), Arc::new(TestHandler)).with_name("users-detail");

		reverser.register(route);

		let mut params = HashMap::new();
		params.insert("id".to_string(), "123".to_string());

		let url = reverser.reverse("users-detail", &params).unwrap();
		assert_eq!(url, "/users/123/");
	}

	#[test]
	fn test_reverse_with_namespace() {
		let mut reverser = UrlReverser::new();

		let route = Route::new(path!("/users/{id}/"), Arc::new(TestHandler))
			.with_name("detail")
			.with_namespace("users");

		reverser.register(route);

		let mut params = HashMap::new();
		params.insert("id".to_string(), "456".to_string());

		let url = reverser.reverse("users:detail", &params).unwrap();
		assert_eq!(url, "/users/456/");
	}

	#[test]
	fn test_reverse_missing_parameter() {
		let mut reverser = UrlReverser::new();

		let route =
			Route::new(path!("/users/{id}/"), Arc::new(TestHandler)).with_name("users-detail");

		reverser.register(route);

		let result = reverser.reverse("users-detail", &HashMap::new());
		assert!(result.is_err());
		assert!(matches!(result.unwrap_err(), ReverseError::Validation(_)));
	}

	#[test]
	fn test_reverse_not_found() {
		let reverser = UrlReverser::new();

		let result = reverser.reverse("nonexistent", &HashMap::new());
		assert!(result.is_err());
		assert!(matches!(result.unwrap_err(), ReverseError::NotFound(_)));
	}

	#[test]
	fn test_reverse_with_helper() {
		let mut reverser = UrlReverser::new();

		let route = Route::new(path!("/users/{id}/posts/{post_id}/"), Arc::new(TestHandler))
			.with_name("user-posts");

		reverser.register(route);

		let url = reverser
			.reverse_with("user-posts", &[("id", "123"), ("post_id", "456")])
			.unwrap();

		assert_eq!(url, "/users/123/posts/456/");
	}

	#[test]
	fn test_has_route() {
		let mut reverser = UrlReverser::new();

		let route = Route::new(path!("/users/"), Arc::new(TestHandler)).with_name("users-list");

		reverser.register(route);

		assert!(reverser.has_route("users-list"));
		assert!(!reverser.has_route("nonexistent"));
	}

	// Type-safe URL reversal tests
	struct HomeUrl;
	impl UrlPattern for HomeUrl {
		const NAME: &'static str = "home";
		const PATTERN: &'static str = reinhardt_routers_macros::path!("/");
	}

	struct UserListUrl;
	impl UrlPattern for UserListUrl {
		const NAME: &'static str = "user-list";
		const PATTERN: &'static str = reinhardt_routers_macros::path!("/users/");
	}

	struct UserDetailUrl;
	impl UrlPattern for UserDetailUrl {
		const NAME: &'static str = "user-detail";
		const PATTERN: &'static str = reinhardt_routers_macros::path!("/users/{id}/");
	}
	impl UrlPatternWithParams for UserDetailUrl {
		const PARAMS: &'static [&'static str] = &["id"];
	}

	struct PostDetailUrl;
	impl UrlPattern for PostDetailUrl {
		const NAME: &'static str = "post-detail";
		const PATTERN: &'static str =
			reinhardt_routers_macros::path!("/users/{user_id}/posts/{post_id}/");
	}
	impl UrlPatternWithParams for PostDetailUrl {
		const PARAMS: &'static [&'static str] = &["user_id", "post_id"];
	}

	#[test]
	fn test_typed_reverse_simple() {
		let url = reverse_typed::<HomeUrl>();
		assert_eq!(url, path!("/"));
	}

	#[test]
	fn test_typed_reverse_user_list() {
		let url = reverse_typed::<UserListUrl>();
		assert_eq!(url, path!("/users/"));
	}

	#[test]
	fn test_typed_reverse_with_params() {
		let mut params = HashMap::new();
		params.insert("id", "123");

		let url = reverse_typed_with_params::<UserDetailUrl>(&params).unwrap();
		assert_eq!(url, "/users/123/");
	}

	#[test]
	fn test_typed_reverse_with_multiple_params() {
		let mut params = HashMap::new();
		params.insert("user_id", "42");
		params.insert("post_id", "100");

		let url = reverse_typed_with_params::<PostDetailUrl>(&params).unwrap();
		assert_eq!(url, "/users/42/posts/100/");
	}

	#[test]
	fn test_typed_reverse_missing_param() {
		let params = HashMap::new();

		let result = reverse_typed_with_params::<UserDetailUrl>(&params);
		assert!(result.is_err());

		if let Err(ReverseError::MissingParameter(param)) = result {
			assert_eq!(param, "id");
		}
	}

	#[test]
	fn test_url_params_builder() {
		let url = UrlParams::<UserDetailUrl>::new()
			.param("id", "456")
			.build()
			.unwrap();

		assert_eq!(url, "/users/456/");
	}

	#[test]
	fn test_url_params_builder_multiple() {
		let url = UrlParams::<PostDetailUrl>::new()
			.param("user_id", "42")
			.param("post_id", "100")
			.build()
			.unwrap();

		assert_eq!(url, "/users/42/posts/100/");
	}

	#[test]
	fn test_url_params_builder_missing() {
		let result = UrlParams::<UserDetailUrl>::new().build();

		assert!(result.is_err());
		assert!(matches!(
			result.unwrap_err(),
			ReverseError::MissingParameter(_)
		));
	}

	// Single-pass algorithm tests
	#[test]
	fn test_single_pass_basic() {
		let mut params = HashMap::new();
		params.insert("id".to_string(), "123".to_string());

		let result = reverse_single_pass("/users/{id}/", &params);
		assert_eq!(result, "/users/123/");
	}

	#[test]
	fn test_single_pass_multiple_params() {
		let mut params = HashMap::new();
		params.insert("user_id".to_string(), "42".to_string());
		params.insert("post_id".to_string(), "100".to_string());

		let result = reverse_single_pass("/users/{user_id}/posts/{post_id}/", &params);
		assert_eq!(result, "/users/42/posts/100/");
	}

	#[test]
	fn test_single_pass_many_params() {
		// Test with 10+ parameters to demonstrate performance improvement
		let mut params = HashMap::new();
		params.insert("p1".to_string(), "v1".to_string());
		params.insert("p2".to_string(), "v2".to_string());
		params.insert("p3".to_string(), "v3".to_string());
		params.insert("p4".to_string(), "v4".to_string());
		params.insert("p5".to_string(), "v5".to_string());
		params.insert("p6".to_string(), "v6".to_string());
		params.insert("p7".to_string(), "v7".to_string());
		params.insert("p8".to_string(), "v8".to_string());
		params.insert("p9".to_string(), "v9".to_string());
		params.insert("p10".to_string(), "v10".to_string());

		let pattern = "/api/{p1}/{p2}/{p3}/{p4}/{p5}/{p6}/{p7}/{p8}/{p9}/{p10}/";
		let result = reverse_single_pass(pattern, &params);
		assert_eq!(result, "/api/v1/v2/v3/v4/v5/v6/v7/v8/v9/v10/");
	}

	#[test]
	fn test_single_pass_missing_param() {
		let params = HashMap::new();

		let result = reverse_single_pass("/users/{id}/", &params);
		// Missing parameter should preserve placeholder
		assert_eq!(result, "/users/{id}/");
	}

	#[test]
	fn test_single_pass_no_params() {
		let params = HashMap::new();

		let result = reverse_single_pass("/users/", &params);
		assert_eq!(result, "/users/");
	}

	#[test]
	fn test_single_pass_empty_pattern() {
		let params = HashMap::new();

		let result = reverse_single_pass("", &params);
		assert_eq!(result, "");
	}

	#[test]
	fn test_single_pass_consecutive_params() {
		let mut params = HashMap::new();
		params.insert("a".to_string(), "1".to_string());
		params.insert("b".to_string(), "2".to_string());

		let result = reverse_single_pass("/{a}{b}/", &params);
		assert_eq!(result, "/12/");
	}

	#[test]
	fn test_single_pass_special_chars_in_values() {
		let mut params = HashMap::new();
		params.insert("id".to_string(), "foo-bar_123".to_string());

		let result = reverse_single_pass("/items/{id}/", &params);
		assert_eq!(result, "/items/foo-bar_123/");
	}

	#[test]
	fn test_single_pass_numeric_values() {
		let mut params = HashMap::new();
		params.insert("id".to_string(), "12345".to_string());

		let result = reverse_single_pass("/items/{id}/", &params);
		assert_eq!(result, "/items/12345/");
	}

	#[test]
	fn test_single_pass_empty_value() {
		let mut params = HashMap::new();
		params.insert("id".to_string(), "".to_string());

		let result = reverse_single_pass("/items/{id}/", &params);
		assert_eq!(result, "/items//");
	}

	#[test]
	fn test_single_pass_pattern_with_no_placeholder() {
		let mut params = HashMap::new();
		params.insert("id".to_string(), "123".to_string());

		let result = reverse_single_pass("/static/path/", &params);
		assert_eq!(result, "/static/path/");
	}

	#[test]
	fn test_single_pass_mixed_content() {
		let mut params = HashMap::new();
		params.insert("id".to_string(), "123".to_string());
		params.insert("action".to_string(), "edit".to_string());

		let result = reverse_single_pass("/items/{id}/actions/{action}/execute", &params);
		assert_eq!(result, "/items/123/actions/edit/execute");
	}

	#[test]
	fn test_single_pass_param_at_start() {
		let mut params = HashMap::new();
		params.insert("lang".to_string(), "ja".to_string());

		let result = reverse_single_pass("{lang}/users/", &params);
		assert_eq!(result, "ja/users/");
	}

	#[test]
	fn test_single_pass_param_at_end() {
		let mut params = HashMap::new();
		params.insert("format".to_string(), "json".to_string());

		let result = reverse_single_pass("/api/data.{format}", &params);
		assert_eq!(result, "/api/data.json");
	}

	#[test]
	fn test_single_pass_unicode_values() {
		let mut params = HashMap::new();
		params.insert("name".to_string(), "ユーザー".to_string());

		let result = reverse_single_pass("/users/{name}/", &params);
		assert_eq!(result, "/users/ユーザー/");
	}

	#[test]
	fn test_single_pass_long_value() {
		let mut params = HashMap::new();
		let long_id = "a".repeat(1000);
		params.insert("id".to_string(), long_id.clone());

		let result = reverse_single_pass("/items/{id}/", &params);
		assert_eq!(result, format!("/items/{}/", long_id));
	}

	// Aho-Corasick algorithm tests
	#[test]
	fn test_aho_corasick_basic() {
		let mut params = HashMap::new();
		params.insert("id".to_string(), "123".to_string());

		let result = reverse_with_aho_corasick("/users/{id}/", &params);
		assert_eq!(result, "/users/123/");
	}

	#[test]
	fn test_aho_corasick_multiple_params() {
		let mut params = HashMap::new();
		params.insert("user_id".to_string(), "42".to_string());
		params.insert("post_id".to_string(), "100".to_string());

		let result = reverse_with_aho_corasick("/users/{user_id}/posts/{post_id}/", &params);
		assert_eq!(result, "/users/42/posts/100/");
	}

	#[test]
	fn test_aho_corasick_many_params() {
		let mut params = HashMap::new();
		for i in 1..=10 {
			params.insert(format!("p{}", i), format!("v{}", i));
		}

		let pattern = "/api/{p1}/{p2}/{p3}/{p4}/{p5}/{p6}/{p7}/{p8}/{p9}/{p10}/";
		let result = reverse_with_aho_corasick(pattern, &params);
		assert_eq!(result, "/api/v1/v2/v3/v4/v5/v6/v7/v8/v9/v10/");
	}

	#[test]
	fn test_aho_corasick_missing_param() {
		let params = HashMap::new();

		let result = reverse_with_aho_corasick("/users/{id}/", &params);
		// Missing parameter should preserve placeholder
		assert_eq!(result, "/users/{id}/");
	}

	#[test]
	fn test_aho_corasick_no_params() {
		let params = HashMap::new();

		let result = reverse_with_aho_corasick("/users/", &params);
		assert_eq!(result, "/users/");
	}

	#[test]
	fn test_aho_corasick_empty_pattern() {
		let params = HashMap::new();

		let result = reverse_with_aho_corasick("", &params);
		assert_eq!(result, "");
	}

	#[test]
	fn test_aho_corasick_consecutive_params() {
		let mut params = HashMap::new();
		params.insert("a".to_string(), "1".to_string());
		params.insert("b".to_string(), "2".to_string());

		let result = reverse_with_aho_corasick("/{a}{b}/", &params);
		assert_eq!(result, "/12/");
	}

	#[test]
	fn test_aho_corasick_special_chars_in_values() {
		let mut params = HashMap::new();
		params.insert("id".to_string(), "foo-bar_123".to_string());

		let result = reverse_with_aho_corasick("/items/{id}/", &params);
		assert_eq!(result, "/items/foo-bar_123/");
	}

	#[test]
	fn test_aho_corasick_unicode() {
		let mut params = HashMap::new();
		params.insert("name".to_string(), "ユーザー".to_string());

		let result = reverse_with_aho_corasick("/users/{name}/", &params);
		assert_eq!(result, "/users/ユーザー/");
	}

	#[test]
	fn test_extract_param_names_basic() {
		let names = extract_param_names("/users/{id}/");
		assert_eq!(names, vec!["id"]);
	}

	#[test]
	fn test_extract_param_names_multiple() {
		let names = extract_param_names("/users/{user_id}/posts/{post_id}/");
		assert_eq!(names, vec!["user_id", "post_id"]);
	}

	#[test]
	fn test_extract_param_names_no_params() {
		let names = extract_param_names("/users/");
		assert!(names.is_empty());
	}

	#[test]
	fn test_extract_param_names_consecutive() {
		let names = extract_param_names("/{a}{b}/");
		assert_eq!(names, vec!["a", "b"]);
	}

	#[test]
	fn test_aho_corasick_vs_single_pass_consistency() {
		let mut params = HashMap::new();
		params.insert("id".to_string(), "123".to_string());
		params.insert("action".to_string(), "edit".to_string());

		let pattern = "/users/{id}/actions/{action}/";

		let result_single = reverse_single_pass(pattern, &params);
		let result_aho = reverse_with_aho_corasick(pattern, &params);

		assert_eq!(
			result_single, result_aho,
			"Both algorithms should produce identical results"
		);
	}

	#[test]
	fn test_aho_corasick_complex_pattern() {
		let mut params = HashMap::new();
		params.insert("org".to_string(), "myorg".to_string());
		params.insert("repo".to_string(), "myrepo".to_string());
		params.insert("branch".to_string(), "main".to_string());
		params.insert("file".to_string(), "README.md".to_string());

		let pattern = "/repos/{org}/{repo}/contents/{file}?ref={branch}";
		let result = reverse_with_aho_corasick(pattern, &params);
		assert_eq!(result, "/repos/myorg/myrepo/contents/README.md?ref=main");
	}

	#[test]
	fn test_performance_comparison_many_params() {
		use std::time::Instant;

		// Create a pattern with 20 parameters
		let mut params = HashMap::new();
		let mut pattern_parts = vec!["/api".to_string()];
		for i in 1..=20 {
			params.insert(format!("p{}", i), format!("v{}", i));
			pattern_parts.push(format!("{{p{}}}", i));
		}
		let pattern = pattern_parts.join("/") + "/";

		// Warm up
		for _ in 0..10 {
			let _ = reverse_single_pass(&pattern, &params);
			let _ = reverse_with_aho_corasick(&pattern, &params);
		}

		// Measure single_pass
		let start = Instant::now();
		for _ in 0..1000 {
			let _ = reverse_single_pass(&pattern, &params);
		}
		let single_pass_duration = start.elapsed();

		// Measure aho_corasick
		let start = Instant::now();
		for _ in 0..1000 {
			let _ = reverse_with_aho_corasick(&pattern, &params);
		}
		let aho_corasick_duration = start.elapsed();

		// Verify both produce same result
		let result_single = reverse_single_pass(&pattern, &params);
		let result_aho = reverse_with_aho_corasick(&pattern, &params);
		assert_eq!(result_single, result_aho);

		// Print performance results (for informational purposes)
		println!("\nPerformance comparison (20 params, 1000 iterations):");
		println!("  Single-pass: {:?}", single_pass_duration);
		println!("  Aho-Corasick: {:?}", aho_corasick_duration);

		if aho_corasick_duration < single_pass_duration {
			let improvement =
				single_pass_duration.as_nanos() as f64 / aho_corasick_duration.as_nanos() as f64;
			println!("  Improvement: {:.2}x faster", improvement);
		}

		// Note: This test doesn't fail, it's for informational purposes
		// Actual performance may vary based on pattern complexity and parameter count
	}

	#[test]
	fn test_performance_few_params() {
		use std::time::Instant;

		// Test with fewer parameters (where single-pass might be faster due to overhead)
		let mut params = HashMap::new();
		params.insert("id".to_string(), "123".to_string());
		params.insert("action".to_string(), "edit".to_string());
		let pattern = "/users/{id}/actions/{action}/";

		// Warm up
		for _ in 0..10 {
			let _ = reverse_single_pass(pattern, &params);
			let _ = reverse_with_aho_corasick(pattern, &params);
		}

		// Measure single_pass
		let start = Instant::now();
		for _ in 0..10000 {
			let _ = reverse_single_pass(pattern, &params);
		}
		let single_pass_duration = start.elapsed();

		// Measure aho_corasick
		let start = Instant::now();
		for _ in 0..10000 {
			let _ = reverse_with_aho_corasick(pattern, &params);
		}
		let aho_corasick_duration = start.elapsed();

		// Verify both produce same result
		let result_single = reverse_single_pass(pattern, &params);
		let result_aho = reverse_with_aho_corasick(pattern, &params);
		assert_eq!(result_single, result_aho);

		// Print performance results
		println!("\nPerformance comparison (2 params, 10000 iterations):");
		println!("  Single-pass: {:?}", single_pass_duration);
		println!("  Aho-Corasick: {:?}", aho_corasick_duration);
	}

	// ===================================================================
	// URL reversal parameter injection prevention tests (Issue #423)
	// ===================================================================

	#[test]
	fn test_reverser_rejects_path_separator_injection() {
		// Arrange
		let mut reverser = UrlReverser::new();
		let route =
			Route::new(path!("/users/{id}/"), Arc::new(TestHandler)).with_name("users-detail");
		reverser.register(route);

		let mut params = HashMap::new();
		params.insert("id".to_string(), "123/../../admin".to_string());

		// Act
		let result = reverser.reverse("users-detail", &params);

		// Assert
		assert!(
			result.is_err(),
			"Reverser should reject path separator injection"
		);
	}

	#[test]
	fn test_reverser_rejects_query_injection() {
		// Arrange
		let mut reverser = UrlReverser::new();
		let route =
			Route::new(path!("/users/{id}/"), Arc::new(TestHandler)).with_name("users-detail");
		reverser.register(route);

		let mut params = HashMap::new();
		params.insert("id".to_string(), "123?admin=true".to_string());

		// Act
		let result = reverser.reverse("users-detail", &params);

		// Assert
		assert!(
			result.is_err(),
			"Reverser should reject query string injection"
		);
	}

	#[test]
	fn test_reverser_rejects_fragment_injection() {
		// Arrange
		let mut reverser = UrlReverser::new();
		let route =
			Route::new(path!("/users/{id}/"), Arc::new(TestHandler)).with_name("users-detail");
		reverser.register(route);

		let mut params = HashMap::new();
		params.insert("id".to_string(), "123#admin".to_string());

		// Act
		let result = reverser.reverse("users-detail", &params);

		// Assert
		assert!(result.is_err(), "Reverser should reject fragment injection");
	}

	#[test]
	fn test_reverser_rejects_encoded_injection() {
		// Arrange
		let mut reverser = UrlReverser::new();
		let route =
			Route::new(path!("/users/{id}/"), Arc::new(TestHandler)).with_name("users-detail");
		reverser.register(route);

		let mut params = HashMap::new();
		params.insert("id".to_string(), "123%2f..%2fadmin".to_string());

		// Act
		let result = reverser.reverse("users-detail", &params);

		// Assert
		assert!(
			result.is_err(),
			"Reverser should reject percent-encoded injection"
		);
	}

	#[test]
	fn test_reverser_allows_safe_values() {
		// Arrange
		let mut reverser = UrlReverser::new();
		let route =
			Route::new(path!("/users/{id}/"), Arc::new(TestHandler)).with_name("users-detail");
		reverser.register(route);

		let mut params = HashMap::new();
		params.insert("id".to_string(), "456".to_string());

		// Act
		let result = reverser.reverse("users-detail", &params);

		// Assert
		assert!(result.is_ok());
		assert_eq!(result.unwrap(), "/users/456/");
	}

	#[test]
	#[should_panic(expected = "contains dangerous characters")]
	fn test_single_pass_rejects_path_separator() {
		// Arrange
		let mut params = HashMap::new();
		params.insert("id".to_string(), "123/admin".to_string());

		// Act - should panic
		reverse_single_pass("/users/{id}/", &params);
	}

	#[test]
	#[should_panic(expected = "contains dangerous characters")]
	fn test_aho_corasick_rejects_path_separator() {
		// Arrange
		let mut params = HashMap::new();
		params.insert("id".to_string(), "123/admin".to_string());

		// Act - should panic
		reverse_with_aho_corasick("/users/{id}/", &params);
	}

	#[test]
	fn test_typed_reverse_rejects_injection() {
		// Arrange
		let mut params = HashMap::new();
		params.insert("id", "123/admin");

		// Act
		let result = reverse_typed_with_params::<UserDetailUrl>(&params);

		// Assert
		assert!(result.is_err(), "Typed reverse should reject injection");
	}

	#[test]
	fn test_reverse_with_helper_rejects_injection() {
		// Arrange
		let mut reverser = UrlReverser::new();
		let route =
			Route::new(path!("/users/{id}/"), Arc::new(TestHandler)).with_name("users-detail");
		reverser.register(route);

		// Act
		let result = reverser.reverse_with("users-detail", &[("id", "123?admin=true")]);

		// Assert
		assert!(
			result.is_err(),
			"reverse_with should reject query injection"
		);
	}
}
