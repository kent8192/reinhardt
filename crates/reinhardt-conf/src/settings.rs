//! # Settings Module
//!
//! Django-inspired settings system for Reinhardt projects.
//! This module provides configuration management for Reinhardt applications.

pub mod advanced;
pub mod builder;
pub mod env;
pub mod env_loader;
pub mod env_parser;
pub mod prelude;
pub mod profile;
pub mod secret_types;
pub mod sources;
pub mod validation;

// Dynamic settings (async feature required)
#[cfg(feature = "async")]
pub mod dynamic;

#[cfg(feature = "async")]
pub mod backends;

#[cfg(feature = "async")]
pub mod secrets;

#[cfg(feature = "encryption")]
pub mod encryption;

#[cfg(feature = "async")]
pub mod audit;

#[cfg(feature = "hot-reload")]
pub mod hot_reload;

pub mod config;
pub mod database_config;
pub mod docs;
pub mod testing;

use reinhardt_utils::staticfiles::storage::StaticFilesConfig;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Contact information for administrators and managers
///
/// Used for error notifications, broken link notifications, etc.
///
/// # Examples
///
/// ```
/// use reinhardt_conf::settings::Contact;
///
/// let admin = Contact::new("John Doe", "john@example.com");
/// assert_eq!(admin.name, "John Doe");
/// assert_eq!(admin.email, "john@example.com");
/// ```
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Contact {
	/// Person's name
	pub name: String,
	/// Email address
	pub email: String,
}

impl Contact {
	/// Create a new contact
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_conf::settings::Contact;
	///
	/// let contact = Contact::new("Alice Smith", "alice@example.com");
	/// assert_eq!(contact.name, "Alice Smith");
	/// assert_eq!(contact.email, "alice@example.com");
	/// ```
	pub fn new(name: impl Into<String>, email: impl Into<String>) -> Self {
		Self {
			name: name.into(),
			email: email.into(),
		}
	}
}

// Re-export from advanced module
pub use advanced::{
	AdvancedSettings, CacheSettings, CorsSettings, DatabaseSettings as AdvancedDatabaseSettings,
	EmailSettings, LoggingSettings, MediaSettings, SessionSettings, SettingsError, StaticSettings,
};

/// Main settings structure for a Reinhardt project
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Settings {
	/// Base directory of the project
	pub base_dir: PathBuf,

	/// Secret key for cryptographic signing (SECURITY WARNING: keep secret in production!)
	pub secret_key: String,

	/// Debug mode (SECURITY WARNING: don't run with debug=true in production!)
	pub debug: bool,

	/// List of allowed host/domain names
	pub allowed_hosts: Vec<String>,

	/// List of installed applications
	///
	/// Defaults to an empty vector. Use the `installed_apps!` macro
	/// to register application modules with compile-time validation.
	pub installed_apps: Vec<String>,

	/// List of middleware classes
	///
	/// Defaults to an empty vector. Configure middleware as needed
	/// for your application.
	pub middleware: Vec<String>,

	/// Root URL configuration module
	pub root_urlconf: String,

	/// Database configurations
	pub databases: HashMap<String, DatabaseConfig>,

	/// Template engine configurations.
	///
	/// **Note:** Currently not consumed by the framework. Reserved for future
	/// template engine integration. Setting this value has no effect on framework
	/// behavior.
	pub templates: Vec<TemplateConfig>,

	/// Static files URL prefix
	pub static_url: String,

	/// Static files root directory
	pub static_root: Option<PathBuf>,

	/// Additional static files directories (STATICFILES_DIRS)
	pub staticfiles_dirs: Vec<PathBuf>,

	/// Media files URL prefix
	pub media_url: String,

	/// Media files root directory
	pub media_root: Option<PathBuf>,

	/// Language code for internationalization.
	///
	/// **Note:** Currently not consumed by the framework. Reserved for future
	/// i18n implementation. Setting this value has no effect on framework behavior.
	pub language_code: String,

	/// Time zone for datetime handling.
	///
	/// **Note:** Currently not consumed by the framework. Reserved for future
	/// timezone support implementation. Setting this value has no effect on
	/// framework behavior.
	pub time_zone: String,

	/// Enable internationalization.
	///
	/// **Note:** Currently not consumed by the framework. Reserved for future
	/// i18n implementation. Setting this value has no effect on framework behavior.
	pub use_i18n: bool,

	/// Use timezone-aware datetimes.
	///
	/// **Note:** Currently not consumed by the framework. Reserved for future
	/// timezone support implementation. Setting this value has no effect on
	/// framework behavior.
	pub use_tz: bool,

	/// Default auto field type for models.
	///
	/// **Note:** Currently not consumed by the framework. Reserved for future
	/// auto field configuration. Setting this value has no effect on framework
	/// behavior.
	pub default_auto_field: String,

	/// HTTPS/Security settings
	/// Header name and value to use for identifying secure requests behind a proxy
	/// Example: Some(("HTTP_X_FORWARDED_PROTO", "https"))
	pub secure_proxy_ssl_header: Option<(String, String)>,

	/// Redirect all HTTP requests to HTTPS
	pub secure_ssl_redirect: bool,

	/// Seconds to set HSTS max-age header
	pub secure_hsts_seconds: Option<u64>,

	/// Include subdomains in HSTS policy
	pub secure_hsts_include_subdomains: bool,

	/// Include preload directive in HSTS header
	pub secure_hsts_preload: bool,

	/// Only send cookies over HTTPS
	pub session_cookie_secure: bool,

	/// Only send CSRF cookie over HTTPS
	pub csrf_cookie_secure: bool,

	/// Automatically append trailing slashes to URLs
	pub append_slash: bool,

	/// List of administrators who receive error notifications
	/// Django equivalent: ADMINS = [('name', 'email'), ...]
	pub admins: Vec<Contact>,

	/// List of managers who receive broken link notifications, etc.
	/// Django equivalent: MANAGERS = [('name', 'email'), ...]
	pub managers: Vec<Contact>,
}

impl Settings {
	/// Create a new Settings instance with default values
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_conf::settings::Settings;
	/// use std::path::PathBuf;
	///
	/// let settings = Settings::new(
	///     PathBuf::from("/app"),
	///     "my-secret-key-12345".to_string()
	/// );
	///
	/// assert_eq!(settings.base_dir, PathBuf::from("/app"));
	/// assert_eq!(settings.secret_key, "my-secret-key-12345");
	/// assert!(settings.debug);
	/// assert_eq!(settings.time_zone, "UTC");
	/// assert!(settings.installed_apps.is_empty());
	/// ```
	pub fn new(base_dir: PathBuf, secret_key: String) -> Self {
		Self {
			base_dir,
			secret_key,
			debug: true,
			allowed_hosts: vec![],
			installed_apps: vec![],
			middleware: vec![],
			root_urlconf: String::new(),

			databases: {
				let mut dbs = HashMap::new();
				dbs.insert("default".to_string(), DatabaseConfig::default());
				dbs
			},
			templates: vec![TemplateConfig::default()],
			static_url: "/static/".to_string(),
			static_root: None,
			staticfiles_dirs: vec![],
			media_url: "/media/".to_string(),
			media_root: None,
			language_code: "en-us".to_string(),
			time_zone: "UTC".to_string(),
			use_i18n: true,
			use_tz: true,
			default_auto_field: "reinhardt.db.models.BigAutoField".to_string(),
			secure_proxy_ssl_header: None,
			secure_ssl_redirect: false,
			secure_hsts_seconds: None,
			secure_hsts_include_subdomains: false,
			secure_hsts_preload: false,
			session_cookie_secure: false,
			csrf_cookie_secure: false,
			append_slash: true,
			admins: vec![],
			managers: vec![],
		}
	}
	/// Add an installed app
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_conf::settings::Settings;
	///
	/// let mut settings = Settings::default();
	/// let initial_count = settings.installed_apps.len();
	/// settings.add_app("myapp");
	///
	/// assert_eq!(settings.installed_apps.len(), initial_count + 1);
	/// assert!(settings.installed_apps.contains(&"myapp".to_string()));
	/// ```
	pub fn add_app(&mut self, app: impl Into<String>) {
		self.installed_apps.push(app.into());
	}
	/// Create settings with a compile-time validated app list
	///
	/// This method accepts a function that returns `Vec<String>` generated by the
	/// `installed_apps!` macro, providing compile-time validation of app names.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_conf::settings::Settings;
	///
	/// let settings = Settings::default()
	///     .with_validated_apps(|| vec![
	///         "reinhardt.contrib.admin".to_string(),
	///         "myapp".to_string(),
	///     ]);
	///
	/// assert_eq!(settings.installed_apps.len(), 2);
	/// assert!(settings.installed_apps.contains(&"myapp".to_string()));
	/// ```
	pub fn with_validated_apps<F>(mut self, app_provider: F) -> Self
	where
		F: FnOnce() -> Vec<String>,
	{
		self.installed_apps = app_provider();
		self
	}
	/// Add an administrator
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_conf::settings::{Settings, Contact};
	///
	/// let mut settings = Settings::default();
	/// settings.add_admin("John Doe", "john@example.com");
	///
	/// assert_eq!(settings.admins.len(), 1);
	/// assert_eq!(settings.admins[0].name, "John Doe");
	/// assert_eq!(settings.admins[0].email, "john@example.com");
	/// ```
	pub fn add_admin(&mut self, name: impl Into<String>, email: impl Into<String>) {
		self.admins.push(Contact::new(name, email));
	}

	/// Add a manager
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_conf::settings::{Settings, Contact};
	///
	/// let mut settings = Settings::default();
	/// settings.add_manager("Jane Smith", "jane@example.com");
	///
	/// assert_eq!(settings.managers.len(), 1);
	/// assert_eq!(settings.managers[0].name, "Jane Smith");
	/// assert_eq!(settings.managers[0].email, "jane@example.com");
	/// ```
	pub fn add_manager(&mut self, name: impl Into<String>, email: impl Into<String>) {
		self.managers.push(Contact::new(name, email));
	}

	/// Set managers to be the same as administrators
	///
	/// This is a common pattern in Django projects where MANAGERS = ADMINS
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_conf::settings::Settings;
	///
	/// let mut settings = Settings::default();
	/// settings.add_admin("John Doe", "john@example.com");
	/// settings.add_admin("Jane Smith", "jane@example.com");
	/// settings.managers_from_admins();
	///
	/// assert_eq!(settings.managers.len(), 2);
	/// assert_eq!(settings.managers, settings.admins);
	/// ```
	pub fn managers_from_admins(&mut self) {
		self.managers = self.admins.clone();
	}

	/// Set administrators with a fluent API
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_conf::settings::{Settings, Contact};
	///
	/// let settings = Settings::default()
	///     .with_admins(vec![
	///         Contact::new("John Doe", "john@example.com"),
	///         Contact::new("Jane Smith", "jane@example.com"),
	///     ]);
	///
	/// assert_eq!(settings.admins.len(), 2);
	/// ```
	pub fn with_admins(mut self, admins: Vec<Contact>) -> Self {
		self.admins = admins;
		self
	}

	/// Set managers with a fluent API
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_conf::settings::{Settings, Contact};
	///
	/// let settings = Settings::default()
	///     .with_managers(vec![
	///         Contact::new("Alice Brown", "alice@example.com"),
	///     ]);
	///
	/// assert_eq!(settings.managers.len(), 1);
	/// ```
	pub fn with_managers(mut self, managers: Vec<Contact>) -> Self {
		self.managers = managers;
		self
	}

	/// Convert Settings to StaticFilesConfig
	///
	/// This method extracts static files related configuration from Settings
	/// and creates a StaticFilesConfig instance suitable for use with CollectStaticCommand.
	///
	/// # Returns
	///
	/// Returns `Ok(StaticFilesConfig)` if static_root is configured,
	/// or `Err` if static_root is None.
	///
	/// # Examples
	///
	/// ```ignore
	/// use reinhardt_conf::settings::Settings;
	/// use std::path::PathBuf;
	///
	/// let settings = Settings::new(
	///     PathBuf::from("/app"),
	///     "secret".to_string()
	/// );
	///
	/// let config = settings.get_static_config().unwrap();
	/// assert_eq!(config.static_url, "/static/");
	/// ```
	pub fn get_static_config(&self) -> Result<StaticFilesConfig, String> {
		let static_root = self
			.static_root
			.clone()
			.ok_or_else(|| "STATIC_ROOT is not configured".to_string())?;

		Ok(StaticFilesConfig {
			static_root,
			static_url: self.static_url.clone(),
			staticfiles_dirs: self.staticfiles_dirs.clone(),
			media_url: Some(self.media_url.clone()),
		})
	}
}

impl Default for Settings {
	fn default() -> Self {
		Self::new(
			PathBuf::from("."),
			"insecure-change-this-in-production".to_string(),
		)
	}
}

// Re-export DatabaseConfig from database_config module
pub use database_config::DatabaseConfig;

/// Template engine configuration
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TemplateConfig {
	/// Template backend/engine
	pub backend: String,

	/// Directories to search for templates
	pub dirs: Vec<PathBuf>,

	/// Search for templates in app directories
	pub app_dirs: bool,

	/// Template engine options
	pub options: HashMap<String, serde_json::Value>,
}

impl TemplateConfig {
	/// Create a new template configuration
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_conf::settings::TemplateConfig;
	///
	/// let config = TemplateConfig::new("reinhardt.template.backends.jinja2.Jinja2");
	///
	/// assert_eq!(config.backend, "reinhardt.template.backends.jinja2.Jinja2");
	/// assert!(config.app_dirs);
	/// assert_eq!(config.dirs.len(), 0);
	/// ```
	pub fn new(backend: impl Into<String>) -> Self {
		Self {
			backend: backend.into(),
			dirs: vec![],
			app_dirs: true,
			options: HashMap::new(),
		}
	}
	/// Add a template directory
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_conf::settings::TemplateConfig;
	/// use std::path::PathBuf;
	///
	/// let config = TemplateConfig::new("reinhardt.template.backends.jinja2.Jinja2")
	///     .add_dir("/app/templates");
	///
	/// assert_eq!(config.dirs.len(), 1);
	/// assert_eq!(config.dirs[0], PathBuf::from("/app/templates"));
	/// ```
	pub fn add_dir(mut self, dir: impl Into<PathBuf>) -> Self {
		self.dirs.push(dir.into());
		self
	}
}

impl Default for TemplateConfig {
	fn default() -> Self {
		let mut options = HashMap::new();
		options.insert(
			"context_processors".to_string(),
			serde_json::json!([
				"reinhardt.template.context_processors.request",
				"reinhardt.contrib.auth.context_processors.auth",
				"reinhardt.contrib.messages.context_processors.messages",
			]),
		);

		Self {
			backend: "reinhardt.template.backends.jinja2.Jinja2".to_string(),
			dirs: vec![],
			app_dirs: true,
			options,
		}
	}
}

/// Middleware configuration
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MiddlewareConfig {
	/// Full path to the middleware class
	pub path: String,

	/// Middleware options
	pub options: HashMap<String, serde_json::Value>,
}

impl MiddlewareConfig {
	/// Create a new middleware configuration
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_conf::settings::MiddlewareConfig;
	///
	/// let middleware = MiddlewareConfig::new("myapp.middleware.CustomMiddleware");
	///
	/// assert_eq!(middleware.path, "myapp.middleware.CustomMiddleware");
	/// assert_eq!(middleware.options.len(), 0);
	/// ```
	pub fn new(path: impl Into<String>) -> Self {
		Self {
			path: path.into(),
			options: HashMap::new(),
		}
	}
	/// Add an option to the middleware
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_conf::settings::MiddlewareConfig;
	///
	/// let middleware = MiddlewareConfig::new("myapp.middleware.CustomMiddleware")
	///     .with_option("timeout", serde_json::json!(30));
	///
	/// assert_eq!(middleware.options.get("timeout"), Some(&serde_json::json!(30)));
	/// ```
	pub fn with_option(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
		self.options.insert(key.into(), value);
		self
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_settings_default_unit() {
		let settings = Settings::default();
		assert!(settings.debug);
		assert_eq!(settings.language_code, "en-us");
		assert_eq!(settings.time_zone, "UTC");
	}

	#[test]
	fn test_template_config() {
		let config = TemplateConfig::default();
		assert!(config.app_dirs);
		assert_eq!(config.backend, "reinhardt.template.backends.jinja2.Jinja2");
	}

	#[test]
	fn test_middleware_config() {
		let middleware = MiddlewareConfig::new("reinhardt.middleware.TestMiddleware")
			.with_option("enabled", serde_json::json!(true));

		assert_eq!(middleware.path, "reinhardt.middleware.TestMiddleware");
		assert_eq!(
			middleware.options.get("enabled"),
			Some(&serde_json::json!(true))
		);
	}

	#[test]
	fn test_contact_creation() {
		let contact = Contact::new("Alice Smith", "alice@example.com");
		assert_eq!(contact.name, "Alice Smith");
		assert_eq!(contact.email, "alice@example.com");
	}

	#[test]
	fn test_settings_admins() {
		let mut settings = Settings::default();
		assert_eq!(settings.admins.len(), 0);

		settings.add_admin("John Doe", "john@example.com");
		assert_eq!(settings.admins.len(), 1);
		assert_eq!(settings.admins[0].name, "John Doe");
		assert_eq!(settings.admins[0].email, "john@example.com");
	}

	#[test]
	fn test_settings_managers() {
		let mut settings = Settings::default();
		assert_eq!(settings.managers.len(), 0);

		settings.add_manager("Jane Smith", "jane@example.com");
		assert_eq!(settings.managers.len(), 1);
		assert_eq!(settings.managers[0].name, "Jane Smith");
		assert_eq!(settings.managers[0].email, "jane@example.com");
	}

	#[test]
	fn test_managers_from_admins() {
		let mut settings = Settings::default();
		settings.add_admin("John Doe", "john@example.com");
		settings.add_admin("Jane Smith", "jane@example.com");

		assert_eq!(settings.managers.len(), 0);

		settings.managers_from_admins();

		assert_eq!(settings.managers.len(), 2);
		assert_eq!(settings.managers, settings.admins);
	}

	#[test]
	fn test_with_admins_fluent_api() {
		let settings = Settings::default().with_admins(vec![
			Contact::new("Alice", "alice@example.com"),
			Contact::new("Bob", "bob@example.com"),
		]);

		assert_eq!(settings.admins.len(), 2);
		assert_eq!(settings.admins[0].name, "Alice");
		assert_eq!(settings.admins[1].name, "Bob");
	}

	#[test]
	fn test_with_managers_fluent_api() {
		let settings =
			Settings::default().with_managers(vec![Contact::new("Charlie", "charlie@example.com")]);

		assert_eq!(settings.managers.len(), 1);
		assert_eq!(settings.managers[0].name, "Charlie");
	}
}
