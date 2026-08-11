//! Model admin configuration and trait
//!
//! This module defines how models are displayed and managed in the admin interface.

use crate::core::admin_query::{AdminQuery, AdminRequestContext};
use crate::types::{AdminError, AdminResult};
use async_trait::async_trait;
use reinhardt_utils::utils_core::text::humanize_field_name;
use std::collections::HashMap;

/// A column displayed in an admin changelist.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ListColumn {
	/// A database-backed field column.
	Field {
		/// Field name to read from the result row.
		field: String,
		/// Display label for the column header.
		label: String,
	},
	/// A value computed after the result row is fetched.
	Computed {
		/// Stable key used in responses and computed-value lookup.
		key: String,
		/// Display label for the column header.
		label: String,
		/// Database field used when this computed column is sorted.
		sort_field: Option<String>,
	},
}

/// Object-safe trait for admin permission checks.
///
/// This trait provides the minimum user information needed for admin
/// permission decisions, without exposing generic type parameters
/// from [`BaseUser`](reinhardt_auth::BaseUser) or [`FullUser`](reinhardt_auth::FullUser).
///
/// A blanket implementation is provided for all types implementing
/// [`FullUser`](reinhardt_auth::FullUser), so any custom user model
/// with `FullUser` will automatically satisfy this trait.
///
/// For simpler user models that only implement `BaseUser` (without `FullUser`),
/// this trait can be implemented manually to enable admin authentication.
pub trait AdminUser: Send + Sync {
	/// Whether the user account is active
	fn is_active(&self) -> bool;

	/// Whether the user is a staff member (can access admin)
	fn is_staff(&self) -> bool;

	/// Whether the user is a superuser (all permissions granted)
	fn is_superuser(&self) -> bool;

	/// The username for audit logging
	fn get_username(&self) -> &str;
}

/// Blanket implementation for all types implementing [`FullUser`](reinhardt_auth::FullUser).
///
/// This ensures that any custom user model with `FullUser` implementation
/// automatically satisfies `AdminUser`.
impl<T: reinhardt_auth::FullUser> AdminUser for T {
	fn is_active(&self) -> bool {
		reinhardt_auth::BaseUser::is_active(self)
	}

	fn is_staff(&self) -> bool {
		reinhardt_auth::FullUser::is_staff(self)
	}

	fn is_superuser(&self) -> bool {
		reinhardt_auth::FullUser::is_superuser(self)
	}

	fn get_username(&self) -> &str {
		reinhardt_auth::FullUser::username(self)
	}
}

/// Trait for configuring model administration
///
/// Implement this trait to customize how a model is displayed and edited in the admin.
#[async_trait]
pub trait ModelAdmin: Send + Sync {
	/// Get the model name
	fn model_name(&self) -> &str;

	/// Get the database table name
	///
	/// By default, returns an empty string as a placeholder.
	/// Implementors should override this to return the actual table name.
	fn table_name(&self) -> &str {
		// Default implementation returns empty string
		// Override in implementations to return actual table name
		""
	}

	/// Get the primary key field name
	///
	/// By default, returns "id".
	fn pk_field(&self) -> &str {
		"id"
	}

	/// Fields to display in list view
	fn list_display(&self) -> Vec<&str> {
		vec!["id"]
	}

	/// Owned descriptors for columns displayed in list view.
	///
	/// The default preserves the legacy [`Self::list_display`] contract by
	/// converting every field to a database-backed descriptor.
	fn list_columns(&self) -> Vec<ListColumn> {
		self.list_display()
			.into_iter()
			.map(|field| ListColumn::Field {
				field: field.to_string(),
				label: humanize_field_name(field),
			})
			.collect()
	}

	/// Resolve a computed changelist column for a fetched result row.
	///
	/// Implement this together with a [`ListColumn::Computed`] descriptor.
	fn computed_list_value(
		&self,
		key: &str,
		_row: &HashMap<String, serde_json::Value>,
	) -> AdminResult<serde_json::Value> {
		Err(AdminError::TemplateError(format!(
			"No computed list column is configured for key '{key}'"
		)))
	}

	/// Date or datetime field used for hierarchical changelist navigation.
	fn date_hierarchy(&self) -> Option<&str> {
		None
	}

	/// Fields that can be used for filtering
	fn list_filter(&self) -> Vec<&str> {
		vec![]
	}

	/// Fields that can be searched
	fn search_fields(&self) -> Vec<&str> {
		vec![]
	}

	/// Fields to display in forms (None = all fields)
	fn fields(&self) -> Option<Vec<&str>> {
		None
	}

	/// Read-only fields
	fn readonly_fields(&self) -> Vec<&str> {
		vec![]
	}

	/// Ordering for list view (prefix with "-" for descending)
	fn ordering(&self) -> Vec<&str> {
		vec!["-id"]
	}

	/// Number of items per page (None = use site default)
	fn list_per_page(&self) -> Option<usize> {
		None
	}

	/// One-level forward foreign keys to select with each changelist row.
	///
	/// Related values are returned as nested objects under the relationship name.
	fn list_select_related(&self) -> Vec<&str> {
		vec![]
	}

	/// Customize the changelist query for a request.
	///
	/// Appended conditions are combined with search and client filters using `AND`
	/// and apply to both list rows and their total count.
	async fn get_queryset(
		&self,
		_user: &dyn AdminUser,
		_request: &AdminRequestContext,
		query: AdminQuery,
	) -> AdminResult<AdminQuery> {
		Ok(query)
	}

	/// Check if user has permission to view this model
	///
	/// Default implementation denies all access (deny-by-default).
	/// Override this method to grant view permission based on user attributes.
	///
	/// # Migration from previous versions
	///
	/// Previously, this method accepted `&(dyn std::any::Any + Send + Sync)`.
	/// It now accepts `&dyn AdminUser` for type-safe permission checks.
	async fn has_view_permission(&self, _user: &dyn AdminUser) -> bool {
		false
	}

	/// Check if user has permission to add instances
	///
	/// Default implementation denies all access (deny-by-default).
	/// Override this method to grant add permission based on user attributes.
	async fn has_add_permission(&self, _user: &dyn AdminUser) -> bool {
		false
	}

	/// Check if user has permission to change instances
	///
	/// Default implementation denies all access (deny-by-default).
	/// Override this method to grant change permission based on user attributes.
	async fn has_change_permission(&self, _user: &dyn AdminUser) -> bool {
		false
	}

	/// Check if user has permission to delete instances
	///
	/// Default implementation denies all access (deny-by-default).
	/// Override this method to grant delete permission based on user attributes.
	async fn has_delete_permission(&self, _user: &dyn AdminUser) -> bool {
		false
	}
}

/// Configuration-based model admin implementation
///
/// Provides a simple way to configure model admin without implementing the trait.
///
/// # Examples
///
/// ```
/// use reinhardt_admin::core::{ModelAdminConfig, ModelAdmin};
///
/// let admin = ModelAdminConfig::builder()
///     .model_name("User")
///     .list_display(vec!["id", "username", "email"])
///     .list_filter(vec!["is_active"])
///     .search_fields(vec!["username", "email"])
///     .allow_all(true)
///     .build()
///     .unwrap();
///
/// assert_eq!(admin.model_name(), "User");
/// ```
#[derive(Debug, Clone)]
pub struct ModelAdminConfig {
	model_name: String,
	table_name: Option<String>,
	pk_field: String,
	list_display: Vec<String>,
	list_filter: Vec<String>,
	search_fields: Vec<String>,
	fields: Option<Vec<String>>,
	readonly_fields: Vec<String>,
	ordering: Vec<String>,
	list_per_page: Option<usize>,
	list_select_related: Vec<String>,
	date_hierarchy: Option<String>,
	allow_view: bool,
	allow_add: bool,
	allow_change: bool,
	allow_delete: bool,
}

impl ModelAdminConfig {
	/// Create a new model admin configuration
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_admin::core::{ModelAdminConfig, ModelAdmin};
	///
	/// let admin = ModelAdminConfig::new("User");
	/// assert_eq!(admin.model_name(), "User");
	/// ```
	pub fn new(model_name: impl Into<String>) -> Self {
		Self {
			model_name: model_name.into(),
			table_name: None,
			pk_field: "id".into(),
			list_display: vec!["id".into()],
			list_filter: vec![],
			search_fields: vec![],
			fields: None,
			readonly_fields: vec![],
			ordering: vec!["-id".into()],
			list_per_page: None,
			list_select_related: vec![],
			date_hierarchy: None,
			allow_view: false,
			allow_add: false,
			allow_change: false,
			allow_delete: false,
		}
	}

	/// Start building a model admin configuration
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_admin::core::ModelAdminConfig;
	///
	/// let admin = ModelAdminConfig::builder()
	///     .model_name("User")
	///     .list_display(vec!["id", "username"])
	///     .build()
	///     .unwrap();
	/// ```
	pub fn builder() -> ModelAdminConfigBuilder {
		ModelAdminConfigBuilder::default()
	}

	/// Set list display fields
	pub fn with_list_display(mut self, fields: Vec<impl Into<String>>) -> Self {
		self.list_display = fields.into_iter().map(Into::into).collect();
		self
	}

	/// Set list filter fields
	pub fn with_list_filter(mut self, fields: Vec<impl Into<String>>) -> Self {
		self.list_filter = fields.into_iter().map(Into::into).collect();
		self
	}

	/// Set search fields
	pub fn with_search_fields(mut self, fields: Vec<impl Into<String>>) -> Self {
		self.search_fields = fields.into_iter().map(Into::into).collect();
		self
	}

	/// Set related fields selected with each changelist row.
	pub fn with_list_select_related(mut self, fields: Vec<impl Into<String>>) -> Self {
		self.list_select_related = fields.into_iter().map(Into::into).collect();
		self
	}

	/// Set the date or datetime field used for hierarchical navigation.
	pub fn with_date_hierarchy(mut self, field: impl Into<String>) -> Self {
		self.date_hierarchy = Some(field.into());
		self
	}
}

#[async_trait]
impl ModelAdmin for ModelAdminConfig {
	fn model_name(&self) -> &str {
		&self.model_name
	}

	fn table_name(&self) -> &str {
		self.table_name
			.as_deref()
			.unwrap_or(self.model_name.as_str())
	}

	fn pk_field(&self) -> &str {
		&self.pk_field
	}

	fn list_display(&self) -> Vec<&str> {
		self.list_display.iter().map(|s| s.as_str()).collect()
	}

	fn list_filter(&self) -> Vec<&str> {
		self.list_filter.iter().map(|s| s.as_str()).collect()
	}

	fn search_fields(&self) -> Vec<&str> {
		self.search_fields.iter().map(|s| s.as_str()).collect()
	}

	fn fields(&self) -> Option<Vec<&str>> {
		self.fields
			.as_ref()
			.map(|f| f.iter().map(|s| s.as_str()).collect())
	}

	fn readonly_fields(&self) -> Vec<&str> {
		self.readonly_fields.iter().map(|s| s.as_str()).collect()
	}

	fn ordering(&self) -> Vec<&str> {
		self.ordering.iter().map(|s| s.as_str()).collect()
	}

	fn list_per_page(&self) -> Option<usize> {
		self.list_per_page
	}

	fn list_select_related(&self) -> Vec<&str> {
		self.list_select_related
			.iter()
			.map(|field| field.as_str())
			.collect()
	}

	fn date_hierarchy(&self) -> Option<&str> {
		self.date_hierarchy.as_deref()
	}

	async fn has_view_permission(&self, _user: &dyn AdminUser) -> bool {
		self.allow_view
	}

	async fn has_add_permission(&self, _user: &dyn AdminUser) -> bool {
		self.allow_add
	}

	async fn has_change_permission(&self, _user: &dyn AdminUser) -> bool {
		self.allow_change
	}

	async fn has_delete_permission(&self, _user: &dyn AdminUser) -> bool {
		self.allow_delete
	}
}

/// Builder for ModelAdminConfig
#[derive(Debug, Default)]
pub struct ModelAdminConfigBuilder {
	model_name: Option<String>,
	table_name: Option<String>,
	pk_field: Option<String>,
	list_display: Option<Vec<String>>,
	list_filter: Option<Vec<String>>,
	search_fields: Option<Vec<String>>,
	fields: Option<Vec<String>>,
	readonly_fields: Option<Vec<String>>,
	ordering: Option<Vec<String>>,
	list_per_page: Option<usize>,
	list_select_related: Option<Vec<String>>,
	date_hierarchy: Option<String>,
	allow_view: Option<bool>,
	allow_add: Option<bool>,
	allow_change: Option<bool>,
	allow_delete: Option<bool>,
}

impl ModelAdminConfigBuilder {
	/// Set the model name
	pub fn model_name(mut self, name: impl Into<String>) -> Self {
		self.model_name = Some(name.into());
		self
	}

	/// Set the database table name
	///
	/// If not set, defaults to the model name.
	pub fn table_name(mut self, name: impl Into<String>) -> Self {
		self.table_name = Some(name.into());
		self
	}

	/// Set the primary key field name
	///
	/// If not set, defaults to "id".
	pub fn pk_field(mut self, field: impl Into<String>) -> Self {
		self.pk_field = Some(field.into());
		self
	}

	/// Set list display fields
	pub fn list_display(mut self, fields: Vec<impl Into<String>>) -> Self {
		self.list_display = Some(fields.into_iter().map(Into::into).collect());
		self
	}

	/// Set list filter fields
	pub fn list_filter(mut self, fields: Vec<impl Into<String>>) -> Self {
		self.list_filter = Some(fields.into_iter().map(Into::into).collect());
		self
	}

	/// Set search fields
	pub fn search_fields(mut self, fields: Vec<impl Into<String>>) -> Self {
		self.search_fields = Some(fields.into_iter().map(Into::into).collect());
		self
	}

	/// Set form fields
	pub fn fields(mut self, fields: Vec<impl Into<String>>) -> Self {
		self.fields = Some(fields.into_iter().map(Into::into).collect());
		self
	}

	/// Set readonly fields
	pub fn readonly_fields(mut self, fields: Vec<impl Into<String>>) -> Self {
		self.readonly_fields = Some(fields.into_iter().map(Into::into).collect());
		self
	}

	/// Set ordering
	pub fn ordering(mut self, fields: Vec<impl Into<String>>) -> Self {
		self.ordering = Some(fields.into_iter().map(Into::into).collect());
		self
	}

	/// Set items per page
	pub fn list_per_page(mut self, count: usize) -> Self {
		self.list_per_page = Some(count);
		self
	}

	/// Set related fields selected with each changelist row.
	pub fn list_select_related(mut self, fields: Vec<impl Into<String>>) -> Self {
		self.list_select_related = Some(fields.into_iter().map(Into::into).collect());
		self
	}

	/// Set the date or datetime field used for hierarchical navigation.
	pub fn date_hierarchy(mut self, field: impl Into<String>) -> Self {
		self.date_hierarchy = Some(field.into());
		self
	}

	/// Set view permission
	///
	/// If not set, defaults to `false` (deny-by-default).
	pub fn allow_view(mut self, allow: bool) -> Self {
		self.allow_view = Some(allow);
		self
	}

	/// Set add permission
	///
	/// If not set, defaults to `false` (deny-by-default).
	pub fn allow_add(mut self, allow: bool) -> Self {
		self.allow_add = Some(allow);
		self
	}

	/// Set change permission
	///
	/// If not set, defaults to `false` (deny-by-default).
	pub fn allow_change(mut self, allow: bool) -> Self {
		self.allow_change = Some(allow);
		self
	}

	/// Set delete permission
	///
	/// If not set, defaults to `false` (deny-by-default).
	pub fn allow_delete(mut self, allow: bool) -> Self {
		self.allow_delete = Some(allow);
		self
	}

	/// Set all permissions (view, add, change, delete) at once
	///
	/// Convenience method for granting or denying all operations.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_admin::core::ModelAdminConfig;
	///
	/// let admin = ModelAdminConfig::builder()
	///     .model_name("User")
	///     .allow_all(true)
	///     .build()
	///     .unwrap();
	/// ```
	pub fn allow_all(mut self, allow: bool) -> Self {
		self.allow_view = Some(allow);
		self.allow_add = Some(allow);
		self.allow_change = Some(allow);
		self.allow_delete = Some(allow);
		self
	}

	/// Build the configuration
	///
	/// # Errors
	///
	/// Returns `AdminError::ValidationError` if `model_name` is not set.
	pub fn build(self) -> AdminResult<ModelAdminConfig> {
		let model_name = self
			.model_name
			.ok_or_else(|| AdminError::ValidationError("model_name is required".to_string()))?;

		Ok(ModelAdminConfig {
			model_name,
			table_name: self.table_name,
			pk_field: self.pk_field.unwrap_or_else(|| "id".into()),
			list_display: self.list_display.unwrap_or_else(|| vec!["id".into()]),
			list_filter: self.list_filter.unwrap_or_default(),
			search_fields: self.search_fields.unwrap_or_default(),
			fields: self.fields,
			readonly_fields: self.readonly_fields.unwrap_or_default(),
			ordering: self.ordering.unwrap_or_else(|| vec!["-id".into()]),
			list_per_page: self.list_per_page,
			list_select_related: self.list_select_related.unwrap_or_default(),
			date_hierarchy: self.date_hierarchy,
			allow_view: self.allow_view.unwrap_or(false),
			allow_add: self.allow_add.unwrap_or(false),
			allow_change: self.allow_change.unwrap_or(false),
			allow_delete: self.allow_delete.unwrap_or(false),
		})
	}
}

#[cfg(all(test, server))]
mod tests {
	use super::*;
	use hyper::Method;
	use reinhardt_db::orm::{Filter, FilterCondition, FilterOperator, FilterValue};
	use reinhardt_http::Request;
	use rstest::rstest;
	use std::net::{IpAddr, Ipv4Addr, SocketAddr};
	use std::sync::Arc;

	/// Dummy AdminUser for testing permission methods
	struct TestAdminUser {
		active: bool,
		staff: bool,
		superuser: bool,
		username: String,
	}

	impl TestAdminUser {
		fn new() -> Self {
			Self {
				active: true,
				staff: true,
				superuser: false,
				username: "test_user".to_string(),
			}
		}
	}

	impl AdminUser for TestAdminUser {
		fn is_active(&self) -> bool {
			self.active
		}

		fn is_staff(&self) -> bool {
			self.staff
		}

		fn is_superuser(&self) -> bool {
			self.superuser
		}

		fn get_username(&self) -> &str {
			&self.username
		}
	}

	#[rstest]
	fn test_model_admin_config_creation() {
		let admin = ModelAdminConfig::new("User");
		assert_eq!(admin.model_name(), "User");
		assert_eq!(admin.list_display(), vec!["id"]);
		assert_eq!(admin.list_filter(), Vec::<&str>::new());
	}

	#[rstest]
	fn test_list_columns_converts_legacy_list_display() {
		// Arrange
		let config = ModelAdminConfig::new("User").with_list_display(vec!["id", "created_at"]);
		let admin: &dyn ModelAdmin = &config;

		// Act
		let columns = admin.list_columns();

		// Assert
		assert_eq!(
			columns,
			vec![
				ListColumn::Field {
					field: "id".to_string(),
					label: "Id".to_string(),
				},
				ListColumn::Field {
					field: "created_at".to_string(),
					label: "Created At".to_string(),
				},
			]
		);
	}

	#[rstest]
	fn test_computed_column_preserves_key_label_and_sort_mapping() {
		// Arrange
		let column = ListColumn::Computed {
			key: "author_name".to_string(),
			label: "Author".to_string(),
			sort_field: Some("author_id".to_string()),
		};

		// Act & Assert
		assert_eq!(
			column,
			ListColumn::Computed {
				key: "author_name".to_string(),
				label: "Author".to_string(),
				sort_field: Some("author_id".to_string()),
			}
		);
	}

	#[rstest]
	fn test_default_computed_list_value_returns_template_error() {
		// Arrange
		let admin = ModelAdminConfig::new("User");

		// Act
		let result = admin.computed_list_value("author_name", &HashMap::new());

		// Assert
		assert!(matches!(result, Err(AdminError::TemplateError(_))));
	}

	#[rstest]
	fn test_date_hierarchy_defaults_to_none() {
		// Arrange & Act
		let config_admin = ModelAdminConfig::new("User");
		let builder_admin = ModelAdminConfig::builder()
			.model_name("User")
			.build()
			.unwrap();

		// Assert
		assert_eq!(config_admin.date_hierarchy(), None);
		assert_eq!(builder_admin.date_hierarchy(), None);
	}

	#[rstest]
	fn test_date_hierarchy_builder_configures_field() {
		// Arrange & Act
		let admin = ModelAdminConfig::builder()
			.model_name("User")
			.date_hierarchy("created_at")
			.build()
			.unwrap();

		// Assert
		assert_eq!(admin.date_hierarchy(), Some("created_at"));
	}

	#[rstest]
	fn test_model_admin_config_builder() {
		let admin = ModelAdminConfig::builder()
			.model_name("User")
			.list_display(vec!["id", "username", "email"])
			.list_filter(vec!["is_active"])
			.search_fields(vec!["username", "email"])
			.list_per_page(50)
			.build()
			.unwrap();

		assert_eq!(admin.model_name(), "User");
		assert_eq!(admin.list_display(), vec!["id", "username", "email"]);
		assert_eq!(admin.list_filter(), vec!["is_active"]);
		assert_eq!(admin.search_fields(), vec!["username", "email"]);
		assert_eq!(admin.list_per_page(), Some(50));
	}

	#[rstest]
	fn test_with_methods() {
		let admin = ModelAdminConfig::new("Post")
			.with_list_display(vec!["id", "title", "author"])
			.with_list_filter(vec!["status", "created_at"])
			.with_search_fields(vec!["title", "content"]);

		assert_eq!(admin.list_display(), vec!["id", "title", "author"]);
		assert_eq!(admin.list_filter(), vec!["status", "created_at"]);
		assert_eq!(admin.search_fields(), vec!["title", "content"]);
	}

	#[rstest]
	fn test_builder_without_model_name_returns_error() {
		// Arrange & Act
		let result = ModelAdminConfig::builder().build();

		// Assert
		assert!(result.is_err());
		let err = result.unwrap_err();
		assert!(err.to_string().contains("model_name is required"));
	}

	/// Helper struct for testing default trait permission behavior
	struct DefaultPermissionAdmin;

	#[async_trait]
	impl ModelAdmin for DefaultPermissionAdmin {
		fn model_name(&self) -> &str {
			"TestModel"
		}
	}

	/// Helper struct for testing explicit permission grants
	struct AllowAllPermissionAdmin;

	#[async_trait]
	impl ModelAdmin for AllowAllPermissionAdmin {
		fn model_name(&self) -> &str {
			"AllowedModel"
		}

		async fn has_view_permission(&self, _user: &dyn AdminUser) -> bool {
			true
		}

		async fn has_add_permission(&self, _user: &dyn AdminUser) -> bool {
			true
		}

		async fn has_change_permission(&self, _user: &dyn AdminUser) -> bool {
			true
		}

		async fn has_delete_permission(&self, _user: &dyn AdminUser) -> bool {
			true
		}
	}

	#[rstest]
	#[tokio::test]
	async fn test_default_permissions_deny_view() {
		// Arrange
		let admin = DefaultPermissionAdmin;
		let user = TestAdminUser::new();

		// Act
		let result = admin.has_view_permission(&user as &dyn AdminUser).await;

		// Assert
		assert!(!result);
	}

	#[rstest]
	#[tokio::test]
	async fn test_default_permissions_deny_add() {
		// Arrange
		let admin = DefaultPermissionAdmin;
		let user = TestAdminUser::new();

		// Act
		let result = admin.has_add_permission(&user as &dyn AdminUser).await;

		// Assert
		assert!(!result);
	}

	#[rstest]
	#[tokio::test]
	async fn test_default_permissions_deny_change() {
		// Arrange
		let admin = DefaultPermissionAdmin;
		let user = TestAdminUser::new();

		// Act
		let result = admin.has_change_permission(&user as &dyn AdminUser).await;

		// Assert
		assert!(!result);
	}

	#[rstest]
	#[tokio::test]
	async fn test_default_permissions_deny_delete() {
		// Arrange
		let admin = DefaultPermissionAdmin;
		let user = TestAdminUser::new();

		// Act
		let result = admin.has_delete_permission(&user as &dyn AdminUser).await;

		// Assert
		assert!(!result);
	}

	#[rstest]
	#[tokio::test]
	async fn test_explicit_override_grants_all_permissions() {
		// Arrange
		let admin = AllowAllPermissionAdmin;
		let user = TestAdminUser::new();

		// Act
		let view = admin.has_view_permission(&user as &dyn AdminUser).await;
		let add = admin.has_add_permission(&user as &dyn AdminUser).await;
		let change = admin.has_change_permission(&user as &dyn AdminUser).await;
		let delete = admin.has_delete_permission(&user as &dyn AdminUser).await;

		// Assert
		assert!(view);
		assert!(add);
		assert!(change);
		assert!(delete);
	}

	#[rstest]
	#[tokio::test]
	async fn test_model_admin_config_inherits_deny_by_default() {
		// Arrange
		let admin = ModelAdminConfig::new("User");
		let user = TestAdminUser::new();

		// Act
		let view = admin.has_view_permission(&user as &dyn AdminUser).await;
		let add = admin.has_add_permission(&user as &dyn AdminUser).await;
		let change = admin.has_change_permission(&user as &dyn AdminUser).await;
		let delete = admin.has_delete_permission(&user as &dyn AdminUser).await;

		// Assert
		assert!(!view);
		assert!(!add);
		assert!(!change);
		assert!(!delete);
	}

	// ==================== ModelAdminConfig field tests ====================

	#[rstest]
	fn test_model_admin_config_custom_pk_field() {
		// Arrange
		let admin = ModelAdminConfig::builder()
			.model_name("User")
			.pk_field("uuid")
			.build()
			.unwrap();

		// Act
		let pk = admin.pk_field();

		// Assert
		assert_eq!(pk, "uuid");
	}

	#[rstest]
	fn test_model_admin_config_default_pk_field() {
		// Arrange
		let admin = ModelAdminConfig::builder()
			.model_name("User")
			.build()
			.unwrap();

		// Act
		let pk = admin.pk_field();

		// Assert
		assert_eq!(pk, "id");
	}

	#[rstest]
	fn test_model_admin_config_custom_table_name() {
		// Arrange
		let admin = ModelAdminConfig::builder()
			.model_name("User")
			.table_name("my_users")
			.build()
			.unwrap();

		// Act
		let table = admin.table_name();

		// Assert
		assert_eq!(table, "my_users");
	}

	#[rstest]
	fn test_model_admin_config_table_name_defaults_to_model_name() {
		// Arrange
		let admin = ModelAdminConfig::builder()
			.model_name("User")
			.build()
			.unwrap();

		// Act
		let table = admin.table_name();

		// Assert
		assert_eq!(table, "User");
	}

	#[rstest]
	#[tokio::test]
	async fn test_model_admin_config_builder_inherits_deny_by_default() {
		// Arrange
		let admin = ModelAdminConfig::builder()
			.model_name("Post")
			.list_display(vec!["id", "title"])
			.build()
			.unwrap();
		let user = TestAdminUser::new();

		// Act
		let view = admin.has_view_permission(&user as &dyn AdminUser).await;
		let add = admin.has_add_permission(&user as &dyn AdminUser).await;

		// Assert
		assert!(!view);
		assert!(!add);
	}

	#[rstest]
	#[tokio::test]
	async fn test_builder_allow_view_grants_view_permission() {
		// Arrange
		let admin = ModelAdminConfig::builder()
			.model_name("Post")
			.allow_view(true)
			.build()
			.unwrap();
		let user = TestAdminUser::new();

		// Act
		let view = admin.has_view_permission(&user as &dyn AdminUser).await;
		let add = admin.has_add_permission(&user as &dyn AdminUser).await;

		// Assert
		assert!(view);
		assert!(!add);
	}

	#[rstest]
	#[tokio::test]
	async fn test_builder_allow_all_grants_all_permissions() {
		// Arrange
		let admin = ModelAdminConfig::builder()
			.model_name("Post")
			.allow_all(true)
			.build()
			.unwrap();
		let user = TestAdminUser::new();

		// Act
		let view = admin.has_view_permission(&user as &dyn AdminUser).await;
		let add = admin.has_add_permission(&user as &dyn AdminUser).await;
		let change = admin.has_change_permission(&user as &dyn AdminUser).await;
		let delete = admin.has_delete_permission(&user as &dyn AdminUser).await;

		// Assert
		assert!(view);
		assert!(add);
		assert!(change);
		assert!(delete);
	}

	#[rstest]
	#[tokio::test]
	async fn test_builder_allow_all_false_denies_all() {
		// Arrange
		let admin = ModelAdminConfig::builder()
			.model_name("Post")
			.allow_all(false)
			.build()
			.unwrap();
		let user = TestAdminUser::new();

		// Act
		let view = admin.has_view_permission(&user as &dyn AdminUser).await;
		let add = admin.has_add_permission(&user as &dyn AdminUser).await;

		// Assert
		assert!(!view);
		assert!(!add);
	}

	#[rstest]
	#[tokio::test]
	async fn test_builder_individual_permissions() {
		// Arrange
		let admin = ModelAdminConfig::builder()
			.model_name("Post")
			.allow_view(true)
			.allow_add(true)
			.allow_change(false)
			.allow_delete(false)
			.build()
			.unwrap();
		let user = TestAdminUser::new();

		// Act
		let view = admin.has_view_permission(&user as &dyn AdminUser).await;
		let add = admin.has_add_permission(&user as &dyn AdminUser).await;
		let change = admin.has_change_permission(&user as &dyn AdminUser).await;
		let delete = admin.has_delete_permission(&user as &dyn AdminUser).await;

		// Assert
		assert!(view);
		assert!(add);
		assert!(!change);
		assert!(!delete);
	}

	// ==================== Decision table: allow_all controls permissions ====================

	#[rstest]
	#[case::allow_all_true(true, true)]
	#[case::allow_all_false(false, false)]
	#[tokio::test]
	async fn test_allow_all_controls_view_permission(
		#[case] allow_all: bool,
		#[case] expected: bool,
	) {
		// Arrange
		let admin = ModelAdminConfig::builder()
			.model_name("PermTest")
			.allow_all(allow_all)
			.build()
			.unwrap();
		let user = TestAdminUser::new();

		// Act
		let result = admin.has_view_permission(&user as &dyn AdminUser).await;

		// Assert
		assert_eq!(result, expected);
	}

	// ==================== Boundary value: list_per_page override ====================

	#[rstest]
	#[case::with_list_per_page(Some(50), Some(50))]
	#[case::without_list_per_page(None, None)]
	fn test_list_per_page_override(
		#[case] override_value: Option<usize>,
		#[case] expected: Option<usize>,
	) {
		// Arrange
		let mut builder = ModelAdminConfig::builder().model_name("PageTest");
		if let Some(v) = override_value {
			builder = builder.list_per_page(v);
		}
		let admin = builder.build().unwrap();

		// Act
		let result = admin.list_per_page();

		// Assert
		assert_eq!(result, expected);
	}

	// ==================== Boundary value: builder model_name validation ====================

	#[rstest]
	#[case::missing_model_name(true)]
	#[case::valid_model_name(false)]
	fn test_builder_model_name_validation(#[case] should_error: bool) {
		// Arrange
		let builder = if should_error {
			// Do not set model_name to trigger error
			ModelAdminConfig::builder()
		} else {
			ModelAdminConfig::builder().model_name("User")
		};

		// Act
		let result = builder.build();

		// Assert
		assert_eq!(
			result.is_err(),
			should_error,
			"should_error={}, got {:?}",
			should_error,
			result
		);
	}

	#[rstest]
	fn test_admin_query_preserves_table_and_retains_conditions() {
		// Arrange
		let owner_filter = Filter::new("owner_id", FilterOperator::Eq, FilterValue::Integer(7));
		let tenant_condition = FilterCondition::Single(Filter::new(
			"tenant_id",
			FilterOperator::Eq,
			FilterValue::String("tenant-a".to_string()),
		));

		// Act
		let query = AdminQuery::new("articles")
			.filter(owner_filter)
			.filter_condition(tenant_condition);

		// Assert
		assert_eq!(query.table_name(), "articles");
		let conditions = query.conditions();
		assert_eq!(conditions.len(), 2);
		let FilterCondition::Single(owner) = &conditions[0] else {
			panic!("expected owner filter first");
		};
		let FilterCondition::Single(tenant) = &conditions[1] else {
			panic!("expected tenant filter second");
		};
		assert_eq!(owner.field, "owner_id");
		assert_eq!(tenant.field, "tenant_id");
	}

	#[rstest]
	fn test_admin_request_context_exposes_read_only_request_data() {
		// Arrange
		let remote_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 8080);
		let request = Request::builder()
			.method(Method::POST)
			.uri("/admin/articles?tenant=tenant-a")
			.header("x-tenant-id", "tenant-a")
			.secure(true)
			.remote_addr(remote_addr)
			.build()
			.unwrap();

		// Act
		let context = AdminRequestContext::new(Arc::new(request));

		// Assert
		assert_eq!(context.method(), Method::POST);
		assert_eq!(context.uri().path(), "/admin/articles");
		assert_eq!(context.uri().query(), Some("tenant=tenant-a"));
		assert_eq!(context.headers()["x-tenant-id"], "tenant-a");
		assert!(context.is_secure());
		assert_eq!(context.remote_addr(), Some(remote_addr));
	}

	#[rstest]
	#[tokio::test]
	async fn test_default_get_queryset_is_identity() {
		// Arrange
		let admin = DefaultPermissionAdmin;
		let user = TestAdminUser::new();
		let request = Request::builder().uri("/admin/test").build().unwrap();
		let context = AdminRequestContext::new(Arc::new(request));
		let query = AdminQuery::new("test_models");

		// Act
		let result = admin
			.get_queryset(&user as &dyn AdminUser, &context, query)
			.await
			.unwrap();

		// Assert
		assert_eq!(result.table_name(), "test_models");
		assert!(result.conditions().is_empty());
	}

	#[rstest]
	fn test_list_select_related_configuration() {
		// Arrange & Act
		let default_admin = ModelAdminConfig::new("Article");
		let built_admin = ModelAdminConfig::builder()
			.model_name("Article")
			.list_select_related(vec!["author", "category"])
			.build()
			.unwrap();
		let fluent_admin =
			ModelAdminConfig::new("Article").with_list_select_related(vec!["author"]);

		// Assert
		assert_eq!(default_admin.list_select_related(), Vec::<&str>::new());
		assert_eq!(
			built_admin.list_select_related(),
			vec!["author", "category"]
		);
		assert_eq!(fluent_admin.list_select_related(), vec!["author"]);
	}
}
