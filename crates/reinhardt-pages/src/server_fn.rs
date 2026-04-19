//! Server Functions (RPC)
//!
//! This module provides runtime support for server functions - a Leptos/Dioxus-style
//! RPC mechanism that allows frontend code to call server-side functions as if they
//! were local.
//!
//! ## Architecture
//!
//! Server functions use the `#[server_fn]` macro to generate:
//!
//! - **Client-side (WASM)**: HTTP request stub
//! - **Server-side (non-WASM)**: Route handler
//!
//! The macro performs conditional compilation to ensure code separation.
//!
//! ## Example
//!
//! ```ignore
//! use reinhardt_pages_macros::server_fn;
//! use reinhardt_pages::server_fn::ServerFnError;
//!
//! #[server_fn]
//! async fn get_user(id: u32) -> Result<User, ServerFnError> {
//!     // Server-side only code
//!     let user = User::find_by_id(id).await?;
//!     Ok(user)
//! }
//!
//! // Client usage (feels like a normal function call!)
//! async fn on_click() {
//!     let user = get_user(42).await?;
//!     println!("User: {:?}", user);
//! }
//! ```
//!
//! ## Features
//!
//! - **Type-safe RPC**: Arguments and return values are type-checked at compile time
//! - **Automatic CSRF protection**: Tokens automatically injected on client side
//! - **Session propagation**: Cookie-based sessions automatically work
//! - **Dependency Injection**: `#[inject]` parameters resolved on server side
//! - **Multiple codecs**: JSON (default), URL encoding, MessagePack
//!
//! ## Codec Selection (Week 4 Day 3)
//!
//! Server functions support multiple serialization formats via the `codec` parameter:
//!
//! ```ignore
//! use reinhardt_pages_macros::server_fn;
//!
//! // JSON codec (default) - human-readable, widely supported
//! #[server_fn] // or explicitly: #[server_fn(codec = "json")]
//! async fn create_post(title: String, body: String) -> Result<Post, ServerFnError> {
//!     // Complex nested structures work well with JSON
//!     Ok(Post { title, body })
//! }
//!
//! // URL encoding codec - for GET requests with simple parameters
//! #[server_fn(codec = "url")]
//! async fn search(query: String, page: u32) -> Result<Vec<SearchResult>, ServerFnError> {
//!     // Simple key-value pairs encoded as query parameters
//!     // Suitable for GET requests
//!     Ok(vec![])
//! }
//!
//! // MessagePack codec (optional, requires "msgpack" feature)
//! #[server_fn(codec = "msgpack")]
//! async fn process_large_data(data: Vec<u8>) -> Result<(), ServerFnError> {
//!     // Binary format, more efficient for large payloads
//!     Ok(())
//! }
//! ```

pub mod codec;
#[cfg(native)]
pub mod injectable;
#[cfg(feature = "msw")]
pub mod mockable;
#[cfg(native)]
pub mod negotiation;
#[cfg(native)]
pub mod registration;
#[cfg(native)]
pub mod registry;
#[cfg(native)]
pub mod router_ext;
pub mod server_fn_trait;

// Re-exports
#[cfg(feature = "msgpack")]
pub use codec::MessagePackCodec;
pub use codec::{Codec, JsonCodec, UrlCodec};
#[cfg(native)]
pub use injectable::{ServerFnBody, ServerFnRequest};
#[cfg(feature = "msw")]
pub use mockable::MockableServerFn;
#[cfg(native)]
pub use negotiation::convert_body_for_codec;
#[cfg(native)]
pub use registration::ServerFnRegistration;
#[cfg(native)]
pub use registry::{ServerFnHandler, ServerFnRoute};
#[cfg(native)]
pub use router_ext::ServerFnRouterExt;
pub use server_fn_trait::{ServerFn, ServerFnError};

// Re-export the macro for convenience
pub use reinhardt_pages_macros::server_fn;

/// Resolves a server function endpoint path by prepending the mount prefix.
///
/// On WASM targets, reads the `<meta name="server-fn-prefix">` tag from the
/// document to determine the mount prefix. The prefix is cached after the first
/// DOM lookup for performance.
///
/// On non-WASM targets, returns the path unchanged (server-side routing handles
/// prefix resolution via router mounting).
///
/// # Examples
///
/// ```ignore
/// // With <meta name="server-fn-prefix" content="/admin"> in the document:
/// assert_eq!(resolve_endpoint("/api/server_fn/get_list"), "/admin/api/server_fn/get_list");
///
/// // Without the meta tag:
/// assert_eq!(resolve_endpoint("/api/server_fn/get_list"), "/api/server_fn/get_list");
/// ```
#[cfg(wasm)]
pub fn resolve_endpoint(path: &str) -> String {
	use std::cell::RefCell;

	thread_local! {
		static CACHED_PREFIX: RefCell<Option<String>> = const { RefCell::new(None) };
	}

	CACHED_PREFIX.with(|cache| {
		let mut cache = cache.borrow_mut();
		if cache.is_none() {
			let prefix = web_sys::window()
				.and_then(|w| w.document())
				.and_then(|d| {
					d.query_selector("meta[name='server-fn-prefix']")
						.ok()
						.flatten()
				})
				.and_then(|el| el.get_attribute("content"))
				.unwrap_or_default();
			*cache = Some(prefix);
		}
		let prefix = cache.as_deref().unwrap_or("");
		let relative = if prefix.is_empty() {
			path.to_string()
		} else {
			let prefix = prefix.trim_end_matches('/');
			format!("{}{}", prefix, path)
		};
		// reqwest requires absolute URLs; prepend the page origin for WASM.
		web_sys::window()
			.and_then(|w| w.location().origin().ok())
			.map(|origin| format!("{}{}", origin, relative))
			.unwrap_or(relative)
	})
}

/// Non-WASM identity implementation - returns the path unchanged.
#[cfg(native)]
pub fn resolve_endpoint(path: &str) -> String {
	path.to_string()
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;

	#[rstest]
	#[case("/api/server_fn/get_list", "/api/server_fn/get_list")]
	#[case("/api/server_fn/admin_login", "/api/server_fn/admin_login")]
	#[case("/custom/endpoint", "/custom/endpoint")]
	fn test_resolve_endpoint_returns_path_unchanged_on_server(
		#[case] input: &str,
		#[case] expected: &str,
	) {
		// Arrange & Act
		let result = resolve_endpoint(input);

		// Assert
		assert_eq!(result, expected);
	}
}
