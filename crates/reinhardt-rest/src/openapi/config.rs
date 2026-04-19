//! OpenAPI configuration for endpoint mounting
//!
//! **Deprecated since 0.1.0-rc.16**: Use `reinhardt_conf::settings::openapi::OpenApiSettings`
//! fragment with the composable settings system instead.
//!
//! This module is retained for backward compatibility. New code should use
//! `OpenApiSettings` from `reinhardt-conf`.

use serde::{Deserialize, Serialize};

/// A security scheme entry for OpenAPI documentation.
#[derive(Clone)]
pub struct SecuritySchemeEntry {
	/// Name of the scheme (e.g., "bearer", "session", "oauth2")
	pub name: String,
	/// The OpenAPI security scheme definition
	pub scheme: utoipa::openapi::security::SecurityScheme,
	/// OAuth2 scopes (if applicable)
	pub scopes: Vec<(String, String)>,
}

impl std::fmt::Debug for SecuritySchemeEntry {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("SecuritySchemeEntry")
			.field("name", &self.name)
			.field("scopes", &self.scopes)
			.finish_non_exhaustive()
	}
}

/// Configuration for OpenAPI endpoint mounting
///
/// This configuration controls how OpenAPI documentation endpoints are
/// automatically mounted by the `runserver` command.
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
/// use reinhardt_rest::openapi::config::OpenApiConfig;
///
/// let config = OpenApiConfig::default();
/// assert!(config.enabled);
/// ```
#[deprecated(
	since = "0.1.0-rc.16",
	note = "use `OpenApiSettings` from `reinhardt_conf::settings::openapi` instead"
)]
#[non_exhaustive]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenApiConfig {
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

	/// Security schemes for OpenAPI documentation.
	///
	/// Derived from application auth backend configuration.
	/// Each entry defines a security scheme (e.g., Bearer, Cookie, OAuth2)
	/// that will appear in the OpenAPI `components.securitySchemes`.
	#[serde(skip)]
	pub security_schemes: Vec<SecuritySchemeEntry>,
}

#[allow(deprecated)] // Internal: OpenApiConfig is deprecated but we still need Default
impl Default for OpenApiConfig {
	fn default() -> Self {
		Self {
			enabled: true,
			json_path: "/api/openapi.json".to_string(),
			swagger_path: "/api/docs".to_string(),
			redoc_path: "/api/redoc".to_string(),
			title: "API Documentation".to_string(),
			version: "1.0.0".to_string(),
			description: None,
			security_schemes: Vec::new(),
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
#[allow(deprecated)] // Tests exercise deprecated OpenApiConfig for backward-compatibility verification
mod tests {
	use super::*;

	#[test]
	fn test_default_config() {
		let config = OpenApiConfig::default();
		assert!(config.enabled);
		assert_eq!(config.json_path, "/api/openapi.json");
		assert_eq!(config.swagger_path, "/api/docs");
		assert_eq!(config.redoc_path, "/api/redoc");
		assert_eq!(config.title, "API Documentation");
		assert_eq!(config.version, "1.0.0");
		assert_eq!(config.description, None);
	}

	#[test]
	fn test_custom_config() {
		let config = OpenApiConfig {
			title: "My API".to_string(),
			version: "2.0.0".to_string(),
			description: Some("Custom API".to_string()),
			..Default::default()
		};
		assert_eq!(config.title, "My API");
		assert_eq!(config.version, "2.0.0");
		assert_eq!(config.description, Some("Custom API".to_string()));
	}

	#[test]
	fn test_serde_serialization() {
		let config = OpenApiConfig::default();
		let json = serde_json::to_string(&config).unwrap();
		let deserialized: OpenApiConfig = serde_json::from_str(&json).unwrap();
		assert_eq!(config.enabled, deserialized.enabled);
		assert_eq!(config.json_path, deserialized.json_path);
	}

	#[test]
	fn test_serde_with_missing_fields() {
		// JSON with missing optional fields should use defaults
		let json = r#"{"title":"Test API"}"#;
		let config: OpenApiConfig = serde_json::from_str(json).unwrap();
		assert_eq!(config.title, "Test API");
		assert_eq!(config.version, "1.0.0"); // Default
		assert!(config.enabled); // Default
	}
}
