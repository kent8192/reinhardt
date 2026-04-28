use crate::viewsets::actions::Action;
use crate::viewsets::filtering_support::{FilterConfig, FilterableViewSet, OrderingConfig};
use crate::viewsets::handler::ModelViewSetHandler;
use crate::viewsets::metadata::{ActionMetadata, get_actions_for_viewset};
use crate::viewsets::middleware::ViewSetMiddleware;
use crate::viewsets::pagination_support::{PaginatedViewSet, PaginationConfig};
use async_trait::async_trait;
use hyper::Method;
use reinhardt_auth::Permission;
use reinhardt_db::orm::{Model, query_types::DbBackend};
use reinhardt_http::{Request, Response, Result};
use reinhardt_rest::filters::FilterBackend;
use reinhardt_rest::serializers::Serializer;
use serde::Serialize;
use serde::de::DeserializeOwned;
use std::collections::HashMap;
use std::marker::PhantomData;
use std::sync::Arc;

/// Extract the primary key value from request path parameters by lookup field
/// name. Returns a JSON string value suitable for `ModelViewSetHandler` methods.
fn extract_pk(request: &Request, lookup_field: &str) -> Result<serde_json::Value> {
	request
		.path_params
		.get(lookup_field)
		.map(|v| serde_json::Value::String(v.clone()))
		.ok_or_else(|| {
			reinhardt_core::exception::Error::Http(format!(
				"Missing path parameter: {}",
				lookup_field
			))
		})
}

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

/// Generic ViewSet without built-in CRUD logic.
///
/// `GenericViewSet<T>` is an extensibility hook for users who want to build a
/// `ViewSet` from scratch with their own dispatch logic. It does **not**
/// perform any CRUD by itself; calling `dispatch()` on a bare `GenericViewSet`
/// always returns a `NotFound` error with guidance pointing to the correct
/// abstractions.
///
/// # Choosing the right ViewSet
///
/// - For automatic CRUD against a database `Model`, use [`ModelViewSet`].
/// - For read-only access (list + retrieve only), use [`ReadOnlyModelViewSet`].
/// - For fully custom behavior, define your own type and `impl ViewSet for YourType`
///   with a hand-written `dispatch()`. `GenericViewSet` is rarely the right choice.
///
/// # Example: composing a custom ViewSet via the builder
///
/// ```
/// use reinhardt_views::viewsets::{GenericViewSet, ViewSet};
///
/// let viewset = GenericViewSet::new("widgets", ());
/// assert_eq!(viewset.get_basename(), "widgets");
/// ```
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

	async fn dispatch(&self, _request: Request, action: Action) -> Result<Response> {
		// `GenericViewSet` carries no built-in CRUD logic on purpose. Users who
		// reach this point typically need one of the concrete ViewSets that *do*
		// implement CRUD, or a hand-written `impl ViewSet` on their own type.
		// Returning a guidance-rich error avoids silent placeholder responses
		// (the regression class behind issue #3985).
		Err(reinhardt_core::exception::Error::NotFound(format!(
			"GenericViewSet has no built-in CRUD logic for action {:?}. \
			 For real CRUD, use ModelViewSet<M, S> or ReadOnlyModelViewSet<M, S>. \
			 To implement custom logic, define your own struct and \
			 `impl ViewSet for YourType` with a hand-written dispatch().",
			action.action_type
		)))
	}
}

/// `ModelViewSet` - combines all CRUD mixins, backed by a real
/// [`ModelViewSetHandler`] for database-backed CRUD.
///
/// Similar to Django REST Framework's `ModelViewSet` but built around Rust
/// type composition. `dispatch()` routes the standard REST verbs to the
/// embedded handler's `list` / `retrieve` / `create` / `update` / `destroy`
/// methods, so registering a `ModelViewSet` with a router yields actual
/// model-backed responses (not placeholders).
pub struct ModelViewSet<M, S>
where
	M: Model + Serialize + DeserializeOwned + Clone + Send + Sync + 'static,
	S: Send + Sync + 'static,
{
	basename: String,
	lookup_field: String,
	pagination_config: Option<PaginationConfig>,
	filter_config: Option<FilterConfig>,
	ordering_config: Option<OrderingConfig>,
	handler: ModelViewSetHandler<M>,
	_serializer: PhantomData<S>,
}

// Implement FilterableViewSet for ModelViewSet
impl<M, S> FilterableViewSet for ModelViewSet<M, S>
where
	M: Model + Serialize + DeserializeOwned + Clone + Send + Sync + 'static,
	S: Send + Sync + 'static,
{
	fn get_filter_config(&self) -> Option<FilterConfig> {
		self.filter_config.clone()
	}

	fn get_ordering_config(&self) -> Option<OrderingConfig> {
		self.ordering_config.clone()
	}
}

impl<M, S> ModelViewSet<M, S>
where
	M: Model + Serialize + DeserializeOwned + Clone + Send + Sync + 'static,
	S: Send + Sync + 'static,
{
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
			handler: ModelViewSetHandler::<M>::new(),
			_serializer: PhantomData,
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
	/// # use reinhardt_views::viewsets::{ModelViewSet, PaginationConfig};
	/// # use reinhardt_db::orm::{FieldSelector, Model};
	/// # use serde::{Deserialize, Serialize};
	/// # #[derive(Clone, Serialize, Deserialize)]
	/// # struct Item { id: Option<i64> }
	/// # #[derive(Clone)] struct ItemFields;
	/// # impl FieldSelector for ItemFields { fn with_alias(self, _: &str) -> Self { self } }
	/// # impl Model for Item {
	/// #     type PrimaryKey = i64; type Fields = ItemFields;
	/// #     fn table_name() -> &'static str { "items" }
	/// #     fn primary_key(&self) -> Option<i64> { self.id }
	/// #     fn set_primary_key(&mut self, v: i64) { self.id = Some(v); }
	/// #     fn new_fields() -> Self::Fields { ItemFields }
	/// # }
	/// // Page number pagination with custom page size
	/// let viewset = ModelViewSet::<Item, ()>::new("items")
	///     .with_pagination(PaginationConfig::page_number(20, Some(100)));
	///
	/// // Limit/offset pagination
	/// let viewset = ModelViewSet::<Item, ()>::new("items")
	///     .with_pagination(PaginationConfig::limit_offset(25, Some(500)));
	///
	/// // Disable pagination
	/// let viewset = ModelViewSet::<Item, ()>::new("items")
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
	/// # use reinhardt_views::viewsets::ModelViewSet;
	/// # use reinhardt_db::orm::{FieldSelector, Model};
	/// # use serde::{Deserialize, Serialize};
	/// # #[derive(Clone, Serialize, Deserialize)]
	/// # struct Item { id: Option<i64> }
	/// # #[derive(Clone)] struct ItemFields;
	/// # impl FieldSelector for ItemFields { fn with_alias(self, _: &str) -> Self { self } }
	/// # impl Model for Item {
	/// #     type PrimaryKey = i64; type Fields = ItemFields;
	/// #     fn table_name() -> &'static str { "items" }
	/// #     fn primary_key(&self) -> Option<i64> { self.id }
	/// #     fn set_primary_key(&mut self, v: i64) { self.id = Some(v); }
	/// #     fn new_fields() -> Self::Fields { ItemFields }
	/// # }
	/// let viewset = ModelViewSet::<Item, ()>::new("items")
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
	/// # use reinhardt_views::viewsets::{ModelViewSet, FilterConfig};
	/// # use reinhardt_db::orm::{FieldSelector, Model};
	/// # use serde::{Deserialize, Serialize};
	/// # #[derive(Clone, Serialize, Deserialize)]
	/// # struct Item { id: Option<i64> }
	/// # #[derive(Clone)] struct ItemFields;
	/// # impl FieldSelector for ItemFields { fn with_alias(self, _: &str) -> Self { self } }
	/// # impl Model for Item {
	/// #     type PrimaryKey = i64; type Fields = ItemFields;
	/// #     fn table_name() -> &'static str { "items" }
	/// #     fn primary_key(&self) -> Option<i64> { self.id }
	/// #     fn set_primary_key(&mut self, v: i64) { self.id = Some(v); }
	/// #     fn new_fields() -> Self::Fields { ItemFields }
	/// # }
	/// let viewset = ModelViewSet::<Item, ()>::new("items")
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
	/// # use reinhardt_views::viewsets::{ModelViewSet, OrderingConfig};
	/// # use reinhardt_db::orm::{FieldSelector, Model};
	/// # use serde::{Deserialize, Serialize};
	/// # #[derive(Clone, Serialize, Deserialize)]
	/// # struct Item { id: Option<i64> }
	/// # #[derive(Clone)] struct ItemFields;
	/// # impl FieldSelector for ItemFields { fn with_alias(self, _: &str) -> Self { self } }
	/// # impl Model for Item {
	/// #     type PrimaryKey = i64; type Fields = ItemFields;
	/// #     fn table_name() -> &'static str { "items" }
	/// #     fn primary_key(&self) -> Option<i64> { self.id }
	/// #     fn set_primary_key(&mut self, v: i64) { self.id = Some(v); }
	/// #     fn new_fields() -> Self::Fields { ItemFields }
	/// # }
	/// let viewset = ModelViewSet::<Item, ()>::new("items")
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

	/// Set the database connection pool used by CRUD handlers.
	///
	/// Without a pool, list/retrieve fall back to the in-memory queryset (if
	/// any), and create/update/destroy will operate only on the queryset.
	pub fn with_pool(mut self, pool: Arc<sqlx::AnyPool>) -> Self {
		self.handler = std::mem::take(&mut self.handler).with_pool(pool);
		self
	}

	/// Set the database backend type (PostgreSQL, MySQL, SQLite).
	pub fn with_db_backend(mut self, backend: DbBackend) -> Self {
		self.handler = std::mem::take(&mut self.handler).with_db_backend(backend);
		self
	}

	/// Set a custom serializer used by CRUD handlers.
	pub fn with_serializer(
		mut self,
		serializer: Arc<dyn Serializer<Input = M, Output = String> + Send + Sync>,
	) -> Self {
		self.handler = std::mem::take(&mut self.handler).with_serializer(serializer);
		self
	}

	/// Provide an in-memory queryset used when no database pool is set.
	pub fn with_queryset(mut self, items: Vec<M>) -> Self {
		self.handler = std::mem::take(&mut self.handler).with_queryset(items);
		self
	}

	/// Add a permission class enforced before each request.
	pub fn add_permission(mut self, permission: Arc<dyn Permission>) -> Self {
		self.handler = std::mem::take(&mut self.handler).add_permission(permission);
		self
	}

	/// Add a filter backend applied to list requests.
	pub fn add_filter_backend(mut self, backend: Arc<dyn FilterBackend>) -> Self {
		self.handler = std::mem::take(&mut self.handler).add_filter_backend(backend);
		self
	}

	/// Convert ViewSet to Handler with action mapping
	/// Returns a ViewSetBuilder for configuration
	pub fn as_view(self) -> crate::viewsets::builder::ViewSetBuilder<Self> {
		crate::viewsets::builder::ViewSetBuilder::new(self)
	}
}

#[async_trait]
impl<M, S> ViewSet for ModelViewSet<M, S>
where
	M: Model + Serialize + DeserializeOwned + Clone + Send + Sync + 'static,
	S: Send + Sync + 'static,
{
	fn get_basename(&self) -> &str {
		&self.basename
	}

	fn get_lookup_field(&self) -> &str {
		&self.lookup_field
	}

	async fn dispatch(&self, request: Request, action: Action) -> Result<Response> {
		// Route to the embedded `ModelViewSetHandler<M>` for real CRUD.
		// Path params have already been populated by the router using the
		// `lookup_field` placeholder, e.g. `/items/{id}/`.
		match (request.method.clone(), action.detail) {
			(Method::GET, false) => self.handler.list(&request).await.map_err(Into::into),
			(Method::POST, false) => self.handler.create(&request).await.map_err(Into::into),
			(Method::GET, true) => {
				let pk = extract_pk(&request, &self.lookup_field)?;
				self.handler
					.retrieve(&request, pk)
					.await
					.map_err(Into::into)
			}
			(Method::PUT, true) | (Method::PATCH, true) => {
				let pk = extract_pk(&request, &self.lookup_field)?;
				self.handler.update(&request, pk).await.map_err(Into::into)
			}
			(Method::DELETE, true) => {
				let pk = extract_pk(&request, &self.lookup_field)?;
				self.handler.destroy(&request, pk).await.map_err(Into::into)
			}
			_ => Err(method_not_allowed(&request.method)),
		}
	}
}

// Implement PaginatedViewSet for ModelViewSet
impl<M, S> PaginatedViewSet for ModelViewSet<M, S>
where
	M: Model + Serialize + DeserializeOwned + Clone + Send + Sync + 'static,
	S: Send + Sync + 'static,
{
	fn get_pagination_config(&self) -> Option<PaginationConfig> {
		self.pagination_config.clone()
	}
}

/// `ReadOnlyModelViewSet` - exposes only `list` and `retrieve` against a real
/// [`ModelViewSetHandler`].
///
/// Other HTTP verbs (POST/PUT/PATCH/DELETE) return `MethodNotAllowed`.
pub struct ReadOnlyModelViewSet<M, S>
where
	M: Model + Serialize + DeserializeOwned + Clone + Send + Sync + 'static,
	S: Send + Sync + 'static,
{
	basename: String,
	lookup_field: String,
	pagination_config: Option<PaginationConfig>,
	filter_config: Option<FilterConfig>,
	ordering_config: Option<OrderingConfig>,
	handler: ModelViewSetHandler<M>,
	_serializer: PhantomData<S>,
}

impl<M, S> ReadOnlyModelViewSet<M, S>
where
	M: Model + Serialize + DeserializeOwned + Clone + Send + Sync + 'static,
	S: Send + Sync + 'static,
{
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
			handler: ModelViewSetHandler::<M>::new(),
			_serializer: PhantomData,
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

	/// Set the database connection pool used by read handlers.
	pub fn with_pool(mut self, pool: Arc<sqlx::AnyPool>) -> Self {
		self.handler = std::mem::take(&mut self.handler).with_pool(pool);
		self
	}

	/// Set the database backend type (PostgreSQL, MySQL, SQLite).
	pub fn with_db_backend(mut self, backend: DbBackend) -> Self {
		self.handler = std::mem::take(&mut self.handler).with_db_backend(backend);
		self
	}

	/// Set a custom serializer used by read handlers.
	pub fn with_serializer(
		mut self,
		serializer: Arc<dyn Serializer<Input = M, Output = String> + Send + Sync>,
	) -> Self {
		self.handler = std::mem::take(&mut self.handler).with_serializer(serializer);
		self
	}

	/// Provide an in-memory queryset used when no database pool is set.
	pub fn with_queryset(mut self, items: Vec<M>) -> Self {
		self.handler = std::mem::take(&mut self.handler).with_queryset(items);
		self
	}

	/// Add a permission class enforced before each request.
	pub fn add_permission(mut self, permission: Arc<dyn Permission>) -> Self {
		self.handler = std::mem::take(&mut self.handler).add_permission(permission);
		self
	}

	/// Add a filter backend applied to list requests.
	pub fn add_filter_backend(mut self, backend: Arc<dyn FilterBackend>) -> Self {
		self.handler = std::mem::take(&mut self.handler).add_filter_backend(backend);
		self
	}

	/// Convert ViewSet to Handler with action mapping
	/// Returns a ViewSetBuilder for configuration
	pub fn as_view(self) -> crate::viewsets::builder::ViewSetBuilder<Self> {
		crate::viewsets::builder::ViewSetBuilder::new(self)
	}
}

#[async_trait]
impl<M, S> ViewSet for ReadOnlyModelViewSet<M, S>
where
	M: Model + Serialize + DeserializeOwned + Clone + Send + Sync + 'static,
	S: Send + Sync + 'static,
{
	fn get_basename(&self) -> &str {
		&self.basename
	}

	fn get_lookup_field(&self) -> &str {
		&self.lookup_field
	}

	async fn dispatch(&self, request: Request, action: Action) -> Result<Response> {
		match (request.method.clone(), action.detail) {
			(Method::GET, false) => self.handler.list(&request).await.map_err(Into::into),
			(Method::GET, true) => {
				let pk = extract_pk(&request, &self.lookup_field)?;
				self.handler
					.retrieve(&request, pk)
					.await
					.map_err(Into::into)
			}
			_ => Err(method_not_allowed(&request.method)),
		}
	}
}

// Implement PaginatedViewSet for ReadOnlyModelViewSet
impl<M, S> PaginatedViewSet for ReadOnlyModelViewSet<M, S>
where
	M: Model + Serialize + DeserializeOwned + Clone + Send + Sync + 'static,
	S: Send + Sync + 'static,
{
	fn get_pagination_config(&self) -> Option<PaginationConfig> {
		self.pagination_config.clone()
	}
}

// Implement FilterableViewSet for ReadOnlyModelViewSet
impl<M, S> FilterableViewSet for ReadOnlyModelViewSet<M, S>
where
	M: Model + Serialize + DeserializeOwned + Clone + Send + Sync + 'static,
	S: Send + Sync + 'static,
{
	fn get_filter_config(&self) -> Option<FilterConfig> {
		self.filter_config.clone()
	}

	fn get_ordering_config(&self) -> Option<OrderingConfig> {
		self.ordering_config.clone()
	}
}

// Manually re-assert the `UnwindSafe` / `RefUnwindSafe` auto traits for the
// public viewset structs. The new `Arc<dyn Serializer ...>` / `Arc<dyn
// Permission>` / `Arc<dyn FilterBackend>` fields introduced by this PR do
// not propagate these markers because trait objects do not implement them
// by default, which would otherwise surface as cargo-semver-checks
// `auto_trait_impl_removed` under the RC phase's no-breaking-change policy.
// The trait objects are only accessed via `&self` / `Arc::clone`, and the
// `Send + Sync` supertraits already guarantee thread safety, so manually
// re-implementing the markers preserves the pre-PR public-API contract.
impl<M, S> std::panic::UnwindSafe for ModelViewSet<M, S>
where
	M: Model + Serialize + DeserializeOwned + Clone + Send + Sync + 'static,
	S: Send + Sync + 'static,
{
}
impl<M, S> std::panic::RefUnwindSafe for ModelViewSet<M, S>
where
	M: Model + Serialize + DeserializeOwned + Clone + Send + Sync + 'static,
	S: Send + Sync + 'static,
{
}

impl<M, S> std::panic::UnwindSafe for ReadOnlyModelViewSet<M, S>
where
	M: Model + Serialize + DeserializeOwned + Clone + Send + Sync + 'static,
	S: Send + Sync + 'static,
{
}
impl<M, S> std::panic::RefUnwindSafe for ReadOnlyModelViewSet<M, S>
where
	M: Model + Serialize + DeserializeOwned + Clone + Send + Sync + 'static,
	S: Send + Sync + 'static,
{
}

#[cfg(test)]
mod tests {
	use super::*;
	use hyper::Method;
	use reinhardt_db::orm::{FieldSelector, Model};
	use serde::{Deserialize, Serialize};
	use std::collections::HashMap;
	use std::sync::Arc;

	/// Minimal `Model` implementation used to satisfy the `ModelViewSet` trait
	/// bounds in unit tests. The previous tests used `ModelViewSet::<(), ()>`,
	/// but bare `()` does not implement `Model` once the bounds were tightened.
	#[derive(Debug, Clone, Serialize, Deserialize)]
	struct DummyModel {
		id: Option<i64>,
	}

	#[derive(Clone)]
	struct DummyFields;

	impl FieldSelector for DummyFields {
		fn with_alias(self, _alias: &str) -> Self {
			self
		}
	}

	impl Model for DummyModel {
		type PrimaryKey = i64;
		type Fields = DummyFields;
		fn table_name() -> &'static str {
			"dummy"
		}
		fn primary_key(&self) -> Option<Self::PrimaryKey> {
			self.id
		}
		fn set_primary_key(&mut self, value: Self::PrimaryKey) {
			self.id = Some(value);
		}
		fn new_fields() -> Self::Fields {
			DummyFields
		}
	}

	#[tokio::test]
	async fn test_viewset_builder_validation_empty_actions() {
		let viewset = ModelViewSet::<DummyModel, ()>::new("test");
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
		let viewset = ModelViewSet::<DummyModel, ()>::new("test");
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
		let viewset = ModelViewSet::<DummyModel, ()>::new("test");
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
		let viewset = ModelViewSet::<DummyModel, ()>::new("test");
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
		let viewset = ModelViewSet::<DummyModel, ()>::new("test");
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
