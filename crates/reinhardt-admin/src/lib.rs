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
		AdminRecord, AdminSite, AdminUser, ExportFormat, ImportBuilder, ImportError, ImportFormat,
		ImportResult, ModelAdmin, ModelAdminConfig, ModelAdminConfigBuilder, ModelPermission,
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
