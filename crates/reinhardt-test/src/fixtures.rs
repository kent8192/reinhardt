//! Test fixtures and utilities for Reinhardt framework testing
//!
//! This module re-exports all fixtures from `reinhardt-testkit` and adds
//! additional fixtures that depend on functional crates (reinhardt-auth,
//! reinhardt-admin).

// Re-export all submodules from testkit for path compatibility
pub use reinhardt_testkit::fixtures::{client, dcl, di, loader, mock, server};

#[cfg(feature = "testcontainers")]
pub use reinhardt_testkit::fixtures::{
	resources, schema, shared_postgres, testcontainers, validator,
};

pub use reinhardt_testkit::fixtures::migrations;

// Admin settings fixtures (re-exported from testkit)
pub use reinhardt_testkit::fixtures::admin;

// Server function fixtures (re-exported from testkit)
#[cfg(feature = "server-fn-test")]
pub use reinhardt_testkit::fixtures::server_fn;

// Re-export all public items from testkit fixtures
pub use reinhardt_testkit::fixtures::*;

// ============================================================================
// Modules specific to reinhardt-test (depend on functional crates)
// ============================================================================

// Authentication fixtures (depends on reinhardt-auth)
pub mod auth;

// Admin panel fixtures (depends on reinhardt-admin)
#[cfg(all(feature = "admin", feature = "testcontainers"))]
pub mod admin_panel;

// Admin migration fixtures (depends on reinhardt-admin)
#[cfg(all(feature = "admin", feature = "testcontainers"))]
pub mod admin_migrations;

// WASM frontend test fixtures and E2E browser testing fixtures
#[cfg(any(
	all(target_arch = "wasm32", feature = "wasm"),
	all(feature = "e2e", not(target_arch = "wasm32"))
))]
pub mod wasm;

// Admin integration fixtures (conditional on admin + testcontainers features)
#[cfg(all(feature = "admin", feature = "testcontainers"))]
pub use admin_migrations::{AdminTableCreator, admin_table_creator};
