use aho_corasick::AhoCorasick;
use matchit::Router as MatchitRouter;
use regex::Regex;
use std::collections::{HashMap, HashSet};

/// Maximum allowed length for a URL pattern string in bytes.
/// Patterns exceeding this limit are rejected to prevent ReDoS attacks
/// from excessively long or complex regex patterns.
const MAX_PATTERN_LENGTH: usize = 1024;

/// Maximum allowed number of path segments in a URL pattern.
/// Patterns with more segments than this are rejected to prevent
/// resource exhaustion from deeply nested URL structures.
const MAX_PATH_SEGMENTS: usize = 32;

/// Maximum allowed size for compiled regex (in bytes).
/// This limits the compiled regex DFA size to prevent memory exhaustion.
const MAX_REGEX_SIZE: usize = 1 << 20; // 1 MiB

/// Convert a type specifier to its corresponding regex pattern
///
/// This function maps type specifiers from `{<type:name>}` syntax
/// to appropriate regex patterns for URL matching.
///
/// # Supported Type Specifiers
///
/// | Type | Pattern | Description |
/// |------|---------|-------------|
/// | `int` | `[0-9]+` | Unsigned integer (legacy) |
/// | `i8`, `i16`, `i32`, `i64` | `-?[0-9]+` | Signed integers |
/// | `u8`, `u16`, `u32`, `u64` | `[0-9]+` | Unsigned integers |
/// | `f32`, `f64` | `-?[0-9]+(?:\.[0-9]+)?` | Floating point |
/// | `str` | `[^/]+` | Any non-slash characters (default) |
/// | `uuid` | UUID regex | UUID format |
/// | `slug` | `[a-z0-9]+(?:-[a-z0-9]+)*` | Lowercase slug |
/// | `path` | `.+` | Any characters **including** path separators (`/`); `..` segments are rejected by post-match validation |
/// | `bool` | `true\|false\|1\|0` | Boolean literals |
/// | `email` | Email regex | Email format |
/// | `date` | `[0-9]{4}-[0-9]{2}-[0-9]{2}` | ISO 8601 date |
fn type_spec_to_regex(type_spec: &str) -> &'static str {
	match type_spec {
		// Basic types (legacy)
		"int" => r"[0-9]+",
		"str" => r"[^/]+",
		"uuid" => r"[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}",
		"slug" => r"[a-z0-9]+(?:-[a-z0-9]+)*",
		// Matches any characters including path separators (`/`).
		// A pattern like `/files/{<path:filepath>}` will match
		// `/files/a/b/c.txt`, capturing `a/b/c.txt` as a single value.
		// Directory traversal (`..` segments) is rejected by post-match
		// validation in extract_params() and match_path_linear().
		"path" => r".+",
		// Signed integers
		"i8" | "i16" | "i32" | "i64" => r"-?[0-9]+",
		// Unsigned integers
		"u8" | "u16" | "u32" | "u64" => r"[0-9]+",
		// Floating point
		"f32" | "f64" => r"-?[0-9]+(?:\.[0-9]+)?",
		// Other types
		"bool" => r"true|false|1|0",
		"email" => r"[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}",
		"date" => r"[0-9]{4}-[0-9]{2}-[0-9]{2}",
		// Default: treat as string
		_ => r"[^/]+",
	}
}

/// Validate that a matched path value does not contain directory traversal sequences.
///
/// This provides defense-in-depth for `path` type parameters by checking
/// extracted values for `..` segments that could enable path traversal.
///
/// Rejects:
/// - `..` as a path segment (forward-slash or backslash separated)
/// - Percent-encoded traversal sequences (`%2e`, `%2f`, `%2E`, `%2F`, `%5c`, `%5C`)
/// - Null bytes (literal or encoded `%00`)
/// - Absolute paths starting with `/` or `\`
fn validate_path_param(value: &str) -> bool {
	// Reject null bytes
	if value.contains('\0') {
		return false;
	}

	// Reject percent-encoded dangerous characters:
	// %2e / %2E = '.', %2f / %2F = '/', %5c / %5C = '\', %00 = null
	let lower = value.to_ascii_lowercase();
	if lower.contains("%2e")
		|| lower.contains("%2f")
		|| lower.contains("%5c")
		|| lower.contains("%00")
	{
		return false;
	}

	// Reject absolute paths
	if value.starts_with('/') || value.starts_with('\\') {
		return false;
	}

	// Check for `..` as a complete path segment (forward-slash separated)
	for segment in value.split('/') {
		if segment == ".." {
			return false;
		}
	}
	// Also reject backslash-separated `..` segments
	for segment in value.split('\\') {
		if segment == ".." {
			return false;
		}
	}

	true
}

/// Validate a parameter value for URL reversal against injection attacks.
///
/// Rejects values containing:
/// - Path separators (`/`, `\`)
/// - Query string delimiters (`?`)
/// - Fragment identifiers (`#`)
/// - Null bytes
/// - Path traversal sequences (`..`)
/// - Percent-encoded dangerous characters (`%2f`, `%2e`, `%5c`, `%3f`, `%23`, `%00`)
pub(crate) fn validate_reverse_param(value: &str) -> bool {
	// Reject null bytes
	if value.contains('\0') {
		return false;
	}

	// Reject path separators and URL-special characters
	if value.contains('/') || value.contains('\\') || value.contains('?') || value.contains('#') {
		return false;
	}

	// Reject path traversal
	if value == ".." || value.starts_with("../") || value.ends_with("/..") || value.contains("/../")
	{
		return false;
	}

	// Reject percent-encoded dangerous characters
	let lower = value.to_ascii_lowercase();
	if lower.contains("%2f")
		|| lower.contains("%2e")
		|| lower.contains("%5c")
		|| lower.contains("%3f")
		|| lower.contains("%23")
		|| lower.contains("%00")
	{
		return false;
	}

	true
}

/// Path pattern for URL matching
/// Similar to Django's URL patterns but using composition
#[derive(Clone, Debug)]
pub struct PathPattern {
	/// Original pattern string (may contain type specifiers)
	pattern: String,
	/// Pattern normalized to `{name}` format for URL reversal
	normalized_pattern: String,
	regex: Regex,
	param_names: Vec<String>,
	/// Parameter names that use the `path` type specifier.
	/// These require post-match validation to reject directory traversal.
	path_type_params: HashSet<String>,
	/// Pre-built Aho-Corasick automaton for efficient URL reversal
	/// This is constructed once during pattern creation for O(n+m+z) reversal
	aho_corasick: Option<AhoCorasick>,
}

/// Parse result containing regex, param names, and normalized pattern for URL reversal
struct ParsePatternResult {
	regex_str: String,
	param_names: Vec<String>,
	/// Parameter names that use the `path` type specifier
	path_type_params: HashSet<String>,
	/// Pattern normalized to `{name}` format for URL reversal
	/// e.g., "/users/{<int:id>}/" -> "/users/{id}/"
	normalized_pattern: String,
}

impl PathPattern {
	/// Create a new path pattern
	/// Patterns like "/users/{id}/" are converted to regex
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_urls::routers::{PathPattern, path};
	///
	/// // Create a simple pattern without parameters
	/// let pattern = PathPattern::new(path!("/users/")).unwrap();
	/// assert_eq!(pattern.pattern(), "/users/");
	///
	/// // Create a pattern with a parameter
	/// let pattern = PathPattern::new(path!("/users/{id}/")).unwrap();
	/// assert_eq!(pattern.param_names(), &["id"]);
	/// ```
	pub fn new(pattern: impl Into<String>) -> Result<Self, String> {
		let pattern = pattern.into();

		// Reject patterns exceeding the maximum length to prevent ReDoS
		if pattern.len() > MAX_PATTERN_LENGTH {
			return Err(format!(
				"Pattern length {} exceeds maximum allowed length of {} bytes",
				pattern.len(),
				MAX_PATTERN_LENGTH
			));
		}

		// Reject patterns with excessive path segments to prevent resource exhaustion
		let segment_count = pattern.split('/').count();
		if segment_count > MAX_PATH_SEGMENTS {
			return Err(format!(
				"Pattern has {} path segments, exceeding maximum of {}",
				segment_count, MAX_PATH_SEGMENTS
			));
		}

		let parse_result = Self::parse_pattern(&pattern)?;

		// Use RegexBuilder with size limits to prevent memory exhaustion
		let regex = regex::RegexBuilder::new(&parse_result.regex_str)
			.size_limit(MAX_REGEX_SIZE)
			.build()
			.map_err(|e| format!("Failed to compile pattern regex: {}", e))?;

		// Build Aho-Corasick automaton for URL reversal if there are parameters
		let aho_corasick = if !parse_result.param_names.is_empty() {
			let placeholders: Vec<String> = parse_result
				.param_names
				.iter()
				.map(|name| format!("{{{}}}", name))
				.collect();

			AhoCorasick::new(&placeholders)
				.map(Some)
				.map_err(|e| format!("Failed to build Aho-Corasick automaton: {}", e))?
		} else {
			None
		};

		Ok(Self {
			pattern,
			normalized_pattern: parse_result.normalized_pattern,
			regex,
			param_names: parse_result.param_names,
			path_type_params: parse_result.path_type_params,
			aho_corasick,
		})
	}

	fn parse_pattern(pattern: &str) -> Result<ParsePatternResult, String> {
		let mut regex_str = String::from("^");
		let mut param_names = Vec::new();
		let mut path_type_params = HashSet::new();
		let mut normalized_pattern = String::new();
		let mut chars = pattern.chars().peekable();

		while let Some(ch) = chars.next() {
			match ch {
				'{' => {
					// Extract parameter content (everything between { and })
					let mut param_content = String::new();
					while let Some(&next_ch) = chars.peek() {
						if next_ch == '}' {
							chars.next(); // consume '}'
							break;
						}
						param_content.push(chars.next().unwrap());
					}

					if param_content.is_empty() {
						return Err("Empty parameter name".to_string());
					}

					// Check for typed parameter syntax: {<type:name>}
					let (param_name, regex_pattern) =
						if param_content.starts_with('<') && param_content.ends_with('>') {
							// Parse {<type:name>}
							let inner = &param_content[1..param_content.len() - 1]; // Remove < >
							if let Some(colon_pos) = inner.find(':') {
								let type_spec = &inner[..colon_pos];
								let name = &inner[colon_pos + 1..];
								if name.is_empty() {
									return Err(format!(
										"Empty parameter name in typed parameter: {{<{}:>}}",
										type_spec
									));
								}
								if type_spec == "path" {
									path_type_params.insert(name.to_string());
								}
								(name.to_string(), type_spec_to_regex(type_spec))
							} else {
								return Err(format!(
									"Invalid typed parameter syntax: {{<{}>}}. Expected {{<type:name>}}",
									inner
								));
							}
						} else {
							// Simple {name} parameter - use default [^/]+
							(param_content, "[^/]+")
						};

					param_names.push(param_name.clone());
					regex_str.push_str(&format!("(?P<{}>{})", param_name, regex_pattern));
					// Write normalized placeholder for URL reversal
					normalized_pattern.push_str(&format!("{{{}}}", param_name));
				}
				_ => {
					// Escape special regex characters
					if ".*+?^${}()|[]\\".contains(ch) {
						regex_str.push('\\');
					}
					regex_str.push(ch);
					// Copy literal characters to normalized pattern
					normalized_pattern.push(ch);
				}
			}
		}

		regex_str.push('$');
		Ok(ParsePatternResult {
			regex_str,
			param_names,
			path_type_params,
			normalized_pattern,
		})
	}
	/// Get the original pattern string
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_urls::routers::{PathPattern, path};
	///
	/// let pattern = PathPattern::new(path!("/users/{id}/")).unwrap();
	/// assert_eq!(pattern.pattern(), "/users/{id}/");
	/// ```
	pub fn pattern(&self) -> &str {
		&self.pattern
	}

	/// Convert pattern to matchit-compatible format
	///
	/// Transforms path-type parameters from `{<path:name>}` to `{*name}`
	/// for use with the matchit radix router. Non-path parameters remain
	/// as `{name}`.
	pub(crate) fn to_matchit_pattern(&self) -> String {
		let mut result = String::new();
		let mut chars = self.pattern.chars().peekable();

		while let Some(ch) = chars.next() {
			if ch == '{' {
				let mut param_content = String::new();
				while let Some(&next_ch) = chars.peek() {
					if next_ch == '}' {
						chars.next();
						break;
					}
					param_content.push(chars.next().unwrap());
				}

				// Check for typed parameter: {<type:name>}
				if param_content.starts_with('<') && param_content.ends_with('>') {
					let inner = &param_content[1..param_content.len() - 1];
					if let Some(colon_pos) = inner.find(':') {
						let type_spec = &inner[..colon_pos];
						let name = &inner[colon_pos + 1..];
						if type_spec == "path" {
							// Convert path type to matchit catch-all: {*name}
							result.push_str(&format!("{{*{}}}", name));
						} else {
							// Other typed params use simple {name}
							result.push_str(&format!("{{{}}}", name));
						}
					} else {
						result.push_str(&format!("{{{}}}", param_content));
					}
				} else {
					// Simple {name} parameter
					result.push_str(&format!("{{{}}}", param_content));
				}
			} else {
				result.push(ch);
			}
		}

		result
	}
	/// Get the list of parameter names in the pattern
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_urls::routers::{PathPattern, path};
	///
	/// let pattern = PathPattern::new(path!("/users/{user_id}/posts/{post_id}/")).unwrap();
	/// assert_eq!(pattern.param_names(), &["user_id", "post_id"]);
	/// ```
	pub fn param_names(&self) -> &[String] {
		&self.param_names
	}

	/// Test if the pattern matches a given path
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_urls::routers::{PathPattern, path};
	///
	/// let pattern = PathPattern::new(path!("/users/{id}/")).unwrap();
	/// assert!(pattern.is_match("/users/123/"));
	/// assert!(!pattern.is_match("/users/"));
	/// ```
	pub fn is_match(&self, path: &str) -> bool {
		self.regex.is_match(path)
	}

	/// Match a path and extract parameters
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_urls::routers::{PathPattern, path};
	///
	/// let pattern = PathPattern::new(path!("/users/{id}/")).unwrap();
	/// let params = pattern.extract_params("/users/123/").unwrap();
	/// assert_eq!(params.get("id"), Some(&"123".to_string()));
	/// ```
	pub fn extract_params(&self, path: &str) -> Option<HashMap<String, String>> {
		self.regex.captures(path).and_then(|captures| {
			let mut params = HashMap::new();
			for name in self.param_names() {
				if let Some(value) = captures.name(name) {
					let val = value.as_str();
					// Validate path-type parameters against directory traversal
					if self.path_type_params.contains(name) && !validate_path_param(val) {
						return None;
					}
					params.insert(name.clone(), val.to_string());
				}
			}
			Some(params)
		})
	}

	/// Reverse URL pattern with parameters using Aho-Corasick algorithm
	///
	/// This method uses pre-built Aho-Corasick automaton for efficient
	/// multi-pattern matching with O(n+m+z) complexity where:
	/// - n: pattern length
	/// - m: total parameter values length
	/// - z: number of placeholder matches
	///
	/// # Arguments
	///
	/// * `params` - HashMap of parameter names to values
	///
	/// # Returns
	///
	/// * `Ok(String)` - Reversed URL with parameters substituted
	/// * `Err(String)` - If required parameters are missing
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_urls::routers::{PathPattern, path};
	/// use std::collections::HashMap;
	///
	/// let pattern = PathPattern::new(path!("/users/{id}/posts/{post_id}/")).unwrap();
	///
	/// let mut params = HashMap::new();
	/// params.insert("id".to_string(), "123".to_string());
	/// params.insert("post_id".to_string(), "456".to_string());
	///
	/// let url = pattern.reverse(&params).unwrap();
	/// assert_eq!(url, "/users/123/posts/456/");
	/// ```
	pub fn reverse(&self, params: &HashMap<String, String>) -> Result<String, String> {
		// Validate all required parameters are present
		for param_name in &self.param_names {
			if !params.contains_key(param_name) {
				return Err(format!("Missing required parameter: {}", param_name));
			}
		}

		// Validate parameter values against injection attacks
		for (name, value) in params {
			if !validate_reverse_param(value) {
				return Err(format!(
					"Invalid parameter value for '{}': contains dangerous characters",
					name
				));
			}
		}

		// If no parameters, return normalized pattern as-is
		if self.param_names.is_empty() {
			return Ok(self.normalized_pattern.clone());
		}

		// Use Aho-Corasick if available, otherwise fallback to single-pass
		match &self.aho_corasick {
			Some(ac) => {
				// Find all matches using Aho-Corasick on normalized pattern
				let mut replacements = Vec::new();
				for mat in ac.find_iter(&self.normalized_pattern) {
					let param_name = &self.param_names[mat.pattern()];
					// We already validated all params exist above
					let value = params.get(param_name).unwrap();
					replacements.push((mat.start(), mat.end(), value.clone()));
				}

				// Apply replacements from right to left to avoid position shifts
				let mut result = self.normalized_pattern.clone();
				for (start, end, value) in replacements.into_iter().rev() {
					result.replace_range(start..end, &value);
				}

				Ok(result)
			}
			None => {
				// Fallback: no parameters, just return normalized pattern
				Ok(self.normalized_pattern.clone())
			}
		}
	}
}

/// Matching mode for PathMatcher
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatchingMode {
	/// Linear O(n) matching through all patterns (default)
	Linear,
	/// Radix Tree O(m) matching using matchit (recommended for >100 routes)
	RadixTree,
}

/// Path matcher - uses composition to match paths
///
/// Supports two matching modes:
/// - **Linear** (default): O(n) search through patterns, suitable for <100 routes
/// - **RadixTree**: O(m) matching using radix tree, recommended for >100 routes
pub struct PathMatcher {
	patterns: Vec<(PathPattern, String)>, // (pattern, handler_id)
	radix_router: Option<RadixRouter>,
	mode: MatchingMode,
}

impl PathMatcher {
	/// Create a new PathMatcher with linear matching (default)
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_urls::routers::PathMatcher;
	///
	/// let matcher = PathMatcher::new();
	/// assert_eq!(matcher.match_path("/users/"), None);
	/// ```
	pub fn new() -> Self {
		Self {
			patterns: Vec::new(),
			radix_router: None,
			mode: MatchingMode::Linear,
		}
	}

	/// Create a new PathMatcher with specified matching mode
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_urls::routers::{PathMatcher, MatchingMode};
	///
	/// let matcher = PathMatcher::with_mode(MatchingMode::RadixTree);
	/// ```
	pub fn with_mode(mode: MatchingMode) -> Self {
		Self {
			patterns: Vec::new(),
			radix_router: if mode == MatchingMode::RadixTree {
				Some(RadixRouter::new())
			} else {
				None
			},
			mode,
		}
	}

	/// Enable radix tree matching mode
	///
	/// Rebuilds the radix router from existing patterns.
	/// Recommended when route count exceeds 100.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_urls::routers::{PathMatcher, PathPattern, path};
	///
	/// let mut matcher = PathMatcher::new();
	/// let pattern = PathPattern::new(path!("/users/")).unwrap();
	/// matcher.add_pattern(pattern, "users_list".to_string());
	///
	/// // Enable radix tree mode
	/// matcher.enable_radix_tree();
	///
	/// let result = matcher.match_path("/users/");
	/// assert!(result.is_some());
	/// ```
	pub fn enable_radix_tree(&mut self) {
		if self.mode == MatchingMode::RadixTree {
			return; // Already enabled
		}

		self.mode = MatchingMode::RadixTree;
		let mut radix_router = RadixRouter::new();

		// Rebuild radix router from existing patterns
		for (pattern, handler_id) in &self.patterns {
			let _ = radix_router.add_route(&pattern.to_matchit_pattern(), handler_id.clone());
		}

		self.radix_router = Some(radix_router);
	}

	/// Get current matching mode
	pub fn mode(&self) -> MatchingMode {
		self.mode
	}
	/// Add a pattern to the matcher
	///
	/// If radix tree mode is enabled, also adds to the radix router.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_urls::routers::{PathMatcher, PathPattern, path};
	///
	/// let mut matcher = PathMatcher::new();
	/// let pattern = PathPattern::new(path!("/users/")).unwrap();
	/// matcher.add_pattern(pattern, "users_list".to_string());
	///
	/// let result = matcher.match_path("/users/");
	/// assert!(result.is_some());
	/// ```
	pub fn add_pattern(&mut self, pattern: PathPattern, handler_id: String) {
		let matchit_pattern = pattern.to_matchit_pattern();
		self.patterns.push((pattern, handler_id.clone()));

		// If radix tree mode is enabled, also add to radix router
		if let Some(ref mut radix_router) = self.radix_router {
			let _ = radix_router.add_route(&matchit_pattern, handler_id);
		}
	}
	/// Match a path and extract parameters
	///
	/// Uses the configured matching mode:
	/// - **Linear**: O(n) search through patterns (default)
	/// - **RadixTree**: O(m) radix tree matching where m = path length
	///
	/// # Performance Notes
	///
	/// - **Linear mode**: O(n×m) where n = route count, m = path length
	///   - Suitable for <100 routes
	///   - Benefits from RouteCache for O(1) on cache hits
	/// - **RadixTree mode**: O(m) where m = path length
	///   - Recommended for >100 routes
	///   - 3-5x faster for large route sets
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_urls::routers::{PathMatcher, PathPattern, path};
	///
	/// let mut matcher = PathMatcher::new();
	/// let pattern = PathPattern::new(path!("/users/{id}/")).unwrap();
	/// matcher.add_pattern(pattern, "users_detail".to_string());
	///
	/// let result = matcher.match_path("/users/123/");
	/// assert!(result.is_some());
	/// let (handler_id, params) = result.unwrap();
	/// assert_eq!(handler_id, "users_detail");
	/// assert_eq!(params.get("id"), Some(&"123".to_string()));
	/// ```
	pub fn match_path(&self, path: &str) -> Option<(String, HashMap<String, String>)> {
		match self.mode {
			MatchingMode::RadixTree => {
				// Use radix tree for O(m) matching
				if let Some(ref radix_router) = self.radix_router {
					let (handler_id, params) = radix_router.match_path(path)?;

					// Validate path-type parameters against directory traversal
					if let Some((pattern, _)) =
						self.patterns.iter().find(|(_, id)| *id == handler_id)
					{
						for (name, value) in &params {
							if pattern.path_type_params.contains(name)
								&& !validate_path_param(value)
							{
								return None;
							}
						}
					}

					Some((handler_id, params))
				} else {
					// Fallback to linear if radix router not initialized
					self.match_path_linear(path)
				}
			}
			MatchingMode::Linear => {
				// Use linear O(n) matching
				self.match_path_linear(path)
			}
		}
	}

	/// Linear pattern matching (O(n))
	fn match_path_linear(&self, path: &str) -> Option<(String, HashMap<String, String>)> {
		'outer: for (pattern, handler_id) in &self.patterns {
			if let Some(captures) = pattern.regex.captures(path) {
				let mut params = HashMap::new();

				for name in pattern.param_names() {
					if let Some(value) = captures.name(name) {
						let val = value.as_str();
						// Validate path-type parameters against directory traversal
						if pattern.path_type_params.contains(name) && !validate_path_param(val) {
							continue 'outer;
						}
						params.insert(name.clone(), val.to_string());
					}
				}

				return Some((handler_id.clone(), params));
			}
		}

		None
	}
}

impl Default for PathMatcher {
	fn default() -> Self {
		Self::new()
	}
}

/// Error type for Radix Router operations
#[derive(Debug, thiserror::Error)]
pub enum RadixRouterError {
	#[error("Invalid pattern: {0}")]
	InvalidPattern(String),
	#[error("Route insertion failed: {0}")]
	InsertionFailed(String),
}

/// Radix Tree-based router using matchit for O(m) matching
///
/// Provides significant performance improvements over linear O(n) matching,
/// especially for applications with large route counts (>100 routes).
///
/// # Performance Characteristics
///
/// - **Insertion**: O(m) where m is the pattern length
/// - **Matching**: O(m) where m is the path length (vs O(n×m) for linear)
/// - **Memory**: O(total pattern characters) for the radix tree structure
///
/// # Pattern Syntax
///
/// Uses matchit's pattern syntax, compatible with Django-style patterns:
/// - `/users/{id}` - Single parameter
/// - `/posts/{post_id}/comments/{comment_id}` - Multiple parameters
/// - `/files/{*path}` - Catch-all wildcard (matches remaining path including `/`)
///
/// # Examples
///
/// ```
/// use reinhardt_urls::routers::{RadixRouter, path};
///
/// let mut router = RadixRouter::new();
///
/// // Register routes
/// router.add_route(path!("/users/"), "users_list".to_string()).unwrap();
/// router.add_route(path!("/users/{id}/"), "users_detail".to_string()).unwrap();
///
/// // Match paths
/// let result = router.match_path("/users/123/");
/// assert!(result.is_some());
/// let (handler_id, params) = result.unwrap();
/// assert_eq!(handler_id, "users_detail");
/// assert_eq!(params.get("id"), Some(&"123".to_string()));
/// ```
pub struct RadixRouter {
	router: MatchitRouter<String>,
}

impl RadixRouter {
	/// Create a new RadixRouter
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_urls::routers::RadixRouter;
	///
	/// let router = RadixRouter::new();
	/// ```
	pub fn new() -> Self {
		Self {
			router: MatchitRouter::new(),
		}
	}

	/// Add a route pattern to the router
	///
	/// Converts Django-style `{param}` patterns to matchit's `{param}` syntax.
	/// Both syntaxes are compatible, so no conversion is needed.
	///
	/// # Arguments
	///
	/// * `pattern` - URL pattern (e.g., `/users/{id}/`)
	/// * `handler_id` - Identifier for the route handler
	///
	/// # Errors
	///
	/// Returns `RadixRouterError::InsertionFailed` if:
	/// - Pattern conflicts with existing routes
	/// - Pattern syntax is invalid
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_urls::routers::{RadixRouter, path};
	///
	/// let mut router = RadixRouter::new();
	/// router.add_route(path!("/users/{id}/"), "users_detail".to_string()).unwrap();
	///
	/// // Catch-all wildcard
	/// router.add_route(path!("/files/{file_path}"), "serve_file".to_string()).unwrap();
	/// ```
	pub fn add_route(&mut self, pattern: &str, handler_id: String) -> Result<(), RadixRouterError> {
		self.router
			.insert(pattern, handler_id)
			.map_err(|e| RadixRouterError::InsertionFailed(e.to_string()))
	}

	/// Match a path and return handler ID with extracted parameters
	///
	/// Performs O(m) matching where m is the path length.
	///
	/// # Arguments
	///
	/// * `path` - Request path to match
	///
	/// # Returns
	///
	/// `Some((handler_id, params))` if matched, `None` otherwise.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_urls::routers::{RadixRouter, path};
	///
	/// let mut router = RadixRouter::new();
	/// router.add_route(path!("/users/{id}/posts/{post_id}/"), "post_detail".to_string()).unwrap();
	///
	/// let result = router.match_path("/users/123/posts/456/");
	/// assert!(result.is_some());
	///
	/// let (handler_id, params) = result.unwrap();
	/// assert_eq!(handler_id, "post_detail");
	/// assert_eq!(params.get("id"), Some(&"123".to_string()));
	/// assert_eq!(params.get("post_id"), Some(&"456".to_string()));
	/// ```
	pub fn match_path(&self, path: &str) -> Option<(String, HashMap<String, String>)> {
		match self.router.at(path) {
			Ok(matched) => {
				let handler_id = matched.value.clone();
				let params: HashMap<String, String> = matched
					.params
					.iter()
					.map(|(k, v)| (k.to_string(), v.to_string()))
					.collect();

				Some((handler_id, params))
			}
			Err(_) => None,
		}
	}
}

impl Default for RadixRouter {
	fn default() -> Self {
		Self::new()
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_simple_pattern() {
		let pattern = PathPattern::new(reinhardt_routers_macros::path!("/users/")).unwrap();
		assert!(pattern.regex.is_match("/users/"));
		assert!(!pattern.regex.is_match("/users/123/"));
	}

	#[test]
	fn test_parameter_pattern() {
		let pattern = PathPattern::new(reinhardt_routers_macros::path!("/users/{id}/")).unwrap();
		assert_eq!(pattern.param_names(), &["id"]);
		assert!(pattern.regex.is_match("/users/123/"));
		assert!(!pattern.regex.is_match("/users/"));
	}

	#[test]
	fn test_pattern_multiple_parameters() {
		let pattern = PathPattern::new(reinhardt_routers_macros::path!(
			"/users/{user_id}/posts/{post_id}/"
		))
		.unwrap();
		assert_eq!(pattern.param_names(), &["user_id", "post_id"]);
		assert!(pattern.regex.is_match("/users/123/posts/456/"));
	}

	#[test]
	fn test_path_matcher() {
		let mut matcher = PathMatcher::new();
		matcher.add_pattern(
			PathPattern::new(reinhardt_routers_macros::path!("/users/")).unwrap(),
			"users_list".to_string(),
		);
		matcher.add_pattern(
			PathPattern::new(reinhardt_routers_macros::path!("/users/{id}/")).unwrap(),
			"users_detail".to_string(),
		);

		let result = matcher.match_path("/users/123/");
		assert!(result.is_some());
		let (handler_id, params) = result.unwrap();
		assert_eq!(handler_id, "users_detail");
		assert_eq!(params.get("id"), Some(&"123".to_string()));
	}

	// ===================================================================
	// URL Reversal Tests with Aho-Corasick
	// ===================================================================

	#[test]
	fn test_reverse_simple_pattern_no_params() {
		let pattern = PathPattern::new(reinhardt_routers_macros::path!("/users/")).unwrap();
		let params = HashMap::new();

		let result = pattern.reverse(&params).unwrap();
		assert_eq!(result, "/users/");
	}

	#[test]
	fn test_reverse_single_parameter() {
		let pattern = PathPattern::new(reinhardt_routers_macros::path!("/users/{id}/")).unwrap();
		let mut params = HashMap::new();
		params.insert("id".to_string(), "123".to_string());

		let result = pattern.reverse(&params).unwrap();
		assert_eq!(result, "/users/123/");
	}

	#[test]
	fn test_reverse_multiple_parameters() {
		let pattern = PathPattern::new(reinhardt_routers_macros::path!(
			"/users/{user_id}/posts/{post_id}/"
		))
		.unwrap();
		let mut params = HashMap::new();
		params.insert("user_id".to_string(), "42".to_string());
		params.insert("post_id".to_string(), "100".to_string());

		let result = pattern.reverse(&params).unwrap();
		assert_eq!(result, "/users/42/posts/100/");
	}

	#[test]
	fn test_reverse_many_parameters() {
		// Test with 10+ parameters to demonstrate Aho-Corasick performance
		let pattern = PathPattern::new(
			"/api/{p1}/{p2}/{p3}/{p4}/{p5}/{p6}/{p7}/{p8}/{p9}/{p10}/{p11}/{p12}/",
		)
		.unwrap();

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
		params.insert("p11".to_string(), "v11".to_string());
		params.insert("p12".to_string(), "v12".to_string());

		let result = pattern.reverse(&params).unwrap();
		assert_eq!(result, "/api/v1/v2/v3/v4/v5/v6/v7/v8/v9/v10/v11/v12/");
	}

	#[test]
	fn test_reverse_consecutive_placeholders() {
		let pattern = PathPattern::new("/{a}{b}/").unwrap();
		let mut params = HashMap::new();
		params.insert("a".to_string(), "1".to_string());
		params.insert("b".to_string(), "2".to_string());

		let result = pattern.reverse(&params).unwrap();
		assert_eq!(result, "/12/");
	}

	#[test]
	fn test_reverse_missing_parameter() {
		let pattern = PathPattern::new(reinhardt_routers_macros::path!("/users/{id}/")).unwrap();
		let params = HashMap::new();

		let result = pattern.reverse(&params);
		assert!(result.is_err());
		assert!(
			result
				.unwrap_err()
				.contains("Missing required parameter: id")
		);
	}

	#[test]
	fn test_reverse_partial_parameters() {
		let pattern = PathPattern::new(reinhardt_routers_macros::path!(
			"/users/{user_id}/posts/{post_id}/"
		))
		.unwrap();
		let mut params = HashMap::new();
		params.insert("user_id".to_string(), "42".to_string());
		// Missing post_id

		let result = pattern.reverse(&params);
		assert!(result.is_err());
		assert!(result.unwrap_err().contains("Missing required parameter"));
	}

	#[test]
	fn test_reverse_special_chars_in_values() {
		let pattern = PathPattern::new(reinhardt_routers_macros::path!("/items/{id}/")).unwrap();
		let mut params = HashMap::new();
		params.insert("id".to_string(), "foo-bar_123".to_string());

		let result = pattern.reverse(&params).unwrap();
		assert_eq!(result, "/items/foo-bar_123/");
	}

	#[test]
	fn test_reverse_numeric_values() {
		let pattern = PathPattern::new(reinhardt_routers_macros::path!("/items/{id}/")).unwrap();
		let mut params = HashMap::new();
		params.insert("id".to_string(), "12345".to_string());

		let result = pattern.reverse(&params).unwrap();
		assert_eq!(result, "/items/12345/");
	}

	#[test]
	fn test_reverse_unicode_values() {
		let pattern = PathPattern::new(reinhardt_routers_macros::path!("/users/{name}/")).unwrap();
		let mut params = HashMap::new();
		params.insert("name".to_string(), "ユーザー".to_string());

		let result = pattern.reverse(&params).unwrap();
		assert_eq!(result, "/users/ユーザー/");
	}

	#[test]
	fn test_reverse_param_at_start() {
		let pattern = PathPattern::new("{lang}/users/").unwrap();
		let mut params = HashMap::new();
		params.insert("lang".to_string(), "ja".to_string());

		let result = pattern.reverse(&params).unwrap();
		assert_eq!(result, "ja/users/");
	}

	#[test]
	fn test_reverse_param_at_end() {
		let pattern = PathPattern::new("/api/data.{format}").unwrap();
		let mut params = HashMap::new();
		params.insert("format".to_string(), "json".to_string());

		let result = pattern.reverse(&params).unwrap();
		assert_eq!(result, "/api/data.json");
	}

	#[test]
	fn test_reverse_complex_mixed_content() {
		let pattern = PathPattern::new("/items/{id}/actions/{action}/execute").unwrap();
		let mut params = HashMap::new();
		params.insert("id".to_string(), "123".to_string());
		params.insert("action".to_string(), "edit".to_string());

		let result = pattern.reverse(&params).unwrap();
		assert_eq!(result, "/items/123/actions/edit/execute");
	}

	#[test]
	fn test_reverse_long_value() {
		let pattern = PathPattern::new(reinhardt_routers_macros::path!("/items/{id}/")).unwrap();
		let mut params = HashMap::new();
		let long_id = "a".repeat(1000);
		params.insert("id".to_string(), long_id.clone());

		let result = pattern.reverse(&params).unwrap();
		assert_eq!(result, format!("/items/{}/", long_id));
	}

	#[test]
	fn test_reverse_empty_value() {
		let pattern = PathPattern::new(reinhardt_routers_macros::path!("/items/{id}/")).unwrap();
		let mut params = HashMap::new();
		params.insert("id".to_string(), "".to_string());

		let result = pattern.reverse(&params).unwrap();
		assert_eq!(result, "/items//");
	}

	#[test]
	fn test_reverse_extra_parameters() {
		// Extra parameters should be ignored
		let pattern = PathPattern::new(reinhardt_routers_macros::path!("/users/{id}/")).unwrap();
		let mut params = HashMap::new();
		params.insert("id".to_string(), "123".to_string());
		params.insert("extra".to_string(), "ignored".to_string());

		let result = pattern.reverse(&params).unwrap();
		assert_eq!(result, "/users/123/");
	}

	// ===================================================================
	// RadixRouter Tests
	// ===================================================================

	#[test]
	fn test_radix_router_basic_matching() {
		let mut router = RadixRouter::new();
		router
			.add_route("/users/", "users_list".to_string())
			.unwrap();
		router
			.add_route("/users/{id}/", "users_detail".to_string())
			.unwrap();

		// Match list route
		let result = router.match_path("/users/");
		assert!(result.is_some());
		let (handler_id, params) = result.unwrap();
		assert_eq!(handler_id, "users_list");
		assert!(params.is_empty());

		// Match detail route
		let result = router.match_path("/users/123/");
		assert!(result.is_some());
		let (handler_id, params) = result.unwrap();
		assert_eq!(handler_id, "users_detail");
		assert_eq!(params.get("id"), Some(&"123".to_string()));
	}

	#[test]
	fn test_radix_router_multiple_parameters() {
		let mut router = RadixRouter::new();
		router
			.add_route("/users/{id}/posts/{post_id}/", "post_detail".to_string())
			.unwrap();

		let result = router.match_path("/users/123/posts/456/");
		assert!(result.is_some());
		let (handler_id, params) = result.unwrap();
		assert_eq!(handler_id, "post_detail");
		assert_eq!(params.get("id"), Some(&"123".to_string()));
		assert_eq!(params.get("post_id"), Some(&"456".to_string()));
	}

	#[test]
	fn test_radix_router_wildcard() {
		let mut router = RadixRouter::new();
		router
			.add_route("/files/{*path}", "serve_file".to_string())
			.unwrap();

		let result = router.match_path("/files/images/logo.png");
		assert!(result.is_some());
		let (handler_id, params) = result.unwrap();
		assert_eq!(handler_id, "serve_file");
		assert_eq!(params.get("path"), Some(&"images/logo.png".to_string()));
	}

	#[test]
	fn test_radix_router_no_match() {
		let mut router = RadixRouter::new();
		router
			.add_route("/users/", "users_list".to_string())
			.unwrap();

		let result = router.match_path("/posts/");
		assert!(result.is_none());
	}

	#[test]
	fn test_path_matcher_radix_tree_mode() {
		let mut matcher = PathMatcher::with_mode(MatchingMode::RadixTree);
		matcher.add_pattern(
			PathPattern::new(reinhardt_routers_macros::path!("/users/")).unwrap(),
			"users_list".to_string(),
		);
		matcher.add_pattern(
			PathPattern::new(reinhardt_routers_macros::path!("/users/{id}/")).unwrap(),
			"users_detail".to_string(),
		);

		assert_eq!(matcher.mode(), MatchingMode::RadixTree);

		let result = matcher.match_path("/users/123/");
		assert!(result.is_some());
		let (handler_id, params) = result.unwrap();
		assert_eq!(handler_id, "users_detail");
		assert_eq!(params.get("id"), Some(&"123".to_string()));
	}

	#[test]
	fn test_path_matcher_enable_radix_tree() {
		let mut matcher = PathMatcher::new();
		matcher.add_pattern(
			PathPattern::new(reinhardt_routers_macros::path!("/users/")).unwrap(),
			"users_list".to_string(),
		);

		// Initially in linear mode
		assert_eq!(matcher.mode(), MatchingMode::Linear);

		// Enable radix tree mode
		matcher.enable_radix_tree();
		assert_eq!(matcher.mode(), MatchingMode::RadixTree);

		// Should still work after mode switch
		let result = matcher.match_path("/users/");
		assert!(result.is_some());
	}

	#[test]
	fn test_path_matcher_linear_vs_radix() {
		// Create two matchers with same routes
		let mut linear_matcher = PathMatcher::new();
		let mut radix_matcher = PathMatcher::with_mode(MatchingMode::RadixTree);

		for i in 1..=10 {
			let pattern = PathPattern::new(format!("/route{}/{{id}}/", i)).unwrap();
			linear_matcher.add_pattern(pattern.clone(), format!("handler_{}", i));
			radix_matcher.add_pattern(pattern, format!("handler_{}", i));
		}

		// Both should produce the same results
		for i in 1..=10 {
			let path = format!("/route{}/123/", i);
			let linear_result = linear_matcher.match_path(&path);
			let radix_result = radix_matcher.match_path(&path);

			assert_eq!(linear_result, radix_result);
			assert!(linear_result.is_some());
		}
	}

	// ===================================================================
	// Path traversal prevention tests (Issue #425)
	// ===================================================================

	#[test]
	fn test_path_type_rejects_traversal() {
		// Arrange
		let pattern = PathPattern::new("/files/{<path:filepath>}").unwrap();

		// Act & Assert - should reject `..` segments
		assert!(
			pattern
				.extract_params("/files/../../../etc/passwd")
				.is_none(),
			"Path type should reject directory traversal"
		);
		assert!(
			pattern
				.extract_params("/files/foo/../../etc/passwd")
				.is_none(),
			"Path type should reject embedded directory traversal"
		);
	}

	#[test]
	fn test_path_type_allows_valid_paths() {
		// Arrange
		let pattern = PathPattern::new("/files/{<path:filepath>}").unwrap();

		// Act
		let result = pattern.extract_params("/files/images/logo.png");

		// Assert
		assert!(result.is_some());
		let params = result.unwrap();
		assert_eq!(params.get("filepath"), Some(&"images/logo.png".to_string()));
	}

	#[test]
	fn test_path_type_allows_dotfiles() {
		// Arrange
		let pattern = PathPattern::new("/files/{<path:filepath>}").unwrap();

		// Act
		let result = pattern.extract_params("/files/.gitignore");

		// Assert
		assert!(result.is_some());
		let params = result.unwrap();
		assert_eq!(params.get("filepath"), Some(&".gitignore".to_string()));
	}

	#[test]
	fn test_path_type_matcher_rejects_traversal() {
		// Arrange
		let mut matcher = PathMatcher::new();
		matcher.add_pattern(
			PathPattern::new("/files/{<path:filepath>}").unwrap(),
			"serve_file".to_string(),
		);

		// Act & Assert
		assert!(
			matcher.match_path("/files/../../../etc/passwd").is_none(),
			"PathMatcher should reject directory traversal in path params"
		);

		// Valid path should work
		let result = matcher.match_path("/files/css/style.css");
		assert!(result.is_some());
	}

	#[test]
	fn test_validate_path_param_function() {
		// Normal paths should pass
		assert!(validate_path_param("images/logo.png"));
		assert!(validate_path_param("css/style.css"));
		assert!(validate_path_param(".gitignore"));
		assert!(validate_path_param("dir/.hidden"));

		// Traversal attacks should fail
		assert!(!validate_path_param("../etc/passwd"));
		assert!(!validate_path_param("foo/../../bar"));
		assert!(!validate_path_param(".."));
		assert!(!validate_path_param("foo/.."));

		// Null bytes should fail
		assert!(!validate_path_param("foo\0bar"));
	}

	// ===================================================================
	// Encoded path traversal prevention tests (Issue #425)
	// ===================================================================

	#[test]
	fn test_validate_path_param_rejects_encoded_traversal() {
		// Arrange & Act & Assert
		// Percent-encoded dot sequences (%2e = '.')
		assert!(!validate_path_param("%2e%2e/%2e%2e/etc/passwd"));
		assert!(!validate_path_param("foo/%2e%2e/bar"));
		assert!(!validate_path_param("%2E%2E/secret"));

		// Percent-encoded slash (%2f = '/')
		assert!(!validate_path_param("foo%2fbar"));
		assert!(!validate_path_param("..%2f..%2fetc%2fpasswd"));
		assert!(!validate_path_param("foo%2Fbar"));

		// Percent-encoded backslash (%5c = '\')
		assert!(!validate_path_param("foo%5cbar"));
		assert!(!validate_path_param("..%5C..%5Csecret"));

		// Percent-encoded null byte (%00)
		assert!(!validate_path_param("file%00.txt"));
	}

	#[test]
	fn test_validate_path_param_rejects_absolute_paths() {
		// Arrange & Act & Assert
		assert!(!validate_path_param("/etc/passwd"));
		assert!(!validate_path_param("\\windows\\system32"));
	}

	#[test]
	fn test_path_type_rejects_encoded_traversal() {
		// Arrange
		let pattern = PathPattern::new("/files/{<path:filepath>}").unwrap();

		// Act & Assert - percent-encoded traversal
		assert!(
			pattern
				.extract_params("/files/%2e%2e/%2e%2e/etc/passwd")
				.is_none(),
			"Path type should reject percent-encoded traversal"
		);
		assert!(
			pattern
				.extract_params("/files/..%2f..%2fetc%2fpasswd")
				.is_none(),
			"Path type should reject mixed encoded traversal"
		);
		assert!(
			pattern.extract_params("/files/foo%00bar").is_none(),
			"Path type should reject encoded null bytes"
		);
	}

	#[test]
	fn test_path_type_rejects_absolute_path_param() {
		// Arrange
		let pattern = PathPattern::new("/files/{<path:filepath>}").unwrap();

		// Act & Assert - absolute paths in parameter value
		// Note: the regex `.+` will match, but validation rejects absolute paths
		assert!(
			pattern.extract_params("/files//etc/passwd").is_none(),
			"Path type should reject absolute path in parameter"
		);
	}

	#[test]
	fn test_radix_tree_mode_rejects_traversal() {
		// Arrange
		let mut matcher = PathMatcher::with_mode(MatchingMode::RadixTree);
		matcher.add_pattern(
			PathPattern::new("/files/{<path:filepath>}").unwrap(),
			"serve_file".to_string(),
		);

		// Act & Assert - should reject traversal in RadixTree mode
		assert!(
			matcher.match_path("/files/../../../etc/passwd").is_none(),
			"RadixTree mode should reject directory traversal in path params"
		);
		assert!(
			matcher.match_path("/files/foo/../../etc/passwd").is_none(),
			"RadixTree mode should reject embedded directory traversal"
		);

		// Valid path should work
		let result = matcher.match_path("/files/css/style.css");
		assert!(result.is_some());
		let (handler_id, params) = result.unwrap();
		assert_eq!(handler_id, "serve_file");
		assert_eq!(params.get("filepath"), Some(&"css/style.css".to_string()));
	}

	#[test]
	fn test_radix_tree_mode_rejects_encoded_traversal() {
		// Arrange
		let mut matcher = PathMatcher::with_mode(MatchingMode::RadixTree);
		matcher.add_pattern(
			PathPattern::new("/files/{<path:filepath>}").unwrap(),
			"serve_file".to_string(),
		);

		// Act & Assert - percent-encoded traversal
		assert!(
			matcher
				.match_path("/files/%2e%2e/%2e%2e/etc/passwd")
				.is_none(),
			"RadixTree mode should reject percent-encoded traversal"
		);
		assert!(
			matcher
				.match_path("/files/..%2f..%2fetc%2fpasswd")
				.is_none(),
			"RadixTree mode should reject mixed encoded traversal"
		);

		// Null byte injection
		assert!(
			matcher.match_path("/files/foo%00bar").is_none(),
			"RadixTree mode should reject encoded null bytes"
		);
	}

	// ===================================================================
	// URL reversal parameter injection prevention tests (Issue #423)
	// ===================================================================

	#[test]
	fn test_reverse_rejects_path_separator_injection() {
		// Arrange
		let pattern = PathPattern::new(reinhardt_routers_macros::path!("/users/{id}/")).unwrap();
		let mut params = HashMap::new();
		params.insert("id".to_string(), "123/../../admin".to_string());

		// Act
		let result = pattern.reverse(&params);

		// Assert
		assert!(
			result.is_err(),
			"Reverse should reject path separators in parameter values"
		);
	}

	#[test]
	fn test_reverse_rejects_query_string_injection() {
		// Arrange
		let pattern = PathPattern::new(reinhardt_routers_macros::path!("/users/{id}/")).unwrap();
		let mut params = HashMap::new();
		params.insert("id".to_string(), "123?admin=true".to_string());

		// Act
		let result = pattern.reverse(&params);

		// Assert
		assert!(
			result.is_err(),
			"Reverse should reject query string delimiters in parameter values"
		);
	}

	#[test]
	fn test_reverse_rejects_fragment_injection() {
		// Arrange
		let pattern = PathPattern::new(reinhardt_routers_macros::path!("/users/{id}/")).unwrap();
		let mut params = HashMap::new();
		params.insert("id".to_string(), "123#fragment".to_string());

		// Act
		let result = pattern.reverse(&params);

		// Assert
		assert!(
			result.is_err(),
			"Reverse should reject fragment identifiers in parameter values"
		);
	}

	#[test]
	fn test_reverse_rejects_encoded_injection() {
		// Arrange
		let pattern = PathPattern::new(reinhardt_routers_macros::path!("/users/{id}/")).unwrap();
		let mut params = HashMap::new();
		params.insert("id".to_string(), "123%2f..%2f..%2fadmin".to_string());

		// Act
		let result = pattern.reverse(&params);

		// Assert
		assert!(
			result.is_err(),
			"Reverse should reject percent-encoded dangerous characters"
		);
	}

	#[test]
	fn test_reverse_allows_safe_values() {
		// Arrange
		let pattern =
			PathPattern::new(reinhardt_routers_macros::path!("/users/{id}/posts/{slug}/")).unwrap();
		let mut params = HashMap::new();
		params.insert("id".to_string(), "123".to_string());
		params.insert("slug".to_string(), "my-blog-post".to_string());

		// Act
		let result = pattern.reverse(&params);

		// Assert
		assert!(result.is_ok());
		assert_eq!(result.unwrap(), "/users/123/posts/my-blog-post/");
	}

	#[test]
	fn test_reverse_allows_unicode_values() {
		// Arrange
		let pattern = PathPattern::new(reinhardt_routers_macros::path!("/users/{name}/")).unwrap();
		let mut params = HashMap::new();
		params.insert("name".to_string(), "ユーザー".to_string());

		// Act
		let result = pattern.reverse(&params);

		// Assert
		assert!(result.is_ok());
		assert_eq!(result.unwrap(), "/users/ユーザー/");
	}

	#[test]
	fn test_validate_reverse_param_function() {
		// Arrange & Act & Assert

		// Safe values should pass
		assert!(validate_reverse_param("123"));
		assert!(validate_reverse_param("my-slug"));
		assert!(validate_reverse_param("foo_bar"));
		assert!(validate_reverse_param("ユーザー"));
		assert!(validate_reverse_param("hello-world-123"));

		// Path separators should fail
		assert!(!validate_reverse_param("foo/bar"));
		assert!(!validate_reverse_param("foo\\bar"));

		// URL-special characters should fail
		assert!(!validate_reverse_param("foo?bar=1"));
		assert!(!validate_reverse_param("foo#bar"));

		// Null bytes should fail
		assert!(!validate_reverse_param("foo\0bar"));

		// Encoded sequences should fail
		assert!(!validate_reverse_param("foo%2fbar"));
		assert!(!validate_reverse_param("foo%2ebar"));
		assert!(!validate_reverse_param("foo%5cbar"));
		assert!(!validate_reverse_param("foo%3fbar"));
		assert!(!validate_reverse_param("foo%23bar"));
		assert!(!validate_reverse_param("foo%00bar"));
	}

	// ===================================================================
	// ReDoS prevention tests (Issue #430)
	// ===================================================================

	#[test]
	fn test_pattern_rejects_excessive_length() {
		// Arrange: a pattern exceeding MAX_PATTERN_LENGTH (1024 bytes)
		let long_pattern = "/".to_string() + &"a".repeat(1025);

		// Act
		let result = PathPattern::new(long_pattern);

		// Assert
		assert!(result.is_err());
		assert!(
			result
				.unwrap_err()
				.contains("exceeds maximum allowed length")
		);
	}

	#[test]
	fn test_pattern_accepts_within_length_limit() {
		// Arrange: a pattern within the limit
		let pattern = "/users/{id}/posts/{post_id}/";

		// Act
		let result = PathPattern::new(pattern);

		// Assert
		assert!(result.is_ok());
	}

	#[test]
	fn test_pattern_rejects_at_boundary() {
		// Arrange: a pattern at exactly the boundary + 1
		let pattern = "/".to_string() + &"a/".repeat(512) + "end";
		if pattern.len() > MAX_PATTERN_LENGTH {
			// Act
			let result = PathPattern::new(pattern);

			// Assert
			assert!(result.is_err());
		}
	}

	// ===================================================================
	// Path segment count limit tests (Issue #431)
	// ===================================================================

	#[test]
	fn test_pattern_rejects_excessive_segments() {
		// Arrange: a pattern with more than MAX_PATH_SEGMENTS segments
		let segments: Vec<&str> = (0..35).map(|_| "seg").collect();
		let pattern = format!("/{}/", segments.join("/"));

		// Act
		let result = PathPattern::new(pattern);

		// Assert
		assert!(result.is_err());
		assert!(result.unwrap_err().contains("exceeding maximum"));
	}

	#[test]
	fn test_pattern_accepts_within_segment_limit() {
		// Arrange: a pattern with few segments
		let pattern = "/a/b/c/d/e/";

		// Act
		let result = PathPattern::new(pattern);

		// Assert
		assert!(result.is_ok());
	}

	#[test]
	fn test_pattern_accepts_at_segment_boundary() {
		// Arrange: a pattern at exactly the maximum segment count
		let segments: Vec<String> = (0..MAX_PATH_SEGMENTS - 2)
			.map(|i| format!("s{}", i))
			.collect();
		let pattern = format!("/{}/", segments.join("/"));

		// Act
		let result = PathPattern::new(&pattern);

		// Assert
		assert!(result.is_ok());
	}
}
