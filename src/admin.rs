//! Admin panel functionality
//!
//! This module provides access to Reinhardt's admin panel system through
//! unified imports from the `reinhardt::admin` namespace.
//!
//! ## Architecture
//!
//! - **core**: Business logic and ModelAdmin trait
//! - **server**: Server Functions for backend operations
//! - **pages**: WASM frontend UI
//! - **adapters**: Unified type imports
//! - **types**: Shared type definitions
//!
//! ## Example (Unified Access)
//!
//! ```rust
//! use reinhardt::admin::*;
//!
//! // Configure admin site
//! let mut site = AdminSite::new("My Admin");
//!
//! // Register models
//! let user_admin = ModelAdminConfig::builder()
//!     .model_name("User")
//!     .table_name("users")
//!     .list_display(vec!["id", "username", "email"])
//!     .build()
//!     .unwrap();
//! site.register("User", user_admin);
//! ```
//!
//! ## Example (Submodule Access)
//!
//! ```ignore
//! // Server-side (non-wasm32):
//! use reinhardt::admin::adapters::AdminSite;
//! use reinhardt::admin::server::get_dashboard;
//!
//! // Client-side (wasm32 only):
//! // use reinhardt::admin::pages::AdminRouter;
//! ```

// Link reinhardt-admin crate to ensure inventory registration is executed
#[cfg(not(target_arch = "wasm32"))]
extern crate reinhardt_admin;

// Re-export submodules for structured access
pub mod adapters {
	pub use reinhardt_admin::adapters::*;
}

#[cfg(not(target_arch = "wasm32"))]
pub mod server {
	pub use reinhardt_admin::server::*;
}

#[cfg(target_arch = "wasm32")]
pub mod pages {
	pub use reinhardt_admin_pages::*;
}

// Also re-export at top level for convenience
pub use adapters::*;

#[cfg(not(target_arch = "wasm32"))]
pub use server::*;

#[cfg(target_arch = "wasm32")]
pub use pages::*;
