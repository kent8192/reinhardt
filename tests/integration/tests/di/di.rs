//! Dependency injection integration tests
//!
//! This module contains integration tests for reinhardt-di with other Reinhardt components:
//! - Cross-crate injection (DI + HTTP)
//! - Database integration (DI + DB + reinhardt-query)
//! - Server integration (DI + Server)

// Cross-crate integration tests
mod cross_crate_injection;
mod database_integration;
mod server_integration;
