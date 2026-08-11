//! # reinhardt-admin
//!
//! Admin functionality for Reinhardt framework.
//!
//! This crate contains admin-related functionality:
//! - **adapters**: Unified server/client imports for admin types
//! - **core**: Admin site registration, model admin configuration, and database helpers
//! - **pages**: Admin page rendering
//! - **server**: Server functions and HTTP handlers
//! - **settings**: Server-side admin settings
//! - **types**: Shared request/response DTOs
//! - Per-object mutation history is persisted atomically without raw field values
//!
//! ## Inline related-model editing
//!
//! A manually configured [`core::ModelAdminConfig`] can include typed
//! [`core::InlineModelAdmin`] descriptors. Each descriptor renders foreign-key
//! children in a tabular or stacked section and may append configured blank
//! rows for child creation. The child model must have its own admin
//! registration for the same table so operation-specific permissions can be
//! checked. Parent and single-field child primary keys must be integer,
//! text-like, or UUID values.
//!
//! Inline submissions cannot choose their relationship value. The server
//! assigns the trusted parent key and persists the parent plus all requested
//! child creates, updates, and deletes in one transaction. Macro declarations,
//! nested inlines, and client-side dynamic row creation are intentionally not
//! provided.
//! - **changelist editing**: Opt-in, validated page batches committed atomically
//!
//! ## Features
//!
//! - `default`: No features enabled by default
//! - `all`: All admin functionality
//!
//! ## Examples
//!
//! A manual [`core::ModelAdmin`] can publish stable action metadata and execute
//! the selected records through the server-owned transaction:
//!
//! ```
//! use async_trait::async_trait;
//! use reinhardt_admin::core::{AdminActionTransaction, AdminUser, ModelAdmin};
//! use reinhardt_admin::types::{
//!     AdminAction, AdminActionOutcome, AdminError, AdminResult, ModelPermission,
//! };
//!
//! struct ArticleAdmin;
//!
//! # async fn publish_selected(
//! #     ids: &[String],
//! #     _transaction: &mut AdminActionTransaction,
//! # ) -> AdminResult<Vec<String>> {
//! #     Ok(ids.to_vec())
//! # }
//! #[async_trait]
//! impl ModelAdmin for ArticleAdmin {
//!     fn model_name(&self) -> &str {
//!         "Article"
//!     }
//!
//!     fn table_name(&self) -> &str {
//!         "articles"
//!     }
//!
//!     fn actions(&self) -> Vec<AdminAction> {
//!         vec![AdminAction::new(
//!             "publish",
//!             "Publish selected",
//!             ModelPermission::Change,
//!             true,
//!         )]
//!     }
//!
//!     async fn execute_action(
//!         &self,
//!         action: &str,
//!         ids: &[String],
//!         transaction: &mut AdminActionTransaction,
//!         _user: &dyn AdminUser,
//!     ) -> AdminResult<AdminActionOutcome> {
//!         if action != "publish" {
//!             return Err(AdminError::InvalidAction(action.to_owned()));
//!         }
//!
//!         let successful_ids = publish_selected(ids, transaction).await?;
//!         let affected = successful_ids.len() as u64;
//!         Ok(AdminActionOutcome::new(successful_ids, affected))
//!     }
//! }
//! ```
//!
//! The server validates CSRF, IDs, selection limits, and the declared model
//! permission before calling the hook. Returning an error rolls back the
//! transaction.
//!
//! ## Available Modules
//!
//! - [`adapters`] - Admin adapter implementations
//! - [`core`] - Admin core functionality
//! - [`pages`] - Admin page rendering
//! - [`server`] - Admin HTTP server
//! - [`types`] - Shared type definitions

#![warn(missing_docs)]
#![cfg_attr(docsrs, feature(doc_cfg))]

pub mod adapters;
#[cfg(server)]
pub mod core;
#[cfg(client)]
pub mod core {
	//! Client-side admin core type stubs.
	//!
	//! The server target exposes the real admin core module. The client target
	//! keeps the same import path available for shared code that names admin
	//! core types in signatures erased by server functions or native-only macros.

	pub use crate::types::{
		AdminAction, AdminActionOutcome, AdminActionRequest, AdminActionTransaction, AdminDatabase,
		AdminRecord, AdminSite, AdminUser, ExportFormat, Fieldset, ImportBuilder, ImportError,
		ImportFormat, ImportResult, InlineModelAdmin, InlineStyle, ModelAdmin, ModelAdminConfig,
		ModelAdminConfigBuilder, ModelPermission,
	};
}
pub mod pages;
pub mod server;
#[cfg(server)]
pub mod settings;
pub mod types;

// Register admin static files for auto-discovery by collectstatic
#[cfg(server)]
const _: () = {
	/// Path to admin static assets directory (embedded CSS/JS placeholder)
	const ADMIN_STATIC_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/assets");

	// Register at compile time using inventory
	reinhardt_apps::register_app_static_files!("admin", ADMIN_STATIC_DIR, "/static/admin/");
};

// Register WASM build output for auto-discovery by collectstatic.
// The dist-admin/ directory may not exist if the WASM SPA has not been built;
// collectstatic gracefully skips non-existent directories.
#[cfg(server)]
const _: () = {
	/// Path to admin WASM build output directory
	const ADMIN_WASM_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/dist-admin");

	reinhardt_apps::register_app_static_files!("admin-wasm", ADMIN_WASM_DIR, "/static/admin/");
};

// Register vendor assets (CSS, JS, fonts) for download via the generic
// `reinhardt-utils::staticfiles::vendor` subsystem. Each entry is collected via
// the `inventory` crate and downloaded lazily on first admin request.
#[cfg(server)]
const _: () = {
	use reinhardt_apps::AppVendorAsset;

	// Open Props v1.7.23 — CSS custom property design tokens
	reinhardt_apps::inventory::submit! {
		AppVendorAsset {
			app_label: "admin",
			url: "https://cdn.jsdelivr.net/npm/open-props@1.7.23/open-props.min.css",
			target: "vendor/open-props.min.css",
			sha256: "",
		}
	}

	// Animate.css v4.1.1 — CSS animation library
	reinhardt_apps::inventory::submit! {
		AppVendorAsset {
			app_label: "admin",
			url: "https://cdn.jsdelivr.net/npm/animate.css@4.1.1/animate.min.css",
			target: "vendor/animate.min.css",
			sha256: "",
		}
	}

	// DM Sans — Latin subset, weight 400 (regular)
	reinhardt_apps::inventory::submit! {
		AppVendorAsset {
			app_label: "admin",
			url: "https://cdn.jsdelivr.net/npm/@fontsource/dm-sans@5.1.1/files/dm-sans-latin-400-normal.woff2",
			target: "vendor/fonts/dm-sans-latin-400-normal.woff2",
			sha256: "",
		}
	}

	// DM Sans — Latin subset, weight 400 italic
	reinhardt_apps::inventory::submit! {
		AppVendorAsset {
			app_label: "admin",
			url: "https://cdn.jsdelivr.net/npm/@fontsource/dm-sans@5.1.1/files/dm-sans-latin-400-italic.woff2",
			target: "vendor/fonts/dm-sans-latin-400-italic.woff2",
			sha256: "",
		}
	}

	// DM Sans — Latin subset, weight 500 (medium)
	reinhardt_apps::inventory::submit! {
		AppVendorAsset {
			app_label: "admin",
			url: "https://cdn.jsdelivr.net/npm/@fontsource/dm-sans@5.1.1/files/dm-sans-latin-500-normal.woff2",
			target: "vendor/fonts/dm-sans-latin-500-normal.woff2",
			sha256: "",
		}
	}

	// DM Sans — Latin subset, weight 600 (semi-bold)
	reinhardt_apps::inventory::submit! {
		AppVendorAsset {
			app_label: "admin",
			url: "https://cdn.jsdelivr.net/npm/@fontsource/dm-sans@5.1.1/files/dm-sans-latin-600-normal.woff2",
			target: "vendor/fonts/dm-sans-latin-600-normal.woff2",
			sha256: "",
		}
	}

	// DM Sans — Latin subset, weight 700 (bold)
	reinhardt_apps::inventory::submit! {
		AppVendorAsset {
			app_label: "admin",
			url: "https://cdn.jsdelivr.net/npm/@fontsource/dm-sans@5.1.1/files/dm-sans-latin-700-normal.woff2",
			target: "vendor/fonts/dm-sans-latin-700-normal.woff2",
			sha256: "",
		}
	}

	// Inter — Latin subset, weight 600 (semi-bold)
	reinhardt_apps::inventory::submit! {
		AppVendorAsset {
			app_label: "admin",
			url: "https://cdn.jsdelivr.net/fontsource/fonts/inter@latest/latin-600-normal.woff2",
			target: "vendor/fonts/inter-latin-600-normal.woff2",
			sha256: "",
		}
	}

	// Inter — Latin subset, weight 700 (bold)
	reinhardt_apps::inventory::submit! {
		AppVendorAsset {
			app_label: "admin",
			url: "https://cdn.jsdelivr.net/fontsource/fonts/inter@latest/latin-700-normal.woff2",
			target: "vendor/fonts/inter-latin-700-normal.woff2",
			sha256: "",
		}
	}

	// Inter — Latin subset, weight 800 (extra-bold)
	reinhardt_apps::inventory::submit! {
		AppVendorAsset {
			app_label: "admin",
			url: "https://cdn.jsdelivr.net/fontsource/fonts/inter@latest/latin-800-normal.woff2",
			target: "vendor/fonts/inter-latin-800-normal.woff2",
			sha256: "",
		}
	}

	// UnoCSS Runtime v66.6.7 — browser-based utility CSS generation engine.
	// Generates Tailwind-compatible utility CSS by observing DOM class names
	// at runtime, eliminating the need for a build-time CLI step.
	reinhardt_apps::inventory::submit! {
		AppVendorAsset {
			app_label: "admin",
			url: "https://cdn.jsdelivr.net/npm/@unocss/runtime@66.6.7/uno.global.js",
			target: "vendor/unocss-runtime.js",
			sha256: "",
		}
	}
};
