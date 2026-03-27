//! # reinhardt-admin
//!
//! Admin functionality for Reinhardt framework.
//!
//! This crate serves as a workspace for admin-related functionality:
//! - **types**: Shared type definitions for admin API
//! - **api**: Backend JSON API for admin panel
//! - **ui**: WASM-based admin panel UI (Dominator + futures-signals)
//!
//! ## Features
//!
//! - `default`: No features enabled by default
//! - `all`: All admin functionality
//!
//! ## Examples
//!
//! See individual subcrates for usage examples:
//! - `reinhardt-admin-types` - Type definitions
//! - `reinhardt-admin-api` - Backend API
//! - `reinhardt-admin-ui` - Frontend UI

#![warn(missing_docs)]
#![cfg_attr(docsrs, feature(doc_cfg))]

pub mod adapters;
pub mod core;
pub mod pages;
pub mod server;
pub mod types;

// Register admin static files for auto-discovery by collectstatic
#[cfg(not(target_arch = "wasm32"))]
const _: () = {
	/// Path to admin static assets directory
	const ADMIN_STATIC_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/assets");

	// Register at compile time using inventory
	reinhardt_apps::register_app_static_files!("admin", ADMIN_STATIC_DIR, "/static/admin/");
};
