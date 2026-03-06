//! URL routing and proxy utilities for Reinhardt framework.
//!
//! This crate provides URL routing, pattern matching, and proxy functionality
//! for the Reinhardt web framework. It is a unified interface over the following
//! internal crates:
//!
//! - `reinhardt-routers`: URL routing and pattern matching with middleware support
//! - `reinhardt-routers-macros`: Compile-time URL validation macros
//! - `reinhardt-proxy`: Lazy relationship loading for ORM
//!
//! ## Features
//!
//! ### Route Middleware Support
//!
//! Per-route middleware configuration is now available. You can attach middleware
//! to specific routes or route groups:
//!
//! ```rust,ignore
//! # use reinhardt_urls::routers::UnifiedRouter;
//! # use hyper::Method;
//! # use reinhardt_http::{Request, Response};
//! # use reinhardt_core::exception::Result;
//!
//! # async fn handler(_req: Request) -> Result<Response> { Ok(Response::ok()) }
//! # async fn users_handler(_req: Request) -> Result<Response> { Ok(Response::ok()) }
//! # async fn settings_handler(_req: Request) -> Result<Response> { Ok(Response::ok()) }
//! let router = UnifiedRouter::new()
//!     .function("/public", Method::GET, handler)
//!     .function("/protected", Method::GET, handler);
//!     // .with_route_middleware(...) // Route-specific middleware
//! ```
//!
//! **Features**:
//! - Per-route middleware configuration
//! - Route group middleware with inheritance
//! - Middleware composition and chaining
//! - Proper execution order: global → group → route → handler
//!
//! See `reinhardt-routers` crate documentation for detailed usage and examples.

#![cfg_attr(docsrs, feature(doc_cfg))]

pub mod proxy;
pub mod routers;

#[cfg(feature = "routers-macros")]
#[cfg_attr(docsrs, doc(cfg(feature = "routers-macros")))]
pub use reinhardt_routers_macros as routers_macros;

// Re-export commonly used types from routers
#[cfg(feature = "routers")]
#[cfg_attr(docsrs, doc(cfg(feature = "routers")))]
pub mod prelude {
	pub use crate::routers::{
		PathPattern, Route, RouteGroup, Router, ServerRouter, UnifiedRouter, clear_script_prefix,
		get_script_prefix, set_script_prefix,
	};
}
