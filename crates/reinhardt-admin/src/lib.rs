//! # reinhardt-admin
//!
//! Admin functionality for Reinhardt framework.
//!
//! This crate contains admin-related functionality:
//! - **adapters**: Unified server/client imports for admin types
//! - **core**: Admin site registration, model admin configuration, and database helpers
//! - **pages**: Admin page rendering
//! - **server**: Server functions and HTTP handlers
//! - Storage-backed `FileField`/`ImageField` admin forms use multipart mutations
//!   when the `file-uploads` feature is enabled
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
//! ## Form customization
//!
//! Custom forms decorate only registered model fields. `normalize` receives
//! owned JSON data and `validate` borrows the normalized data; both hooks must
//! be synchronous and pure. Field
//! errors use their canonical field name; global errors have no field and are
//! returned to the client as `_all` with HTTP 422.
//!
//! ```
//! use reinhardt_admin::core::{AdminForm, AdminFormData, AdminFormErrors, AdminFormMode};
//! use serde_json::Value;
//!
//! #[derive(Debug)]
//! struct ArticleForm;
//!
//! impl AdminForm for ArticleForm {
//!     fn normalize(
//!         &self,
//!         _mode: AdminFormMode,
//!         mut data: AdminFormData,
//!     ) -> Result<AdminFormData, AdminFormErrors> {
//!         if let Some(Value::String(title)) = data.get_mut("title") {
//!             *title = title.trim().to_owned();
//!         }
//!         Ok(data)
//!     }
//!
//!     fn validate(
//!         &self,
//!         _mode: AdminFormMode,
//!         data: &AdminFormData,
//!     ) -> Result<(), AdminFormErrors> {
//!         if data.get("title") == Some(&Value::String(String::new())) {
//!             return Err(AdminFormErrors::field("title", "Title is required"));
//!         }
//!         Ok(())
//!     }
//! }
//! ```
//!
//! Builder overlays are applied property by property after inferred and
//! relation widgets. A form adapter's `schema()` overlays them last. They can
//! strengthen requiredness, but cannot make a model-required field optional.
//!
//! ```
//! use reinhardt_admin::core::{
//!     AdminWidget, FormFieldOverride, ModelAdmin, ModelAdminConfig, PrepopulatedField,
//! };
//!
//! let admin = ModelAdminConfig::builder()
//!     .model_name("Article")
//!     .fields(vec!["title", "body", "slug"])
//!     .formfield_overrides(vec![
//!         FormFieldOverride::new("body").widget(AdminWidget::TextArea { rows: Some(8) }),
//!     ])
//!     .prepopulated_fields(vec![PrepopulatedField::new("slug", ["title"])])
//!     .build()
//!     .unwrap();
//!
//! assert_eq!(admin.prepopulated_fields()[0].target, "slug");
//! ```
//!
//! The equivalent `#[admin]` declaration uses a closed grammar. The custom
//! form type implements `AdminForm + Default + 'static`; the macro initializes
//! one shared default value.
//!
//! ```
//! # extern crate reinhardt_admin as reinhardt_admin_adapters;
//! use reinhardt_admin::adapters::AdminForm;
//! use reinhardt_macros::{admin, model};
//! use serde::{Deserialize, Serialize};
//!
//! #[model(app_label = "docs", table_name = "articles")]
//! #[derive(Clone, Debug, Deserialize, Serialize)]
//! struct Article {
//!     #[field(primary_key = true)]
//!     id: i64,
//!     #[field(max_length = 255)]
//!     title: String,
//!     #[field(max_length = 255)]
//!     body: String,
//!     #[field(max_length = 255)]
//!     slug: String,
//! }
//!
//! #[derive(Debug, Default)]
//! struct ArticleForm;
//!
//! impl AdminForm for ArticleForm {}
//!
//! #[admin(model,
//!     for = Article,
//!     name = "Article",
//!     form = ArticleForm,
//!     formfield_overrides = [(body, widget = textarea, rows = 8)],
//!     prepopulated_fields = [(slug, sources = [title])],
//! )]
//! struct ArticleAdmin;
//! ```
//!
//! Prepopulation is client-side per mount: a non-empty edit target stays
//! locked, and editing or clearing a target makes it sticky. It never causes
//! server-side recomputation. Foreign-key and many-to-many widgets retain their
//! existing relation lookup, permission, and save-time validation contracts.
//! Arbitrary components, asynchronous validation, and virtual fields are not
//! supported.
//!
//! Many-to-many fields can use the same horizontal or vertical selector
//! configuration through [`core::ModelAdmin`], [`core::ModelAdminConfig`], or
//! the `admin` attribute macro:
//!
//! ```ignore
//! use reinhardt_admin::core::{ModelAdmin, ModelAdminConfig};
//!
//! impl ModelAdmin for ArticleAdmin {
//!     fn model_name(&self) -> &str { "Article" }
//!     fn table_name(&self) -> &str { "blog_articles" }
//!     fn filter_horizontal(&self) -> Vec<&str> { vec!["tags"] }
//!     fn filter_vertical(&self) -> Vec<&str> { vec!["reviewers"] }
//! }
//!
//! let configured = ModelAdminConfig::builder()
//!     .model_name("Article")
//!     .table_name("blog_articles")
//!     .filter_horizontal(vec!["tags"])
//!     .filter_vertical(vec!["reviewers"])
//!     .build()?;
//!
//! #[admin(model,
//!     for = Article,
//!     name = "Article",
//!     filter_horizontal = [tags],
//!     filter_vertical = [reviewers],
//! )]
//! pub struct ArticleAdmin;
//! # Ok::<(), reinhardt_admin::types::AdminError>(())
//! ```
//!
//! Field names are matched exactly. The layouts cannot overlap, and selector
//! fields must be registered many-to-many relations. Reading or searching
//! options requires related-model View permission, which is checked again on
//! save. Lookup pages return at most 50 options, and **Load more** appends later
//! pages without dropping chosen values. Parent and join-table mutations share one atomic transaction, so a
//! join failure rolls back the parent mutation.
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
//!             return Err(AdminError::ValidationError(format!("Invalid action: {action}")));
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
//! `ModelAdmin::fields()` remains the flat form configuration. Use
//! `ModelAdmin::fieldsets()` when the form needs ordered groups instead:
//!
//! ```rust
//! use reinhardt_admin::core::{Fieldset, ModelAdmin, ModelAdminConfig};
//!
//! let flat = ModelAdminConfig::builder()
//!     .model_name("Article")
//!     .fields(vec!["title", "body"])
//!     .build()
//!     .unwrap();
//! assert_eq!(flat.fields(), Some(vec!["title", "body"]));
//! assert_eq!(flat.fieldsets(), None);
//!
//! let grouped = ModelAdminConfig::builder()
//!     .model_name("Article")
//!     .fieldsets(vec![
//!         Fieldset::new(Some("Content"), &["title", "body"]),
//!         Fieldset::new(Some("Publishing"), &["published_at"]).collapsed(),
//!     ])
//!     .build()
//!     .unwrap();
//! assert_eq!(grouped.fields(), None);
//! assert!(grouped.fieldsets().unwrap()[1].collapsed);
//! ```
//!
//! The `#[admin]` macro uses the same descriptors:
//!
//! ```ignore
//! use reinhardt::admin;
//! use crate::models::Article;
//!
//! #[admin(model,
//!     for = Article,
//!     name = "Article",
//!     fieldsets = [
//!         (title = "Content", fields = [title, body]),
//!         (fields = [published_at], collapsed = true)
//!     ]
//! )]
//! struct ArticleAdmin;
//! ```
//!
//! `collapsed` controls only the initial native `<details>` state; it is not
//! persisted. Nested fieldsets, custom layout classes, layout grids, and inline
//! form configuration are intentionally unsupported.
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
#[doc(hidden)]
pub use adapters::{
	AdminForm, AdminUser, AdminWidget, FormFieldOverride, ModelAdmin, PrepopulatedField,
};
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
		AdminForm, AdminFormData, AdminFormError, AdminFormErrors, AdminFormMode, AdminFormResult,
		AdminQuery, AdminRecord, AdminRequestContext, AdminSite, AdminUser, AdminWidget,
		ExportFormat, Fieldset, FormFieldOverride, ImportBuilder, ImportError, ImportFormat,
		ImportResult, InlineModelAdmin, InlineStyle, ListColumn, ModelAdmin, ModelAdminConfig,
		ModelAdminConfigBuilder, ModelPermission, PrepopulatedField,
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
