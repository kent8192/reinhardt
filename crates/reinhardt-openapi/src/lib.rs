//! # Reinhardt OpenAPI Router
//!
//! OpenAPI router wrapper that automatically adds documentation endpoints.
//!
//! ## Overview
//!
//! This crate provides a router wrapper that intercepts requests to OpenAPI
//! documentation paths and serves them from memory, delegating all other
//! requests to the wrapped handler.
//!
//! ## Example
//!
//! ```rust,ignore
//! use reinhardt_openapi::OpenApiRouter;
//! use reinhardt_urls::routers::BasicRouter;
//!
//! fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Create your existing router
//!     let router = BasicRouter::new();
//!
//!     // Wrap with OpenAPI endpoints
//!     let wrapped = OpenApiRouter::wrap(router)?;
//!
//!     // The wrapped router now serves:
//!     // - /api/openapi.json (OpenAPI spec)
//!     // - /api/docs (Swagger UI)
//!     // - /api/redoc (Redoc UI)
//!     Ok(())
//! }
//! ```
//!
//! ## Separation Rationale
//!
//! This crate exists separately from `reinhardt-rest` to break a circular
//! dependency chain:
//!
//! ```text
//! reinhardt-urls → reinhardt-views → reinhardt-rest → reinhardt-urls (cycle!)
//! ```
//!
//! By placing `OpenApiRouter` in its own crate that depends on both
//! `reinhardt-urls` and `reinhardt-rest`, we avoid this cycle:
//!
//! ```text
//! reinhardt-openapi
//!     ├── reinhardt-urls (Route, Router trait)
//!     └── reinhardt-rest (generate_openapi_schema, SwaggerUI, RedocUI)
//! ```

mod router_wrapper;

pub use reinhardt_rest::openapi::SchemaError;
pub use router_wrapper::AuthGuard;
pub use router_wrapper::OpenApiRouter;
