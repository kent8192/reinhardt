//! Admin site management
//!
//! The `AdminSite` is the central registry for all admin models and provides
//! routing, authentication, and rendering functionality.

use crate::core::{AdminRouter, ModelAdmin};
use crate::types::{AdminError, AdminResult};
use async_trait::async_trait;
use dashmap::DashMap;
use parking_lot::RwLock;
use reinhardt_db::orm::DatabaseConnection;
use reinhardt_di::{DiResult, Injectable, InjectionContext, SingletonScope};
use reinhardt_urls::routers::ServerRouter;
use std::sync::Arc;

/// The main admin site that manages all registered models
///
/// # Examples
///
/// ```
/// use reinhardt_admin::core::AdminSite;
///
/// let admin = AdminSite::new("My Application");
/// assert_eq!(admin.name(), "My Application");
/// ```
#[derive(Clone)]
pub struct AdminSite {
	/// Site name displayed in the admin interface
	name: String,

	/// URL prefix for admin routes (default: "/admin")
	url_prefix: String,

	/// Registry of model admins indexed by model name
	registry: Arc<DashMap<String, Arc<dyn ModelAdmin>>>,

	/// Site-level configuration
	config: Arc<RwLock<AdminSiteConfig>>,

	/// Favicon data (PNG, ICO, etc.)
	favicon_data: Arc<RwLock<Option<Vec<u8>>>>,
}

/// Configuration for the admin site
#[derive(Debug, Clone)]
pub struct AdminSiteConfig {
	/// Site title shown in browser tab
	pub site_title: String,

	/// Header text shown at the top of admin pages
	pub site_header: String,

	/// Index page title
	pub index_title: String,

	/// Items per page in list views
	pub list_per_page: usize,

	/// Enable search functionality
	pub enable_search: bool,

	/// Enable filtering functionality
	pub enable_filters: bool,
}

impl Default for AdminSiteConfig {
	fn default() -> Self {
		Self {
			site_title: "Admin Panel".into(),
			site_header: "Administration".into(),
			index_title: "Dashboard".into(),
			list_per_page: 100,
			enable_search: true,
			enable_filters: true,
		}
	}
}

impl AdminSite {
	/// Create a new admin site
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_admin::core::AdminSite;
	///
	/// let admin = AdminSite::new("E-commerce Admin");
	/// ```
	pub fn new(name: impl Into<String>) -> Self {
		Self {
			name: name.into(),
			url_prefix: "/admin".into(),
			registry: Arc::new(DashMap::new()),
			config: Arc::new(RwLock::new(AdminSiteConfig::default())),
			favicon_data: Arc::new(RwLock::new(None)),
		}
	}

	/// Get the site name
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_admin::core::AdminSite;
	///
	/// let admin = AdminSite::new("My Admin");
	/// assert_eq!(admin.name(), "My Admin");
	/// ```
	pub fn name(&self) -> &str {
		&self.name
	}

	/// Set the URL prefix for admin routes
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_admin::core::AdminSite;
	///
	/// let mut admin = AdminSite::new("Admin");
	/// admin.set_url_prefix("/manage");
	/// assert_eq!(admin.url_prefix(), "/manage");
	/// ```
	pub fn set_url_prefix(&mut self, prefix: impl Into<String>) {
		self.url_prefix = prefix.into();
	}

	/// Get the URL prefix
	pub fn url_prefix(&self) -> &str {
		&self.url_prefix
	}

	/// Set favicon data from bytes
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_admin::core::AdminSite;
	///
	/// let admin = AdminSite::new("Admin");
	/// admin.set_favicon(vec![0x89, 0x50, 0x4E, 0x47]); // PNG magic bytes
	/// assert!(admin.favicon_data().is_some());
	/// ```
	pub fn set_favicon(&self, data: Vec<u8>) {
		*self.favicon_data.write() = Some(data);
	}

	/// Get favicon data (cloned)
	///
	/// Returns None if no favicon has been configured.
	pub fn favicon_data(&self) -> Option<Vec<u8>> {
		self.favicon_data.read().clone()
	}

	/// Configure the admin site
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_admin::core::AdminSite;
	/// use reinhardt_admin::core::site::AdminSiteConfig;
	///
	/// let admin = AdminSite::new("Admin");
	/// admin.configure(|config| {
	///     config.site_title = "My Custom Admin".into();
	///     config.list_per_page = 50;
	/// });
	/// ```
	pub fn configure<F>(&self, f: F)
	where
		F: FnOnce(&mut AdminSiteConfig),
	{
		let mut config = self.config.write();
		f(&mut config);
	}

	/// Get the current configuration
	pub fn config(&self) -> AdminSiteConfig {
		self.config.read().clone()
	}

	/// Register a model with the admin site
	///
	/// # Examples
	///
	/// ```ignore
	/// use reinhardt_admin::core::{AdminSite, ModelAdminConfig};
	///
	/// let admin = AdminSite::new("Admin");
	///
	/// let user_admin = ModelAdminConfig::builder()
	///     .model_name("User")
	///     .list_display(vec!["id", "username", "email"])
	///     .build()?;
	///
	/// admin.register("User", user_admin);
	/// ```
	pub fn register(
		&self,
		model_name: impl Into<String>,
		admin: impl ModelAdmin + 'static,
	) -> AdminResult<()> {
		let model_name = model_name.into();
		// Use entry API for atomic check-and-insert to avoid TOCTOU race condition
		match self.registry.entry(model_name) {
			dashmap::mapref::entry::Entry::Occupied(entry) => Err(AdminError::ValidationError(
				format!("Model '{}' is already registered", entry.key()),
			)),
			dashmap::mapref::entry::Entry::Vacant(entry) => {
				entry.insert(Arc::new(admin));
				Ok(())
			}
		}
	}

	/// Unregister a model from the admin site
	///
	/// # Examples
	///
	/// ```ignore
	/// use reinhardt_admin::core::AdminSite;
	///
	/// let admin = AdminSite::new("Admin");
	/// // ... register User ...
	/// admin.unregister("User");
	/// ```
	pub fn unregister(&self, model_name: &str) -> AdminResult<()> {
		self.registry
			.remove(model_name)
			.ok_or_else(|| AdminError::ModelNotRegistered(model_name.into()))?;
		Ok(())
	}

	/// Check if a model is registered
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_admin::core::AdminSite;
	///
	/// let admin = AdminSite::new("Admin");
	/// assert!(!admin.is_registered("User"));
	/// ```
	pub fn is_registered(&self, model_name: &str) -> bool {
		self.registry.contains_key(model_name)
	}

	/// Get the admin for a specific model
	///
	/// # Examples
	///
	/// ```ignore
	/// use reinhardt_admin::core::AdminSite;
	///
	/// let admin = AdminSite::new("Admin");
	/// // ... register User ...
	/// let user_admin = admin.get_model_admin("User").unwrap();
	/// ```
	pub fn get_model_admin(&self, model_name: &str) -> AdminResult<Arc<dyn ModelAdmin>> {
		self.registry
			.get(model_name)
			.map(|entry| Arc::clone(entry.value()))
			.ok_or_else(|| AdminError::ModelNotRegistered(model_name.into()))
	}

	/// Get all registered model names
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_admin::core::AdminSite;
	///
	/// let admin = AdminSite::new("Admin");
	/// assert_eq!(admin.registered_models().len(), 0);
	/// ```
	pub fn registered_models(&self) -> Vec<String> {
		self.registry
			.iter()
			.map(|entry| entry.key().clone())
			.collect()
	}

	/// Get the number of registered models
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_admin::core::AdminSite;
	///
	/// let admin = AdminSite::new("Admin");
	/// assert_eq!(admin.model_count(), 0);
	/// ```
	pub fn model_count(&self) -> usize {
		self.registry.len()
	}

	/// Clear all registered models
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_admin::core::AdminSite;
	///
	/// let admin = AdminSite::new("Admin");
	/// admin.clear();
	/// assert_eq!(admin.model_count(), 0);
	/// ```
	pub fn clear(&self) {
		self.registry.clear();
	}

	/// Build a ServerRouter from this admin site
	///
	/// Creates HTTP endpoints for all registered models including:
	/// - `GET /` - Dashboard (list of registered models)
	/// - `GET /favicon.ico` - Favicon
	/// - `GET /{model}/` - List model instances
	/// - `GET /{model}/{id}/` - Get model instance detail
	/// - `POST /{model}/` - Create model instance
	/// - `PUT /{model}/{id}/` - Update model instance
	/// - `DELETE /{model}/{id}/` - Delete model instance
	/// - `POST /{model}/bulk-delete/` - Bulk delete model instances
	/// - `GET /{model}/export/` - Export model data
	/// - `POST /{model}/import/` - Import model data
	///
	/// # Examples
	///
	/// ```rust,no_run
	/// use reinhardt_admin::core::{AdminSite, ModelAdminConfig};
	/// use reinhardt_db::orm::DatabaseConnection;
	///
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// let admin = AdminSite::new("My Admin");
	/// admin.register("User", ModelAdminConfig::new("User"))?;
	///
	/// let conn = DatabaseConnection::connect("postgres://localhost/mydb").await?;
	/// let router = admin.get_urls(conn);
	/// # Ok(())
	/// # }
	/// ```
	pub fn get_urls(self, _db: DatabaseConnection) -> ServerRouter {
		let url_prefix = self.url_prefix.clone();
		crate::core::router::admin_routes().with_prefix(&url_prefix)
	}

	/// Get an AdminRouter for more control over route building
	///
	/// Use this when you need to customize the router before building,
	/// such as setting a custom favicon.
	///
	/// # Examples
	///
	/// ```rust,no_run
	/// use reinhardt_admin::core::{AdminSite, ModelAdminConfig};
	/// use reinhardt_db::orm::DatabaseConnection;
	///
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// let admin = AdminSite::new("My Admin");
	/// admin.register("User", ModelAdminConfig::new("User"))?;
	///
	/// let conn = DatabaseConnection::connect("postgres://localhost/mydb").await?;
	/// let router = admin.get_router(conn)
	///     .with_favicon_file("branding/logo.png")?
	///     .build();
	/// # Ok(())
	/// # }
	/// ```
	pub fn get_router(self, _db: DatabaseConnection) -> AdminRouter {
		AdminRouter::from_arc(Arc::new(self))
	}

	/// Configure dependency injection container for admin panel
	///
	/// Registers AdminSite, AdminDatabase, and optional favicon data
	/// as singletons in the DI container. This allows handlers to use
	/// `#[inject]` to automatically receive these dependencies.
	///
	/// # Examples
	///
	/// ```rust,no_run
	/// use reinhardt_admin::core::{AdminSite, AdminDatabase};
	/// use reinhardt_di::SingletonScope;
	/// use reinhardt_db::orm::DatabaseConnection;
	/// use std::sync::Arc;
	///
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// let admin_site = Arc::new(AdminSite::new("My Admin"));
	/// let conn = DatabaseConnection::connect("postgres://localhost/mydb").await?;
	/// let admin_db = AdminDatabase::new(conn);
	///
	/// let singleton = Arc::new(SingletonScope::new());
	/// AdminSite::configure_di(&singleton, Arc::clone(&admin_site), admin_db, None);
	/// # Ok(())
	/// # }
	/// ```
	pub fn configure_di(
		singleton: &SingletonScope,
		site: Arc<AdminSite>,
		db: crate::core::AdminDatabase,
		favicon_data: Option<Vec<u8>>,
	) {
		// Set favicon data on AdminSite if provided
		if let Some(data) = favicon_data {
			site.set_favicon(data);
		}

		// Register AdminSite as singleton
		singleton.set(site);

		// Register AdminDatabase as singleton
		singleton.set(Arc::new(db));
	}
}

/// Injectable trait implementation for AdminSite
///
/// This allows AdminSite to be injected via the DI container.
/// The implementation resolves `Arc<AdminSite>` from the container
/// and clones the inner value.
#[async_trait]
impl Injectable for AdminSite {
	async fn inject(ctx: &InjectionContext) -> DiResult<Self> {
		// Resolve Arc<AdminSite> from the container and clone it
		ctx.resolve::<Self>().await.map(|arc| (*arc).clone())
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::core::ModelAdminConfig;
	use rstest::rstest;

	#[rstest]
	fn test_admin_site_creation() {
		let admin = AdminSite::new("Test Admin");
		assert_eq!(admin.name(), "Test Admin");
		assert_eq!(admin.url_prefix(), "/admin");
		assert_eq!(admin.model_count(), 0);
	}

	#[rstest]
	fn test_url_prefix() {
		let mut admin = AdminSite::new("Admin");
		admin.set_url_prefix("/manage");
		assert_eq!(admin.url_prefix(), "/manage");
	}

	#[rstest]
	fn test_configuration() {
		let admin = AdminSite::new("Admin");
		admin.configure(|config| {
			config.site_title = "Custom Title".into();
			config.list_per_page = 25;
		});

		let config = admin.config();
		assert_eq!(config.site_title, "Custom Title");
		assert_eq!(config.list_per_page, 25);
	}

	#[rstest]
	fn test_register_and_unregister() {
		let admin = AdminSite::new("Admin");
		let model_admin = ModelAdminConfig::new("User");

		assert!(!admin.is_registered("User"));

		admin.register("User", model_admin).unwrap();
		assert!(admin.is_registered("User"));
		assert_eq!(admin.model_count(), 1);

		admin.unregister("User").unwrap();
		assert!(!admin.is_registered("User"));
		assert_eq!(admin.model_count(), 0);
	}

	#[rstest]
	fn test_unregister_nonexistent() {
		let admin = AdminSite::new("Admin");
		let result = admin.unregister("NonExistent");
		assert!(result.is_err());
	}

	#[rstest]
	fn test_get_model_admin() {
		let admin = AdminSite::new("Admin");
		let model_admin = ModelAdminConfig::new("User");

		admin.register("User", model_admin).unwrap();

		let retrieved = admin.get_model_admin("User");
		assert!(retrieved.is_ok());
	}

	#[rstest]
	fn test_get_nonexistent_model_admin() {
		let admin = AdminSite::new("Admin");
		let result = admin.get_model_admin("NonExistent");
		assert!(result.is_err());
	}

	#[rstest]
	fn test_registered_models() {
		let admin = AdminSite::new("Admin");

		admin
			.register("User", ModelAdminConfig::new("User"))
			.unwrap();
		admin
			.register("Post", ModelAdminConfig::new("Post"))
			.unwrap();

		let models = admin.registered_models();
		assert_eq!(models.len(), 2);
		assert!(models.contains(&"User".into()));
		assert!(models.contains(&"Post".into()));
	}

	#[rstest]
	fn test_clear() {
		let admin = AdminSite::new("Admin");

		admin
			.register("User", ModelAdminConfig::new("User"))
			.unwrap();
		admin
			.register("Post", ModelAdminConfig::new("Post"))
			.unwrap();

		assert_eq!(admin.model_count(), 2);

		admin.clear();
		assert_eq!(admin.model_count(), 0);
	}

	#[rstest]
	fn test_duplicate_registration_returns_error() {
		// Arrange
		let admin = AdminSite::new("Admin");
		admin
			.register("User", ModelAdminConfig::new("User"))
			.unwrap();

		// Act
		let result = admin.register("User", ModelAdminConfig::new("User"));

		// Assert
		assert!(result.is_err());
		let err = result.unwrap_err();
		assert!(err.to_string().contains("already registered"));
	}

	#[rstest]
	fn test_default_config() {
		let config = AdminSiteConfig::default();
		assert_eq!(config.site_title, "Admin Panel");
		assert_eq!(config.site_header, "Administration");
		assert_eq!(config.list_per_page, 100);
		assert!(config.enable_search);
		assert!(config.enable_filters);
	}
}
