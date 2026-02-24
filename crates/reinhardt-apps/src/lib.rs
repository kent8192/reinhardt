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
//! ## Quick Start
//!
//! ```rust,ignore
//! use reinhardt_apps::{ApplicationBuilder, AppConfig, Settings};
//!
//! // Build an application with multiple apps
//! let app = ApplicationBuilder::new()
//!     .settings(Settings::default())
//!     .add_app(AppConfig::new("users", "myproject.users"))
//!     .add_app(AppConfig::new("blog", "myproject.blog"))
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
//! - From `reinhardt-http`: [`Request`], [`Response`], [`StreamBody`]
//! - From `reinhardt-conf`: [`Settings`], [`DatabaseConfig`], [`MiddlewareConfig`]
//! - From `reinhardt-exception`: [`Error`], [`Result`]
//! - From `reinhardt-server`: [`HttpServer`], [`serve`]
//! - From `reinhardt-types`: [`Handler`], [`Middleware`], [`MiddlewareChain`]

pub mod apps;
pub mod builder;
pub mod discovery;
pub mod hooks;
pub mod registry;
pub mod signals;
pub mod validation;

// Re-export from reinhardt-http
pub use reinhardt_http::{Request, Response, StreamBody, StreamingResponse};

// Re-export from reinhardt-conf
pub use reinhardt_conf::settings::{DatabaseConfig, MiddlewareConfig, Settings, TemplateConfig};

// Re-export from reinhardt-exception
pub use reinhardt_core::exception::{Error, Result};

// Re-export from reinhardt-server
pub use reinhardt_server::{HttpServer, serve};

// Re-export from reinhardt-types
pub use reinhardt_http::{Handler, Middleware, MiddlewareChain};

// Re-export inventory for macro usage
pub use inventory;

// Re-export from apps module
pub use apps::{
	AppCommandConfig, AppConfig, AppError, AppLocaleConfig, AppMediaConfig, AppResult,
	AppStaticFilesConfig, Apps, BaseCommand, LocaleProvider, MediaProvider, StaticFilesProvider,
	get_app_commands, get_app_locales, get_app_media, get_app_static_files,
};

// Re-export from builder module
pub use builder::{
	Application, ApplicationBuilder, ApplicationDatabaseConfig, BuildError, BuildResult,
	RouteConfig,
};

// Re-export from registry module
pub use registry::{
	MODELS, ModelMetadata, RELATIONSHIPS, RelationshipMetadata, RelationshipType,
	ReverseRelationMetadata, ReverseRelationType, finalize_reverse_relations, find_model,
	get_models_for_app, get_registered_models, get_registered_relationships,
	get_relationships_for_model, get_relationships_to_model, get_reverse_relations_for_model,
	register_reverse_relation,
};

// Re-export from discovery module
pub use discovery::{
	MigrationMetadata, RelationMetadata, RelationType, build_reverse_relations,
	create_reverse_relation, discover_all_models, discover_migrations, discover_models,
};

// Re-export from validation module
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

		assert_eq!(request.query_params.get("foo"), Some(&"bar".to_string()));
		assert_eq!(request.query_params.get("baz"), Some(&"qux".to_string()));
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
