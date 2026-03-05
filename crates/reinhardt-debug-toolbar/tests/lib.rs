//! Integration tests for reinhardt-debug-toolbar crate
//!
//! This test suite provides comprehensive testing for the debug toolbar functionality
//! including SQL query debugging, request information display, and panel system.

// Test modules organized by category
pub mod common;
pub mod features;
pub mod integration;
pub mod unit;

// Re-export common test utilities for convenience
pub use common::{
	builders::{
		CacheOperationBuilder, PerformanceMarkerBuilder, SqlQueryBuilder, TemplateInfoBuilder,
	},
	fixtures::*,
	helpers::*,
	mock_panel::MockPanel,
};
