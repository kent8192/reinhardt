//! Application builder module
//!
//! Provides a builder pattern for configuring and constructing Reinhardt applications.
//! Inspired by Django's application configuration system.

use crate::apps::{AppConfig, AppError, Apps};
use std::collections::HashMap;
use std::sync::Arc;
use thiserror::Error;

/// Errors that can occur when building an application
#[derive(Debug, Error)]
pub enum BuildError {
	#[error("Application error: {0}")]
	App(#[from] AppError),

	#[error("Invalid configuration: {0}")]
	InvalidConfig(String),

	#[error("Missing required configuration: {0}")]
	MissingConfig(String),

	#[error("Route configuration error: {0}")]
	RouteError(String),

	#[error("Database configuration error: {0}")]
	DatabaseError(String),
}

pub type BuildResult<T> = Result<T, BuildError>;

/// Route definition for the application
/// Lightweight wrapper around path patterns
#[derive(Clone)]
pub struct RouteConfig {
	pub path: String,
	pub handler_name: String,
	pub name: Option<String>,
	pub namespace: Option<String>,
}

impl RouteConfig {
	/// Create a new route configuration
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_apps::builder::RouteConfig;
	///
	/// let route = RouteConfig::new("/users/", "UserListHandler");
	/// assert_eq!(route.path, "/users/");
	/// assert_eq!(route.handler_name, "UserListHandler");
	/// ```
	pub fn new(path: impl Into<String>, handler_name: impl Into<String>) -> Self {
		Self {
			path: path.into(),
			handler_name: handler_name.into(),
			name: None,
			namespace: None,
		}
	}

	/// Set the route name
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_apps::builder::RouteConfig;
	///
	/// let route = RouteConfig::new("/users/", "UserListHandler")
	///     .with_name("user-list");
	/// assert_eq!(route.name, Some("user-list".to_string()));
	/// ```
	pub fn with_name(mut self, name: impl Into<String>) -> Self {
		self.name = Some(name.into());
		self
	}

	/// Set the route namespace
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_apps::builder::RouteConfig;
	///
	/// let route = RouteConfig::new("/users/", "UserListHandler")
	///     .with_namespace("api");
	/// assert_eq!(route.namespace, Some("api".to_string()));
	/// ```
	pub fn with_namespace(mut self, namespace: impl Into<String>) -> Self {
		self.namespace = Some(namespace.into());
		self
	}

	/// Get the full name including namespace
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_apps::builder::RouteConfig;
	///
	/// let route = RouteConfig::new("/users/", "UserListHandler")
	///     .with_namespace("api")
	///     .with_name("list");
	/// assert_eq!(route.full_name(), Some("api:list".to_string()));
	///
	/// let route = RouteConfig::new("/users/", "UserListHandler")
	///     .with_name("list");
	/// assert_eq!(route.full_name(), Some("list".to_string()));
	///
	/// let route = RouteConfig::new("/users/", "UserListHandler");
	/// assert_eq!(route.full_name(), None);
	/// ```
	pub fn full_name(&self) -> Option<String> {
		match (&self.namespace, &self.name) {
			(Some(ns), Some(name)) => Some(format!("{}:{}", ns, name)),
			(None, Some(name)) => Some(name.clone()),
			_ => None,
		}
	}
}

/// Database configuration for the application
#[derive(Clone, Debug)]
pub struct ApplicationDatabaseConfig {
	pub url: String,
	pub pool_size: Option<u32>,
	pub max_overflow: Option<u32>,
	pub timeout: Option<u64>,
}

impl ApplicationDatabaseConfig {
	/// Create a new database configuration
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_apps::builder::ApplicationDatabaseConfig;
	///
	/// let db_config = ApplicationDatabaseConfig::new("postgresql://localhost/mydb");
	/// assert_eq!(db_config.url, "postgresql://localhost/mydb");
	/// ```
	pub fn new(url: impl Into<String>) -> Self {
		Self {
			url: url.into(),
			pool_size: None,
			max_overflow: None,
			timeout: None,
		}
	}

	/// Set the connection pool size
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_apps::builder::ApplicationDatabaseConfig;
	///
	/// let db_config = ApplicationDatabaseConfig::new("postgresql://localhost/mydb")
	///     .with_pool_size(10);
	/// assert_eq!(db_config.pool_size, Some(10));
	/// ```
	pub fn with_pool_size(mut self, size: u32) -> Self {
		self.pool_size = Some(size);
		self
	}

	/// Set the maximum overflow connections
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_apps::builder::ApplicationDatabaseConfig;
	///
	/// let db_config = ApplicationDatabaseConfig::new("postgresql://localhost/mydb")
	///     .with_max_overflow(5);
	/// assert_eq!(db_config.max_overflow, Some(5));
	/// ```
	pub fn with_max_overflow(mut self, overflow: u32) -> Self {
		self.max_overflow = Some(overflow);
		self
	}

	/// Set the connection timeout
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_apps::builder::ApplicationDatabaseConfig;
	///
	/// let db_config = ApplicationDatabaseConfig::new("postgresql://localhost/mydb")
	///     .with_timeout(30);
	/// assert_eq!(db_config.timeout, Some(30));
	/// ```
	pub fn with_timeout(mut self, timeout: u64) -> Self {
		self.timeout = Some(timeout);
		self
	}
}

/// Builder for constructing Reinhardt applications
/// Inspired by Django's application configuration system
pub struct ApplicationBuilder {
	apps: Vec<AppConfig>,
	middleware: Vec<String>,
	url_patterns: Vec<RouteConfig>,
	database_config: Option<ApplicationDatabaseConfig>,
	settings: HashMap<String, String>,
}

impl ApplicationBuilder {
	/// Create a new application builder
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_apps::builder::ApplicationBuilder;
	///
	/// let builder = ApplicationBuilder::new();
	/// let app = builder.build().unwrap();
	/// assert_eq!(app.apps().len(), 0);
	/// ```
	pub fn new() -> Self {
		Self {
			apps: Vec::new(),
			middleware: Vec::new(),
			url_patterns: Vec::new(),
			database_config: None,
			settings: HashMap::new(),
		}
	}

	/// Add an application configuration
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_apps::builder::ApplicationBuilder;
	/// use reinhardt_apps::AppConfig;
	///
	/// let app_config = AppConfig::new("myapp", "myapp");
	/// let builder = ApplicationBuilder::new()
	///     .add_app(app_config);
	/// let app = builder.build().unwrap();
	/// assert_eq!(app.apps().len(), 1);
	/// ```
	pub fn add_app(mut self, app: AppConfig) -> Self {
		self.apps.push(app);
		self
	}

	/// Add multiple application configurations
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_apps::builder::ApplicationBuilder;
	/// use reinhardt_apps::AppConfig;
	///
	/// let apps = vec![
	///     AppConfig::new("app1", "app1"),
	///     AppConfig::new("app2", "app2"),
	/// ];
	/// let builder = ApplicationBuilder::new()
	///     .add_apps(apps);
	/// let app = builder.build().unwrap();
	/// assert_eq!(app.apps().len(), 2);
	/// ```
	pub fn add_apps(mut self, apps: Vec<AppConfig>) -> Self {
		self.apps.extend(apps);
		self
	}

	/// Add a middleware
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_apps::builder::ApplicationBuilder;
	///
	/// let builder = ApplicationBuilder::new()
	///     .add_middleware("CorsMiddleware");
	/// let app = builder.build().unwrap();
	/// assert_eq!(app.middleware().len(), 1);
	/// ```
	pub fn add_middleware(mut self, middleware: impl Into<String>) -> Self {
		self.middleware.push(middleware.into());
		self
	}

	/// Add multiple middleware
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_apps::builder::ApplicationBuilder;
	///
	/// let middleware = vec!["CorsMiddleware", "AuthMiddleware"];
	/// let builder = ApplicationBuilder::new()
	///     .add_middlewares(middleware);
	/// let app = builder.build().unwrap();
	/// assert_eq!(app.middleware().len(), 2);
	/// ```
	pub fn add_middlewares<S: Into<String>>(mut self, middleware: Vec<S>) -> Self {
		self.middleware
			.extend(middleware.into_iter().map(|m| m.into()));
		self
	}

	/// Add a URL pattern
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_apps::builder::{ApplicationBuilder, RouteConfig};
	///
	/// let route = RouteConfig::new("/users/", "UserListHandler");
	/// let builder = ApplicationBuilder::new()
	///     .add_url_pattern(route);
	/// let app = builder.build().unwrap();
	/// assert_eq!(app.url_patterns().len(), 1);
	/// ```
	pub fn add_url_pattern(mut self, pattern: RouteConfig) -> Self {
		self.url_patterns.push(pattern);
		self
	}

	/// Add multiple URL patterns
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_apps::builder::{ApplicationBuilder, RouteConfig};
	///
	/// let patterns = vec![
	///     RouteConfig::new("/users/", "UserListHandler"),
	///     RouteConfig::new("/posts/", "PostListHandler"),
	/// ];
	/// let builder = ApplicationBuilder::new()
	///     .add_url_patterns(patterns);
	/// let app = builder.build().unwrap();
	/// assert_eq!(app.url_patterns().len(), 2);
	/// ```
	pub fn add_url_patterns(mut self, patterns: Vec<RouteConfig>) -> Self {
		self.url_patterns.extend(patterns);
		self
	}

	/// Set the database configuration
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_apps::builder::{ApplicationBuilder, ApplicationDatabaseConfig};
	///
	/// let db_config = ApplicationDatabaseConfig::new("postgresql://localhost/mydb");
	/// let builder = ApplicationBuilder::new()
	///     .database(db_config);
	/// let app = builder.build().unwrap();
	/// assert!(app.database_config().is_some());
	/// ```
	pub fn database(mut self, config: ApplicationDatabaseConfig) -> Self {
		self.database_config = Some(config);
		self
	}

	/// Add a custom setting
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_apps::builder::ApplicationBuilder;
	///
	/// let builder = ApplicationBuilder::new()
	///     .add_setting("DEBUG", "true");
	/// let app = builder.build().unwrap();
	/// assert_eq!(app.settings().get("DEBUG"), Some(&"true".to_string()));
	/// ```
	pub fn add_setting(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
		self.settings.insert(key.into(), value.into());
		self
	}

	/// Add multiple custom settings
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_apps::builder::ApplicationBuilder;
	/// use std::collections::HashMap;
	///
	/// let mut settings = HashMap::new();
	/// settings.insert("DEBUG".to_string(), "true".to_string());
	/// settings.insert("SECRET_KEY".to_string(), "secret".to_string());
	///
	/// let builder = ApplicationBuilder::new()
	///     .add_settings(settings);
	/// let app = builder.build().unwrap();
	/// assert_eq!(app.settings().get("DEBUG"), Some(&"true".to_string()));
	/// ```
	pub fn add_settings(mut self, settings: HashMap<String, String>) -> Self {
		self.settings.extend(settings);
		self
	}

	/// Validate the configuration
	fn validate(&self) -> BuildResult<()> {
		// Validate all app configurations
		for app in &self.apps {
			app.validate_label()?;
		}

		// Check for duplicate app labels
		let mut labels = std::collections::HashSet::new();
		for app in &self.apps {
			if !labels.insert(&app.label) {
				return Err(BuildError::InvalidConfig(format!(
					"Duplicate app label: {}",
					app.label
				)));
			}
		}

		// Check for duplicate route names
		let mut route_names = std::collections::HashSet::new();
		for pattern in &self.url_patterns {
			if let Some(full_name) = pattern.full_name()
				&& !route_names.insert(full_name.clone())
			{
				return Err(BuildError::RouteError(format!(
					"Duplicate route name: {}",
					full_name
				)));
			}
		}

		Ok(())
	}

	/// Build the application
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_apps::builder::ApplicationBuilder;
	/// use reinhardt_apps::AppConfig;
	///
	/// let app_config = AppConfig::new("myapp", "myapp");
	/// let builder = ApplicationBuilder::new()
	///     .add_app(app_config)
	///     .add_middleware("CorsMiddleware");
	///
	/// let app = builder.build().unwrap();
	/// assert_eq!(app.apps().len(), 1);
	/// assert_eq!(app.middleware().len(), 1);
	/// ```
	pub fn build(self) -> BuildResult<Application> {
		// Validate configuration
		self.validate()?;

		// Create the Apps registry
		let installed_apps: Vec<String> = self.apps.iter().map(|app| app.name.clone()).collect();
		let apps_registry = Apps::new(installed_apps);

		// Register all app configurations
		for app in &self.apps {
			apps_registry.register(app.clone())?;
		}

		// Populate the registry
		apps_registry.populate()?;

		Ok(Application {
			apps: self.apps,
			middleware: self.middleware,
			url_patterns: self.url_patterns,
			database_config: self.database_config,
			settings: self.settings,
			apps_registry: Arc::new(apps_registry),
		})
	}

	/// Build the application and register it with the DI system
	///
	/// This method builds the application and registers both the `Application`
	/// and `Apps` instances in the provided `SingletonScope`.
	///
	/// # Examples
	///
	/// ```ignore
	/// use reinhardt_apps::builder::ApplicationBuilder;
	/// use reinhardt_apps::AppConfig;
	/// use reinhardt_di::SingletonScope;
	/// use std::sync::Arc;
	///
	/// let singleton = Arc::new(SingletonScope::new());
	/// let app_config = AppConfig::new("myapp", "myapp");
	/// let app = ApplicationBuilder::new()
	///     .add_app(app_config)
	///     .build_with_di(singleton.clone())
	///     .unwrap();
	/// ```
	#[cfg(feature = "di")]
	pub fn build_with_di(
		self,
		singleton_scope: Arc<reinhardt_di::SingletonScope>,
	) -> BuildResult<Arc<Application>> {
		let app = self.build()?;
		let app = Arc::new(app);

		// Register Application in SingletonScope
		singleton_scope.set(app.clone());

		// Register Apps in SingletonScope
		singleton_scope.set(app.apps_registry.clone());

		Ok(app)
	}
}

impl Default for ApplicationBuilder {
	fn default() -> Self {
		Self::new()
	}
}

/// The built application
pub struct Application {
	apps: Vec<AppConfig>,
	middleware: Vec<String>,
	url_patterns: Vec<RouteConfig>,
	database_config: Option<ApplicationDatabaseConfig>,
	settings: HashMap<String, String>,
	apps_registry: Arc<Apps>,
}

impl Application {
	/// Get the registered applications
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_apps::builder::ApplicationBuilder;
	/// use reinhardt_apps::AppConfig;
	///
	/// let app_config = AppConfig::new("myapp", "myapp");
	/// let builder = ApplicationBuilder::new()
	///     .add_app(app_config);
	/// let app = builder.build().unwrap();
	///
	/// assert_eq!(app.apps().len(), 1);
	/// assert_eq!(app.apps()[0].label, "myapp");
	/// ```
	pub fn apps(&self) -> &[AppConfig] {
		&self.apps
	}

	/// Get the middleware stack
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_apps::builder::ApplicationBuilder;
	///
	/// let builder = ApplicationBuilder::new()
	///     .add_middleware("CorsMiddleware")
	///     .add_middleware("AuthMiddleware");
	/// let app = builder.build().unwrap();
	///
	/// assert_eq!(app.middleware().len(), 2);
	/// assert_eq!(app.middleware()[0], "CorsMiddleware");
	/// ```
	pub fn middleware(&self) -> &[String] {
		&self.middleware
	}

	/// Get the URL patterns
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_apps::builder::{ApplicationBuilder, RouteConfig};
	///
	/// let route = RouteConfig::new("/users/", "UserListHandler");
	/// let builder = ApplicationBuilder::new()
	///     .add_url_pattern(route);
	/// let app = builder.build().unwrap();
	///
	/// assert_eq!(app.url_patterns().len(), 1);
	/// assert_eq!(app.url_patterns()[0].path, "/users/");
	/// ```
	pub fn url_patterns(&self) -> &[RouteConfig] {
		&self.url_patterns
	}

	/// Get the database configuration
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_apps::builder::{ApplicationBuilder, ApplicationDatabaseConfig};
	///
	/// let db_config = ApplicationDatabaseConfig::new("postgresql://localhost/mydb");
	/// let builder = ApplicationBuilder::new()
	///     .database(db_config);
	/// let app = builder.build().unwrap();
	///
	/// assert!(app.database_config().is_some());
	/// assert_eq!(app.database_config().unwrap().url, "postgresql://localhost/mydb");
	/// ```
	pub fn database_config(&self) -> Option<&ApplicationDatabaseConfig> {
		self.database_config.as_ref()
	}

	/// Get the custom settings
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_apps::builder::ApplicationBuilder;
	///
	/// let builder = ApplicationBuilder::new()
	///     .add_setting("DEBUG", "true");
	/// let app = builder.build().unwrap();
	///
	/// assert_eq!(app.settings().get("DEBUG"), Some(&"true".to_string()));
	/// ```
	pub fn settings(&self) -> &HashMap<String, String> {
		&self.settings
	}

	/// Get the apps registry
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_apps::builder::ApplicationBuilder;
	/// use reinhardt_apps::AppConfig;
	///
	/// let app_config = AppConfig::new("myapp", "myapp");
	/// let builder = ApplicationBuilder::new()
	///     .add_app(app_config);
	/// let app = builder.build().unwrap();
	///
	/// assert!(app.apps_registry().is_ready());
	/// assert!(app.apps_registry().is_installed("myapp"));
	/// ```
	pub fn apps_registry(&self) -> &Apps {
		&self.apps_registry
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use serial_test::serial;

	#[test]
	fn test_route_config_creation() {
		let route = RouteConfig::new("/users/", "UserListHandler")
			.with_name("user-list")
			.with_namespace("api");

		assert_eq!(route.path, "/users/");
		assert_eq!(route.handler_name, "UserListHandler");
		assert_eq!(route.name, Some("user-list".to_string()));
		assert_eq!(route.namespace, Some("api".to_string()));
	}

	#[test]
	fn test_route_config_full_name() {
		let route = RouteConfig::new("/users/", "UserListHandler")
			.with_namespace("api")
			.with_name("list");
		assert_eq!(route.full_name(), Some("api:list".to_string()));

		let route = RouteConfig::new("/users/", "UserListHandler").with_name("list");
		assert_eq!(route.full_name(), Some("list".to_string()));

		let route = RouteConfig::new("/users/", "UserListHandler");
		assert_eq!(route.full_name(), None);
	}

	#[test]
	fn test_database_config_creation() {
		let db_config = ApplicationDatabaseConfig::new("postgresql://localhost/mydb")
			.with_pool_size(10)
			.with_max_overflow(5)
			.with_timeout(30);

		assert_eq!(db_config.url, "postgresql://localhost/mydb");
		assert_eq!(db_config.pool_size, Some(10));
		assert_eq!(db_config.max_overflow, Some(5));
		assert_eq!(db_config.timeout, Some(30));
	}

	#[test]
	#[serial(apps_registry)]
	fn test_application_builder_basic() {
		// Arrange - Reset global state before test
		crate::registry::reset_global_registry();

		let app = ApplicationBuilder::new().build().unwrap();

		assert_eq!(app.apps().len(), 0);
		assert_eq!(app.middleware().len(), 0);
		assert_eq!(app.url_patterns().len(), 0);
		assert!(app.database_config().is_none());
	}

	#[test]
	#[serial(apps_registry)]
	fn test_application_builder_with_apps() {
		// Arrange - Reset global state before test
		crate::registry::reset_global_registry();

		let app_config = AppConfig::new("myapp", "myapp");
		let app = ApplicationBuilder::new()
			.add_app(app_config)
			.build()
			.unwrap();

		assert_eq!(app.apps().len(), 1);
		assert_eq!(app.apps()[0].label, "myapp");
		assert!(app.apps_registry().is_installed("myapp"));
	}

	#[test]
	#[serial(apps_registry)]
	fn test_application_builder_with_multiple_apps() {
		// Arrange - Reset global state before test
		crate::registry::reset_global_registry();

		let apps = vec![
			AppConfig::new("app1", "app1"),
			AppConfig::new("app2", "app2"),
		];
		let app = ApplicationBuilder::new().add_apps(apps).build().unwrap();

		assert_eq!(app.apps().len(), 2);
		assert!(app.apps_registry().is_installed("app1"));
		assert!(app.apps_registry().is_installed("app2"));
	}

	#[test]
	#[serial(apps_registry)]
	fn test_application_builder_with_middleware() {
		// Arrange - Reset global state before test
		crate::registry::reset_global_registry();

		let app = ApplicationBuilder::new()
			.add_middleware("CorsMiddleware")
			.add_middleware("AuthMiddleware")
			.build()
			.unwrap();

		assert_eq!(app.middleware().len(), 2);
		assert_eq!(app.middleware()[0], "CorsMiddleware");
		assert_eq!(app.middleware()[1], "AuthMiddleware");
	}

	#[test]
	#[serial(apps_registry)]
	fn test_application_builder_with_middlewares() {
		// Arrange - Reset global state before test
		crate::registry::reset_global_registry();

		let middleware = vec!["CorsMiddleware", "AuthMiddleware"];
		let app = ApplicationBuilder::new()
			.add_middlewares(middleware)
			.build()
			.unwrap();

		assert_eq!(app.middleware().len(), 2);
	}

	#[test]
	#[serial(apps_registry)]
	fn test_application_builder_with_url_patterns() {
		// Arrange - Reset global state before test
		crate::registry::reset_global_registry();

		let route = RouteConfig::new("/users/", "UserListHandler");
		let app = ApplicationBuilder::new()
			.add_url_pattern(route)
			.build()
			.unwrap();

		assert_eq!(app.url_patterns().len(), 1);
		assert_eq!(app.url_patterns()[0].path, "/users/");
	}

	#[test]
	#[serial(apps_registry)]
	fn test_application_builder_with_database() {
		// Arrange - Reset global state before test
		crate::registry::reset_global_registry();

		let db_config = ApplicationDatabaseConfig::new("postgresql://localhost/mydb");
		let app = ApplicationBuilder::new()
			.database(db_config)
			.build()
			.unwrap();

		assert!(app.database_config().is_some());
		assert_eq!(
			app.database_config().unwrap().url,
			"postgresql://localhost/mydb"
		);
	}

	#[test]
	#[serial(apps_registry)]
	fn test_application_builder_with_settings() {
		// Arrange - Reset global state before test
		crate::registry::reset_global_registry();

		let app = ApplicationBuilder::new()
			.add_setting("DEBUG", "true")
			.add_setting("SECRET_KEY", "secret")
			.build()
			.unwrap();

		assert_eq!(app.settings().get("DEBUG"), Some(&"true".to_string()));
		assert_eq!(
			app.settings().get("SECRET_KEY"),
			Some(&"secret".to_string())
		);
	}

	#[test]
	#[serial(apps_registry)]
	fn test_application_builder_validation_duplicate_apps() {
		// Arrange - Reset global state before test
		crate::registry::reset_global_registry();

		let result = ApplicationBuilder::new()
			.add_app(AppConfig::new("myapp", "myapp"))
			.add_app(AppConfig::new("another", "myapp"))
			.build();

		assert!(result.is_err());
		match result {
			Err(BuildError::InvalidConfig(msg)) => {
				assert_eq!(msg, "Duplicate app label: myapp");
			}
			_ => panic!("Expected InvalidConfig error"),
		}
	}

	#[test]
	#[serial(apps_registry)]
	fn test_application_builder_validation_duplicate_routes() {
		// Arrange - Reset global state before test
		crate::registry::reset_global_registry();

		let result = ApplicationBuilder::new()
			.add_url_pattern(RouteConfig::new("/users/", "Handler1").with_name("users"))
			.add_url_pattern(RouteConfig::new("/posts/", "Handler2").with_name("users"))
			.build();

		assert!(result.is_err());
		match result {
			Err(BuildError::RouteError(msg)) => {
				assert_eq!(msg, "Duplicate route name: users");
			}
			_ => panic!("Expected RouteError"),
		}
	}

	#[test]
	#[serial(apps_registry)]
	fn test_application_builder_method_chaining() {
		// Arrange - Reset global state before test
		crate::registry::reset_global_registry();

		let app = ApplicationBuilder::new()
			.add_app(AppConfig::new("app1", "app1"))
			.add_middleware("CorsMiddleware")
			.add_url_pattern(RouteConfig::new("/api/", "ApiHandler"))
			.database(ApplicationDatabaseConfig::new("postgresql://localhost/db"))
			.add_setting("DEBUG", "true")
			.build()
			.unwrap();

		assert_eq!(app.apps().len(), 1);
		assert_eq!(app.middleware().len(), 1);
		assert_eq!(app.url_patterns().len(), 1);
		assert!(app.database_config().is_some());
		assert_eq!(app.settings().get("DEBUG"), Some(&"true".to_string()));
	}

	#[test]
	#[serial(apps_registry)]
	fn test_application_builder_apps_registry_ready() {
		// Arrange - Reset global state before test
		crate::registry::reset_global_registry();

		let app = ApplicationBuilder::new()
			.add_app(AppConfig::new("myapp", "myapp"))
			.build()
			.unwrap();

		assert!(app.apps_registry().is_ready());
		assert!(app.apps_registry().is_apps_ready());
		assert!(app.apps_registry().is_models_ready());
	}

	#[test]
	#[serial(apps_registry)]
	fn test_application_builder_invalid_app_label() {
		// Arrange - Reset global state before test
		crate::registry::reset_global_registry();

		let result = ApplicationBuilder::new()
			.add_app(AppConfig::new("myapp", "my-app"))
			.build();

		assert!(result.is_err());
		match result {
			Err(BuildError::App(AppError::InvalidLabel(_))) => {}
			_ => panic!("Expected InvalidLabel error"),
		}
	}

	#[test]
	fn test_route_config_without_name() {
		let route = RouteConfig::new("/api/v1/users/", "UserHandler");
		assert_eq!(route.full_name(), None);
	}

	#[test]
	fn test_database_config_minimal() {
		let db_config = ApplicationDatabaseConfig::new("sqlite::memory:");
		assert_eq!(db_config.url, "sqlite::memory:");
		assert_eq!(db_config.pool_size, None);
		assert_eq!(db_config.max_overflow, None);
		assert_eq!(db_config.timeout, None);
	}

	#[test]
	#[serial(apps_registry)]
	fn test_application_builder_empty_settings() {
		// Arrange - Reset global state before test
		crate::registry::reset_global_registry();

		let app = ApplicationBuilder::new().build().unwrap();
		assert!(app.settings().is_empty());
	}
}
