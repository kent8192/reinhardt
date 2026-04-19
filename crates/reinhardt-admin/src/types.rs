//! Shared type definitions for Reinhardt admin panel
//!
//! This crate provides common type definitions used by both the admin panel API
//! (backend) and UI (frontend) components.
//!
//! # Main modules
//!
//! - [`errors`]: Error types and result type alias
//! - [`models`]: Model information types
//! - [`requests`]: Request body types for API endpoints
//! - [`responses`]: Response types for API endpoints

pub mod errors;
pub mod models;
pub mod requests;
pub mod responses;
pub mod wasm_stubs;

// Re-export all public types
pub use errors::*;
pub use models::*;
pub use requests::*;
pub use responses::*;

// WASM-only type stubs
//
// For WASM targets, re-export stub types from wasm_stubs module
// These stubs allow Server Function client code to type-check correctly
#[cfg(client)]
pub use wasm_stubs::{
	AdminDatabase, AdminRecord, AdminSite, ExportFormat, ImportBuilder, ImportError, ImportFormat,
	ImportResult, ModelAdmin,
};
