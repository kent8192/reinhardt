use crate::viewsets::actions::Action;
use crate::viewsets::filtering_support::{FilterConfig, FilterableViewSet, OrderingConfig};
use crate::viewsets::metadata::{ActionMetadata, get_actions_for_viewset};
use crate::viewsets::middleware::ViewSetMiddleware;
use crate::viewsets::pagination_support::{PaginatedViewSet, PaginationConfig};
use async_trait::async_trait;
use hyper::Method;
use reinhardt_http::{Request, Response, Result};
use std::collections::HashMap;
use std::sync::Arc;

/// Create a `MethodNotAllowed` error for the given HTTP method.
fn method_not_allowed(method: &Method) -> reinhardt_core::exception::Error {
	reinhardt_core::exception::Error::MethodNotAllowed(format!("Method {} not allowed", method))
}

/// ViewSet trait - similar to Django REST Framework's ViewSet
/// Uses composition of mixins instead of inheritance
#[async_trait]
pub trait ViewSet: Send + Sync {
	/// Get the basename for URL routing
	fn get_basename(&self) -> &str;

	/// Get the lookup field for detail routes
	/// Defaults to "id" if not overridden
	fn get_lookup_field(&self) -> &str {
		"id"
	}

	/// Dispatch request to appropriate action
	async fn dispatch(&self, request: Request, action: Action) -> Result<Response>;

	/// Dispatch request with dependency injection context
	///
	/// Get extra actions defined on this ViewSet
	/// Returns custom actions decorated with `#[action]` or manually registered
	fn get_extra_actions(&self) -> Vec<ActionMetadata> {
		let viewset_type = std::any::type_name::<Self>();

		// Try inventory-based registration first
		let mut actions = get_actions_for_viewset(viewset_type);

		// Also check manual registration
		let manual_actions = crate::viewsets::registry::get_registered_actions(viewset_type);
		actions.extend(manual_actions);

		actions
	}

	/// Get URL map for extra actions
	/// Returns empty map for uninitialized ViewSets
	fn get_extra_action_url_map(&self) -> HashMap<String, String> {
		HashMap::new()
	}

	/// Get current base URL (only available after initialization)
	fn get_current_base_url(&self) -> Option<String> {
		None
	}

	/// Reverse an action name to a URL
	fn reverse_action(&self, _action_name: &str, _args: &[&str]) -> Result<String> {
		Err(reinhardt_core::exception::Error::NotFound(
			"ViewSet not bound to router".to_string(),
		))
	}

	/// Get middleware for this ViewSet
	/// Returns None if no middleware is configured
	fn get_middleware(&self) -> Option<Arc<dyn ViewSetMiddleware>> {
		None
	}

	/// Check if login is required for this ViewSet
	fn requires_login(&self) -> bool {
		false
	}

	/// Get required permissions for this ViewSet
	fn get_required_permissions(&self) -> Vec<String> {
		Vec::new()
	}
}

/// Generic ViewSet implementation
/// Composes functionality through trait bounds
// Allow dead_code: generic container for composable ViewSet implementations via trait bounds
#[allow(dead_code)]
#[derive(Clone)]
pub struct GenericViewSet<T> {
	basename: String,
	handler: T,
}

impl<T: 'static> GenericViewSet<T> {
	/// Creates a new `GenericViewSet` with the given basename and handler.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_views::viewsets::{GenericViewSet, ViewSet};
	///
	/// let viewset = GenericViewSet::new("users", ());
	/// assert_eq!(viewset.get_basename(), "users");
	/// ```
	pub fn new(basename: impl Into<String>, handler: T) -> Self {
		Self {
			basename: basename.into(),
			handler,
		}
	}

	/// Convert ViewSet to Handler with action mapping
	/// Returns a ViewSetBuilder for configuration
	///
	/// # Examples
	///
	/// ```ignore
	/// use reinhardt_views::{viewset_actions, viewsets::GenericViewSet};
	/// use hyper::Method;
	///
	/// let viewset = GenericViewSet::new("users", ());
	/// let actions = viewset_actions!(GET => "list");
	/// let handler = viewset.as_view().with_actions(actions).build();
	/// ```
	pub fn as_view(self) -> crate::viewsets::builder::ViewSetBuilder<Self>
	where
		T: Send + Sync,
	{
		crate::viewsets::builder::ViewSetBuilder::new(self)
	}
}

#[async_trait]
impl<T: Send + Sync> ViewSet for GenericViewSet<T> {
	fn get_basename(&self) -> &str {
		&self.basename
	}

	async fn dispatch(&self, _request: Request, _action: Action) -> Result<Response> {
		// Default implementation delegates to mixins if available
		// This would be extended with actual mixin dispatch logic
		Err(reinhardt_core::exception::Error::NotFound(
			"Action not implemented".to_string(),
		))
	}
}

/// ModelViewSet - combines all CRUD mixins
/// Similar to Django REST Framework's ModelViewSet but using composition
pub struct ModelViewSet<M, S> {
	basename: String,
	lookup_field: String,
	pagination_config: Option<PaginationConfig>,
	filter_config: Option<FilterConfig>,
	ordering_config: Option<OrderingConfig>,
	_model: std::marker::PhantomData<M>,
	_serializer: std::marker::PhantomData<S>,
}

// Implement FilterableViewSet for ModelViewSet
impl<M, S> FilterableViewSet for ModelViewSet<M, S>
where
	M: Send + Sync,
	S: Send + Sync,
{
	fn get_filter_config(&self) -> Option<FilterConfig> {
		self.filter_config.clone()
	}

	fn get_ordering_config(&self) -> Option<OrderingConfig> {
		self.ordering_config.clone()
	}
}

impl<M: 'static, S: 'static> ModelViewSet<M, S> {
	/// Creates a new `ModelViewSet` with the given basename.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_views::viewsets::{ModelViewSet, ViewSet};
	/// use reinhardt_db::prelude::Model;
	/// use serde::{Serialize, Deserialize};
	///
	/// #[derive(Serialize, Deserialize, Clone, Debug)]
	/// struct User {
	///     id: Option<i64>,
	///     username: String,
	/// }
	///
	/// #[derive(Clone)]
	/// struct UserFields;
	///
	/// impl reinhardt_db::orm::FieldSelector for UserFields {
	///     fn with_alias(self, _alias: &str) -> Self { self }
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
	/// let viewset = ModelViewSet::<User, reinhardt_rest::serializers::JsonSerializer<User>>::new("users");
	/// assert_eq!(viewset.get_basename(), "users");
	/// ```
	pub fn new(basename: impl Into<String>) -> Self {
		Self {
			basename: basename.into(),
			lookup_field: "id".to_string(),
			pagination_config: Some(PaginationConfig::default()),
			filter_config: None,
			ordering_config: None,
			_model: std::marker::PhantomData,
			_serializer: std::marker::PhantomData,
		}
	}

	/// Set custom lookup field for this ViewSet
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_views::viewsets::{ModelViewSet, ViewSet};
	/// use reinhardt_db::prelude::Model;
	/// use serde::{Serialize, Deserialize};
	///
	/// #[derive(Serialize, Deserialize, Clone, Debug)]
	/// struct User {
	///     id: Option<i64>,
	///     username: String,
	/// }
	///
	/// #[derive(Clone)]
	/// struct UserFields;
	///
	/// impl reinhardt_db::orm::FieldSelector for UserFields {
	///     fn with_alias(self, _alias: &str) -> Self { self }
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
	/// let viewset = ModelViewSet::<User, ()>::new("users")
	///     .with_lookup_field("username");
	/// assert_eq!(viewset.get_lookup_field(), "username");
	/// ```
	pub fn with_lookup_field(mut self, field: impl Into<String>) -> Self {
		self.lookup_field = field.into();
		self
	}

	/// Set pagination configuration for this ViewSet
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_views::viewsets::{ModelViewSet, PaginationConfig};
	///
	/// // Page number pagination with custom page size
	/// let viewset = ModelViewSet::<(), ()>::new("items")
	///     .with_pagination(PaginationConfig::page_number(20, Some(100)));
	///
	/// // Limit/offset pagination
	/// let viewset = ModelViewSet::<(), ()>::new("items")
	///     .with_pagination(PaginationConfig::limit_offset(25, Some(500)));
	///
	/// // Disable pagination
	/// let viewset = ModelViewSet::<(), ()>::new("items")
	///     .with_pagination(PaginationConfig::none());
	/// ```
	pub fn with_pagination(mut self, config: PaginationConfig) -> Self {
		self.pagination_config = Some(config);
		self
	}

	/// Disable pagination for this ViewSet
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_views::viewsets::ModelViewSet;
	///
	/// let viewset = ModelViewSet::<(), ()>::new("items")
	///     .without_pagination();
	/// ```
	pub fn without_pagination(mut self) -> Self {
		self.pagination_config = None;
		self
	}

	/// Set filter configuration for this ViewSet
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_views::viewsets::{ModelViewSet, FilterConfig};
	///
	/// let viewset = ModelViewSet::<(), ()>::new("items")
	///     .with_filters(
	///         FilterConfig::new()
	///             .with_filterable_fields(vec!["status", "category"])
	///             .with_search_fields(vec!["title", "description"])
	///     );
	/// ```
	pub fn with_filters(mut self, config: FilterConfig) -> Self {
		self.filter_config = Some(config);
		self
	}

	/// Set ordering configuration for this ViewSet
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_views::viewsets::{ModelViewSet, OrderingConfig};
	///
	/// let viewset = ModelViewSet::<(), ()>::new("items")
	///     .with_ordering(
	///         OrderingConfig::new()
	///             .with_ordering_fields(vec!["created_at", "title", "id"])
	///             .with_default_ordering(vec!["-created_at"])
	///     );
	/// ```
	pub fn with_ordering(mut self, config: OrderingConfig) -> Self {
		self.ordering_config = Some(config);
		self
	}

	/// Convert ViewSet to Handler with action mapping
	/// Returns a ViewSetBuilder for configuration
	pub fn as_view(self) -> crate::viewsets::builder::ViewSetBuilder<Self>
	where
		M: Send + Sync,
		S: Send + Sync,
	{
		crate::viewsets::builder::ViewSetBuilder::new(self)
	}
}

#[async_trait]
impl<M, S> ViewSet for ModelViewSet<M, S>
where
	M: Send + Sync,
	S: Send + Sync,
{
	fn get_basename(&self) -> &str {
		&self.basename
	}

	fn get_lookup_field(&self) -> &str {
		&self.lookup_field
	}

	async fn dispatch(&self, request: Request, action: Action) -> Result<Response> {
		// Route to appropriate handler based on HTTP method and action
		match (request.method.clone(), action.detail) {
			(Method::GET, false) => {
				// List action
				self.handle_list(request).await
			}
			(Method::GET, true) => {
				// Retrieve action
				self.handle_retrieve(request).await
			}
			(Method::POST, false) => {
				// Create action
				self.handle_create(request).await
			}
			(Method::PUT, true) | (Method::PATCH, true) => {
				// Update action
				self.handle_update(request).await
			}
			(Method::DELETE, true) => {
				// Destroy action
				self.handle_destroy(request).await
			}
			_ => Err(method_not_allowed(&request.method)),
		}
	}
}

impl<M, S> ModelViewSet<M, S>
where
	M: Send + Sync,
	S: Send + Sync,
{
	async fn handle_list(&self, _request: Request) -> Result<Response> {
		// Implementation would query all objects and serialize them
		Response::ok()
			.with_json(&serde_json::json!([]))
			.map_err(|e| reinhardt_core::exception::Error::Http(e.to_string()))
	}

	async fn handle_retrieve(&self, _request: Request) -> Result<Response> {
		// Implementation would get object by ID and serialize it
		Response::ok()
			.with_json(&serde_json::json!({}))
			.map_err(|e| reinhardt_core::exception::Error::Http(e.to_string()))
	}

	async fn handle_create(&self, _request: Request) -> Result<Response> {
		// Implementation would deserialize, validate, and create object
		Response::created()
			.with_json(&serde_json::json!({}))
			.map_err(|e| reinhardt_core::exception::Error::Http(e.to_string()))
	}

	async fn handle_update(&self, _request: Request) -> Result<Response> {
		// Implementation would deserialize, validate, and update object
		Response::ok()
			.with_json(&serde_json::json!({}))
			.map_err(|e| reinhardt_core::exception::Error::Http(e.to_string()))
	}

	async fn handle_destroy(&self, _request: Request) -> Result<Response> {
		// Implementation would delete object
		Ok(Response::no_content())
	}
}

// Implement PaginatedViewSet for ModelViewSet
impl<M, S> PaginatedViewSet for ModelViewSet<M, S>
where
	M: Send + Sync,
	S: Send + Sync,
{
	fn get_pagination_config(&self) -> Option<PaginationConfig> {
		self.pagination_config.clone()
	}
}

/// ReadOnlyModelViewSet - only list and retrieve
/// Demonstrates selective composition of mixins
pub struct ReadOnlyModelViewSet<M, S> {
	basename: String,
	lookup_field: String,
	pagination_config: Option<PaginationConfig>,
	filter_config: Option<FilterConfig>,
	ordering_config: Option<OrderingConfig>,
	_model: std::marker::PhantomData<M>,
	_serializer: std::marker::PhantomData<S>,
}

impl<M: 'static, S: 'static> ReadOnlyModelViewSet<M, S> {
	/// Creates a new `ReadOnlyModelViewSet` with the given basename.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_views::viewsets::{ReadOnlyModelViewSet, ViewSet};
	/// use reinhardt_db::prelude::Model;
	/// use serde::{Serialize, Deserialize};
	///
	/// #[derive(Serialize, Deserialize, Clone, Debug)]
	/// struct User {
	///     id: Option<i64>,
	///     username: String,
	/// }
	///
	/// #[derive(Clone)]
	/// struct UserFields;
	///
	/// impl reinhardt_db::orm::FieldSelector for UserFields {
	///     fn with_alias(self, _alias: &str) -> Self { self }
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
	/// let viewset = ReadOnlyModelViewSet::<User, reinhardt_rest::serializers::JsonSerializer<User>>::new("users");
	/// assert_eq!(viewset.get_basename(), "users");
	/// ```
	pub fn new(basename: impl Into<String>) -> Self {
		Self {
			basename: basename.into(),
			lookup_field: "id".to_string(),
			pagination_config: Some(PaginationConfig::default()),
			filter_config: None,
			ordering_config: None,
			_model: std::marker::PhantomData,
			_serializer: std::marker::PhantomData,
		}
	}

	/// Set custom lookup field for this ViewSet
	pub fn with_lookup_field(mut self, field: impl Into<String>) -> Self {
		self.lookup_field = field.into();
		self
	}

	/// Set pagination configuration for this ViewSet
	pub fn with_pagination(mut self, config: PaginationConfig) -> Self {
		self.pagination_config = Some(config);
		self
	}

	/// Disable pagination for this ViewSet
	pub fn without_pagination(mut self) -> Self {
		self.pagination_config = None;
		self
	}

	/// Set filter configuration for this ViewSet
	///
	/// # Examples
	///
	/// ```ignore
	/// use reinhardt_views::viewsets::{ReadOnlyModelViewSet, FilterConfig};
	///
	/// let viewset = ReadOnlyModelViewSet::<MyModel, MySerializer>::new("items")
	///     .with_filters(
	///         FilterConfig::new()
	///             .with_filterable_fields(vec!["status", "category"])
	///             .with_search_fields(vec!["title", "description"])
	///     );
	/// ```
	pub fn with_filters(mut self, config: FilterConfig) -> Self {
		self.filter_config = Some(config);
		self
	}

	/// Set ordering configuration for this ViewSet
	///
	/// # Examples
	///
	/// ```ignore
	/// use reinhardt_views::viewsets::{ReadOnlyModelViewSet, OrderingConfig};
	///
	/// let viewset = ReadOnlyModelViewSet::<MyModel, MySerializer>::new("items")
	///     .with_ordering(
	///         OrderingConfig::new()
	///             .with_ordering_fields(vec!["created_at", "title"])
	///             .with_default_ordering(vec!["-created_at"])
	///     );
	/// ```
	pub fn with_ordering(mut self, config: OrderingConfig) -> Self {
		self.ordering_config = Some(config);
		self
	}

	/// Convert ViewSet to Handler with action mapping
	/// Returns a ViewSetBuilder for configuration
	pub fn as_view(self) -> crate::viewsets::builder::ViewSetBuilder<Self>
	where
		M: Send + Sync,
		S: Send + Sync,
	{
		crate::viewsets::builder::ViewSetBuilder::new(self)
	}
}

#[async_trait]
impl<M, S> ViewSet for ReadOnlyModelViewSet<M, S>
where
	M: Send + Sync,
	S: Send + Sync,
{
	fn get_basename(&self) -> &str {
		&self.basename
	}

	fn get_lookup_field(&self) -> &str {
		&self.lookup_field
	}

	async fn dispatch(&self, request: Request, action: Action) -> Result<Response> {
		match (request.method.clone(), action.detail) {
			(Method::GET, false) => {
				// List only
				Response::ok()
					.with_json(&serde_json::json!([]))
					.map_err(|e| reinhardt_core::exception::Error::Http(e.to_string()))
			}
			(Method::GET, true) => {
				// Retrieve only
				Response::ok()
					.with_json(&serde_json::json!({}))
					.map_err(|e| reinhardt_core::exception::Error::Http(e.to_string()))
			}
			_ => Err(method_not_allowed(&request.method)),
		}
	}
}

// Implement PaginatedViewSet for ReadOnlyModelViewSet
impl<M, S> PaginatedViewSet for ReadOnlyModelViewSet<M, S>
where
	M: Send + Sync,
	S: Send + Sync,
{
	fn get_pagination_config(&self) -> Option<PaginationConfig> {
		self.pagination_config.clone()
	}
}

// Implement FilterableViewSet for ReadOnlyModelViewSet
impl<M, S> FilterableViewSet for ReadOnlyModelViewSet<M, S>
where
	M: Send + Sync,
	S: Send + Sync,
{
	fn get_filter_config(&self) -> Option<FilterConfig> {
		self.filter_config.clone()
	}

	fn get_ordering_config(&self) -> Option<OrderingConfig> {
		self.ordering_config.clone()
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use hyper::Method;
	use std::collections::HashMap;
	use std::sync::Arc;

	#[tokio::test]
	async fn test_viewset_builder_validation_empty_actions() {
		let viewset = ModelViewSet::<(), ()>::new("test");
		let builder = viewset.as_view();

		// Test that empty actions causes build to fail
		let result = builder.build();
		assert!(result.is_err());

		// Check error message without unwrapping
		match result {
			Err(e) => assert!(
				e.to_string()
					.contains("The `actions` argument must be provided")
			),
			Ok(_) => panic!("Expected error but got success"),
		}
	}

	#[tokio::test]
	async fn test_viewset_builder_name_suffix_mutual_exclusivity() {
		let viewset = ModelViewSet::<(), ()>::new("test");
		let builder = viewset.as_view();

		// Test that providing both name and suffix fails
		let result = builder
			.with_name("test_name")
			.and_then(|b| b.with_suffix("test_suffix"));

		assert!(result.is_err());

		// Check error message without unwrapping
		match result {
			Err(e) => assert!(e.to_string().contains("received both `name` and `suffix`")),
			Ok(_) => panic!("Expected error but got success"),
		}
	}

	#[tokio::test]
	async fn test_viewset_builder_successful_build() {
		let viewset = ModelViewSet::<(), ()>::new("test");
		let mut actions = HashMap::new();
		actions.insert(Method::GET, "list".to_string());

		let builder = viewset.as_view();
		let result = builder.with_actions(actions).build();

		let handler = result.unwrap();

		// Test that handler is created successfully
		// Handler should be created without errors
		assert!(Arc::strong_count(&handler) > 0);
	}

	#[tokio::test]
	async fn test_viewset_builder_with_name() {
		let viewset = ModelViewSet::<(), ()>::new("test");
		let mut actions = HashMap::new();
		actions.insert(Method::GET, "list".to_string());

		let builder = viewset.as_view();
		let result = builder
			.with_actions(actions)
			.with_name("test_view")
			.and_then(|b| b.build());

		assert!(result.is_ok());
	}

	#[tokio::test]
	async fn test_viewset_builder_with_suffix() {
		let viewset = ModelViewSet::<(), ()>::new("test");
		let mut actions = HashMap::new();
		actions.insert(Method::GET, "list".to_string());

		let builder = viewset.as_view();
		let result = builder
			.with_actions(actions)
			.with_suffix("_list")
			.and_then(|b| b.build());

		assert!(result.is_ok());
	}
}
