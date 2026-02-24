//! Django Admin Framework
//!
//! Auto-generated CRUD interface for models with:
//! - ModelAdmin configuration
//! - AdminSite registration
//! - List/Change/Add views
//! - Filters and search
//! - Bulk actions
//! - Permissions integration

use async_trait::async_trait;
use reinhardt_core::exception::{Error, Result};
use reinhardt_core::security::xss::escape_html;
use reinhardt_db::orm::Model;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::marker::PhantomData;

/// Error type for admin operations
#[derive(Debug, thiserror::Error)]
pub enum AdminError {
	#[error("Model not found: {0}")]
	ModelNotFound(String),

	#[error("Field not found: {0}")]
	FieldNotFound(String),

	#[error("Invalid filter: {0}")]
	InvalidFilter(String),

	#[error("Permission denied: {0}")]
	PermissionDenied(String),

	#[error("Query error: {0}")]
	QueryError(String),
}

impl From<AdminError> for Error {
	fn from(err: AdminError) -> Self {
		Error::Validation(err.to_string())
	}
}

/// Base trait for admin views
#[async_trait]
pub trait AdminView: Send + Sync {
	/// Render the admin view
	async fn render(&self) -> Result<String>;

	/// Check if the current user has permission to view this admin
	fn has_view_permission(&self) -> bool {
		true
	}

	/// Check if the current user has permission to add objects
	fn has_add_permission(&self) -> bool {
		true
	}

	/// Check if the current user has permission to change objects
	fn has_change_permission(&self) -> bool {
		true
	}

	/// Check if the current user has permission to delete objects
	fn has_delete_permission(&self) -> bool {
		true
	}
}

/// A registry for admin interfaces similar to Django's ModelAdmin.
///
/// This allows you to register models and customize how they appear
/// in admin interfaces.
///
/// # Examples
///
/// ```rust,no_run
/// use reinhardt_views::admin::ModelAdmin;
/// use reinhardt_db::orm::Model;
/// use serde::{Serialize, Deserialize};
///
/// #[derive(Debug, Clone, Serialize, Deserialize)]
/// struct Article {
///     id: Option<i64>,
///     title: String,
///     content: String,
/// }
///
/// #[derive(Clone)]
/// struct ArticleFields;
///
/// impl reinhardt_db::orm::FieldSelector for ArticleFields {
///     fn with_alias(self, _alias: &str) -> Self {
///         self
///     }
/// }
///
/// impl Model for Article {
///     type PrimaryKey = i64;
///     type Fields = ArticleFields;
///     fn table_name() -> &'static str { "articles" }
///     fn primary_key(&self) -> Option<Self::PrimaryKey> { self.id }
///     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
///     fn new_fields() -> Self::Fields { ArticleFields }
/// }
///
/// let admin = ModelAdmin::<Article>::new()
///     .with_list_display(vec!["id".to_string(), "title".to_string()])
///     .with_search_fields(vec!["title".to_string(), "content".to_string()]);
/// ```
pub struct ModelAdmin<M>
where
	M: Model + Serialize + for<'de> Deserialize<'de> + Send + Sync + Clone,
{
	list_display: Vec<String>,
	search_fields: Vec<String>,
	list_filter: Vec<String>,
	ordering: Vec<String>,
	list_per_page: usize,
	show_full_result_count: bool,
	readonly_fields: Vec<String>,
	queryset: Option<Vec<M>>,
	_phantom: PhantomData<M>,
}

impl<T: Model + Serialize + for<'de> Deserialize<'de> + Clone> ModelAdmin<T> {
	/// Creates a new ModelAdmin with default settings
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_views::admin::ModelAdmin;
	/// use reinhardt_db::orm::Model;
	/// use serde::{Serialize, Deserialize};
	///
	/// #[derive(Debug, Clone, Serialize, Deserialize)]
	/// struct User {
	///     id: Option<i64>,
	///     username: String,
	/// }
	///
	/// #[derive(Clone)]
	/// struct UserFields;
	///
	/// impl reinhardt_db::orm::FieldSelector for UserFields {
	///     fn with_alias(self, _alias: &str) -> Self {
	///         self
	///     }
	/// }
	///
	/// impl Model for User {
	///     type PrimaryKey = i64;
	///     type Fields = UserFields;
	///     fn table_name() -> &'static str { "users" }
	///     fn primary_key(&self) -> Option<Self::PrimaryKey> { self.id }
	///     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
	///     fn new_fields() -> Self::Fields { UserFields }
	/// }
	///
	/// let admin = ModelAdmin::<User>::new();
	/// assert_eq!(admin.list_per_page(), 100);
	/// ```
	pub fn new() -> Self {
		Self {
			list_display: vec![],
			list_filter: vec![],
			search_fields: vec![],
			ordering: vec![],
			list_per_page: 100,
			show_full_result_count: true,
			readonly_fields: vec![],
			queryset: None,
			_phantom: PhantomData,
		}
	}

	/// Sets the fields to display in the list view
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_views::admin::ModelAdmin;
	/// use reinhardt_db::orm::Model;
	/// use serde::{Serialize, Deserialize};
	///
	/// #[derive(Debug, Clone, Serialize, Deserialize)]
	/// struct Article {
	///     id: Option<i64>,
	///     title: String,
	/// }
	///
	/// #[derive(Clone)]
	/// struct ArticleFields;
	///
	/// impl reinhardt_db::orm::FieldSelector for ArticleFields {
	///     fn with_alias(self, _alias: &str) -> Self {
	///         self
	///     }
	/// }
	///
	/// impl Model for Article {
	///     type PrimaryKey = i64;
	///     type Fields = ArticleFields;
	///     fn table_name() -> &'static str { "articles" }
	///     fn primary_key(&self) -> Option<Self::PrimaryKey> { self.id }
	///     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
	///     fn new_fields() -> Self::Fields { ArticleFields }
	/// }
	///
	/// let admin = ModelAdmin::<Article>::new()
	///     .with_list_display(vec!["id".to_string(), "title".to_string()]);
	/// assert_eq!(admin.list_display().len(), 2);
	/// ```
	pub fn with_list_display(mut self, fields: Vec<String>) -> Self {
		self.list_display = fields;
		self
	}

	/// Sets the fields to filter by in the list view
	pub fn with_list_filter(mut self, fields: Vec<String>) -> Self {
		self.list_filter = fields;
		self
	}

	/// Sets the fields to search in
	pub fn with_search_fields(mut self, fields: Vec<String>) -> Self {
		self.search_fields = fields;
		self
	}

	/// Sets the ordering for the list view
	pub fn with_ordering(mut self, fields: Vec<String>) -> Self {
		self.ordering = fields;
		self
	}

	/// Sets the number of items per page
	pub fn with_list_per_page(mut self, count: usize) -> Self {
		self.list_per_page = count;
		self
	}

	/// Sets whether to show full result count
	pub fn with_show_full_result_count(mut self, show: bool) -> Self {
		self.show_full_result_count = show;
		self
	}

	/// Sets the readonly fields
	pub fn with_readonly_fields(mut self, fields: Vec<String>) -> Self {
		self.readonly_fields = fields;
		self
	}

	/// Sets a custom queryset for the admin
	pub fn with_queryset(mut self, queryset: Vec<T>) -> Self {
		self.queryset = Some(queryset);
		self
	}

	/// Gets the list of fields to display
	pub fn list_display(&self) -> &[String] {
		&self.list_display
	}

	/// Gets the list of filter fields
	pub fn list_filter(&self) -> &[String] {
		&self.list_filter
	}

	/// Gets the search fields
	pub fn search_fields(&self) -> &[String] {
		&self.search_fields
	}

	/// Gets the ordering fields
	pub fn ordering(&self) -> &[String] {
		&self.ordering
	}

	/// Gets the number of items per page
	pub fn list_per_page(&self) -> usize {
		self.list_per_page
	}

	/// Gets whether to show full result count
	pub fn show_full_result_count(&self) -> bool {
		self.show_full_result_count
	}

	/// Gets the readonly fields
	pub fn readonly_fields(&self) -> &[String] {
		&self.readonly_fields
	}

	/// Gets the queryset for this admin
	pub async fn get_queryset(&self) -> Result<Vec<T>> {
		match &self.queryset {
			Some(qs) => Ok(qs.clone()),
			None => Ok(Vec::new()),
		}
	}

	/// Renders the list view as HTML
	pub async fn render_list(&self) -> Result<String> {
		let objects = self.get_queryset().await?;
		let count = objects.len();

		let mut html = String::from("<div class=\"admin-list\">\n");
		html.push_str(&format!("<h2>{} List</h2>\n", escape_html(T::table_name())));
		html.push_str(&format!("<p>Total: {} items</p>\n", count));

		// Table header
		html.push_str("<table>\n<thead>\n<tr>\n");
		for field in &self.list_display {
			html.push_str(&format!("<th>{}</th>\n", escape_html(field)));
		}
		html.push_str("</tr>\n</thead>\n<tbody>\n");

		// Table rows
		for obj in objects {
			html.push_str("<tr>\n");
			let obj_json =
				serde_json::to_value(&obj).map_err(|e| Error::Serialization(e.to_string()))?;

			for field in &self.list_display {
				let value = obj_json
					.get(field)
					.map(|v| v.to_string())
					.unwrap_or_else(|| "-".to_string());
				// Escape user-controlled values to prevent XSS
				html.push_str(&format!("<td>{}</td>\n", escape_html(&value)));
			}
			html.push_str("</tr>\n");
		}

		html.push_str("</tbody>\n</table>\n</div>");

		Ok(html)
	}

	/// Searches the queryset based on search fields
	pub fn search(&self, query: &str, objects: Vec<T>) -> Vec<T> {
		if query.is_empty() || self.search_fields.is_empty() {
			return objects;
		}

		objects
			.into_iter()
			.filter(|obj| {
				let obj_json = serde_json::to_value(obj).ok();
				if let Some(json) = obj_json {
					self.search_fields.iter().any(|field| {
						json.get(field)
							.and_then(|v| v.as_str())
							.map(|s| s.to_lowercase().contains(&query.to_lowercase()))
							.unwrap_or(false)
					})
				} else {
					false
				}
			})
			.collect()
	}

	/// Filters the queryset based on filter criteria
	pub fn filter(&self, filters: &HashMap<String, String>, objects: Vec<T>) -> Vec<T> {
		if filters.is_empty() {
			return objects;
		}

		objects
			.into_iter()
			.filter(|obj| {
				let obj_json = serde_json::to_value(obj).ok();
				if let Some(json) = obj_json {
					filters.iter().all(|(field, value)| {
						json.get(field)
							.map(|v| {
								// Handle different value types
								match v {
									serde_json::Value::String(s) => s == value,
									serde_json::Value::Bool(b) => {
										value.to_lowercase() == b.to_string()
									}
									serde_json::Value::Number(n) => {
										// Create owned string once and compare
										let n_str = n.to_string();
										n_str == value.as_str()
									}
									_ => {
										// For other types, compare string representations
										if let Some(s) = v.as_str() {
											s == value.as_str()
										} else {
											// Create owned string once and compare with borrowed value
											let v_str = v.to_string();
											v_str == value.as_str()
										}
									}
								}
							})
							.unwrap_or(false)
					})
				} else {
					false
				}
			})
			.collect()
	}
}

#[async_trait]
impl<T: Model + Serialize + for<'de> Deserialize<'de> + Clone + Send + Sync> AdminView
	for ModelAdmin<T>
{
	async fn render(&self) -> Result<String> {
		self.render_list().await
	}
}

impl<T: Model + Serialize + for<'de> Deserialize<'de> + Clone> Default for ModelAdmin<T> {
	fn default() -> Self {
		Self::new()
	}
}
