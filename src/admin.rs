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
//! ```no_run
//! // Server-side (non-wasm32):
//! use reinhardt::admin::adapters::AdminSite;
//! use reinhardt::admin::server::get_dashboard;
//!
//! // Client-side (wasm32 only):
//! // use reinhardt::admin::pages::AdminRouter;
//! ```

// Link reinhardt-admin crate to ensure inventory registration is executed
#[cfg(native)]
extern crate reinhardt_admin;

/// Admin interface adapter implementations.
pub mod adapters {
	pub use reinhardt_admin::adapters::*;
}

/// Core admin registration and configuration types.
#[cfg(native)]
pub mod core {
	pub use reinhardt_admin::core::*;
}

/// Server-side admin route handlers and views.
#[cfg(native)]
pub mod server {
	pub use reinhardt_admin::server::*;
}

// Admin pages module is not yet available as a separate crate.
// WASM admin UI will be provided by a future reinhardt-admin-pages crate.

// Re-export core router for admin route mounting
#[cfg(native)]
pub use reinhardt_admin::core::{AdminUser, admin_routes_with_di, admin_static_routes};

// Also re-export at top level for convenience
pub use adapters::*;

#[cfg(native)]
pub use server::*;

// WASM admin pages re-export will be added when reinhardt-admin-pages crate is available.
