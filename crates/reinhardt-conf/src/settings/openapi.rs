//! OpenAPI documentation endpoint configuration
//!
//! Provides composable OpenAPI configuration as a `SettingsFragment`.
//!
//! # TOML Configuration
//!
//! ```toml
//! [openapi]
//! enabled = true
//! title = "My API"
//! version = "2.0.0"
//! swagger_path = "/api/swagger"
//! redoc_path = "/api/redoc"
//! json_path = "/api/openapi.json"
//! description = "API documentation"
//! ```

use reinhardt_macros::settings;

/// OpenAPI documentation endpoint configuration fragment.
///
/// Controls how OpenAPI documentation endpoints are mounted by the
/// `runserver` command. Maps to the `[openapi]` TOML section.
///
/// # Default Paths
///
/// - Swagger UI: `/api/docs`
/// - Redoc UI: `/api/redoc`
/// - OpenAPI JSON: `/api/openapi.json`
///
/// # Example
///
/// ```rust
/// use reinhardt_conf::settings::openapi::OpenApiSettings;
/// use reinhardt_conf::settings::fragment::SettingsFragment;
///
/// let settings = OpenApiSettings::default();
/// assert!(settings.enabled);
/// assert_eq!(OpenApiSettings::section(), "openapi");
/// ```
#[settings(fragment = true, section = "openapi")]
#[non_exhaustive]
pub struct OpenApiSettings {
	/// Enable OpenAPI endpoints (default: true)
	///
	/// Set to `false` to disable automatic endpoint mounting.
	/// This is equivalent to using the `--no-docs` command-line flag.
	#[serde(default = "default_true")]
	pub enabled: bool,

	/// OpenAPI JSON endpoint path (default: "/api/openapi.json")
	///
	/// The path where the OpenAPI 3.0 JSON schema will be served.
	#[serde(default = "default_json_path")]
	pub json_path: String,

	/// Swagger UI endpoint path (default: "/api/docs")
	///
	/// The path where the interactive Swagger UI will be served.
	#[serde(default = "default_swagger_path")]
	pub swagger_path: String,

	/// Redoc UI endpoint path (default: "/api/redoc")
	///
	/// The path where the alternative Redoc UI will be served.
	#[serde(default = "default_redoc_path")]
	pub redoc_path: String,

	/// API title (default: "API Documentation")
	///
	/// The title displayed in the OpenAPI schema and documentation UIs.
	#[serde(default = "default_title")]
	pub title: String,

	/// API version (default: "1.0.0")
	///
	/// The version displayed in the OpenAPI schema.
	#[serde(default = "default_version")]
	pub version: String,

	/// API description (optional)
	///
	/// An optional description displayed in the OpenAPI schema and documentation UIs.
	#[serde(default)]
	pub description: Option<String>,
}

impl Default for OpenApiSettings {
	fn default() -> Self {
		Self {
			enabled: true,
			json_path: "/api/openapi.json".to_string(),
			swagger_path: "/api/docs".to_string(),
			redoc_path: "/api/redoc".to_string(),
			title: "API Documentation".to_string(),
			version: "1.0.0".to_string(),
			description: None,
		}
	}
}

// Default value functions for serde
fn default_true() -> bool {
	true
}

fn default_json_path() -> String {
	"/api/openapi.json".to_string()
}

fn default_swagger_path() -> String {
	"/api/docs".to_string()
}

fn default_redoc_path() -> String {
	"/api/redoc".to_string()
}

fn default_title() -> String {
	"API Documentation".to_string()
}

fn default_version() -> String {
	"1.0.0".to_string()
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::settings::fragment::SettingsFragment;
	use rstest::rstest;

	#[rstest]
	fn test_openapi_section_name() {
		// Arrange / Act
		let section = OpenApiSettings::section();

		// Assert
		assert_eq!(section, "openapi");
	}

	#[rstest]
	fn test_openapi_default_values() {
		// Arrange / Act
		let settings = OpenApiSettings::default();

		// Assert
		assert!(settings.enabled);
		assert_eq!(settings.json_path, "/api/openapi.json");
		assert_eq!(settings.swagger_path, "/api/docs");
		assert_eq!(settings.redoc_path, "/api/redoc");
		assert_eq!(settings.title, "API Documentation");
		assert_eq!(settings.version, "1.0.0");
		assert_eq!(settings.description, None);
	}

	#[rstest]
	fn test_openapi_serde_roundtrip() {
		// Arrange
		let original = OpenApiSettings {
			title: "My API".to_string(),
			version: "2.0.0".to_string(),
			description: Some("Custom API".to_string()),
			..Default::default()
		};

		// Act
		let json = serde_json::to_string(&original).unwrap();
		let deserialized: OpenApiSettings = serde_json::from_str(&json).unwrap();

		// Assert
		assert_eq!(deserialized.title, "My API");
		assert_eq!(deserialized.version, "2.0.0");
		assert_eq!(deserialized.description, Some("Custom API".to_string()));
		assert!(deserialized.enabled);
	}

	#[rstest]
	fn test_openapi_serde_missing_fields_use_defaults() {
		// Arrange
		let json = r#"{"title":"Test API"}"#;

		// Act
		let settings: OpenApiSettings = serde_json::from_str(json).unwrap();

		// Assert
		assert_eq!(settings.title, "Test API");
		assert_eq!(settings.version, "1.0.0");
		assert!(settings.enabled);
		assert_eq!(settings.json_path, "/api/openapi.json");
		assert_eq!(settings.swagger_path, "/api/docs");
		assert_eq!(settings.redoc_path, "/api/redoc");
		assert_eq!(settings.description, None);
	}

	#[rstest]
	fn test_openapi_toml_deserialization() {
		// Arrange
		let toml_str = r#"
enabled = true
title = "My REST API"
version = "3.0.0"
swagger_path = "/docs/swagger"
redoc_path = "/docs/redoc"
json_path = "/docs/openapi.json"
description = "Full API documentation"
"#;

		// Act
		let settings: OpenApiSettings = toml::from_str(toml_str).expect("failed to parse TOML");

		// Assert
		assert!(settings.enabled);
		assert_eq!(settings.title, "My REST API");
		assert_eq!(settings.version, "3.0.0");
		assert_eq!(settings.swagger_path, "/docs/swagger");
		assert_eq!(settings.redoc_path, "/docs/redoc");
		assert_eq!(settings.json_path, "/docs/openapi.json");
		assert_eq!(
			settings.description,
			Some("Full API documentation".to_string())
		);
	}

	#[rstest]
	fn test_openapi_toml_partial_uses_defaults() {
		// Arrange
		let toml_str = r#"
title = "Partial API"
"#;

		// Act
		let settings: OpenApiSettings = toml::from_str(toml_str).expect("failed to parse TOML");

		// Assert
		assert!(settings.enabled);
		assert_eq!(settings.title, "Partial API");
		assert_eq!(settings.version, "1.0.0");
		assert_eq!(settings.swagger_path, "/api/docs");
	}

	#[rstest]
	fn test_openapi_disabled() {
		// Arrange
		let json = r#"{"enabled":false}"#;

		// Act
		let settings: OpenApiSettings = serde_json::from_str(json).unwrap();

		// Assert
		assert!(!settings.enabled);
		assert_eq!(settings.title, "API Documentation");
	}

	#[rstest]
	fn test_has_openapi_settings_trait() {
		// Arrange
		struct Wrapper {
			openapi: OpenApiSettings,
		}

		impl HasOpenApiSettings for Wrapper {
			fn openapi(&self) -> &OpenApiSettings {
				&self.openapi
			}
		}

		let wrapper = Wrapper {
			openapi: OpenApiSettings {
				title: "Trait Test".to_string(),
				..Default::default()
			},
		};

		// Act
		let settings = wrapper.openapi();

		// Assert
		assert_eq!(settings.title, "Trait Test");
	}
}
