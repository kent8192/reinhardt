//! Server function testing utilities.
//!
//! This module re-exports server function testing utilities from `reinhardt-testkit`
//! and adds additional exports from `reinhardt-pages` for convenience.
//!
//! # Features
//!
//! - **Enhanced Test Context**: DI-based test context with authentication support
//! - **Mock HTTP**: Request/response mocking for server function testing
//! - **Authentication Mocking**: Test user and session simulation
//! - **Assertions**: Server function result assertions
//! - **Transaction Management**: Database transaction utilities for test isolation

// Re-export all public items from testkit's server_fn module
pub use reinhardt_testkit::server_fn::*;

// Re-export commonly used types from reinhardt-pages for convenience
#[cfg(native)]
pub use reinhardt_pages::testing::{ServerFnTestable, TestSessionData};
