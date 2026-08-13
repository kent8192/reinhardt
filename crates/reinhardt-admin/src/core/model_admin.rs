//! Model admin configuration and trait
//!
//! This module defines how models are displayed and managed in the admin interface.

use crate::core::admin_query::{AdminQuery, AdminRequestContext};
use crate::core::{AdminActionTransaction, InlineModelAdmin};
use crate::types::{AdminAction, AdminActionOutcome, AdminError, AdminResult, Fieldset};
use async_trait::async_trait;
use reinhardt_utils::utils_core::text::humanize_field_name;
use std::collections::{HashMap, HashSet};

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

	/// Fields that can be edited directly in list view.
	///
	/// The default is empty, so list views are read-only unless fields are explicitly enabled.
	fn list_editable(&self) -> Vec<&str> {
		vec![]
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

	/// Fieldsets to display in forms (None = no grouped layout).
	fn fieldsets(&self) -> Option<Vec<Fieldset>> {
		None
	}

	/// Related child models editable on the same form.
	fn inlines(&self) -> Vec<InlineModelAdmin> {
		Vec::new()
	}

	/// Read-only fields
	fn readonly_fields(&self) -> Vec<&str> {
		vec![]
	}

	/// Relation fields rendered with autocomplete controls.
	fn autocomplete_fields(&self) -> Vec<&str> {
		vec![]
	}

	/// Relation fields rendered as raw ID inputs.
	fn raw_id_fields(&self) -> Vec<&str> {
		vec![]
	}

	/// Return a display label for an object represented by field values.
	fn object_label(&self, _values: &HashMap<String, serde_json::Value>) -> Option<String> {
		None
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

	/// Actions available for this model.
	fn actions(&self) -> Vec<AdminAction> {
		Vec::new()
	}

	/// Executes an action for the selected model instances.
	///
	/// All database writes must use `transaction`, which is owned and committed
	/// or rolled back by the server action endpoint.
	async fn execute_action(
		&self,
		action: &str,
		_ids: &[String],
		_transaction: &mut AdminActionTransaction,
		_user: &dyn AdminUser,
	) -> AdminResult<AdminActionOutcome> {
		Err(AdminError::ValidationError(format!(
			"Invalid action: {action}"
		)))
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
///     .list_editable(vec!["username", "email"])
///     .list_filter(vec!["is_active"])
///     .search_fields(vec!["username", "email"])
///     .allow_all(true)
///     .build()
///     .unwrap();
///
/// assert_eq!(admin.model_name(), "User");
/// assert_eq!(admin.list_editable(), vec!["username", "email"]);
/// ```
#[derive(Debug, Clone)]
pub struct ModelAdminConfig {
	model_name: String,
	table_name: Option<String>,
	pk_field: String,
	list_display: Vec<String>,
	list_editable: Vec<String>,
	list_filter: Vec<String>,
	search_fields: Vec<String>,
	fields: Option<Vec<String>>,
	fieldsets: Option<Vec<Fieldset>>,
	inlines: Vec<InlineModelAdmin>,
	readonly_fields: Vec<String>,
	autocomplete_fields: Vec<String>,
	raw_id_fields: Vec<String>,
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
			list_editable: vec![],
			list_filter: vec![],
			search_fields: vec![],
			fields: None,
			fieldsets: None,
			inlines: Vec::new(),
			readonly_fields: vec![],
			autocomplete_fields: vec![],
			raw_id_fields: vec![],
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

	/// Set fields that can be edited directly in list view.
	pub fn with_list_editable(mut self, fields: Vec<impl Into<String>>) -> Self {
		self.list_editable = fields.into_iter().map(Into::into).collect();
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

	fn list_editable(&self) -> Vec<&str> {
		self.list_editable.iter().map(|s| s.as_str()).collect()
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

	fn fieldsets(&self) -> Option<Vec<Fieldset>> {
		self.fieldsets.clone()
	}

	fn inlines(&self) -> Vec<InlineModelAdmin> {
		self.inlines.clone()
	}

	fn readonly_fields(&self) -> Vec<&str> {
		self.readonly_fields.iter().map(|s| s.as_str()).collect()
	}

	fn autocomplete_fields(&self) -> Vec<&str> {
		self.autocomplete_fields
			.iter()
			.map(|s| s.as_str())
			.collect()
	}

	fn raw_id_fields(&self) -> Vec<&str> {
		self.raw_id_fields.iter().map(|s| s.as_str()).collect()
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
	list_editable: Option<Vec<String>>,
	list_filter: Option<Vec<String>>,
	search_fields: Option<Vec<String>>,
	fields: Option<Vec<String>>,
	fieldsets: Option<Vec<Fieldset>>,
	inlines: Option<Vec<InlineModelAdmin>>,
	readonly_fields: Option<Vec<String>>,
	autocomplete_fields: Option<Vec<String>>,
	raw_id_fields: Option<Vec<String>>,
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

	/// Set fields that can be edited directly in list view.
	pub fn list_editable(mut self, fields: Vec<impl Into<String>>) -> Self {
		self.list_editable = Some(fields.into_iter().map(Into::into).collect());
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

	/// Set grouped form fields.
	pub fn fieldsets(mut self, fieldsets: Vec<Fieldset>) -> Self {
		self.fieldsets = Some(fieldsets);
		self
	}

	/// Set related child model configurations.
	pub fn inlines(mut self, inlines: Vec<InlineModelAdmin>) -> Self {
		self.inlines = Some(inlines);
		self
	}

	/// Set readonly fields
	pub fn readonly_fields(mut self, fields: Vec<impl Into<String>>) -> Self {
		self.readonly_fields = Some(fields.into_iter().map(Into::into).collect());
		self
	}

	/// Set relation fields rendered with autocomplete controls.
	pub fn autocomplete_fields(mut self, fields: Vec<impl Into<String>>) -> Self {
		self.autocomplete_fields = Some(fields.into_iter().map(Into::into).collect());
		self
	}

	/// Set relation fields rendered as raw ID inputs.
	pub fn raw_id_fields(mut self, fields: Vec<impl Into<String>>) -> Self {
		self.raw_id_fields = Some(fields.into_iter().map(Into::into).collect());
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
		let autocomplete_fields = self.autocomplete_fields.unwrap_or_default();
		let raw_id_fields = self.raw_id_fields.unwrap_or_default();

		if autocomplete_fields
			.iter()
			.any(|field| raw_id_fields.contains(field))
		{
			return Err(AdminError::ValidationError(
				"autocomplete_fields and raw_id_fields cannot contain the same field".to_string(),
			));
		}
		validate_fieldsets(self.fields.is_some(), self.fieldsets.as_deref())?;
		let inlines = self.inlines.unwrap_or_default();
		InlineModelAdmin::validate_resolved(&inlines)?;

		Ok(ModelAdminConfig {
			model_name,
			table_name: self.table_name,
			pk_field: self.pk_field.unwrap_or_else(|| "id".into()),
			list_display: self.list_display.unwrap_or_else(|| vec!["id".into()]),
			list_editable: self.list_editable.unwrap_or_default(),
			list_filter: self.list_filter.unwrap_or_default(),
			search_fields: self.search_fields.unwrap_or_default(),
			fields: self.fields,
			fieldsets: self.fieldsets,
			inlines,
			readonly_fields: self.readonly_fields.unwrap_or_default(),
			autocomplete_fields,
			raw_id_fields,
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

/// Resolve the configured form fields and optional grouped layout.
pub fn resolve_form_fields(
	admin: &dyn ModelAdmin,
) -> AdminResult<(Vec<String>, Option<Vec<Fieldset>>)> {
	let fields = admin.fields();
	let fieldsets = admin.fieldsets();
	validate_fieldsets(fields.is_some(), fieldsets.as_deref())?;

	if let Some(fieldsets) = fieldsets {
		let fields = fieldsets
			.iter()
			.flat_map(|fieldset| fieldset.fields.iter().cloned())
			.collect();
		Ok((fields, Some(fieldsets)))
	} else {
		let fields = fields.unwrap_or_else(|| admin.list_display());
		Ok((fields.into_iter().map(String::from).collect(), None))
	}
}

fn validate_fieldsets(fields_configured: bool, fieldsets: Option<&[Fieldset]>) -> AdminResult<()> {
	if fields_configured && fieldsets.is_some() {
		return Err(AdminError::ValidationError(
			"fields and fieldsets cannot be configured together".to_string(),
		));
	}

	let Some(fieldsets) = fieldsets else {
		return Ok(());
	};
	if fieldsets.is_empty() {
		return Err(AdminError::ValidationError(
			"fieldsets cannot be empty".to_string(),
		));
	}
	let mut fields = HashSet::new();
	for fieldset in fieldsets {
		if fieldset.fields.is_empty() {
			return Err(AdminError::ValidationError(
				"fieldsets cannot contain empty groups".to_string(),
			));
		}
		for field in &fieldset.fields {
			if !fields.insert(field.as_str()) {
				return Err(AdminError::ValidationError(format!(
					"field '{field}' is repeated across fieldsets"
				)));
			}
		}
	}
	Ok(())
}

#[cfg(all(test, server))]
mod tests {
	use super::*;
	use crate::core::{AdminActionTransaction, AdminDatabase};
	use crate::types::{AdminActionOutcome, ModelPermission};
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
		// Arrange
		let admin = ModelAdminConfig::new("User");

		// Act
		let autocomplete_fields = admin.autocomplete_fields();
		let raw_id_fields = admin.raw_id_fields();

		// Assert
		assert_eq!(admin.model_name(), "User");
		assert_eq!(admin.list_display(), vec!["id"]);
		assert_eq!(admin.list_editable(), Vec::<&str>::new());
		assert_eq!(admin.list_filter(), Vec::<&str>::new());
		assert_eq!(autocomplete_fields, Vec::<&str>::new());
		assert_eq!(raw_id_fields, Vec::<&str>::new());
	}

	#[rstest]
	fn test_model_admin_relation_field_defaults() {
		// Arrange
		let admin = DefaultPermissionAdmin;
		let values = std::collections::HashMap::new();

		// Act
		let autocomplete_fields = admin.autocomplete_fields();
		let raw_id_fields = admin.raw_id_fields();
		let object_label = admin.object_label(&values);

		// Assert
		assert_eq!(autocomplete_fields, Vec::<&str>::new());
		assert_eq!(raw_id_fields, Vec::<&str>::new());
		assert_eq!(object_label, None);
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
			.list_editable(vec!["username"])
			.list_filter(vec!["is_active"])
			.search_fields(vec!["username", "email"])
			.list_per_page(50)
			.build()
			.unwrap();

		assert_eq!(admin.model_name(), "User");
		assert_eq!(admin.list_display(), vec!["id", "username", "email"]);
		assert_eq!(admin.list_editable(), vec!["username"]);
		assert_eq!(admin.list_filter(), vec!["is_active"]);
		assert_eq!(admin.search_fields(), vec!["username", "email"]);
		assert_eq!(admin.list_per_page(), Some(50));
	}

	#[rstest]
	fn test_model_admin_config_builder_stores_relation_fields() {
		// Arrange
		let admin = ModelAdminConfig::builder()
			.model_name("Article")
			.autocomplete_fields(vec!["author"])
			.raw_id_fields(vec!["category_id"])
			.build()
			.unwrap();

		// Act
		let autocomplete_fields = admin.autocomplete_fields();
		let raw_id_fields = admin.raw_id_fields();

		// Assert
		assert_eq!(autocomplete_fields, vec!["author"]);
		assert_eq!(raw_id_fields, vec!["category_id"]);
	}

	#[rstest]
	fn test_model_admin_config_builder_rejects_exact_relation_field_overlap() {
		// Arrange
		let builder = ModelAdminConfig::builder()
			.model_name("Article")
			.autocomplete_fields(vec!["author"])
			.raw_id_fields(vec!["author"]);

		// Act
		let result = builder.build();

		// Assert
		assert!(matches!(result, Err(AdminError::ValidationError(_))));
	}

	#[rstest]
	fn test_with_methods() {
		let admin = ModelAdminConfig::new("Post")
			.with_list_display(vec!["id", "title", "author"])
			.with_list_editable(vec!["title"])
			.with_list_filter(vec!["status", "created_at"])
			.with_search_fields(vec!["title", "content"]);

		assert_eq!(admin.list_display(), vec!["id", "title", "author"]);
		assert_eq!(admin.list_editable(), vec!["title"]);
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

	#[rstest]
	fn test_model_admin_default_fieldsets_is_none() {
		let admin = DefaultPermissionAdmin;

		assert_eq!(admin.fieldsets(), None);
	}

	#[rstest]
	fn test_builder_fieldsets_retain_title_order_and_collapsed_state() {
		let admin = ModelAdminConfig::builder()
			.model_name("Article")
			.fieldsets(vec![
				Fieldset::new(Some("Main"), &["title", "body"]),
				Fieldset::new(Some("Publishing"), &["published_at"]).collapsed(),
			])
			.build()
			.unwrap();

		let fieldsets = admin.fieldsets().unwrap();

		assert_eq!(fieldsets[0].title.as_deref(), Some("Main"));
		assert_eq!(fieldsets[0].fields, vec!["title", "body"]);
		assert!(!fieldsets[0].collapsed);
		assert_eq!(fieldsets[1].title.as_deref(), Some("Publishing"));
		assert_eq!(fieldsets[1].fields, vec!["published_at"]);
		assert!(fieldsets[1].collapsed);
	}

	#[rstest]
	fn test_resolve_form_fields_preserves_flat_fields() {
		let admin = ModelAdminConfig::builder()
			.model_name("Article")
			.fields(vec!["title", "body"])
			.build()
			.unwrap();

		let (fields, fieldsets) = resolve_form_fields(&admin).unwrap();

		assert_eq!(fields, vec!["title", "body"]);
		assert_eq!(fieldsets, None);
	}

	#[rstest]
	fn test_resolve_form_fields_flattens_fieldsets_in_declared_order() {
		let admin = ModelAdminConfig::builder()
			.model_name("Article")
			.fieldsets(vec![
				Fieldset::new(Some("Main"), &["title", "body"]),
				Fieldset::new(Some("Publishing"), &["published_at"]).collapsed(),
			])
			.build()
			.unwrap();

		let (fields, fieldsets) = resolve_form_fields(&admin).unwrap();

		assert_eq!(fields, vec!["title", "body", "published_at"]);
		assert_eq!(fieldsets.unwrap()[1].title.as_deref(), Some("Publishing"));
	}

	#[rstest]
	fn test_builder_rejects_fields_and_fieldsets_together() {
		let result = ModelAdminConfig::builder()
			.model_name("Article")
			.fields(vec!["title"])
			.fieldsets(vec![Fieldset::new(None, &["body"])])
			.build();

		assert!(matches!(result, Err(AdminError::ValidationError(_))));
	}

	#[rstest]
	fn test_builder_rejects_empty_fieldsets() {
		let result = ModelAdminConfig::builder()
			.model_name("Article")
			.fieldsets(vec![Fieldset::new(Some("Main"), &[])])
			.build();

		assert!(matches!(result, Err(AdminError::ValidationError(_))));
	}

	#[rstest]
	fn test_builder_rejects_empty_fieldset_collection() {
		let result = ModelAdminConfig::builder()
			.model_name("Article")
			.fieldsets(vec![])
			.build();

		assert!(matches!(result, Err(AdminError::ValidationError(_))));
	}

	#[rstest]
	fn test_builder_rejects_repeated_fieldset_fields() {
		let result = ModelAdminConfig::builder()
			.model_name("Article")
			.fieldsets(vec![
				Fieldset::new(Some("Main"), &["title"]),
				Fieldset::new(Some("Publishing"), &["title"]),
			])
			.build();

		assert!(matches!(result, Err(AdminError::ValidationError(_))));
	}

	#[rstest]
	fn test_resolve_form_fields_rejects_manual_fields_and_fieldsets_together() {
		struct InvalidAdmin;

		#[async_trait]
		impl ModelAdmin for InvalidAdmin {
			fn model_name(&self) -> &str {
				"Article"
			}

			fn fields(&self) -> Option<Vec<&str>> {
				Some(vec!["title"])
			}

			fn fieldsets(&self) -> Option<Vec<Fieldset>> {
				Some(vec![Fieldset::new(None, &["body"])])
			}
		}

		assert!(matches!(
			resolve_form_fields(&InvalidAdmin),
			Err(AdminError::ValidationError(_))
		));
	}

	#[rstest]
	fn test_resolve_form_fields_rejects_manual_empty_fieldset() {
		struct InvalidAdmin;

		#[async_trait]
		impl ModelAdmin for InvalidAdmin {
			fn model_name(&self) -> &str {
				"Article"
			}

			fn fieldsets(&self) -> Option<Vec<Fieldset>> {
				Some(vec![Fieldset::new(Some("Main"), &[])])
			}
		}

		assert!(matches!(
			resolve_form_fields(&InvalidAdmin),
			Err(AdminError::ValidationError(_))
		));
	}

	#[rstest]
	fn test_resolve_form_fields_rejects_manual_empty_fieldsets() {
		struct InvalidAdmin;

		#[async_trait]
		impl ModelAdmin for InvalidAdmin {
			fn model_name(&self) -> &str {
				"Article"
			}

			fn fieldsets(&self) -> Option<Vec<Fieldset>> {
				Some(vec![])
			}
		}

		assert!(matches!(
			resolve_form_fields(&InvalidAdmin),
			Err(AdminError::ValidationError(_))
		));
	}

	#[rstest]
	fn test_resolve_form_fields_rejects_manual_repeated_fieldset_fields() {
		struct InvalidAdmin;

		#[async_trait]
		impl ModelAdmin for InvalidAdmin {
			fn model_name(&self) -> &str {
				"Article"
			}

			fn fieldsets(&self) -> Option<Vec<Fieldset>> {
				Some(vec![
					Fieldset::new(Some("Main"), &["title"]),
					Fieldset::new(Some("Publishing"), &["title"]),
				])
			}
		}

		assert!(matches!(
			resolve_form_fields(&InvalidAdmin),
			Err(AdminError::ValidationError(_))
		));
	}

	/// Helper struct for testing default trait permission behavior
	struct DefaultPermissionAdmin;

	#[async_trait]
	impl ModelAdmin for DefaultPermissionAdmin {
		fn model_name(&self) -> &str {
			"TestModel"
		}
	}

	/// Helper struct for testing configured action metadata.
	struct ActionAdmin;

	#[async_trait]
	impl ModelAdmin for ActionAdmin {
		fn model_name(&self) -> &str {
			"ActionModel"
		}

		fn actions(&self) -> Vec<AdminAction> {
			vec![AdminAction::new(
				"publish",
				"Publish selected",
				ModelPermission::Change,
				true,
			)]
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

	#[rstest]
	fn test_default_actions_are_empty() {
		// Arrange
		let admin = DefaultPermissionAdmin;

		// Act
		let actions = admin.actions();

		// Assert
		assert_eq!(actions, Vec::<AdminAction>::new());
	}

	#[rstest]
	fn test_configured_action_metadata_is_preserved() {
		// Arrange
		let admin = ActionAdmin;

		// Act
		let actions = admin.actions();

		// Assert
		assert_eq!(actions.len(), 1);
		assert_eq!(actions[0].name, "publish");
		assert_eq!(actions[0].label, "Publish selected");
		assert_eq!(actions[0].permission, ModelPermission::Change);
		assert!(actions[0].requires_confirmation);
	}

	#[rstest]
	fn test_admin_action_outcome_preserves_successful_ids_and_affected_count() {
		// Arrange
		let successful_ids = vec!["7".to_string(), "11".to_string()];

		// Act
		let outcome = AdminActionOutcome::new(successful_ids.clone(), 3);

		// Assert
		assert_eq!(outcome.successful_ids, successful_ids);
		assert_eq!(outcome.affected, 3);
	}

	#[rstest]
	fn test_actions_dispatch_through_model_admin_trait_object() {
		// Arrange
		let admin: Arc<dyn ModelAdmin> = Arc::new(ActionAdmin);

		// Act
		let actions = admin.actions();

		// Assert
		assert_eq!(actions.len(), 1);
		assert_eq!(actions[0].name, "publish");
	}

	#[rstest]
	#[tokio::test]
	async fn test_execute_action_through_model_admin_trait_object_returns_validation_error() {
		// Arrange
		let admin: Arc<dyn ModelAdmin> = Arc::new(DefaultPermissionAdmin);
		let user = TestAdminUser::new();
		let owner = reinhardt_db::backends::DatabaseConnection::connect_sqlite("sqlite::memory:")
			.await
			.unwrap();
		let lease = reinhardt_db::orm::DatabaseConnectionLease::register(owner).unwrap();
		let db = AdminDatabase::new(lease.handle());
		let ids = vec!["1".to_string()];

		// Act
		let error = db
			.connection()
			.atomic_write(async |transaction| {
				let transaction: &mut AdminActionTransaction = transaction;
				Ok::<_, reinhardt_core::exception::Error>(
					admin
						.execute_action("publish", &ids, transaction, &user)
						.await
						.unwrap_err(),
				)
			})
			.await
			.unwrap();

		// Assert
		assert_eq!(
			error.to_string(),
			"Validation error: Invalid action: publish"
		);
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
