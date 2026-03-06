//! Server function testing utilities.
//!
//! This module provides comprehensive testing utilities for server functions,
//! including context management, authentication mocking, and assertions.
//!
//! # Features
//!
//! - **Enhanced Test Context**: DI-based test context with authentication support
//! - **Mock HTTP**: Request/response mocking for server function testing
//! - **Authentication Mocking**: Test user and session simulation
//! - **Assertions**: Server function result assertions
//! - **Transaction Management**: Database transaction utilities for test isolation

mod assertions;
mod auth;
mod context;
mod mock_request;
mod transaction;

// Re-export all public items
pub use assertions::*;
pub use auth::*;
pub use context::*;
pub use mock_request::*;
pub use transaction::*;
