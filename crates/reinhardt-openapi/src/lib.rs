#![warn(missing_docs)]

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
//! Define your application routes in a `routes()` function, then wrap
//! the router with `OpenApiRouter` during server setup:
//!
//! ```rust,ignore
//! use reinhardt_openapi::OpenApiRouter;
//! use reinhardt_urls::routers::BasicRouter;
//!
//! // Define routes using the project-standard routes() function.
//! // The #[cfg_attr(native, routes(standalone))] attribute registers
//! // this function as the application entry point in native builds.
//! #[cfg_attr(native, routes(standalone))]
//! pub fn routes() -> BasicRouter {
//!     BasicRouter::new()
//!     // ... mount app routes here ...
//! }
//!
//! // In server setup, wrap the routes() output with OpenApiRouter:
//! fn start_server() -> Result<(), Box<dyn std::error::Error>> {
//!     let router = routes();
//!
//!     // Wrap with OpenAPI endpoints
//!     let api_router = OpenApiRouter::wrap(router)?;
//!
//!     // api_router now serves:
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
