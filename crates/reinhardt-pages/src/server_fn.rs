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
//! ## Automatic Native Registration
//!
//! On native targets, an ordinary `#[server_fn]` submits its route metadata to
//! the linked inventory by default. Define each application's router beneath
//! the module that contains its `#[app_config]`, then collect only that
//! application's functions at its router boundary:
//!
//! ```ignore
//! use reinhardt::pages::server_fn::ServerFnRouterExt;
//! use reinhardt::ServerRouter;
//!
//! pub fn server_url_patterns() -> ServerRouter {
//!     ServerRouter::new().auto_server_fns(module_path!())
//! }
//! ```
//!
//! `auto_server_fns` is native-only. Its inventory metadata and route handlers
//! are not emitted for browser WASM, while generated markers and client stubs
//! remain available to shared client code. Applications do not need an
//! `inventory` dependency, a framework crate alias, or a custom cfg alias.
//!
//! Use `#[server_fn(auto_register = false)]` when a framework-owned or
//! deliberately explicit router must mount the marker with `.server_fn(...)`.
//! Do not combine automatic and explicit registration for the same endpoint.
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

#[cfg(native)]
pub mod auto_registration;
pub mod codec;
#[cfg(native)]
pub mod injectable;
pub mod metadata;
#[cfg(feature = "msw")]
pub mod mockable;
pub mod model_set;
#[cfg(native)]
pub mod negotiation;
#[cfg(native)]
pub mod registration;
#[cfg(native)]
pub mod registry;
#[cfg(native)]
pub mod router_ext;
pub mod server_fn_trait;
pub mod set;

// Re-exports
#[cfg(native)]
pub use auto_registration::{
	ServerFnInventoryEntry, ServerFnInventoryError, ServerFnRegister, validate_server_fn_inventory,
};
#[cfg(feature = "msgpack")]
pub use codec::MessagePackCodec;
pub use codec::{Codec, JsonCodec, UrlCodec};
#[cfg(native)]
pub use injectable::{ServerFnBody, ServerFnRequest};
pub use metadata::{
	ServerFnMetadata, ServerFnQueryArg, ServerFnQueryResult, ServerFnRequestMetadata,
	ServerFnResponseMetadata,
};
#[cfg(feature = "msw")]
pub use mockable::MockableServerFn;
#[cfg(all(native, feature = "model-server-fnset"))]
pub use model_set::{
	AllowAllPolicy, AllowAllPrincipal, CollectionActionContext, CollectionReadActionContext,
	CreateActionContext, CreateModelInput, DetailActionContext, DetailReadActionContext,
	ModelServerFnResource, ModelServerFnSet, PatchModelInput, PolicyPrincipal, ServerFnSetAction,
	ServerFnSetPolicy, UpdateModelInput,
};
pub use model_set::{
	FieldError, FieldErrors, ModelServerFnSetLink, Page, PageRequest, ServerFnListQuery,
	ServerFnResource, ServerFnSetError, ValidatedPageRequest,
};
#[cfg(native)]
pub use negotiation::convert_body_for_codec;
#[cfg(native)]
pub use registration::ServerFnRegistration;
#[cfg(native)]
pub use registry::{ServerFnHandler, ServerFnRoute};
#[cfg(native)]
pub use router_ext::ServerFnRouterExt;
pub use server_fn_trait::{
	ServerFn, ServerFnError, ServerFnErrorKind, ServerFnErrorPayload, ServerFnFieldError,
};
pub use set::{
	NamedServerFnSet, ServerFnSet, ServerFnSetActionMetadata, ServerFnSetActions,
	ServerFnSetChainExt, ServerFnSetCons, ServerFnSetMetadata, ServerFnSetNil,
	ServerFnSetRegistration,
};

#[cfg(all(native, feature = "testing", feature = "msw"))]
pub use crate::testing::component::server_fn_mock::{
	has_active_scope as has_active_server_fn_mock_scope, try_call_active_mock,
};

#[cfg(all(native, feature = "msw", not(feature = "testing")))]
/// Returns false outside component-test builds.
pub const fn has_active_server_fn_mock_scope() -> bool {
	false
}

#[cfg(all(native, feature = "msw", not(feature = "testing")))]
/// No-op native server-function mock probe outside component-test builds.
pub fn try_call_active_mock<S>(_: S::Args) -> Option<Result<S::Response, ServerFnError>>
where
	S: MockableServerFn + 'static,
	S::Args: 'static,
	S::Response: 'static,
{
	None
}

// Re-export the macro for convenience
pub use reinhardt_pages_macros::{server_fn, server_fnset};

#[cfg(any(wasm, test))]
fn prefixed_same_origin_path(prefix: &str, path: &str) -> String {
	let prefix = prefix.trim();
	if prefix.is_empty()
		|| !prefix.starts_with('/')
		|| prefix.starts_with("//")
		|| prefix_contains_url_parser_redirects(prefix)
	{
		return path.to_string();
	}

	let prefix = prefix.trim_end_matches('/');
	format!("{}{}", prefix, path)
}

#[cfg(any(wasm, test))]
fn prefix_contains_url_parser_redirects(prefix: &str) -> bool {
	prefix
		.chars()
		.any(|character| matches!(character, '\\' | '?' | '#') || character.is_ascii_whitespace())
		|| prefix.split('/').any(is_dot_path_segment)
}

#[cfg(any(wasm, test))]
fn is_dot_path_segment(segment: &str) -> bool {
	let mut dot_count = 0;
	let mut bytes = segment.as_bytes();

	while let Some((&byte, rest)) = bytes.split_first() {
		if byte == b'.' {
			dot_count += 1;
			bytes = rest;
			continue;
		}

		if byte == b'%' && rest.len() >= 2 && percent_encoded_byte(rest[0], rest[1]) == Some(b'.') {
			dot_count += 1;
			bytes = &rest[2..];
			continue;
		}

		return false;
	}

	matches!(dot_count, 1 | 2)
}

#[cfg(any(wasm, test))]
fn percent_encoded_byte(high: u8, low: u8) -> Option<u8> {
	Some(hex_value(high)? * 16 + hex_value(low)?)
}

#[cfg(any(wasm, test))]
fn hex_value(byte: u8) -> Option<u8> {
	match byte {
		b'0'..=b'9' => Some(byte - b'0'),
		b'a'..=b'f' => Some(byte - b'a' + 10),
		b'A'..=b'F' => Some(byte - b'A' + 10),
		_ => None,
	}
}

/// Resolves a server function endpoint path by prepending the client mount prefix.
///
/// Reads the `<meta name="server-fn-prefix">` tag from the document to
/// determine the mount prefix. The prefix is cached after the first DOM lookup
/// for performance. Only same-origin absolute path prefixes are honored;
/// absolute URLs, scheme-relative URLs, relative prefixes, parser slash
/// equivalents, query or fragment delimiters, and dot-segment prefixes are
/// ignored so credentialed server function requests cannot be redirected by
/// DOM-controlled metadata.
///
/// # Examples
///
/// ```ignore
/// // With <meta name="server-fn-prefix" content="/admin"> in the document:
/// assert_eq!(resolve_endpoint("/api/server_fn/get_list"), "/admin/api/server_fn/get_list");
///
/// // Without the meta tag:
/// assert_eq!(resolve_endpoint("/api/server_fn/get_list"), "/api/server_fn/get_list");
///
/// // Cross-origin prefixes are ignored:
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
		let relative = prefixed_same_origin_path(prefix, path);

		web_sys::window()
			.and_then(|w| w.location().href().ok())
			.and_then(|base| web_sys::Url::new_with_base(&relative, &base).ok())
			.map(|url| url.href())
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
	#[case("", "/api/server_fn/get_list", "/api/server_fn/get_list")]
	#[case("/admin", "/api/server_fn/get_list", "/admin/api/server_fn/get_list")]
	#[case("/admin/", "/api/server_fn/get_list", "/admin/api/server_fn/get_list")]
	#[case(
		"/admin/v1",
		"/api/server_fn/get_list",
		"/admin/v1/api/server_fn/get_list"
	)]
	#[case(
		"//attacker.example",
		"/api/server_fn/get_list",
		"/api/server_fn/get_list"
	)]
	#[case(
		"https://attacker.example",
		"/api/server_fn/get_list",
		"/api/server_fn/get_list"
	)]
	#[case("admin", "/api/server_fn/get_list", "/api/server_fn/get_list")]
	#[case(
		"/\\attacker.example",
		"/api/server_fn/get_list",
		"/api/server_fn/get_list"
	)]
	#[case(
		"/\n/attacker.example",
		"/api/server_fn/get_list",
		"/api/server_fn/get_list"
	)]
	#[case("/admin?x=", "/api/server_fn/get_list", "/api/server_fn/get_list")]
	#[case("/admin#x", "/api/server_fn/get_list", "/api/server_fn/get_list")]
	#[case("/admin/../evil", "/api/server_fn/get_list", "/api/server_fn/get_list")]
	#[case(
		"/admin/%2e%2e/evil",
		"/api/server_fn/get_list",
		"/api/server_fn/get_list"
	)]
	#[case(
		"/admin/%2E/evil",
		"/api/server_fn/get_list",
		"/api/server_fn/get_list"
	)]
	fn test_prefixed_same_origin_path_rejects_cross_origin_prefixes(
		#[case] prefix: &str,
		#[case] path: &str,
		#[case] expected: &str,
	) {
		let result = prefixed_same_origin_path(prefix, path);

		assert_eq!(result, expected);
	}

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
