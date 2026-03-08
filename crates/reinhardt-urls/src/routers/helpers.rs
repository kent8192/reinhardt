/// Helper functions for URL pattern definition
/// Inspired by Django's django.urls.conf module
use crate::routers::Route;
use nom::{
	IResult, Parser,
	branch::alt,
	bytes::complete::{tag, take_while1},
	character::complete::{anychar, char},
	combinator::map,
	multi::many0,
};
use reinhardt_http::Handler;

/// Create a route using simple path syntax
/// Similar to Django's path() function
///
/// This function accepts a handler directly without requiring `Arc` wrapping.
/// The `Arc` is created internally for you.
///
/// # Examples
///
/// ```
/// use reinhardt_urls::routers::path;
/// use reinhardt_http::Handler;
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
/// // Simple path without parameters - no Arc::new() needed!
/// let route = path("/users/", DummyHandler);
/// assert_eq!(route.path, "/users/");
///
/// // Path with parameters
/// let route = path("/users/{id}/", DummyHandler)
///     .with_name("user-detail");
/// assert_eq!(route.name, Some("user-detail".to_string()));
/// ```
pub fn path<H>(pattern: impl Into<String>, handler: H) -> Route
where
	H: Handler + 'static,
{
	Route::from_handler(pattern, handler)
}

/// Create a route using regex syntax
/// Similar to Django's re_path() function
///
/// Converts Django-style regex patterns to Reinhardt's pattern format.
/// Named groups `(?P<name>...)` are converted to `{name}` format.
///
/// This function accepts a handler directly without requiring `Arc` wrapping.
/// The `Arc` is created internally for you.
///
/// # Examples
///
/// ```
/// use reinhardt_urls::routers::re_path;
/// use reinhardt_http::Handler;
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
/// // Regex with named groups - no Arc::new() needed!
/// let route = re_path(r"^users/(?P<id>\d+)/$", DummyHandler)
///     .with_name("user-detail");
/// assert_eq!(route.path, "users/{id}/");
/// assert_eq!(route.name, Some("user-detail".to_string()));
/// ```
pub fn re_path<H>(regex: impl Into<String>, handler: H) -> Route
where
	H: Handler + 'static,
{
	let regex_str = regex.into();

	// Convert Django-style regex to our path pattern format
	let pattern = convert_regex_to_pattern(&regex_str);

	Route::from_handler(pattern, handler)
}

/// Convert Django-style regex pattern to our {param} format
/// This implementation uses nom parser combinators to properly handle
/// complex regex patterns including nested groups and escaped characters,
/// similar to Django's sophisticated implementation
fn convert_regex_to_pattern(regex: &str) -> String {
	let mut result = regex.to_string();

	// Remove common regex anchors that are implicit in our system
	result = result.strip_prefix("^").unwrap_or(&result).to_string();
	result = result.strip_suffix("$").unwrap_or(&result).to_string();

	// Parse and convert the pattern using nom
	match parse_regex_pattern(&result) {
		Ok((_, converted)) => converted,
		Err(_) => result, // Fallback to original if parsing fails
	}
}

/// Parse a regex pattern and convert named groups to {param} format
fn parse_regex_pattern(input: &str) -> IResult<&str, String> {
	let (input, parts) = many0(alt((
		map(parse_named_group, |name| format!("{{{}}}", name)),
		map(parse_escaped_char, String::from),
		map(parse_non_group_char, |c| c.to_string()),
	)))
	.parse(input)?;

	Ok((input, parts.join("")))
}

/// Parse a named group (?P<name>pattern) and extract the name
fn parse_named_group(input: &str) -> IResult<&str, String> {
	let (input, _) = tag("(?P<")(input)?;
	let (input, name) = take_while1(|c: char| c.is_alphanumeric() || c == '_')(input)?;
	let (input, _) = char('>')(input)?;

	// Parse the group content, handling nested parentheses
	let (input, _) = parse_group_content(input)?;

	Ok((input, name.to_string()))
}

/// Parse the content of a group, properly handling nested parentheses
fn parse_group_content(input: &str) -> IResult<&str, String> {
	let mut depth = 1;
	let mut chars = Vec::new();
	let mut remaining = input;
	let mut escaped = false;

	while !remaining.is_empty() && depth > 0 {
		let (rest, c) = anychar(remaining)?;

		if escaped {
			chars.push(c);
			escaped = false;
		} else if c == '\\' {
			chars.push(c);
			escaped = true;
		} else if c == '(' {
			chars.push(c);
			depth += 1;
		} else if c == ')' {
			depth -= 1;
			if depth > 0 {
				chars.push(c);
			}
		} else {
			chars.push(c);
		}

		remaining = rest;
	}

	Ok((remaining, chars.into_iter().collect()))
}

/// Parse an escaped character and unescape common ones
fn parse_escaped_char(input: &str) -> IResult<&str, String> {
	let (input, _) = char('\\')(input)?;
	let (input, c) = anychar(input)?;

	let result = match c {
		// Unescape common path characters
		'/' | '.' | '-' | '_' => c.to_string(),
		// Keep regex special sequences as-is for potential future use
		'd' | 'w' | 's' | 'D' | 'W' | 'S' | 'b' | 'B' => format!("\\{}", c),
		// Keep other escapes
		_ => format!("\\{}", c),
	};

	Ok((input, result))
}

/// Parse any character that's not part of a named group or escape sequence
fn parse_non_group_char(input: &str) -> IResult<&str, char> {
	// Match any character that doesn't start a named group or escape
	let (input, c) = anychar(input)?;

	if c == '(' {
		// Check if this is the start of a named group
		if input.starts_with("?P<") {
			return Err(nom::Err::Error(nom::error::Error::new(
				input,
				nom::error::ErrorKind::Tag,
			)));
		}
	}

	if c == '\\' {
		return Err(nom::Err::Error(nom::error::Error::new(
			input,
			nom::error::ErrorKind::Tag,
		)));
	}

	Ok((input, c))
}

/// Include another router's patterns under a prefix
/// Similar to Django's include() function
///
/// Returns a special marker type that DefaultRouter recognizes
/// to include all routes from the included router with the given prefix.
///
/// # Examples
///
/// ```
/// use reinhardt_urls::routers::{DefaultRouter, Router, path};
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
/// let mut users_router = DefaultRouter::new();
/// users_router.add_route(path("/", handler.clone()).with_name("list"));
///
/// let mut main_router = DefaultRouter::new();
/// let users_routes = users_router.get_routes().to_vec();
/// main_router.mount("/api/users", users_routes, Some("users".to_string()));
/// ```
pub struct IncludedRouter {
	/// URL prefix to prepend to all contained routes.
	pub prefix: String,
	/// Routes included under this prefix.
	pub routes: Vec<Route>,
	/// Optional namespace for reverse URL resolution.
	pub namespace: Option<String>,
}

impl IncludedRouter {
	/// Create a new `IncludedRouter` with the given prefix and routes.
	pub fn new(prefix: impl Into<String>, routes: Vec<Route>) -> Self {
		Self {
			prefix: prefix.into(),
			routes,
			namespace: None,
		}
	}

	/// Set the namespace for this included router.
	pub fn with_namespace(mut self, namespace: impl Into<String>) -> Self {
		self.namespace = Some(namespace.into());
		self
	}
}

/// Create an IncludedRouter from a list of routes
/// Similar to Django's include() function
///
/// # Arguments
///
/// * `prefix` - URL prefix to prepend to all included routes
/// * `routes` - Vector of routes to include
///
/// # Examples
///
/// ```
/// use reinhardt_urls::routers::{include_routes, path};
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
/// let user_routes = vec![
///     path("/", handler.clone()).with_name("list"),
///     path("/{id}/", handler).with_name("detail"),
/// ];
///
/// let included = include_routes("/users", user_routes)
///     .with_namespace("users");
/// assert_eq!(included.prefix, "/users");
/// assert_eq!(included.namespace, Some("users".to_string()));
/// ```
pub fn include_routes(prefix: impl Into<String>, routes: Vec<Route>) -> IncludedRouter {
	IncludedRouter::new(prefix, routes)
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::routers_macros::path;

	#[test]
	fn test_convert_regex_to_pattern() {
		// Simple regex
		assert_eq!(convert_regex_to_pattern(r"^users/$"), "users/");

		// Regex with named group
		assert_eq!(
			convert_regex_to_pattern(r"^users/(?P<id>\d+)/$"),
			"users/{id}/"
		);

		// Multiple named groups
		assert_eq!(
			convert_regex_to_pattern(r"^users/(?P<user_id>\d+)/posts/(?P<post_id>\d+)/$"),
			"users/{user_id}/posts/{post_id}/"
		);

		// Named group with complex pattern
		assert_eq!(
			convert_regex_to_pattern(r"^articles/(?P<year>\d{4})/(?P<month>\d{2})/$"),
			"articles/{year}/{month}/"
		);

		// Nested parentheses in pattern
		assert_eq!(
			convert_regex_to_pattern(r"^data/(?P<slug>[a-z]+(-[a-z]+)*)/$"),
			"data/{slug}/"
		);

		// Escaped characters
		assert_eq!(
			convert_regex_to_pattern(r"^files/(?P<path>[\w\/\-\.]+)/$"),
			"files/{path}/"
		);

		// Mixed escaped and unescaped slashes
		assert_eq!(
			convert_regex_to_pattern(r"^api\/v1/users/(?P<id>\d+)/$"),
			"api/v1/users/{id}/"
		);

		// Multiple patterns with different regex constructs
		assert_eq!(
			convert_regex_to_pattern(r"^(?P<category>\w+)/(?P<slug>[\w-]+)/(?P<id>\d+)/$"),
			"{category}/{slug}/{id}/"
		);

		// Pattern without anchors
		assert_eq!(
			convert_regex_to_pattern(r"users/(?P<id>\d+)/"),
			"users/{id}/"
		);

		// Pattern with only start anchor
		assert_eq!(
			convert_regex_to_pattern(r"^users/(?P<id>\d+)/"),
			"users/{id}/"
		);

		// Pattern with only end anchor
		assert_eq!(
			convert_regex_to_pattern(r"users/(?P<id>\d+)/$"),
			"users/{id}/"
		);
	}

	#[test]
	fn test_included_router_namespace() {
		let routes = vec![];
		let included = IncludedRouter::new(path!("/api"), routes).with_namespace("api");

		assert_eq!(included.prefix, path!("/api"));
		assert_eq!(included.namespace, Some("api".to_string()));
	}
}
