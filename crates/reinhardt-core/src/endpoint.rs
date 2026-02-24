#![cfg(not(target_arch = "wasm32"))]

//! Endpoint metadata trait for HTTP Method Macros
//!
//! This module provides the `EndpointInfo` trait that HTTP Method Macros
//! (`#[get]`, `#[post]`, etc.) implement to provide route metadata.

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
#[derive(Debug, Clone)]
pub struct EndpointMetadata {
	pub path: &'static str,
	pub method: &'static str,
	pub name: Option<&'static str>,
	pub function_name: &'static str,
	pub module_path: &'static str,

	/// Type name of the request body (e.g., "CreateUserRequest")
	/// Extracted from parameter extractors like `Json<T>`, `Form<T>`, `Body<T>`
	pub request_body_type: Option<&'static str>,

	/// Content-Type of the request body (e.g., "application/json", "application/x-www-form-urlencoded")
	pub request_content_type: Option<&'static str>,
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
