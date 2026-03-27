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
#[cfg(not(target_arch = "wasm32"))]
pub mod injectable;
#[cfg(not(target_arch = "wasm32"))]
pub mod negotiation;
#[cfg(not(target_arch = "wasm32"))]
pub mod registration;
#[cfg(not(target_arch = "wasm32"))]
pub mod registry;
#[cfg(not(target_arch = "wasm32"))]
pub mod router_ext;
pub mod server_fn_trait;

// Re-exports
#[cfg(feature = "msgpack")]
pub use codec::MessagePackCodec;
pub use codec::{Codec, JsonCodec, UrlCodec};
#[cfg(not(target_arch = "wasm32"))]
pub use injectable::{ServerFnBody, ServerFnRequest};
#[cfg(not(target_arch = "wasm32"))]
pub use negotiation::convert_body_for_codec;
#[cfg(not(target_arch = "wasm32"))]
pub use registration::ServerFnRegistration;
#[cfg(not(target_arch = "wasm32"))]
pub use registry::{ServerFnHandler, ServerFnRoute};
#[cfg(not(target_arch = "wasm32"))]
pub use router_ext::ServerFnRouterExt;
pub use server_fn_trait::{ServerFn, ServerFnError};

// Re-export the macro for convenience
pub use reinhardt_pages_macros::server_fn;
