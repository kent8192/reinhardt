//! # Reinhardt Apps
//!
//! Django-inspired application configuration and registry system for Reinhardt.
//!
//! ## Overview
//!
//! This crate provides the infrastructure for managing Django-style applications
//! in a Reinhardt project. It handles application registration, configuration,
//! model discovery, and lifecycle management.
//!
//! ## Features
//!
//! - **[`AppConfig`]**: Application configuration with metadata and settings
//! - **[`Apps`]**: Central registry for all installed applications
//! - **[`ApplicationBuilder`]**: Builder pattern for fluent application construction
//! - **Model Discovery**: Automatic model and migration discovery via [`discovery`] module
//! - **Signals**: Application lifecycle signals via [`signals`] module
//! - **Validation**: Registry validation for circular dependencies and duplicates
//! - **Native Module Ownership**: `#[app_config]` records the owning module for
//!   native inventory consumers such as server-function route collection
//!
//! ## Modules
//!
//! - [`apps`]: Core [`AppConfig`] and [`Apps`] registry
//! - [`builder`]: [`ApplicationBuilder`] for fluent application construction
//! - [`discovery`]: Automatic model, migration, and relationship discovery
//! - [`registry`]: Global model and relationship registry ([`MODELS`], [`RELATIONSHIPS`])
//! - [`signals`]: Application lifecycle signals
//! - [`validation`]: Registry validation utilities
//!
//! ## Native Module Ownership
//!
//! On native targets, `#[app_config]` contributes an [`AppModuleRegistration`]
//! for its defining module. Inventory-backed features use that registration to
//! select declarations owned by one application. The related
//! [`iter_app_module_registrations`] and [`resolve_app_module_owner`] APIs are
//! native-only because browser WASM does not link inventory constructors.
//!
//! ## Quick Start
//!
//! ```rust,ignore
//! use reinhardt_apps::{ApplicationBuilder, AppConfig};
//!
//! // Build an application with multiple apps
//! let app = ApplicationBuilder::new()
//!     .add_setting("DEBUG", "true")
//!     .add_app(AppConfig::new("myproject.users", "users"))
//!     .add_app(AppConfig::new("myproject.blog", "blog"))
//!     .build()
//!     .expect("Failed to build application");
//!
//! // Check if an app is installed
//! if app.apps_registry().is_installed("users") {
//!     println!("Users app is installed");
//! }
//! ```
//!
//! ## Model Registry
//!
//! Models are automatically discovered and registered in the global registry:
//!
//! ```rust,ignore
//! use reinhardt_apps::{get_registered_models, find_model};
//!
//! // Get all registered models
//! let models = get_registered_models();
//!
//! // Find a specific model by name
//! if let Some(user_model) = find_model("User") {
//!     println!("Found User model in app: {}", user_model.app_label);
//! }
//! ```
//!
//! ## Application Lifecycle
//!
//! 1. **Configuration**: Define [`AppConfig`] for each application
//! 2. **Registration**: Add apps to [`ApplicationBuilder`]
//! 3. **Discovery**: Models and migrations are automatically discovered
//! 4. **Validation**: Registry is validated for consistency
//! 5. **Ready**: Application signals are fired when setup is complete
//!
//! ## Re-exports
//!
//! This crate re-exports commonly used types from other Reinhardt crates:
//!
//! - From `reinhardt-http`: `Request`, `Response`, `StreamBody`
//! - From `reinhardt-conf`: `Settings`, `DatabaseConfig`, `MiddlewareConfig`
//! - From `reinhardt-core::exception`: `Error`, `Result`
//! - From `reinhardt-server`: `HttpServer`, `serve`
//! - From `reinhardt-http`: `Handler`, `Middleware`, `MiddlewareChain`

#![warn(missing_docs)]

// Cross-target modules. These define the pure trait/data core (`AppLabel`,
// `AppConfig`, lifecycle signals) that must be reachable from
// `wasm32-unknown-unknown` callers (e.g., client-side code that names an
// `AppLabel` impl without pulling in the server runtime).
pub mod apps;
pub mod signals;

// Native-only modules.
//
// - `registry` uses `linkme::distributed_slice`, whose link-section
//   constructors are not supported on `wasm32-unknown-unknown`.
// - `builder`, `discovery`, `hooks`, `validation` depend on
//   `reinhardt-server`, `reinhardt-conf`, `reinhardt-utils`, or
//   `reinhardt-di`, none of which compile on wasm32 today (they pull in
//   tokio's `net` feature → mio).
#[cfg(native)]
pub mod builder;
#[cfg(native)]
pub mod discovery;
#[cfg(native)]
pub mod hooks;
#[cfg(native)]
pub mod registry;
#[cfg(native)]
pub mod validation;

// Re-export from reinhardt-http (cross-target).
pub use reinhardt_http::{Request, Response, StreamBody, StreamingResponse};

// Re-export from reinhardt-conf (native-only: pulls in tokio runtime).
#[cfg(native)]
pub use reinhardt_conf::settings::{DatabaseConfig, MiddlewareConfig};
// `TemplateConfig` is deprecated in favor of the `TemplateSettings` fragment;
// keep the re-export available during the 0.2 compatibility window.
#[cfg(native)]
#[allow(deprecated)]
pub use reinhardt_conf::settings::TemplateConfig;

// Re-export from reinhardt-core::exception (cross-target).
pub use reinhardt_core::exception::{Error, Result};

// Re-export from reinhardt-server (native-only).
#[cfg(native)]
pub use reinhardt_server::{HttpServer, serve};

// Re-export from reinhardt-http (cross-target).
pub use reinhardt_http::{Handler, Middleware, MiddlewareChain, SyncHandler, SyncHandlerAdapter};

// Re-export inventory for macro usage (native-only; inventory relies on
// link-section constructors not portable to `wasm32-unknown-unknown`).
#[cfg(native)]
pub use inventory;

// Re-export from apps module. Cross-target items (`AppLabel`, `AppConfig`, and
// the trait providers) are unconditional; the `AppVendorAsset` re-export and
// the `get_app_*` collector helpers depend on `reinhardt-utils` and remain
// native-only.
#[cfg(native)]
pub use apps::{
	AppCommandConfig, AppLocaleConfig, AppMediaConfig, AppModuleRegistration,
	AppModuleResolutionError, AppStaticFilesConfig, AppVendorAsset, BaseCommand, get_app_commands,
	get_app_locales, get_app_media, get_app_static_files, iter_app_module_registrations,
	resolve_app_module_owner,
};
pub use apps::{
	AppConfig, AppError, AppLabel, AppResult, Apps, LocaleProvider, MediaProvider,
	StaticFilesProvider,
};

// Re-export from builder module (native-only).
#[cfg(native)]
pub use builder::{
	Application, ApplicationBuilder, ApplicationDatabaseConfig, BuildError, BuildResult,
	RouteConfig,
};

// Re-export from registry module (native-only).
#[cfg(native)]
pub use registry::{
	MODELS, ModelMetadata, RELATIONSHIPS, RelationshipMetadata, RelationshipType,
	ReverseRelationMetadata, ReverseRelationType, finalize_reverse_relations, find_model,
	get_models_for_app, get_registered_models, get_registered_relationships,
	get_relationships_for_model, get_relationships_to_model, get_reverse_relations_for_model,
	register_reverse_relation,
};

// Re-export from discovery module (native-only).
#[cfg(native)]
pub use discovery::{
	MigrationMetadata, RelationMetadata, RelationType, build_reverse_relations,
	create_reverse_relation, discover_all_models, discover_migrations, discover_models,
};

// Re-export from validation module (native-only).
#[cfg(native)]
pub use validation::{
	ValidationError, ValidationResult, check_circular_relationships, check_duplicate_model_names,
	check_duplicate_table_names, validate_registry,
};

#[cfg(test)]
mod tests {
	use super::*;
	use bytes::Bytes;
	use hyper::{HeaderMap, Method, Uri, Version};

	#[test]
	fn test_request_query_params() {
		let uri = Uri::from_static("/test?foo=bar&baz=qux");
		let request = Request::builder()
			.method(Method::GET)
			.uri(uri)
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();

		assert_eq!(request.query_params.get("foo"), Some("bar"));
		assert_eq!(request.query_params.get("baz"), Some("qux"));
	}

	#[test]
	fn test_response_creation() {
		let response = Response::ok();
		assert_eq!(response.status, hyper::StatusCode::OK);

		let response = Response::created();
		assert_eq!(response.status, hyper::StatusCode::CREATED);

		let response = Response::not_found();
		assert_eq!(response.status, hyper::StatusCode::NOT_FOUND);
	}

	#[test]
	fn test_response_with_json_unit() {
		use serde_json::json;

		let data = json!({
			"message": "Hello, world!"
		});

		let response = Response::ok().with_json(&data).unwrap();

		let body_str = String::from_utf8(response.body.to_vec()).unwrap();
		let parsed: serde_json::Value = serde_json::from_str(&body_str).unwrap();
		assert_eq!(parsed["message"], "Hello, world!");
		assert_eq!(
			response.headers.get(hyper::header::CONTENT_TYPE).unwrap(),
			"application/json"
		);
	}

	#[test]
	fn test_error_status_codes() {
		assert_eq!(Error::NotFound("test".into()).status_code(), 404);
		assert_eq!(Error::Authentication("test".into()).status_code(), 401);
		assert_eq!(Error::Authorization("test".into()).status_code(), 403);
		assert_eq!(Error::Validation("test".into()).status_code(), 400);
		assert_eq!(Error::Internal("test".into()).status_code(), 500);
	}
}
