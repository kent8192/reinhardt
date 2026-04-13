//! MSW-style network-level request interception for WASM testing.
//!
//! Intercepts `window.fetch()` calls at the browser API level, providing
//! realistic API mocking without modifying application code.
//!
//! # Overview
//!
//! This module provides `MockServiceWorker` which overrides `window.fetch`
//! to intercept HTTP requests and return mock responses. It supports:
//!
//! - Type-safe `server_fn` mocking via `MockableServerFn`
//! - REST endpoint mocking via [`rest`] builder helpers
//! - Request recording and assertion via `CallQuery` and `ServerFnCallQuery`
//! - Configurable behavior for unhandled requests via `UnhandledPolicy`
//!
//! # Example
//!
//! ```rust,ignore
//! use reinhardt_test::msw::*;
//!
//! #[wasm_bindgen_test]
//! async fn test_component() {
//!     let worker = MockServiceWorker::new();
//!     worker.handle(rest::get("/api/users").respond(MockResponse::json(vec![1, 2])));
//!     worker.start().await;
//!     // ... test component ...
//!     worker.calls_to("/api/users").assert_called();
//! }
//! ```

// On native builds, worker.rs and interceptor.rs are behind #[cfg(wasm)],
// making handler/recorder/context types appear unused in lib mode.
// They ARE exercised in WASM builds and native unit tests.
#![allow(dead_code, clippy::type_complexity)]

mod context;
pub(crate) mod handler;
mod interceptor;
mod matcher;
pub(crate) mod recorder;
mod response;
pub mod rest;

pub use context::TestContext;
pub use handler::InterceptedRequest;
pub use matcher::{Segment, UrlMatcher};
pub use recorder::{CallQuery, RecordedRequest, ServerFnCallQuery};
pub use response::MockResponse;

// WASM-only: MockServiceWorker requires window.fetch interop
#[cfg(wasm)]
mod worker;
#[cfg(wasm)]
pub use worker::{MockServiceWorker, UnhandledPolicy};
