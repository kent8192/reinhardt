//! Admin site management
//!
//! The `AdminSite` is the central registry for all admin models and provides
//! routing, authentication, and rendering functionality.

use crate::core::ModelAdmin;
use crate::core::model_admin::AdminUser;
use crate::server::admin_auth::{AdminLoginAuthenticator, AdminUserLoader};
use crate::server::type_inference::find_model_by_table_name;
use crate::types::{AdminError, AdminResult};
use async_trait::async_trait;
use dashmap::DashMap;
use parking_lot::RwLock;
use reinhardt_core::macros::injectable;
use reinhardt_db::migrations::FieldType as DbFieldType;
use reinhardt_di::{DiResult, Injectable, InjectionContext, KeyedFactoryOutput};
use std::collections::HashSet;
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
#[injectable(scope = Singleton, prebuilt = true)]
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

	/// Type-erased user loader for admin authentication.
	///
	/// When `None`, [`AdminDefaultUser`] is used as a fallback.
	///
	/// [`AdminDefaultUser`]: crate::server::user::AdminDefaultUser
	user_loader: Option<Arc<AdminUserLoader>>,

	/// Type-erased login authenticator for admin login.
	///
	/// When `None`, [`AdminDefaultUser`] is used as a fallback.
	///
	/// [`AdminDefaultUser`]: crate::server::user::AdminDefaultUser
	login_authenticator: Option<Arc<AdminLoginAuthenticator>>,

	/// JWT secret for token generation during admin login.
	///
	/// When `None`, admin login is disabled.
	jwt_secret: Option<Vec<u8>>,
}

/// Provider key for the admin site dependency.
#[reinhardt_di::injectable_key]
pub struct AdminSiteKey;

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
			user_loader: None,
			login_authenticator: None,
			jwt_secret: None,
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

	/// Set the user type for admin authentication.
	///
	/// This determines which database table and model is used to load the
	/// authenticated user in admin server functions. The type `U` must
	/// implement `BaseUser`, `AdminUser`, and the ORM trait (`Model`).
	/// Types annotated with `#[model(...)]` and `#[user(full = true)]`
	/// satisfy this automatically via the blanket `impl<T: FullUser> AdminUser for T`.
	/// Simpler user models that only implement `BaseUser` can manually
	/// implement `AdminUser` to use admin authentication.
	///
	/// If this method is not called, [`AdminDefaultUser`] (table `auth_user`)
	/// is used as the default.
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_admin::core::AdminSite;
	///
	/// let mut site = AdminSite::new("My Admin");
	/// site.set_user_type::<MyCustomUser>();
	/// ```
	///
	/// [`AdminDefaultUser`]: crate::server::user::AdminDefaultUser
	pub fn set_user_type<U>(&mut self) -> &mut Self
	where
		U: reinhardt_auth::BaseUser
			+ AdminUser
			+ reinhardt_db::orm::Model
			+ Clone
			+ Send
			+ Sync
			+ 'static,
		<U as reinhardt_auth::BaseUser>::PrimaryKey: std::str::FromStr + ToString + Send + Sync,
		<<U as reinhardt_auth::BaseUser>::PrimaryKey as std::str::FromStr>::Err: std::fmt::Debug,
		<U as reinhardt_db::orm::Model>::PrimaryKey:
			From<<U as reinhardt_auth::BaseUser>::PrimaryKey>,
	{
		self.user_loader = Some(Arc::new(
			crate::server::admin_auth::create_admin_user_loader::<U>(),
		));
		self.login_authenticator = Some(Arc::new(
			crate::server::admin_auth::create_admin_login_authenticator::<U>(),
		));
		self
	}

	/// Returns the registered user loader, if any.
	pub(crate) fn user_loader(&self) -> Option<Arc<AdminUserLoader>> {
		self.user_loader.clone()
	}

	/// Returns the registered login authenticator, if any.
	pub(crate) fn login_authenticator(&self) -> Option<Arc<AdminLoginAuthenticator>> {
		self.login_authenticator.clone()
	}

	/// Sets the JWT secret used for token generation during admin login.
	///
	/// The secret should be at least 32 bytes for adequate security.
	/// Without this, admin login functionality will be disabled.
	///
	/// # Example
	///
	/// ```ignore
	/// use reinhardt_admin::core::AdminSite;
	///
	/// let mut site = AdminSite::new("Admin");
	/// site.set_jwt_secret(b"my-very-secret-key-at-least-32-bytes!");
	/// ```
	pub fn set_jwt_secret(&mut self, secret: &[u8]) -> &mut Self {
		self.jwt_secret = Some(secret.to_vec());
		self
	}

	/// Returns the JWT secret, if configured.
	pub(crate) fn jwt_secret(&self) -> Option<&[u8]> {
		self.jwt_secret.as_deref()
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
		// Reject case-insensitive duplicates (URLs are lowercased, so "User" and "user"
		// would collide at /admin/user/).
		let needle = model_name.to_lowercase();
		if let Some(existing) = self
			.registry
			.iter()
			.find(|e| e.key().to_lowercase() == needle)
		{
			return Err(AdminError::ValidationError(format!(
				"Model '{}' is already registered (as '{}')",
				model_name,
				existing.key()
			)));
		}
		let table_name = admin.table_name().to_owned();
		if let Some(existing) = self
			.registry
			.iter()
			.find(|entry| entry.value().table_name() == table_name)
		{
			return Err(AdminError::ValidationError(format!(
				"Table '{}' is already registered as '{}'",
				table_name,
				existing.key()
			)));
		}
		validate_list_editable(&admin)?;
		validate_actions(&admin)?;
		self.registry.insert(model_name, Arc::new(admin));
		Ok(())
	}

	/// Unregister a model from the admin site
	///
	/// # Examples
	///
	/// ```no_run
	/// use reinhardt_admin::core::AdminSite;
	///
	/// let admin = AdminSite::new("Admin");
	/// // ... register User ...
	/// admin.unregister("User");
	/// ```
	pub fn unregister(&self, model_name: &str) -> AdminResult<()> {
		let needle = model_name.to_lowercase();
		let key = self
			.registry
			.iter()
			.find(|entry| entry.key().to_lowercase() == needle)
			.map(|entry| entry.key().clone())
			.ok_or_else(|| AdminError::ModelNotRegistered(model_name.into()))?;
		self.registry.remove(&key);
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
		let needle = model_name.to_lowercase();
		self.registry
			.iter()
			.any(|entry| entry.key().to_lowercase() == needle)
	}

	/// Get the admin for a specific model
	///
	/// # Examples
	///
	/// ```no_run
	/// use reinhardt_admin::core::AdminSite;
	///
	/// let admin = AdminSite::new("Admin");
	/// // ... register User ...
	/// let user_admin = admin.get_model_admin("User").unwrap();
	/// ```
	pub fn get_model_admin(&self, model_name: &str) -> AdminResult<Arc<dyn ModelAdmin>> {
		let needle = model_name.to_lowercase();
		self.registry
			.iter()
			.find(|entry| entry.key().to_lowercase() == needle)
			.map(|entry| Arc::clone(entry.value()))
			.ok_or_else(|| AdminError::ModelNotRegistered(model_name.into()))
	}

	pub(crate) fn get_model_admin_by_table_name(
		&self,
		table_name: &str,
	) -> AdminResult<Arc<dyn ModelAdmin>> {
		self.registry
			.iter()
			.find(|entry| entry.value().table_name() == table_name)
			.map(|entry| Arc::clone(entry.value()))
			.ok_or_else(|| AdminError::ModelNotRegistered(table_name.into()))
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
}

fn validate_list_editable(admin: &dyn ModelAdmin) -> AdminResult<()> {
	let list_editable = admin.list_editable();
	if list_editable.is_empty() {
		return Ok(());
	}

	let list_display = admin.list_display();
	let readonly_fields = admin.readonly_fields();
	let mut seen = HashSet::new();
	for field in &list_editable {
		if !seen.insert(*field) {
			return Err(AdminError::ValidationError(format!(
				"Field '{field}' appears more than once in list_editable"
			)));
		}
		if !list_display.contains(field) {
			return Err(AdminError::ValidationError(format!(
				"Field '{field}' is not in list_display"
			)));
		}
		if *field == admin.pk_field() {
			return Err(AdminError::ValidationError(format!(
				"Field '{field}' is the primary key and cannot be list_editable"
			)));
		}
		if list_display.first() == Some(field) {
			return Err(AdminError::ValidationError(format!(
				"Field '{field}' is the first list_display field and cannot be list_editable"
			)));
		}
		if readonly_fields.contains(field) {
			return Err(AdminError::ValidationError(format!(
				"Field '{field}' is read-only and cannot be list_editable"
			)));
		}
	}

	let table_name = if admin.table_name().is_empty() {
		admin.model_name()
	} else {
		admin.table_name()
	};
	let metadata = find_model_by_table_name(table_name).ok_or_else(|| {
		AdminError::ValidationError(format!(
			"Model '{}' is not registered in model metadata",
			table_name
		))
	})?;
	for field in list_editable {
		let metadata = metadata.fields.get(field).ok_or_else(|| {
			AdminError::ValidationError(format!("Field '{field}' is not a model field"))
		})?;
		if metadata.generated.is_some() {
			return Err(AdminError::ValidationError(format!(
				"Field '{field}' is a generated column and cannot be list_editable"
			)));
		}
		if metadata
			.params
			.get("auto_now")
			.is_some_and(|value| value.eq_ignore_ascii_case("true"))
		{
			return Err(AdminError::ValidationError(format!(
				"Field '{field}' uses auto_now and cannot be list_editable"
			)));
		}
		if metadata
			.params
			.get("auto_now_add")
			.is_some_and(|value| value.eq_ignore_ascii_case("true"))
		{
			return Err(AdminError::ValidationError(format!(
				"Field '{field}' uses auto_now_add and cannot be list_editable"
			)));
		}
		if matches!(field, "password_hash" | "password_salt") {
			return Err(AdminError::ValidationError(format!(
				"Field '{field}' is sensitive and cannot be list_editable"
			)));
		}
		if matches!(
			&metadata.field_type,
			DbFieldType::Binary
				| DbFieldType::Blob
				| DbFieldType::TinyBlob
				| DbFieldType::MediumBlob
				| DbFieldType::LongBlob
				| DbFieldType::Bytea
		) {
			return Err(AdminError::ValidationError(format!(
				"Field '{field}' is binary and cannot be list_editable"
			)));
		}
		if matches!(
			&metadata.field_type,
			DbFieldType::ForeignKey { .. }
				| DbFieldType::OneToOne { .. }
				| DbFieldType::ManyToMany { .. }
		) {
			return Err(AdminError::ValidationError(format!(
				"Field '{field}' is a relation and cannot be list_editable"
			)));
		}
		if let DbFieldType::Array(inner) = &metadata.field_type {
			if matches!(
				inner.as_ref(),
				DbFieldType::Char(_)
					| DbFieldType::VarChar(_)
					| DbFieldType::Text
					| DbFieldType::TinyText
					| DbFieldType::MediumText
					| DbFieldType::LongText
					| DbFieldType::CIText
			) {
				return Err(AdminError::ValidationError(format!(
					"Field '{field}' is a string array and cannot be list_editable"
				)));
			}
			let supported = match inner.as_ref() {
				DbFieldType::Integer
				| DbFieldType::SmallInteger
				| DbFieldType::TinyInt
				| DbFieldType::MediumInt
				| DbFieldType::BigInteger
				| DbFieldType::Boolean
				| DbFieldType::Float
				| DbFieldType::Double
				| DbFieldType::Real
				| DbFieldType::Uuid => true,
				DbFieldType::Custom(name) => matches!(name.as_str(), "u8" | "u16" | "u32"),
				_ => false,
			};
			if !supported {
				return Err(AdminError::ValidationError(format!(
					"Field '{field}' has an unsupported array element type and cannot be list_editable"
				)));
			}
		}
		if matches!(
			&metadata.field_type,
			DbFieldType::HStore
				| DbFieldType::Int4Range
				| DbFieldType::Int8Range
				| DbFieldType::NumRange
				| DbFieldType::DateRange
				| DbFieldType::TsRange
				| DbFieldType::TsTzRange
				| DbFieldType::TsVector
				| DbFieldType::TsQuery
		) {
			return Err(AdminError::ValidationError(format!(
				"Field '{field}' has no supported inline update encoding"
			)));
		}
	}

	Ok(())
}

fn validate_actions(admin: &dyn ModelAdmin) -> AdminResult<()> {
	let mut names = HashSet::new();
	for action in admin.actions() {
		if action.name.is_empty() {
			return Err(AdminError::ValidationError(
				"Admin action name cannot be empty".to_owned(),
			));
		}
		if !names.insert(action.name.clone()) {
			return Err(AdminError::ValidationError(format!(
				"Admin action '{}' is registered more than once",
				action.name
			)));
		}
	}
	Ok(())
}

/// Injectable trait implementation for AdminSite
///
/// Resolves `AdminSite` directly from the singleton scope.
/// The `AdminSite` must be registered via `admin_routes_with_di()` which
/// returns a `DiRegistrationList` to be attached to the router.
#[async_trait]
impl Injectable for AdminSite {
	async fn inject(ctx: &InjectionContext) -> DiResult<Self> {
		ctx.get_singleton::<Self>()
			.map(|arc| (*arc).clone())
			.ok_or_else(|| reinhardt_di::DiError::NotRegistered {
				type_name: "AdminSite".into(),
				hint: "AdminSite must be registered as a singleton. \
				       Use admin_routes_with_di(site) and attach the returned \
				       DiRegistrationList via .with_di_registrations() on UnifiedRouter."
					.into(),
			})
	}
}

#[reinhardt_di::injectable(scope = "singleton")]
async fn admin_site_provider(
	#[inject] site: AdminSite,
) -> KeyedFactoryOutput<AdminSiteKey, AdminSite> {
	KeyedFactoryOutput::new(site)
}

#[cfg(all(test, server))]
mod tests {
	use super::*;
	use crate::core::ModelAdminConfig;
	use reinhardt_db::migrations::{
		FieldMetadata, FieldType, GeneratedColumnDefinition, GeneratedStorage, ModelMetadata,
		global_registry,
	};
	use reinhardt_di::SingletonScope;
	use rstest::rstest;
	use serial_test::serial;
	use uuid::Uuid;

	struct ModelRegistryGuard {
		app_label: String,
		model_name: String,
	}

	impl Drop for ModelRegistryGuard {
		fn drop(&mut self) {
			global_registry().remove_model(&self.app_label, &self.model_name);
		}
	}

	fn register_list_editable_model(
		fields: impl IntoIterator<Item = (&'static str, FieldMetadata)>,
	) -> (String, ModelRegistryGuard) {
		let suffix = Uuid::new_v4().simple().to_string();
		let app_label = format!("admin_list_editable_{suffix}");
		let model_name = format!("ListEditable{suffix}");
		let mut metadata = ModelMetadata::new(&app_label, &model_name, &model_name);
		for (name, field) in fields {
			metadata.add_field(name.to_string(), field);
		}
		global_registry().register_model(metadata);
		(
			model_name.clone(),
			ModelRegistryGuard {
				app_label,
				model_name,
			},
		)
	}

	struct ListEditableAdmin {
		model_name: String,
		pk_field: &'static str,
		list_display: Vec<&'static str>,
		list_editable: Vec<&'static str>,
		readonly_fields: Vec<&'static str>,
	}

	#[async_trait]
	impl ModelAdmin for ListEditableAdmin {
		fn model_name(&self) -> &str {
			&self.model_name
		}

		fn table_name(&self) -> &str {
			&self.model_name
		}

		fn pk_field(&self) -> &str {
			self.pk_field
		}

		fn list_display(&self) -> Vec<&str> {
			self.list_display.clone()
		}

		fn list_editable(&self) -> Vec<&str> {
			self.list_editable.clone()
		}

		fn readonly_fields(&self) -> Vec<&str> {
			self.readonly_fields.clone()
		}
	}

	struct EmptyActionAdmin;

	#[async_trait::async_trait]
	impl ModelAdmin for EmptyActionAdmin {
		fn model_name(&self) -> &str {
			"EmptyActionModel"
		}

		fn actions(&self) -> Vec<crate::types::AdminAction> {
			vec![crate::types::AdminAction::new(
				"",
				"Empty",
				crate::types::ModelPermission::Change,
				false,
			)]
		}
	}

	fn list_editable_admin(
		model_name: String,
		list_display: Vec<&'static str>,
		list_editable: Vec<&'static str>,
	) -> ListEditableAdmin {
		ListEditableAdmin {
			model_name,
			pk_field: "id",
			list_display,
			list_editable,
			readonly_fields: vec![],
		}
	}

	#[rstest]
	fn test_admin_site_creation() {
		let admin = AdminSite::new("Test Admin");
		assert_eq!(admin.name(), "Test Admin");
		assert_eq!(admin.url_prefix(), "/admin");
		assert_eq!(admin.model_count(), 0);
	}

	#[rstest]
	#[tokio::test]
	async fn test_admin_site_resolves_through_keyed_provider() {
		let singleton = Arc::new(SingletonScope::new());
		let site = Arc::new(AdminSite::new("Registry Admin"));
		singleton.set_arc(site);
		let ctx = reinhardt_di::InjectionContext::builder(singleton).build();

		let result = reinhardt_di::KeyedDepends::<AdminSiteKey, AdminSite>::resolve_from_registry(
			&ctx, true,
		)
		.await;

		assert!(result.is_ok());
		assert_eq!(result.unwrap().name(), "Registry Admin");
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
	fn test_get_model_admin_by_table_name_ignores_route_alias() {
		// Arrange
		let site = AdminSite::new("Admin");
		let model = ModelAdminConfig::builder()
			.model_name("Child")
			.table_name("child_records")
			.build()
			.expect("child admin should build");
		site.register("child-route", model)
			.expect("child route should register");

		// Act
		let admin = site
			.get_model_admin_by_table_name("child_records")
			.expect("table identity should resolve the child admin");

		// Assert
		assert_eq!(admin.model_name(), "Child");
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

	#[rstest]
	fn test_set_arc_stores_admin_site_with_correct_type_id() {
		// Arrange
		let singleton = SingletonScope::new();
		let site = Arc::new(AdminSite::new("Test Admin"));

		// Act - use set_arc which stores with TypeId::of::<AdminSite>()
		singleton.set_arc(site);

		// Assert - should be retrievable as AdminSite (not Arc<AdminSite>)
		assert!(
			singleton.get::<AdminSite>().is_some(),
			"AdminSite should be retrievable via get::<AdminSite>()"
		);
	}

	#[rstest]
	fn test_set_arc_preserves_favicon_data() {
		// Arrange
		let singleton = SingletonScope::new();
		let site = Arc::new(AdminSite::new("Test Admin"));
		let favicon = vec![0x89, 0x50, 0x4E, 0x47]; // PNG magic bytes

		// Act
		site.set_favicon(favicon.clone());
		singleton.set_arc(site);

		// Assert
		let retrieved = singleton.get::<AdminSite>().unwrap();
		assert_eq!(retrieved.favicon_data(), Some(favicon));
	}

	#[rstest]
	#[tokio::test]
	async fn test_admin_site_inject_resolves_from_singleton() {
		// Arrange
		let singleton = Arc::new(SingletonScope::new());
		let site = Arc::new(AdminSite::new("Injectable Admin"));
		singleton.set_arc(site);
		let ctx = reinhardt_di::InjectionContext::builder(singleton).build();

		// Act
		let result = AdminSite::inject(&ctx).await;

		// Assert
		assert!(result.is_ok());
		assert_eq!(result.unwrap().name(), "Injectable Admin");
	}

	#[rstest]
	#[tokio::test]
	async fn test_admin_site_inject_returns_error_when_not_registered() {
		// Arrange
		let singleton = Arc::new(SingletonScope::new());
		let ctx = reinhardt_di::InjectionContext::builder(singleton).build();

		// Act
		let result = AdminSite::inject(&ctx).await;

		// Assert
		assert!(result.is_err());
		let err = result.err().unwrap();
		assert!(
			err.to_string().contains("AdminSite"),
			"Error should mention AdminSite, got: {}",
			err
		);
	}

	#[rstest]
	#[tokio::test]
	async fn test_admin_site_inject_error_hint_mentions_routes_with_di() {
		// Arrange
		let singleton = Arc::new(SingletonScope::new());
		let ctx = reinhardt_di::InjectionContext::builder(singleton).build();

		// Act
		let result = AdminSite::inject(&ctx).await;

		// Assert
		assert!(result.is_err());
		let err = result.err().unwrap();
		assert!(
			err.to_string().contains("admin_routes_with_di"),
			"Error hint should mention admin_routes_with_di, got: {}",
			err
		);
	}

	// ---- Case-insensitive registry tests (Fixes #3353) ----

	#[rstest]
	fn test_get_model_admin_case_insensitive() {
		let admin = AdminSite::new("Admin");
		admin
			.register("User", ModelAdminConfig::new("User"))
			.unwrap();

		assert!(admin.get_model_admin("User").is_ok());
		assert!(admin.get_model_admin("user").is_ok());
		assert!(admin.get_model_admin("USER").is_ok());
		assert!(admin.get_model_admin("uSeR").is_ok());
		assert!(admin.get_model_admin("nonexistent").is_err());
	}

	#[rstest]
	fn test_is_registered_case_insensitive() {
		let admin = AdminSite::new("Admin");
		admin
			.register("User", ModelAdminConfig::new("User"))
			.unwrap();

		assert!(admin.is_registered("User"));
		assert!(admin.is_registered("user"));
		assert!(admin.is_registered("USER"));
		assert!(!admin.is_registered("Post"));
	}

	#[rstest]
	fn test_register_rejects_case_insensitive_duplicate() {
		let admin = AdminSite::new("Admin");
		admin
			.register("User", ModelAdminConfig::new("User"))
			.unwrap();

		let result = admin.register("user", ModelAdminConfig::new("user"));
		assert!(result.is_err());
		assert!(
			result
				.unwrap_err()
				.to_string()
				.contains("already registered")
		);
	}

	#[rstest]
	#[serial(admin_model_registry)]
	fn test_register_accepts_valid_list_editable_field() {
		// Arrange
		let (model_name, _guard) = register_list_editable_model([
			("id", FieldMetadata::new(FieldType::Integer)),
			("title", FieldMetadata::new(FieldType::Text)),
		]);
		let site = AdminSite::new("Admin");

		// Act
		let result = site.register(
			model_name.clone(),
			list_editable_admin(model_name, vec!["id", "title"], vec!["title"]),
		);

		// Assert
		result.expect("valid inline field should register");
		assert_eq!(site.model_count(), 1);
	}

	#[rstest]
	#[serial(admin_model_registry)]
	#[case::duplicate(
		vec!["id", "title"],
		vec!["title", "title"],
		None,
		"Field 'title' appears more than once in list_editable"
	)]
	#[case::missing_from_display(
		vec!["id", "title"],
		vec!["status"],
		None,
		"Field 'status' is not in list_display"
	)]
	#[case::primary_key(
		vec!["title", "id"],
		vec!["id"],
		None,
		"Field 'id' is the primary key and cannot be list_editable"
	)]
	#[case::first_display_link(
		vec!["title", "id"],
		vec!["title"],
		None,
		"Field 'title' is the first list_display field and cannot be list_editable"
	)]
	#[case::readonly(
		vec!["id", "status"],
		vec!["status"],
		Some("status"),
		"Field 'status' is read-only and cannot be list_editable"
	)]
	#[case::unknown_metadata(
		vec!["id", "computed"],
		vec!["computed"],
		None,
		"Field 'computed' is not a model field"
	)]
	fn test_register_rejects_invalid_list_editable(
		#[case] list_display: Vec<&'static str>,
		#[case] list_editable: Vec<&'static str>,
		#[case] readonly_field: Option<&'static str>,
		#[case] expected: &str,
	) {
		// Arrange
		let (model_name, _guard) = register_list_editable_model([
			("id", FieldMetadata::new(FieldType::Integer)),
			("title", FieldMetadata::new(FieldType::Text)),
			("status", FieldMetadata::new(FieldType::Text)),
			("generated", FieldMetadata::new(FieldType::Text)),
		]);
		let site = AdminSite::new("Admin");
		let mut model_admin = list_editable_admin(model_name.clone(), list_display, list_editable);
		if let Some(field) = readonly_field {
			model_admin.readonly_fields.push(field);
		}

		// Act
		let error = site
			.register(model_name, model_admin)
			.expect_err("invalid inline field must reject registration");

		// Assert
		assert!(matches!(
			error,
			AdminError::ValidationError(message) if message == expected
		));
		assert_eq!(site.model_count(), 0);
	}

	#[rstest]
	#[serial(admin_model_registry)]
	fn test_register_rejects_generated_list_editable_field() {
		// Arrange
		let (model_name, _guard) = register_list_editable_model([
			("id", FieldMetadata::new(FieldType::Integer)),
			("title", FieldMetadata::new(FieldType::Text)),
			("status", FieldMetadata::new(FieldType::Text)),
			(
				"generated",
				FieldMetadata::new(FieldType::Text).with_generated(
					GeneratedColumnDefinition::tokens(
						"SchemaExpr::col(\"title\")",
						GeneratedStorage::Stored,
					),
				),
			),
		]);
		let site = AdminSite::new("Admin");

		// Act
		let error = site
			.register(
				model_name.clone(),
				list_editable_admin(model_name, vec!["id", "generated"], vec!["generated"]),
			)
			.expect_err("generated inline field must reject registration");

		// Assert
		assert!(matches!(
			error,
			AdminError::ValidationError(message)
				if message == "Field 'generated' is a generated column and cannot be list_editable"
		));
		assert_eq!(site.model_count(), 0);
	}

	#[rstest]
	#[serial(admin_model_registry)]
	fn test_register_rejects_binary_list_editable_fields() {
		for field_type in [
			FieldType::Binary,
			FieldType::Blob,
			FieldType::TinyBlob,
			FieldType::MediumBlob,
			FieldType::LongBlob,
			FieldType::Bytea,
		] {
			let (model_name, _guard) = register_list_editable_model([
				("id", FieldMetadata::new(FieldType::Integer)),
				("payload", FieldMetadata::new(field_type)),
			]);
			let site = AdminSite::new("Admin");

			let error = site
				.register(
					model_name.clone(),
					list_editable_admin(model_name, vec!["id", "payload"], vec!["payload"]),
				)
				.expect_err("binary fields must not be inline editable");

			assert!(matches!(
				error,
				AdminError::ValidationError(message)
					if message == "Field 'payload' is binary and cannot be list_editable"
			));
			assert_eq!(site.model_count(), 0);
		}
	}

	#[rstest]
	#[serial(admin_model_registry)]
	fn test_register_rejects_auto_now_list_editable_field() {
		for value in ["true", "True"] {
			let (model_name, _guard) = register_list_editable_model([
				("id", FieldMetadata::new(FieldType::Integer)),
				(
					"updated_at",
					FieldMetadata::new(FieldType::DateTime).with_param("auto_now", value),
				),
			]);
			let site = AdminSite::new("Admin");

			let error = site
				.register(
					model_name.clone(),
					list_editable_admin(model_name, vec!["id", "updated_at"], vec!["updated_at"]),
				)
				.expect_err("auto_now fields must not be inline editable");

			assert!(matches!(
				error,
				AdminError::ValidationError(message)
					if message == "Field 'updated_at' uses auto_now and cannot be list_editable"
			));
			assert_eq!(site.model_count(), 0);
		}
	}

	#[rstest]
	#[serial(admin_model_registry)]
	fn test_register_rejects_relation_list_editable_field() {
		let (model_name, _guard) = register_list_editable_model([
			("id", FieldMetadata::new(FieldType::Integer)),
			(
				"owner",
				FieldMetadata::new(FieldType::ForeignKey {
					to_table: "accounts".to_string(),
					to_field: "id".to_string(),
					on_delete: reinhardt_db::migrations::ForeignKeyAction::Cascade,
				}),
			),
		]);
		let site = AdminSite::new("Admin");

		let error = site
			.register(
				model_name.clone(),
				list_editable_admin(model_name, vec!["id", "owner"], vec!["owner"]),
			)
			.expect_err("relation fields must not be inline editable");

		assert!(matches!(
			error,
			AdminError::ValidationError(message)
				if message == "Field 'owner' is a relation and cannot be list_editable"
		));
		assert_eq!(site.model_count(), 0);
	}

	#[rstest]
	#[serial(admin_model_registry)]
	fn test_register_rejects_string_array_list_editable_field() {
		let (model_name, _guard) = register_list_editable_model([
			("id", FieldMetadata::new(FieldType::Integer)),
			(
				"tags",
				FieldMetadata::new(FieldType::Array(Box::new(FieldType::VarChar(32)))),
			),
		]);
		let site = AdminSite::new("Admin");

		let error = site
			.register(
				model_name.clone(),
				list_editable_admin(model_name, vec!["id", "tags"], vec!["tags"]),
			)
			.expect_err("string arrays must not be inline editable");

		assert!(matches!(
			error,
			AdminError::ValidationError(message)
				if message == "Field 'tags' is a string array and cannot be list_editable"
		));
		assert_eq!(site.model_count(), 0);
	}

	#[rstest]
	fn test_register_rejects_duplicate_table_name() {
		let site = AdminSite::new("Admin");
		site.register(
			"ChildPrimary",
			ModelAdminConfig::builder()
				.model_name("ChildPrimary")
				.table_name("child_records")
				.build()
				.expect("first admin should build"),
		)
		.expect("first table registration should succeed");

		let error = site
			.register(
				"ChildAlias",
				ModelAdminConfig::builder()
					.model_name("ChildAlias")
					.table_name("child_records")
					.build()
					.expect("second admin should build"),
			)
			.expect_err("duplicate table registration must be rejected");

		assert!(matches!(
			error,
			AdminError::ValidationError(message)
				if message == "Table 'child_records' is already registered as 'ChildPrimary'"
		));
	}

	#[rstest]
	fn test_register_rejects_empty_action_name() {
		// Arrange
		let site = AdminSite::new("Admin");

		// Act
		let error = site
			.register("EmptyAction", EmptyActionAdmin)
			.expect_err("empty action names must be rejected");

		// Assert
		assert!(matches!(
			error,
			AdminError::ValidationError(message)
				if message == "Admin action name cannot be empty"
		));
	}

	#[rstest]
	fn test_unregister_case_insensitive() {
		let admin = AdminSite::new("Admin");
		admin
			.register("User", ModelAdminConfig::new("User"))
			.unwrap();

		admin.unregister("user").unwrap();
		assert!(!admin.is_registered("User"));
	}
}
