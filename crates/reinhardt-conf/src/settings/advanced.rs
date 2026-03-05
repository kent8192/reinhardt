//! Advanced settings and configuration
//!
//! This module provides a flexible configuration system inspired by Django's settings.
//! Settings can be loaded from environment variables, configuration files, or code.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Main application settings
#[non_exhaustive]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdvancedSettings {
	/// Debug mode
	#[serde(default)]
	pub debug: bool,

	/// Secret key for cryptographic signing
	#[serde(default = "default_secret_key")]
	pub secret_key: String,

	/// Allowed hosts
	#[serde(default)]
	pub allowed_hosts: Vec<String>,

	/// Database configuration
	#[serde(default)]
	pub database: DatabaseSettings,

	/// Cache configuration
	#[serde(default)]
	pub cache: CacheSettings,

	/// Session configuration
	#[serde(default)]
	pub session: SessionSettings,

	/// CORS configuration
	#[serde(default)]
	pub cors: CorsSettings,

	/// Static files configuration
	#[serde(default)]
	pub static_files: StaticSettings,

	/// Media files configuration
	#[serde(default)]
	pub media: MediaSettings,

	/// Email configuration
	#[serde(default)]
	pub email: EmailSettings,

	/// Logging configuration
	#[serde(default)]
	pub logging: LoggingSettings,

	/// Custom application-specific settings
	#[serde(default)]
	pub custom: HashMap<String, serde_json::Value>,
}

impl Default for AdvancedSettings {
	fn default() -> Self {
		Self {
			debug: false,
			secret_key: "change-me-in-production".to_string(),
			allowed_hosts: vec!["localhost".to_string(), "127.0.0.1".to_string()],
			database: DatabaseSettings::default(),
			cache: CacheSettings::default(),
			session: SessionSettings::default(),
			cors: CorsSettings::default(),
			static_files: StaticSettings::default(),
			media: MediaSettings::default(),
			email: EmailSettings::default(),
			logging: LoggingSettings::default(),
			custom: HashMap::new(),
		}
	}
}

impl AdvancedSettings {
	/// Create new settings with defaults
	pub fn new() -> Self {
		Self::default()
	}
	/// Validate settings
	///
	pub fn validate(&self) -> Result<(), SettingsError> {
		if self.secret_key == "change-me-in-production" && !self.debug {
			return Err(SettingsError::ValidationError(
				"SECRET_KEY must be changed in production".to_string(),
			));
		}

		if self.secret_key.len() < 32 {
			return Err(SettingsError::ValidationError(
				"SECRET_KEY must be at least 32 characters".to_string(),
			));
		}

		if self.allowed_hosts.is_empty() && !self.debug {
			return Err(SettingsError::ValidationError(
				"ALLOWED_HOSTS must not be empty in production".to_string(),
			));
		}

		Ok(())
	}
	/// Load settings from environment variables
	///
	pub fn from_env() -> Result<Self, SettingsError> {
		let mut settings = Self::default();

		if let Ok(debug) = std::env::var("REINHARDT_DEBUG") {
			settings.debug = debug.to_lowercase() == "true" || debug == "1";
		}

		if let Ok(secret) = std::env::var("REINHARDT_SECRET_KEY") {
			settings.secret_key = secret;
		}

		if let Ok(hosts) = std::env::var("REINHARDT_ALLOWED_HOSTS") {
			settings.allowed_hosts = hosts.split(',').map(|s| s.trim().to_string()).collect();
		}

		// Database
		if let Ok(url) = std::env::var("DATABASE_URL") {
			settings.database.url = url;
		}

		// Cache
		if let Ok(backend) = std::env::var("CACHE_BACKEND") {
			settings.cache.backend = backend;
		}

		Ok(settings)
	}
	/// Load settings from a configuration file
	///
	pub fn from_file(path: impl Into<PathBuf>) -> Result<Self, SettingsError> {
		let path = path.into();
		let contents = std::fs::read_to_string(&path).map_err(|e| {
			SettingsError::FileError(format!("Failed to read {}: {}", path.display(), e))
		})?;

		let settings: AdvancedSettings =
			if path.extension().and_then(|s| s.to_str()) == Some("toml") {
				toml::from_str(&contents)
					.map_err(|e| SettingsError::ParseError(format!("TOML parse error: {}", e)))?
			} else if path.extension().and_then(|s| s.to_str()) == Some("json") {
				serde_json::from_str(&contents)
					.map_err(|e| SettingsError::ParseError(format!("JSON parse error: {}", e)))?
			} else {
				return Err(SettingsError::UnsupportedFormat(
					"Supported formats: .toml, .json".to_string(),
				));
			};

		Ok(settings)
	}
	/// Set a custom setting
	///
	pub fn set<T: Serialize>(
		&mut self,
		key: impl Into<String>,
		value: T,
	) -> Result<(), SettingsError> {
		let json_value = serde_json::to_value(value)
			.map_err(|e| SettingsError::SerializationError(e.to_string()))?;
		self.custom.insert(key.into(), json_value);
		Ok(())
	}
	/// Get a custom setting
	///
	pub fn get<T: for<'de> Deserialize<'de>>(&self, key: &str) -> Option<T> {
		self.custom
			.get(key)
			.and_then(|v| serde_json::from_value(v.clone()).ok())
	}
}

/// Database settings
#[non_exhaustive]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseSettings {
	pub url: String,
	pub max_connections: u32,
	pub min_connections: u32,
	pub connect_timeout: u64,
	pub idle_timeout: u64,
}

impl Default for DatabaseSettings {
	fn default() -> Self {
		Self {
			url: "sqlite::memory:".to_string(),
			max_connections: 10,
			min_connections: 1,
			connect_timeout: 30,
			idle_timeout: 600,
		}
	}
}

/// Cache settings
#[non_exhaustive]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheSettings {
	pub backend: String,
	pub location: Option<String>,
	pub timeout: u64,
}

impl Default for CacheSettings {
	fn default() -> Self {
		Self {
			backend: "memory".to_string(),
			location: None,
			timeout: 300,
		}
	}
}

/// Session settings
#[non_exhaustive]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionSettings {
	pub engine: String,
	pub cookie_name: String,
	pub cookie_age: u64,
	pub cookie_secure: bool,
	pub cookie_httponly: bool,
	pub cookie_samesite: String,
}

impl Default for SessionSettings {
	fn default() -> Self {
		Self {
			engine: "cookie".to_string(),
			cookie_name: "sessionid".to_string(),
			cookie_age: 1209600, // 2 weeks
			cookie_secure: false,
			cookie_httponly: true,
			cookie_samesite: "lax".to_string(),
		}
	}
}

/// CORS settings
#[non_exhaustive]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorsSettings {
	pub allow_origins: Vec<String>,
	pub allow_methods: Vec<String>,
	pub allow_headers: Vec<String>,
	pub allow_credentials: bool,
	pub max_age: u64,
}

impl Default for CorsSettings {
	fn default() -> Self {
		Self {
			allow_origins: vec!["*".to_string()],
			allow_methods: vec![
				"GET".to_string(),
				"POST".to_string(),
				"PUT".to_string(),
				"PATCH".to_string(),
				"DELETE".to_string(),
			],
			allow_headers: vec!["*".to_string()],
			allow_credentials: false,
			max_age: 3600,
		}
	}
}

/// Static files settings
#[non_exhaustive]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StaticSettings {
	pub url: String,
	pub root: PathBuf,
}

impl Default for StaticSettings {
	fn default() -> Self {
		Self {
			url: "/static/".to_string(),
			root: PathBuf::from("static"),
		}
	}
}

/// Media files settings
#[non_exhaustive]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaSettings {
	pub url: String,
	pub root: PathBuf,
}

impl Default for MediaSettings {
	fn default() -> Self {
		Self {
			url: "/media/".to_string(),
			root: PathBuf::from("media"),
		}
	}
}

/// Email settings
#[non_exhaustive]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmailSettings {
	pub backend: String,
	pub host: String,
	pub port: u16,
	pub username: Option<String>,
	pub password: Option<String>,
	pub use_tls: bool,
	pub use_ssl: bool,
	pub from_email: String,

	/// List of (name, email) tuples for site administrators
	/// Used by mail_admins() helper
	#[serde(default)]
	pub admins: Vec<(String, String)>,

	/// List of (name, email) tuples for site managers
	/// Used by mail_managers() helper
	#[serde(default)]
	pub managers: Vec<(String, String)>,

	/// Email address for server error notifications
	#[serde(default = "default_server_email")]
	pub server_email: String,

	/// Prefix for email subjects (e.g., `"[Django]"`)
	#[serde(default)]
	pub subject_prefix: String,

	/// Connection timeout in seconds
	pub timeout: Option<u64>,

	/// Path to SSL certificate file
	pub ssl_certfile: Option<PathBuf>,

	/// Path to SSL key file
	pub ssl_keyfile: Option<PathBuf>,

	/// Directory path for file-based email backend.
	/// Required when backend is "file".
	#[serde(default)]
	pub file_path: Option<PathBuf>,
}

fn default_secret_key() -> String {
	"change-me-in-production".to_string()
}

fn default_server_email() -> String {
	"root@localhost".to_string()
}

impl Default for EmailSettings {
	fn default() -> Self {
		Self {
			backend: "console".to_string(),
			host: "localhost".to_string(),
			port: 25,
			username: None,
			password: None,
			use_tls: false,
			use_ssl: false,
			from_email: "noreply@example.com".to_string(),
			admins: Vec::new(),
			managers: Vec::new(),
			server_email: default_server_email(),
			subject_prefix: String::new(),
			timeout: None,
			ssl_certfile: None,
			ssl_keyfile: None,
			file_path: None,
		}
	}
}

/// Logging settings
#[non_exhaustive]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingSettings {
	pub level: String,
	pub format: String,
}

impl Default for LoggingSettings {
	fn default() -> Self {
		Self {
			level: "info".to_string(),
			format: "text".to_string(),
		}
	}
}

/// Settings error
#[non_exhaustive]
#[derive(Debug, thiserror::Error)]
pub enum SettingsError {
	#[error("File error: {0}")]
	FileError(String),

	#[error("Parse error: {0}")]
	ParseError(String),

	#[error("Validation error: {0}")]
	ValidationError(String),

	#[error("Unsupported format: {0}")]
	UnsupportedFormat(String),

	#[error("Serialization error: {0}")]
	SerializationError(String),
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_default_settings() {
		let settings = AdvancedSettings::default();
		assert!(!settings.debug);
		assert_eq!(settings.database.url, "sqlite::memory:");
		assert_eq!(settings.cache.backend, "memory");
	}

	#[test]
	fn test_settings_validation() {
		let mut settings = AdvancedSettings::default();

		// Should fail with default secret key
		assert!(settings.validate().is_err());

		// Should pass with proper secret key
		settings.secret_key = "a".repeat(32);
		assert!(settings.validate().is_ok());
	}

	#[test]
	fn test_custom_settings() {
		let mut settings = AdvancedSettings::default();

		settings.set("api_version", "v1").unwrap();
		settings.set("max_upload_size", 10485760_u64).unwrap();

		let version: String = settings.get("api_version").unwrap();
		assert_eq!(version, "v1");

		let max_size: u64 = settings.get("max_upload_size").unwrap();
		assert_eq!(max_size, 10485760);
	}
}
