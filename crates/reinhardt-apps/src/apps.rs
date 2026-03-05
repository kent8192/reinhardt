//! # Application Registry
//!
//! Django-inspired application configuration and registry system.
//! This module provides the infrastructure for managing Django-style apps
//! in a Reinhardt project.
//!
//! This module provides both string-based (runtime) and type-safe (compile-time)
//! application registry mechanisms.

use crate::signals;
use std::collections::HashMap;
use std::error::Error;
use std::sync::{Arc, Mutex, PoisonError};
use thiserror::Error as ThisError;

/// Errors that can occur when working with the application registry
#[derive(Debug, ThisError)]
pub enum AppError {
	#[error("Application not found: {0}")]
	NotFound(String),

	#[error("Application already registered: {0}")]
	AlreadyRegistered(String),

	#[error("Invalid application label: {0}")]
	InvalidLabel(String),

	#[error("Duplicate application label: {0}")]
	DuplicateLabel(String),

	#[error("Duplicate application name: {0}")]
	DuplicateName(String),

	#[error("Application registry not ready")]
	NotReady,

	#[error("Application configuration error: {0}")]
	ConfigError(String),

	#[error("Registry state error: {0}")]
	RegistryState(String),
}

pub type AppResult<T> = Result<T, AppError>;

/// Configuration for a single application
#[derive(Clone, Debug)]
pub struct AppConfig {
	/// The full Python-style name of the application (e.g., "myapp" or "myproject.apps.MyAppConfig")
	pub name: String,

	/// The short label for the application (e.g., "myapp")
	pub label: String,

	/// Human-readable name for the application
	pub verbose_name: Option<String>,

	/// Filesystem path to the application
	pub path: Option<String>,

	/// Default auto field type for models in this app
	pub default_auto_field: Option<String>,

	/// Whether the app has been populated with models
	pub models_ready: bool,
}

impl AppConfig {
	/// Create a new AppConfig with required fields
	pub fn new(name: impl Into<String>, label: impl Into<String>) -> Self {
		Self {
			name: name.into(),
			label: label.into(),
			verbose_name: None,
			path: None,
			default_auto_field: None,
			models_ready: false,
		}
	}

	/// Set the verbose name for the application
	pub fn with_verbose_name(mut self, verbose_name: impl Into<String>) -> Self {
		self.verbose_name = Some(verbose_name.into());
		self
	}

	/// Set the path for the application.
	///
	/// The path is validated to reject path traversal sequences (`..`),
	/// absolute paths (starting with `/` or a Windows drive letter), and
	/// null bytes. These restrictions prevent path traversal attacks when
	/// the path is later used to locate application resources on disk.
	///
	/// # Errors
	///
	/// Returns [`AppError::ConfigError`] if the path contains disallowed
	/// sequences.
	pub fn with_path(mut self, path: impl Into<String>) -> AppResult<Self> {
		let path = path.into();
		Self::validate_path(&path)?;
		self.path = Some(path);
		Ok(self)
	}

	/// Validates an application path to prevent path traversal and injection.
	///
	/// Rejects paths that contain:
	/// - Path traversal sequences (`..`)
	/// - Absolute paths (starting with `/` or a Windows drive letter like `C:\`)
	/// - Null bytes (`\0`)
	/// - Control characters
	fn validate_path(path: &str) -> AppResult<()> {
		if path.is_empty() {
			return Err(AppError::ConfigError(
				"application path cannot be empty".to_string(),
			));
		}

		// Reject null bytes
		if path.contains('\0') {
			return Err(AppError::ConfigError(
				"application path must not contain null bytes".to_string(),
			));
		}

		// Reject control characters (prevents log injection)
		if path.chars().any(|c| c.is_control()) {
			return Err(AppError::ConfigError(
				"application path must not contain control characters".to_string(),
			));
		}

		// Reject absolute paths (Unix-style or Windows-style)
		if path.starts_with('/') || path.starts_with('\\') {
			return Err(AppError::ConfigError(
				"application path must be relative, not absolute".to_string(),
			));
		}

		// Reject Windows drive letter paths (e.g., C:\, D:/)
		if path.len() >= 2 && path.as_bytes()[0].is_ascii_alphabetic() && path.as_bytes()[1] == b':'
		{
			return Err(AppError::ConfigError(
				"application path must be relative, not absolute".to_string(),
			));
		}

		// Reject path traversal sequences
		for component in path.split(['/', '\\']) {
			if component == ".." {
				return Err(AppError::ConfigError(
					"application path must not contain path traversal sequences".to_string(),
				));
			}
		}

		Ok(())
	}

	/// Set the default auto field for the application
	pub fn with_default_auto_field(mut self, field: impl Into<String>) -> Self {
		self.default_auto_field = Some(field.into());
		self
	}

	/// Validate the application label
	pub fn validate_label(&self) -> AppResult<()> {
		if self.label.is_empty() {
			return Err(AppError::InvalidLabel("Label cannot be empty".to_string()));
		}

		// Check if label is a valid Rust identifier
		if !self
			.label
			.chars()
			.next()
			.map(|c| c.is_alphabetic() || c == '_')
			.unwrap_or(false)
		{
			return Err(AppError::InvalidLabel(format!(
				"Label '{}' must start with a letter or underscore",
				self.label
			)));
		}

		if !self.label.chars().all(|c| c.is_alphanumeric() || c == '_') {
			return Err(AppError::InvalidLabel(format!(
				"Label '{}' must contain only alphanumeric characters and underscores",
				self.label
			)));
		}

		Ok(())
	}

	/// Ready hook for the application
	///
	/// This method is called when the application is ready, after all configurations
	/// have been loaded and models have been registered. Override this method in
	/// custom application configurations to perform initialization tasks.
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_apps::AppConfig;
	///
	/// let config = AppConfig::new("myapp", "myapp");
	/// config.ready().expect("Ready hook should succeed");
	/// ```
	pub fn ready(&self) -> Result<(), Box<dyn Error>> {
		// Default implementation does nothing
		// Applications can override this by implementing custom AppConfig structs
		Ok(())
	}
}

// ============================================================================
// Resource Provider Traits
// ============================================================================

/// Trait for providing static file directories
///
/// Applications can implement this trait to provide static files
/// that will be automatically discovered by collectstatic.
pub trait StaticFilesProvider {
	/// Get the static files directory for this app
	///
	/// Returns None if the app does not provide static files
	fn static_dir(&self) -> Option<std::path::PathBuf> {
		None
	}

	/// Get the static URL prefix for this app
	///
	/// Default: "/static/{app_label}/"
	fn static_url_prefix(&self) -> Option<String> {
		None
	}
}

/// Trait for providing locale directories
///
/// Applications can implement this trait to provide translation files
/// that will be automatically discovered by makemessages.
pub trait LocaleProvider {
	/// Get the locale directory for this app
	///
	/// Returns None if the app does not provide translations
	fn locale_dir(&self) -> Option<std::path::PathBuf> {
		None
	}
}

/// Trait for providing media directories
///
/// Applications can implement this trait to provide initial media files
/// that will be automatically discovered by collectmedia.
pub trait MediaProvider {
	/// Get the media directory for this app
	///
	/// Returns None if the app does not provide media files
	fn media_dir(&self) -> Option<std::path::PathBuf> {
		None
	}

	/// Get the media URL prefix for this app
	///
	/// Default: "/media/{app_label}/"
	fn media_url_prefix(&self) -> Option<String> {
		None
	}
}

/// Default implementations for AppConfig
impl StaticFilesProvider for AppConfig {
	fn static_dir(&self) -> Option<std::path::PathBuf> {
		// Default: {app_path}/static/
		if let Some(path) = &self.path {
			let static_path = std::path::PathBuf::from(path).join("static");
			if static_path.exists() && static_path.is_dir() {
				return Some(static_path);
			}
		}
		None
	}

	fn static_url_prefix(&self) -> Option<String> {
		Some(format!("/static/{}/", self.label))
	}
}

impl LocaleProvider for AppConfig {
	fn locale_dir(&self) -> Option<std::path::PathBuf> {
		// Default: {app_path}/locale/
		if let Some(path) = &self.path {
			let locale_path = std::path::PathBuf::from(path).join("locale");
			if locale_path.exists() && locale_path.is_dir() {
				return Some(locale_path);
			}
		}
		None
	}
}

impl MediaProvider for AppConfig {
	fn media_dir(&self) -> Option<std::path::PathBuf> {
		// Default: {app_path}/media/
		if let Some(path) = &self.path {
			let media_path = std::path::PathBuf::from(path).join("media");
			if media_path.exists() && media_path.is_dir() {
				return Some(media_path);
			}
		}
		None
	}

	fn media_url_prefix(&self) -> Option<String> {
		Some(format!("/media/{}/", self.label))
	}
}

/// Main application registry
///
/// This is the central registry for all installed applications in a Reinhardt project.
/// It manages application configuration, initialization order, and provides
/// methods to query installed applications.
#[derive(Clone)]
pub struct Apps {
	/// List of installed application identifiers
	installed_apps: Vec<String>,

	/// Map of application labels to their configurations
	app_configs: Arc<Mutex<HashMap<String, AppConfig>>>,

	/// Map of application names to their labels
	app_names: Arc<Mutex<HashMap<String, String>>>,

	/// Whether the registry has been populated
	ready: Arc<Mutex<bool>>,

	/// Whether app configs have been populated
	apps_ready: Arc<Mutex<bool>>,

	/// Whether models have been populated
	models_ready: Arc<Mutex<bool>>,
}

impl Apps {
	/// Create a new application registry
	pub fn new(installed_apps: Vec<String>) -> Self {
		Self {
			installed_apps,
			app_configs: Arc::new(Mutex::new(HashMap::new())),
			app_names: Arc::new(Mutex::new(HashMap::new())),
			ready: Arc::new(Mutex::new(false)),
			apps_ready: Arc::new(Mutex::new(false)),
			models_ready: Arc::new(Mutex::new(false)),
		}
	}

	/// Check if the registry is fully ready
	pub fn is_ready(&self) -> bool {
		*self.ready.lock().unwrap_or_else(PoisonError::into_inner)
	}

	/// Check if app configurations are ready
	pub fn is_apps_ready(&self) -> bool {
		*self
			.apps_ready
			.lock()
			.unwrap_or_else(PoisonError::into_inner)
	}

	/// Check if models are ready
	pub fn is_models_ready(&self) -> bool {
		*self
			.models_ready
			.lock()
			.unwrap_or_else(PoisonError::into_inner)
	}

	/// Register an application configuration
	pub fn register(&self, config: AppConfig) -> AppResult<()> {
		// Validate the configuration
		config.validate_label()?;

		let mut configs = self
			.app_configs
			.lock()
			.unwrap_or_else(PoisonError::into_inner);
		let mut names = self
			.app_names
			.lock()
			.unwrap_or_else(PoisonError::into_inner);

		// Check for duplicate label
		if configs.contains_key(&config.label) {
			return Err(AppError::DuplicateLabel(config.label.clone()));
		}

		// Check for duplicate name
		if names.contains_key(&config.name) {
			return Err(AppError::DuplicateName(config.name.clone()));
		}

		// Store the configuration
		names.insert(config.name.clone(), config.label.clone());
		configs.insert(config.label.clone(), config);

		Ok(())
	}

	/// Get an application configuration by label
	pub fn get_app_config(&self, label: &str) -> AppResult<AppConfig> {
		self.app_configs
			.lock()
			.unwrap_or_else(PoisonError::into_inner)
			.get(label)
			.cloned()
			.ok_or_else(|| AppError::NotFound(label.to_string()))
	}

	/// Get all registered application configurations
	pub fn get_app_configs(&self) -> Vec<AppConfig> {
		self.app_configs
			.lock()
			.unwrap_or_else(PoisonError::into_inner)
			.values()
			.cloned()
			.collect()
	}

	/// Check if an application is installed
	///
	/// Acquires locks on both `app_names` and `app_configs` before checking,
	/// ensuring a consistent snapshot and avoiding TOCTOU race conditions
	/// where state could change between individual lock acquisitions.
	pub fn is_installed(&self, name: &str) -> bool {
		if self.installed_apps.contains(&name.to_string()) {
			return true;
		}

		// Hold both locks simultaneously for a consistent snapshot
		let names = self
			.app_names
			.lock()
			.unwrap_or_else(PoisonError::into_inner);
		let configs = self
			.app_configs
			.lock()
			.unwrap_or_else(PoisonError::into_inner);

		names.contains_key(name) || configs.contains_key(name)
	}

	/// Populate the registry with application configurations
	///
	/// This method initializes all registered applications by:
	/// 1. Creating AppConfig instances for each installed app
	/// 2. Calling the ready() method on each AppConfig
	/// 3. Loading model definitions from the global registry
	/// 4. Building reverse relations between models
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_apps::Apps;
	///
	/// let apps = Apps::new(vec!["myapp".to_string()]);
	/// apps.populate().expect("Failed to populate apps");
	/// ```
	pub fn populate(&self) -> AppResult<()> {
		// Mark as apps_ready
		*self
			.apps_ready
			.lock()
			.unwrap_or_else(PoisonError::into_inner) = true;

		// 1. Import and instantiate AppConfig for each installed app
		// Detect duplicate entries in the installed_apps list itself
		{
			let mut seen = std::collections::HashSet::new();
			for app_name in &self.installed_apps {
				if !seen.insert(app_name) {
					return Err(AppError::DuplicateLabel(app_name.clone()));
				}
			}
		}

		for app_name in &self.installed_apps {
			let app_config = AppConfig::new(app_name.clone(), app_name.clone());

			// Skip apps already registered via register() to avoid overwriting
			let mut configs = self
				.app_configs
				.lock()
				.unwrap_or_else(PoisonError::into_inner);
			if configs.contains_key(&app_config.label) {
				continue;
			}
			configs.insert(app_config.label.clone(), app_config.clone());
			drop(configs);

			self.app_names
				.lock()
				.unwrap_or_else(PoisonError::into_inner)
				.insert(app_name.clone(), app_config.label.clone());
		}

		// 2. Call ready() method on each AppConfig and send signals
		let configs = self
			.app_configs
			.lock()
			.unwrap_or_else(PoisonError::into_inner);
		for app_config in configs.values() {
			// Call the ready hook
			app_config.ready().map_err(|e| {
				AppError::ConfigError(format!(
					"Ready hook failed for app '{}': {}",
					app_config.label, e
				))
			})?;

			// Send the app_ready signal
			signals::app_ready().send(app_config);
		}
		drop(configs); // Release lock early

		// 3. Load model definitions from global ModelRegistry
		// The models are already registered via #[derive(Model)] macro
		// which automatically registers them at construction time

		// 4. Build reverse relations between models
		if !*self
			.models_ready
			.lock()
			.unwrap_or_else(PoisonError::into_inner)
		{
			crate::discovery::build_reverse_relations()?;
			// Finalize reverse relations to make them immutable
			crate::registry::finalize_reverse_relations();
		}

		// Mark as models_ready
		*self
			.models_ready
			.lock()
			.unwrap_or_else(PoisonError::into_inner) = true;
		*self.ready.lock().unwrap_or_else(PoisonError::into_inner) = true;

		Ok(())
	}

	/// Clear all cached data (for testing)
	pub fn clear_cache(&self) {
		self.app_configs
			.lock()
			.unwrap_or_else(PoisonError::into_inner)
			.clear();
		self.app_names
			.lock()
			.unwrap_or_else(PoisonError::into_inner)
			.clear();
		*self.ready.lock().unwrap_or_else(PoisonError::into_inner) = false;
		*self
			.apps_ready
			.lock()
			.unwrap_or_else(PoisonError::into_inner) = false;
		*self
			.models_ready
			.lock()
			.unwrap_or_else(PoisonError::into_inner) = false;
	}
}

// DI integration (feature-gated)
#[cfg(feature = "di")]
mod di_integration {
	use super::*;
	use reinhardt_di::{DiError, DiResult, Injectable, InjectionContext};

	#[async_trait::async_trait]
	impl Injectable for Apps {
		async fn inject(ctx: &InjectionContext) -> DiResult<Self> {
			// Get from singleton scope
			if let Some(apps) = ctx.get_singleton::<Apps>() {
				return Ok((*apps).clone());
			}

			Err(DiError::NotFound(std::any::type_name::<Apps>().to_string()))
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;
	use serial_test::serial;

	#[rstest]
	fn test_app_config_creation() {
		// Arrange & Act
		let config = AppConfig::new("myapp", "myapp")
			.with_verbose_name("My Application")
			.with_default_auto_field("BigAutoField");

		// Assert
		assert_eq!(config.name, "myapp");
		assert_eq!(config.label, "myapp");
		assert_eq!(config.verbose_name, Some("My Application".to_string()));
		assert_eq!(config.default_auto_field, Some("BigAutoField".to_string()));
	}

	#[rstest]
	fn test_app_config_validation() {
		// Arrange
		let valid = AppConfig::new("myapp", "myapp");
		let invalid = AppConfig::new("myapp", "my-app");
		let empty = AppConfig::new("myapp", "");

		// Act & Assert
		assert!(valid.validate_label().is_ok());
		assert!(invalid.validate_label().is_err());
		assert!(empty.validate_label().is_err());
	}

	#[rstest]
	fn test_apps_registry() {
		// Arrange
		let apps = Apps::new(vec!["myapp".to_string(), "anotherapp".to_string()]);

		// Act & Assert
		assert!(apps.is_installed("myapp"));
		assert!(apps.is_installed("anotherapp"));
		assert!(!apps.is_installed("notinstalled"));
	}

	#[rstest]
	fn test_register_app() {
		// Arrange
		let apps = Apps::new(vec![]);
		let config = AppConfig::new("myapp", "myapp");

		// Act & Assert
		assert!(apps.register(config).is_ok());
		assert!(apps.get_app_config("myapp").is_ok());
	}

	#[rstest]
	fn test_duplicate_registration() {
		// Arrange
		let apps = Apps::new(vec![]);
		let config1 = AppConfig::new("myapp", "myapp");
		let config2 = AppConfig::new("myapp", "myapp");
		apps.register(config1).unwrap();

		// Act
		let result = apps.register(config2);

		// Assert
		assert!(result.is_err());
	}

	#[rstest]
	fn test_get_app_configs() {
		// Arrange
		let apps = Apps::new(vec![]);
		apps.register(AppConfig::new("app1", "app1")).unwrap();
		apps.register(AppConfig::new("app2", "app2")).unwrap();

		// Act
		let configs = apps.get_app_configs();

		// Assert
		assert_eq!(configs.len(), 2);
	}

	#[rstest]
	#[serial(apps_registry)]
	fn test_populate() {
		// Arrange - Reset global state before test
		crate::registry::reset_global_registry();

		// Arrange
		let apps = Apps::new(vec![]);
		assert!(!apps.is_ready());

		// Act
		apps.populate().unwrap();

		// Assert
		assert!(apps.is_ready());
		assert!(apps.is_apps_ready());
		assert!(apps.is_models_ready());
	}

	#[rstest]
	#[serial(apps_registry)]
	fn test_populate_with_installed_apps() {
		// Arrange - Reset global state before test
		crate::registry::reset_global_registry();

		// Arrange
		let apps = Apps::new(vec!["myapp".to_string(), "anotherapp".to_string()]);
		assert!(!apps.is_ready());

		// Act
		let result = apps.populate();

		// Assert
		assert!(result.is_ok());
		assert!(apps.is_ready());
		assert!(apps.is_apps_ready());
		assert!(apps.is_models_ready());
		assert!(apps.get_app_config("myapp").is_ok());
		assert!(apps.get_app_config("anotherapp").is_ok());
		let myapp_config = apps.get_app_config("myapp").unwrap();
		assert_eq!(myapp_config.label, "myapp");
	}

	// ==========================================================================
	// Path Validation Tests
	// ==========================================================================

	#[rstest]
	#[case("apps/myapp")]
	#[case("myapp")]
	#[case("src/apps/myapp")]
	#[case("my_app")]
	#[case("my-app")]
	fn test_with_path_accepts_valid_relative_paths(#[case] path: &str) {
		// Act
		let result = AppConfig::new("myapp", "myapp").with_path(path);

		// Assert
		assert!(result.is_ok(), "expected valid path: {path}");
		assert_eq!(result.unwrap().path, Some(path.to_string()));
	}

	#[rstest]
	fn test_with_path_rejects_empty() {
		// Act
		let result = AppConfig::new("myapp", "myapp").with_path("");

		// Assert
		let err = result.unwrap_err();
		assert!(err.to_string().contains("cannot be empty"));
	}

	#[rstest]
	#[case("../etc/passwd")]
	#[case("apps/../../../etc/shadow")]
	#[case("apps/..")]
	fn test_with_path_rejects_traversal(#[case] path: &str) {
		// Act
		let result = AppConfig::new("myapp", "myapp").with_path(path);

		// Assert
		let err = result.unwrap_err();
		assert!(
			err.to_string().contains("path traversal"),
			"expected traversal error for '{path}', got: {err}"
		);
	}

	#[rstest]
	#[case("/etc/passwd")]
	#[case("/absolute/path")]
	#[case("\\windows\\path")]
	#[case("C:\\Windows\\System32")]
	#[case("D:/data")]
	fn test_with_path_rejects_absolute(#[case] path: &str) {
		// Act
		let result = AppConfig::new("myapp", "myapp").with_path(path);

		// Assert
		let err = result.unwrap_err();
		assert!(
			err.to_string().contains("relative, not absolute"),
			"expected absolute path error for '{path}', got: {err}"
		);
	}

	#[rstest]
	fn test_with_path_rejects_null_bytes() {
		// Act
		let result = AppConfig::new("myapp", "myapp").with_path("apps/my\0app");

		// Assert
		let err = result.unwrap_err();
		assert!(err.to_string().contains("null bytes"));
	}

	#[rstest]
	#[case("apps/my\napp")]
	#[case("apps/my\rapp")]
	fn test_with_path_rejects_control_chars(#[case] path: &str) {
		// Act
		let result = AppConfig::new("myapp", "myapp").with_path(path);

		// Assert
		let err = result.unwrap_err();
		assert!(
			err.to_string().contains("control characters"),
			"expected control char error for path, got: {err}"
		);
	}
}

// ============================================================================
// Type-safe application registry (compile-time checked)
// ============================================================================

/// Trait for applications that can be accessed at compile time
///
/// Implement this trait for each application in your project.
/// The compiler will ensure that only valid application labels can be used.
///
/// # Example
///
/// ```rust
/// use reinhardt_apps::apps::AppLabel;
///
/// pub struct AuthApp;
/// impl AppLabel for AuthApp {
///     const LABEL: &'static str = "auth";
/// }
/// ```
pub trait AppLabel {
	/// The unique label for this application
	const LABEL: &'static str;
}

impl Apps {
	/// Type-safe get_app_config method
	///
	/// This method ensures at compile time that only valid application types
	/// can be used.
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_apps::apps::{Apps, AppLabel};
	///
	/// pub struct AuthApp;
	/// impl AppLabel for AuthApp {
	///     const LABEL: &'static str = "auth";
	/// }
	///
	/// let apps = Apps::new(vec!["auth".to_string()]);
	/// // This will compile because AuthApp implements AppLabel
	/// let result = apps.get_app_config_typed::<AuthApp>();
	/// ```
	pub fn get_app_config_typed<A: AppLabel>(&self) -> AppResult<AppConfig> {
		self.get_app_config(A::LABEL)
	}

	/// Type-safe check if an application is installed
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_apps::apps::{Apps, AppLabel};
	///
	/// pub struct AuthApp;
	/// impl AppLabel for AuthApp {
	///     const LABEL: &'static str = "auth";
	/// }
	///
	/// let apps = Apps::new(vec!["auth".to_string()]);
	/// assert!(apps.is_installed_typed::<AuthApp>());
	/// ```
	pub fn is_installed_typed<A: AppLabel>(&self) -> bool {
		self.is_installed(A::LABEL)
	}
}

#[cfg(test)]
mod typed_tests {
	use super::*;

	// Test application types
	struct AuthApp;
	impl AppLabel for AuthApp {
		const LABEL: &'static str = "auth";
	}

	struct ContentTypesApp;
	impl AppLabel for ContentTypesApp {
		const LABEL: &'static str = "contenttypes";
	}

	struct SessionsApp;
	impl AppLabel for SessionsApp {
		const LABEL: &'static str = "sessions";
	}

	#[test]
	fn test_typed_is_installed() {
		let apps = Apps::new(vec!["auth".to_string(), "contenttypes".to_string()]);

		assert!(apps.is_installed_typed::<AuthApp>());
		assert!(apps.is_installed_typed::<ContentTypesApp>());
		assert!(!apps.is_installed_typed::<SessionsApp>());
	}

	#[test]
	fn test_typed_get_app_config() {
		let apps = Apps::new(vec![]);
		let config = AppConfig::new("auth", "auth");
		apps.register(config).unwrap();

		let retrieved = apps.get_app_config_typed::<AuthApp>();
		assert!(retrieved.is_ok());
		assert_eq!(retrieved.unwrap().label, "auth");
	}

	#[test]
	fn test_typed_get_app_config_not_found() {
		let apps = Apps::new(vec![]);

		let result = apps.get_app_config_typed::<SessionsApp>();
		assert!(result.is_err());

		if let Err(AppError::NotFound(label)) = result {
			assert_eq!(label, "sessions");
		}
	}

	#[test]
	fn test_apps_typed_and_regular_mixed() {
		let apps = Apps::new(vec!["auth".to_string()]);
		let config = AppConfig::new("auth", "auth");
		apps.register(config).unwrap();

		// Can use both typed and regular methods
		assert!(apps.is_installed_typed::<AuthApp>());
		assert!(apps.is_installed("auth"));

		let typed = apps.get_app_config_typed::<AuthApp>().unwrap();
		let regular = apps.get_app_config("auth").unwrap();

		assert_eq!(typed.label, regular.label);
	}
}

// ============================================================================
// Global Registry (inventory-based)
// ============================================================================

/// Base trait for custom management commands
///
/// Applications can implement this trait to provide custom commands
/// that will be automatically discovered by the manage.py CLI.
pub trait BaseCommand: Send + Sync {
	/// Command name (e.g., "createsuperuser")
	fn name(&self) -> &str;

	/// Command help text
	fn help(&self) -> &str;

	/// Execute the command
	fn execute(&mut self, args: Vec<String>) -> Result<(), Box<dyn std::error::Error>>;
}

/// Static files configuration from an app
///
/// Applications can register their static files directories using this struct.
/// Registered configurations will be automatically discovered by collectstatic.
/// Uses static string references for compile-time registration.
pub struct AppStaticFilesConfig {
	pub app_label: &'static str,
	pub static_dir: &'static str,
	pub url_prefix: &'static str,
}

inventory::collect!(AppStaticFilesConfig);

/// Locale configuration from an app
///
/// Applications can register their locale directories using this struct.
/// Registered configurations will be automatically discovered by makemessages.
/// Uses static string references for compile-time registration.
pub struct AppLocaleConfig {
	pub app_label: &'static str,
	pub locale_dir: &'static str,
}

inventory::collect!(AppLocaleConfig);

/// Command configuration from an app
///
/// Applications can register their custom management commands using this struct.
/// Registered commands will be automatically discovered by the manage.py CLI.
/// Uses static string references for compile-time registration.
pub struct AppCommandConfig {
	pub app_label: &'static str,
	pub command_name: &'static str,
	pub command_fn: fn() -> Box<dyn BaseCommand>,
}

inventory::collect!(AppCommandConfig);

/// Media files configuration from an app
///
/// Applications can register their media files directories using this struct.
/// Registered configurations will be automatically discovered by collectmedia.
/// Uses static string references for compile-time registration.
pub struct AppMediaConfig {
	pub app_label: &'static str,
	pub media_dir: &'static str,
	pub url_prefix: &'static str,
}

inventory::collect!(AppMediaConfig);

// ============================================================================
// Registration Macros
// ============================================================================

/// Register static files for an application
///
/// # Example
///
/// ```rust,ignore
/// use reinhardt_apps::register_app_static_files;
/// use std::path::PathBuf;
///
/// register_app_static_files!(
///     "myapp",
///     PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("static"),
///     "/static/myapp/"
/// );
/// ```
#[macro_export]
macro_rules! register_app_static_files {
	($app_label:expr, $static_dir:expr, $url_prefix:expr) => {
		$crate::inventory::submit! {
			$crate::AppStaticFilesConfig {
				app_label: $app_label,
				static_dir: $static_dir,
				url_prefix: $url_prefix,
			}
		}
	};
}

/// Register locale directory for an application
///
/// # Example
///
/// ```rust,ignore
/// use reinhardt_apps::register_app_locale;
/// use std::path::PathBuf;
///
/// register_app_locale!(
///     "myapp",
///     PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("locale")
/// );
/// ```
#[macro_export]
macro_rules! register_app_locale {
	($app_label:expr, $locale_dir:expr) => {
		$crate::inventory::submit! {
			$crate::AppLocaleConfig {
				app_label: $app_label,
				locale_dir: $locale_dir,
			}
		}
	};
}

/// Register a custom management command
///
/// # Example
///
/// ```rust,ignore
/// use reinhardt_apps::{register_app_command, BaseCommand};
///
/// struct MyCommand;
/// impl BaseCommand for MyCommand {
///     fn name(&self) -> &str { "mycommand" }
///     fn help(&self) -> &str { "My custom command" }
///     fn execute(&mut self, args: Vec<String>) -> Result<(), Box<dyn std::error::Error>> {
///         Ok(())
///     }
/// }
///
/// register_app_command!(
///     "myapp",
///     "mycommand",
///     || Box::new(MyCommand)
/// );
/// ```
#[macro_export]
macro_rules! register_app_command {
	($app_label:expr, $command_name:expr, $command_fn:expr) => {
		$crate::inventory::submit! {
			$crate::AppCommandConfig {
				app_label: $app_label,
				command_name: $command_name,
				command_fn: $command_fn,
			}
		}
	};
}

/// Register media files directory for an application
///
/// # Example
///
/// ```rust,ignore
/// use reinhardt_apps::register_app_media;
/// use std::path::PathBuf;
///
/// register_app_media!(
///     "myapp",
///     PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("media"),
///     "/media/myapp/"
/// );
/// ```
#[macro_export]
macro_rules! register_app_media {
	($app_label:expr, $media_dir:expr, $url_prefix:expr) => {
		$crate::inventory::submit! {
			$crate::AppMediaConfig {
				app_label: $app_label,
				media_dir: $media_dir,
				url_prefix: $url_prefix,
			}
		}
	};
}

// ============================================================================
// Getter Functions
// ============================================================================

/// Get all registered static files configurations
///
/// Returns all static files configurations that have been registered via
/// `register_app_static_files!` macro.
///
/// # Example
///
/// ```rust
/// use reinhardt_apps::get_app_static_files;
///
/// let configs = get_app_static_files();
/// for config in configs {
///     println!("App: {}, Dir: {}", config.app_label, config.static_dir);
/// }
/// ```
pub fn get_app_static_files() -> Vec<&'static AppStaticFilesConfig> {
	inventory::iter::<AppStaticFilesConfig>().collect()
}

/// Get all registered locale configurations
///
/// Returns all locale configurations that have been registered via
/// `register_app_locale!` macro.
///
/// # Example
///
/// ```rust
/// use reinhardt_apps::get_app_locales;
///
/// let configs = get_app_locales();
/// for config in configs {
///     println!("App: {}, Dir: {}", config.app_label, config.locale_dir);
/// }
/// ```
pub fn get_app_locales() -> Vec<&'static AppLocaleConfig> {
	inventory::iter::<AppLocaleConfig>().collect()
}

/// Get all registered command configurations
///
/// Returns all command configurations that have been registered via
/// `register_app_command!` macro.
///
/// # Example
///
/// ```rust
/// use reinhardt_apps::get_app_commands;
///
/// let configs = get_app_commands();
/// for config in configs {
///     println!("App: {}, Command: {}", config.app_label, config.command_name);
/// }
/// ```
pub fn get_app_commands() -> Vec<&'static AppCommandConfig> {
	inventory::iter::<AppCommandConfig>().collect()
}

/// Get all registered media configurations
///
/// Returns all media configurations that have been registered via
/// `register_app_media!` macro.
///
/// # Example
///
/// ```rust
/// use reinhardt_apps::get_app_media;
///
/// let configs = get_app_media();
/// for config in configs {
///     println!("App: {}, Dir: {}", config.app_label, config.media_dir);
/// }
/// ```
pub fn get_app_media() -> Vec<&'static AppMediaConfig> {
	inventory::iter::<AppMediaConfig>().collect()
}
