//! Core type definitions and common types for the Reinhardt framework.
//!
//! This crate provides foundational types used across the Reinhardt framework.
//!
//! ## Features
//!
//! - `page` - Page types for component rendering (`Page`, `PageElement`, `Head`, etc.)
//! - `http` - HTTP-related types (moved to `reinhardt-http` crate)
//!
//! ## Note
//!
//! HTTP-related types (`Handler`, `Middleware`, `MiddlewareChain`, `Request`, `Response`)
//! have been moved to `reinhardt-http` crate to prevent circular dependencies.
//!
//! Use `reinhardt-http` for HTTP types:
//! `use reinhardt_http::{Handler, Middleware, Request, Response};`

#[cfg(feature = "page")]
pub mod page;
