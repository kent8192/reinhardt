use crate::viewsets::actions::Action;
use crate::viewsets::filtering_support::{FilterConfig, FilterableViewSet, OrderingConfig};
use crate::viewsets::handler::{ModelViewSetHandler, ViewError};
use crate::viewsets::metadata::{ActionMetadata, get_actions_for_viewset};
use crate::viewsets::middleware::{CompositeMiddleware, ViewSetMiddleware};
use crate::viewsets::pagination_support::{PaginatedViewSet, PaginationConfig};
use async_trait::async_trait;
use hyper::Method;
use reinhardt_auth::Permission;
use reinhardt_db::orm::{FilterCondition, Model, query_types::DbBackend};
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
	///
	/// The default implementation enforces [`Self::requires_login`] and
	/// [`Self::get_required_permissions`]. Implementations that override this
	/// method are responsible for composing those declarations into the returned
	/// middleware.
	fn get_middleware(&self) -> Option<Arc<dyn ViewSetMiddleware>> {
		let permissions = self.get_required_permissions();
		if !self.requires_login() && permissions.is_empty() {
			return None;
		}

		let mut middleware = CompositeMiddleware::new();
		if self.requires_login() {
			middleware = middleware.with_authentication(true);
		}
		if !permissions.is_empty() {
			middleware = middleware.with_permissions(permissions);
		}
		Some(Arc::new(middleware))
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
	S: Serializer<Input = M, Output = String> + Default + Send + Sync + 'static,
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
	S: Serializer<Input = M, Output = String> + Default + Send + Sync + 'static,
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
	S: Serializer<Input = M, Output = String> + Default + Send + Sync + 'static,
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
	///     type Objects = reinhardt_db::orm::Manager<Self>;
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
			handler: ModelViewSetHandler::<M>::new().with_serializer(Arc::new(S::default())),
			_serializer: PhantomData,
		}
	}

	/// Set the model field used by detail routes and object queries.
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
	///     type Objects = reinhardt_db::orm::Manager<Self>;
	///     fn table_name() -> &'static str { "users" }
	///     fn primary_key(&self) -> Option<Self::PrimaryKey> { self.id }
	///     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
	///     fn new_fields() -> Self::Fields { UserFields }
	/// }
	///
	/// let viewset = ModelViewSet::<User, reinhardt_rest::serializers::JsonSerializer<User>>::new("users")
	///     .with_lookup_field("username");
	/// assert_eq!(viewset.get_lookup_field(), "username");
	/// ```
	pub fn with_lookup_field(mut self, field: impl Into<String>) -> Self {
		self.lookup_field = field.into();
		self.handler =
			std::mem::take(&mut self.handler).with_lookup_field(self.lookup_field.clone());
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
	/// #     type Objects = reinhardt_db::orm::Manager<Self>;
	/// #     fn table_name() -> &'static str { "items" }
	/// #     fn primary_key(&self) -> Option<i64> { self.id }
	/// #     fn set_primary_key(&mut self, v: i64) { self.id = Some(v); }
	/// #     fn new_fields() -> Self::Fields { ItemFields }
	/// # }
	/// // Page number pagination with custom page size
	/// let viewset = ModelViewSet::<Item, reinhardt_rest::serializers::JsonSerializer<Item>>::new("items")
	///     .with_pagination(PaginationConfig::page_number(20, Some(100)));
	///
	/// // Limit/offset pagination
	/// let viewset = ModelViewSet::<Item, reinhardt_rest::serializers::JsonSerializer<Item>>::new("items")
	///     .with_pagination(PaginationConfig::limit_offset(25, Some(500)));
	///
	/// // Disable pagination
	/// let viewset = ModelViewSet::<Item, reinhardt_rest::serializers::JsonSerializer<Item>>::new("items")
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
	/// #     type Objects = reinhardt_db::orm::Manager<Self>;
	/// #     fn table_name() -> &'static str { "items" }
	/// #     fn primary_key(&self) -> Option<i64> { self.id }
	/// #     fn set_primary_key(&mut self, v: i64) { self.id = Some(v); }
	/// #     fn new_fields() -> Self::Fields { ItemFields }
	/// # }
	/// let viewset = ModelViewSet::<Item, reinhardt_rest::serializers::JsonSerializer<Item>>::new("items")
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
	/// #     type Objects = reinhardt_db::orm::Manager<Self>;
	/// #     fn table_name() -> &'static str { "items" }
	/// #     fn primary_key(&self) -> Option<i64> { self.id }
	/// #     fn set_primary_key(&mut self, v: i64) { self.id = Some(v); }
	/// #     fn new_fields() -> Self::Fields { ItemFields }
	/// # }
	/// let viewset = ModelViewSet::<Item, reinhardt_rest::serializers::JsonSerializer<Item>>::new("items")
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
	/// #     type Objects = reinhardt_db::orm::Manager<Self>;
	/// #     fn table_name() -> &'static str { "items" }
	/// #     fn primary_key(&self) -> Option<i64> { self.id }
	/// #     fn set_primary_key(&mut self, v: i64) { self.id = Some(v); }
	/// #     fn new_fields() -> Self::Fields { ItemFields }
	/// # }
	/// let viewset = ModelViewSet::<Item, reinhardt_rest::serializers::JsonSerializer<Item>>::new("items")
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

	/// Scope database queries using the current request.
	///
	/// The synchronous, fallible hook returns one [`FilterCondition`] and requires
	/// [`Self::with_pool`]. It scopes list, retrieve, update, and destroy, but not
	/// create; assign ownership during create in the serializer, permission layer,
	/// or database. Middleware must resolve asynchronous scope data before
	/// dispatch, and the hook reads application-defined request extensions.
	/// [`Self::with_queryset`] static `Vec` data is separate and is not filtered.
	/// Scoped-out objects and malformed detail lookup values produce 404.
	pub fn with_queryset_fn<F>(mut self, queryset_fn: F) -> Self
	where
		F: Fn(&Request) -> std::result::Result<FilterCondition, ViewError> + Send + Sync + 'static,
	{
		self.handler = std::mem::take(&mut self.handler).with_queryset_fn(queryset_fn);
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
	S: Serializer<Input = M, Output = String> + Default + Send + Sync + 'static,
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
	S: Serializer<Input = M, Output = String> + Default + Send + Sync + 'static,
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
	S: Serializer<Input = M, Output = String> + Default + Send + Sync + 'static,
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
	S: Serializer<Input = M, Output = String> + Default + Send + Sync + 'static,
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
	///     type Objects = reinhardt_db::orm::Manager<Self>;
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
			handler: ModelViewSetHandler::<M>::new().with_serializer(Arc::new(S::default())),
			_serializer: PhantomData,
		}
	}

	/// Set the model field used by detail routes and object queries.
	pub fn with_lookup_field(mut self, field: impl Into<String>) -> Self {
		self.lookup_field = field.into();
		self.handler =
			std::mem::take(&mut self.handler).with_lookup_field(self.lookup_field.clone());
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

	/// Scope database queries using the current request.
	///
	/// The synchronous, fallible hook returns one [`FilterCondition`] and requires
	/// [`Self::with_pool`]. It scopes list and retrieve. Middleware must resolve
	/// asynchronous scope data before dispatch, and the hook reads
	/// application-defined request extensions. [`Self::with_queryset`] static
	/// `Vec` data is separate and is not filtered. Scoped-out objects and malformed
	/// detail lookup values produce 404.
	pub fn with_queryset_fn<F>(mut self, queryset_fn: F) -> Self
	where
		F: Fn(&Request) -> std::result::Result<FilterCondition, ViewError> + Send + Sync + 'static,
	{
		self.handler = std::mem::take(&mut self.handler).with_queryset_fn(queryset_fn);
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
	S: Serializer<Input = M, Output = String> + Default + Send + Sync + 'static,
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
	S: Serializer<Input = M, Output = String> + Default + Send + Sync + 'static,
{
	fn get_pagination_config(&self) -> Option<PaginationConfig> {
		self.pagination_config.clone()
	}
}

// Implement FilterableViewSet for ReadOnlyModelViewSet
impl<M, S> FilterableViewSet for ReadOnlyModelViewSet<M, S>
where
	M: Model + Serialize + DeserializeOwned + Clone + Send + Sync + 'static,
	S: Serializer<Input = M, Output = String> + Default + Send + Sync + 'static,
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
	S: Serializer<Input = M, Output = String> + Default + Send + Sync + 'static,
{
}
impl<M, S> std::panic::RefUnwindSafe for ModelViewSet<M, S>
where
	M: Model + Serialize + DeserializeOwned + Clone + Send + Sync + 'static,
	S: Serializer<Input = M, Output = String> + Default + Send + Sync + 'static,
{
}

impl<M, S> std::panic::UnwindSafe for ReadOnlyModelViewSet<M, S>
where
	M: Model + Serialize + DeserializeOwned + Clone + Send + Sync + 'static,
	S: Serializer<Input = M, Output = String> + Default + Send + Sync + 'static,
{
}
impl<M, S> std::panic::RefUnwindSafe for ReadOnlyModelViewSet<M, S>
where
	M: Model + Serialize + DeserializeOwned + Clone + Send + Sync + 'static,
	S: Serializer<Input = M, Output = String> + Default + Send + Sync + 'static,
{
}

#[cfg(test)]
mod tests {
	use super::*;
	use hyper::Method;
	use reinhardt_db::orm::{FieldSelector, Filter, FilterOperator, Model};
	use serde::{Deserialize, Serialize};
	use std::collections::HashMap;
	use std::sync::Arc;

	/// Minimal `Model` implementation used to satisfy the `ModelViewSet` trait
	/// bounds in unit tests.
	#[derive(Debug, Clone, Serialize, Deserialize)]
	struct DummyModel {
		id: Option<i64>,
		secret: String,
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
		type Objects = reinhardt_db::orm::Manager<Self>;
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

	#[derive(Default)]
	struct RedactingDummySerializer;

	impl Serializer for RedactingDummySerializer {
		type Input = DummyModel;
		type Output = String;

		fn serialize(
			&self,
			input: &Self::Input,
		) -> std::result::Result<Self::Output, reinhardt_rest::serializers::SerializerError> {
			serde_json::to_string(&serde_json::json!({ "id": input.id })).map_err(|e| {
				reinhardt_rest::serializers::SerializerError::Serde {
					message: format!("Serialization error: {}", e),
				}
			})
		}

		fn deserialize(
			&self,
			output: &Self::Output,
		) -> std::result::Result<Self::Input, reinhardt_rest::serializers::SerializerError> {
			let value: serde_json::Value = serde_json::from_str(output).map_err(|e| {
				reinhardt_rest::serializers::SerializerError::Serde {
					message: format!("Deserialization error: {}", e),
				}
			})?;
			if value.get("secret").is_some() {
				return Err(reinhardt_rest::serializers::SerializerError::Serde {
					message: "secret is not writable".to_string(),
				});
			}
			Ok(DummyModel {
				id: value.get("id").and_then(serde_json::Value::as_i64),
				secret: String::new(),
			})
		}
	}

	#[test]
	fn queryset_fn_builders_preserve_viewset_object_safety() {
		let model: Arc<dyn ViewSet> = Arc::new(
			ModelViewSet::<DummyModel, RedactingDummySerializer>::new("test").with_queryset_fn(
				|_| Ok(Filter::new("organization_id", FilterOperator::Eq, 1_i64.into()).into()),
			),
		);
		let read_only: Arc<dyn ViewSet> = Arc::new(
			ReadOnlyModelViewSet::<DummyModel, RedactingDummySerializer>::new("test")
				.with_queryset_fn(|_| {
					Ok(Filter::new("organization_id", FilterOperator::Eq, 1_i64.into()).into())
				}),
		);

		assert_eq!(model.get_basename(), "test");
		assert_eq!(read_only.get_basename(), "test");
	}

	#[tokio::test]
	async fn test_model_viewset_new_wires_declared_serializer() {
		let viewset = ModelViewSet::<DummyModel, RedactingDummySerializer>::new("test")
			.with_queryset(vec![DummyModel {
				id: Some(7),
				secret: "hidden".to_string(),
			}]);
		let request = Request::builder()
			.method(Method::GET)
			.uri("/test/")
			.body(bytes::Bytes::new())
			.build()
			.unwrap();

		let response = viewset.dispatch(request, Action::list()).await.unwrap();

		assert_eq!(response.status, hyper::StatusCode::OK);
		assert_eq!(response.body, bytes::Bytes::from_static(br#"[{"id":7}]"#));
	}

	#[tokio::test]
	async fn test_viewset_builder_validation_empty_actions() {
		let viewset = ModelViewSet::<
			DummyModel,
			reinhardt_rest::serializers::JsonSerializer<DummyModel>,
		>::new("test");
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
		let viewset = ModelViewSet::<
			DummyModel,
			reinhardt_rest::serializers::JsonSerializer<DummyModel>,
		>::new("test");
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
		let viewset = ModelViewSet::<
			DummyModel,
			reinhardt_rest::serializers::JsonSerializer<DummyModel>,
		>::new("test");
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
		let viewset = ModelViewSet::<
			DummyModel,
			reinhardt_rest::serializers::JsonSerializer<DummyModel>,
		>::new("test");
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
		let viewset = ModelViewSet::<
			DummyModel,
			reinhardt_rest::serializers::JsonSerializer<DummyModel>,
		>::new("test");
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
