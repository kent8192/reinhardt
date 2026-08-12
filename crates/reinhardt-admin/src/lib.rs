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
//!
//! ## Features
//!
//! - `default`: No features enabled by default
//! - `all`: All admin functionality
//!
//! ## Examples
//!
//! ### Foreign-key relation fields
//!
//! Relation controls are opt-in. `autocomplete_fields` renders a searchable
//! foreign-key control, while `raw_id_fields` renders a direct relation-ID
//! input. Either a logical relation name (`author`) or its persisted ID column
//! (`author_id`) can be configured; the server normalizes both names before
//! rendering or saving and honors an explicit foreign-key `to_field`.
//!
//! ```
//! use reinhardt_admin::core::{ModelAdmin, ModelAdminConfig};
//!
//! let post_admin = ModelAdminConfig::builder()
//!     .model_name("Post")
//!     .autocomplete_fields(vec!["author"])
//!     .raw_id_fields(vec!["editor_id"])
//!     .allow_all(true)
//!     .build()
//!     .expect("relation configuration is valid");
//!
//! assert_eq!(post_admin.autocomplete_fields(), vec!["author"]);
//! assert_eq!(post_admin.raw_id_fields(), vec!["editor_id"]);
//! ```
//!
//! Autocomplete searches use the related admin's `search_fields` and require
//! that list to be non-empty. `ModelAdmin::object_label` may provide a custom
//! label; the related target-field value is used when it returns `None`. Every
//! lookup checks view permission on both the source and related admins before
//! exposing rows or labels. Create and update revalidate the related view
//! permission, scalar ID, target existence, and foreign-key nullability after
//! the field allowlist/readonly checks and before sanitization or the database
//! write. Relation requests are bounded to a 200-byte query, pages 1 through
//! 10,000, and page sizes of 1 through 100 (default 20).
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
		AdminDatabase, AdminRecord, AdminSite, AdminUser, ExportFormat, ImportBuilder, ImportError,
		ImportFormat, ImportResult, ModelAdmin, ModelAdminConfig, ModelAdminConfigBuilder,
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
