#![cfg(native)]

//! Endpoint metadata trait for HTTP Method Macros
//!
//! This module provides the `EndpointInfo` trait that HTTP Method Macros
//! (`#[get]`, `#[post]`, etc.) implement to provide route metadata.

pub mod auth_protection;

pub use auth_protection::{AuthProtection, validate_endpoint_security};

use hyper::Method;

/// Endpoint metadata for OpenAPI generation
///
/// This struct is automatically submitted to the global inventory
/// by HTTP method decorator macros (`#[get]`, `#[post]`, etc.) at compile time.
/// It can be collected at runtime using `inventory::iter::<EndpointMetadata>()`.
///
/// # Example
///
/// ```rust,no_run
/// use reinhardt_core::endpoint::EndpointMetadata;
///
/// // Collect all registered endpoints
/// for metadata in inventory::iter::<EndpointMetadata>() {
///     println!("{} {}", metadata.method, metadata.path);
/// }
/// ```
// NOTE: #[non_exhaustive] is intentionally omitted in the pre-1.0 phase.
// EndpointMetadata is constructed via struct literals in both proc-macro codegen
// (reinhardt-core-macros) and test code across multiple crates. Adding
// #[non_exhaustive] would require a builder or constructor, which adds complexity
// without benefit before the public API stabilizes at 1.0.
#[derive(Debug, Clone)]
pub struct EndpointMetadata {
	/// URL path pattern for this endpoint (e.g., "/users/{id}/").
	pub path: &'static str,
	/// HTTP method name (e.g., "GET", "POST").
	pub method: &'static str,
	/// Optional route name for URL reversal.
	pub name: Option<&'static str>,
	/// Name of the handler function.
	pub function_name: &'static str,
	/// Module path where the handler is defined.
	pub module_path: &'static str,

	/// Type name of the request body (e.g., "CreateUserRequest")
	/// Extracted from parameter extractors like `Json<T>`, `Form<T>`, `Body<T>`
	pub request_body_type: Option<&'static str>,

	/// Content-Type of the request body (e.g., "application/json", "application/x-www-form-urlencoded")
	pub request_content_type: Option<&'static str>,

	/// Additional response definitions beyond the default 200
	/// Each entry: (status_code, description)
	pub responses: &'static [EndpointResponse],

	/// Response headers
	/// Each entry: (header_name, description)
	pub headers: &'static [EndpointHeader],

	/// Security requirements (e.g., "bearer", "api_key")
	pub security: &'static [&'static str],

	/// Authentication protection level detected from handler parameters.
	pub auth_protection: AuthProtection,

	/// Human-readable description of the guard expression (if any).
	pub guard_description: Option<&'static str>,
}

/// A response definition for an endpoint
#[derive(Debug, Clone, Copy)]
pub struct EndpointResponse {
	/// HTTP status code (e.g., 201, 404)
	pub status: u16,
	/// Description of the response
	pub description: &'static str,
}

/// A response header definition for an endpoint
#[derive(Debug, Clone, Copy)]
pub struct EndpointHeader {
	/// Header name (e.g., "X-Request-Id")
	pub name: &'static str,
	/// Description of the header
	pub description: &'static str,
}

// Register EndpointMetadata as a collectible type with inventory
inventory::collect!(EndpointMetadata);

/// Trait for endpoint metadata used by HTTP Method Macros
///
/// This trait is automatically implemented by HTTP Method Macros (`#[get]`, `#[post]`, etc.)
/// for the generated View types. It provides the route path, HTTP method, and name
/// for URL reversal.
///
/// # Examples
///
/// The HTTP Method Macro generates a View type that implements this trait:
///
/// ```rust,ignore
/// # use reinhardt_core_macros::get;
/// # use reinhardt_http::Response;
/// # use reinhardt_core::endpoint::EndpointInfo;
/// # use hyper::Method;
/// #[get("/users/{id}/", name = "get_user")]
/// pub async fn get_user(id: i64) -> Result<Response, Box<dyn std::error::Error>> {
///     Ok(Response::ok())
/// }
///
/// // Generates:
/// // pub struct GetUserView;
/// //
/// // impl EndpointInfo for GetUserView {
/// //     fn path() -> &'static str { "/users/{id}/" }
/// //     fn method() -> Method { Method::GET }
/// //     fn name() -> &'static str { "get_user" }
/// // }
/// ```
pub trait EndpointInfo: Send + Sync {
	/// Returns the route path pattern
	///
	/// Example: "/users/{id}/"
	fn path() -> &'static str;

	/// Returns the HTTP method for this endpoint
	///
	/// Example: Method::GET
	fn method() -> Method;

	/// Returns the route name for URL reversal
	///
	/// Example: "get_user"
	fn name() -> &'static str;
}
