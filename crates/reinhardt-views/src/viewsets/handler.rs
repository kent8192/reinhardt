/// ViewSetHandler - wraps a ViewSet as a Handler
use crate::{Action, ViewSet};
use async_trait::async_trait;
use hyper::Method;
use parking_lot::RwLock;
use reinhardt_auth::{Permission, PermissionContext};
use reinhardt_db::orm::{Model, query_types::DbBackend};
use reinhardt_http::{Handler, Request, Response, Result};
use reinhardt_rest::filters::FilterBackend;
use reinhardt_rest::serializers::{ModelSerializer, Serializer};
use serde::Serialize;
use serde::de::DeserializeOwned;
use std::collections::HashMap;
use std::marker::PhantomData;
use std::sync::Arc;

/// Handler implementation that wraps a ViewSet
pub struct ViewSetHandler<V: ViewSet> {
	viewset: Arc<V>,
	action_map: HashMap<Method, String>,
	#[allow(dead_code)]
	name: Option<String>,
	#[allow(dead_code)]
	suffix: Option<String>,

	// Attributes set after as_view() is called
	// These mirror Django REST Framework's behavior
	args: RwLock<Option<Vec<String>>>,
	kwargs: RwLock<Option<HashMap<String, String>>>,
	has_handled_request: RwLock<bool>,
}

// parking_lot::RwLock does not use poisoning, so ViewSetHandler
// remains safe to use across unwind boundaries.
impl<V: ViewSet> std::panic::RefUnwindSafe for ViewSetHandler<V> {}

impl<V: ViewSet> ViewSetHandler<V> {
	pub fn new(
		viewset: Arc<V>,
		action_map: HashMap<Method, String>,
		name: Option<String>,
		suffix: Option<String>,
	) -> Self {
		Self {
			viewset,
			action_map,
			name,
			suffix,
			args: RwLock::new(None),
			kwargs: RwLock::new(None),
			has_handled_request: RwLock::new(false),
		}
	}

	/// Check if args attribute is set (for testing)
	pub fn has_args(&self) -> bool {
		self.args.read().is_some()
	}

	/// Check if kwargs attribute is set (for testing)
	pub fn has_kwargs(&self) -> bool {
		self.kwargs.read().is_some()
	}

	/// Check if request attribute is set (for testing)
	pub fn has_request(&self) -> bool {
		*self.has_handled_request.read()
	}

	/// Check if action_map is set (for testing)
	pub fn has_action_map(&self) -> bool {
		!self.action_map.is_empty()
	}
}

#[async_trait]
impl<V: ViewSet + 'static> Handler for ViewSetHandler<V> {
	async fn handle(&self, mut request: Request) -> Result<Response> {
		// Set attributes when handling request (DRF behavior)
		*self.has_handled_request.write() = true;
		*self.args.write() = Some(Vec::new());

		// Extract path parameters from URI
		let kwargs = extract_path_params(&request);
		*self.kwargs.write() = Some(kwargs);

		// Process middleware before ViewSet
		if let Some(middleware) = self.viewset.get_middleware()
			&& let Some(response) = middleware.process_request(&mut request).await?
		{
			return Ok(response);
		}

		// Resolve action from HTTP method
		let action_name = self.action_map.get(&request.method).ok_or_else(|| {
			reinhardt_core::exception::Error::Http(format!("Method {} not allowed", request.method))
		})?;

		// Create Action from name
		let action = Action::from_name(action_name);

		// Dispatch to ViewSet
		let response = self.viewset.dispatch(request, action).await?;

		// Process middleware after ViewSet
		Ok(response)
	}
}

/// Extract path parameters from request
/// Simple implementation - in production would use router's path matching
fn extract_path_params(request: &Request) -> HashMap<String, String> {
	let mut params = HashMap::new();

	// Simple extraction: if path has pattern like /resource/123/
	// extract "123" as the "id" parameter
	let path = request.uri.path();
	let segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();

	// If we have at least 2 segments, assume second is an ID
	if segments.len() >= 2 {
		// Check if second segment looks like a numeric ID
		if segments[1].parse::<i64>().is_ok() || !segments[1].is_empty() {
			params.insert("id".to_string(), segments[1].to_string());
		}
	}

	params
}

/// Error type for ModelViewSetHandler
#[derive(Debug)]
pub enum ViewError {
	Serialization(String),
	Permission(String),
	NotFound(String),
	BadRequest(String),
	Internal(String),
	DatabaseError(String),
}

impl std::fmt::Display for ViewError {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			ViewError::Serialization(msg) => write!(f, "Serialization error: {}", msg),
			ViewError::Permission(msg) => write!(f, "Permission denied: {}", msg),
			ViewError::NotFound(msg) => write!(f, "Not found: {}", msg),
			ViewError::BadRequest(msg) => write!(f, "Bad request: {}", msg),
			ViewError::Internal(msg) => write!(f, "Internal error: {}", msg),
			ViewError::DatabaseError(msg) => write!(f, "Database error: {}", msg),
		}
	}
}

impl std::error::Error for ViewError {}

/// Django REST Framework-style ViewSet handler for models
///
/// Provides automatic CRUD operations with permission checks, filtering,
/// pagination, and serialization for Model types.
///
/// # Examples
///
/// ```no_run
/// # use reinhardt_views::viewsets::ModelViewSetHandler;
/// # use reinhardt_db::orm::Model;
/// # use serde::{Serialize, Deserialize};
/// #
/// # #[derive(Serialize, Deserialize, Clone, Debug)]
/// # struct User {
/// #     id: Option<i64>,
/// #     username: String,
/// # }
/// #
/// # #[derive(Clone)]
/// # struct UserFields;
/// #
/// # impl reinhardt_db::orm::FieldSelector for UserFields {
/// #     fn with_alias(self, _alias: &str) -> Self { self }
/// # }
/// #
/// # impl Model for User {
/// #     type PrimaryKey = i64;
/// #     type Fields = UserFields;
/// #     fn table_name() -> &'static str { "users" }
/// #     fn primary_key(&self) -> Option<Self::PrimaryKey> { self.id }
/// #     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
/// #     fn new_fields() -> Self::Fields { UserFields }
/// # }
/// #
/// # async fn example() {
/// let handler = ModelViewSetHandler::<User>::new();
/// # }
/// ```
pub struct ModelViewSetHandler<T>
where
	T: Model + Serialize + DeserializeOwned + Clone + Send + Sync + 'static,
{
	queryset: Option<Vec<T>>,
	serializer_class: Option<Arc<dyn Serializer<Input = T, Output = String> + Send + Sync>>,
	permission_classes: Vec<Arc<dyn Permission>>,
	filter_backends: Vec<Arc<dyn FilterBackend>>,
	pagination_class: Option<reinhardt_core::pagination::PaginatorImpl>,
	pool: Option<Arc<sqlx::AnyPool>>,
	/// Database backend type (default: PostgreSQL)
	db_backend: DbBackend,
	_phantom: PhantomData<T>,
}

impl<T> ModelViewSetHandler<T>
where
	T: Model + Serialize + DeserializeOwned + Clone + Send + Sync + 'static,
{
	/// Create a new ModelViewSetHandler
	///
	/// # Examples
	///
	/// ```
	/// # use reinhardt_views::viewsets::ModelViewSetHandler;
	/// # use reinhardt_db::orm::Model;
	/// # use serde::{Serialize, Deserialize};
	/// #
	/// # #[derive(Debug, Clone, Serialize, Deserialize)]
	/// # struct User {
	/// #     id: Option<i64>,
	/// #     username: String,
	/// # }
	/// #
	/// # #[derive(Clone)]
	/// # struct UserFields;
	/// #
	/// # impl reinhardt_db::orm::FieldSelector for UserFields {
	/// #     fn with_alias(self, _alias: &str) -> Self { self }
	/// # }
	/// #
	/// # impl Model for User {
	/// #     type PrimaryKey = i64;
	/// #     type Fields = UserFields;
	/// #     fn table_name() -> &'static str { "users" }
	/// #     fn primary_key(&self) -> Option<Self::PrimaryKey> { self.id }
	/// #     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
	/// #     fn new_fields() -> Self::Fields { UserFields }
	/// # }
	/// let handler = ModelViewSetHandler::<User>::new();
	/// ```
	pub fn new() -> Self {
		Self {
			queryset: None,
			serializer_class: None,
			permission_classes: Vec::new(),
			filter_backends: Vec::new(),
			pagination_class: None,
			pool: None,
			db_backend: DbBackend::Postgres, // Default to PostgreSQL
			_phantom: PhantomData,
		}
	}

	/// Set the queryset (in-memory data) for this handler
	///
	/// # Examples
	///
	/// ```
	/// # use reinhardt_views::viewsets::ModelViewSetHandler;
	/// # use reinhardt_db::orm::Model;
	/// # use serde::{Serialize, Deserialize};
	/// #
	/// # #[derive(Debug, Clone, Serialize, Deserialize)]
	/// # struct User {
	/// #     id: Option<i64>,
	/// #     username: String,
	/// # }
	/// #
	/// # #[derive(Clone)]
	/// # struct UserFields;
	/// #
	/// # impl reinhardt_db::orm::FieldSelector for UserFields {
	/// #     fn with_alias(self, _alias: &str) -> Self { self }
	/// # }
	/// #
	/// # impl Model for User {
	/// #     type PrimaryKey = i64;
	/// #     type Fields = UserFields;
	/// #     fn table_name() -> &'static str { "users" }
	/// #     fn primary_key(&self) -> Option<Self::PrimaryKey> { self.id }
	/// #     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
	/// #     fn new_fields() -> Self::Fields { UserFields }
	/// # }
	/// let users = vec![
	///     User { id: Some(1), username: "alice".to_string() },
	///     User { id: Some(2), username: "bob".to_string() },
	/// ];
	/// let handler = ModelViewSetHandler::<User>::new()
	///     .with_queryset(users);
	/// ```
	pub fn with_queryset(mut self, queryset: Vec<T>) -> Self {
		self.queryset = Some(queryset);
		self
	}

	/// Set the serializer class for this handler
	///
	/// # Examples
	///
	/// ```
	/// # use reinhardt_views::viewsets::ModelViewSetHandler;
	/// # use reinhardt_rest::serializers::ModelSerializer;
	/// # use reinhardt_db::orm::Model;
	/// # use serde::{Serialize, Deserialize};
	/// # use std::sync::Arc;
	/// #
	/// # #[derive(Debug, Clone, Serialize, Deserialize)]
	/// # struct User {
	/// #     id: Option<i64>,
	/// #     username: String,
	/// # }
	/// #
	/// # #[derive(Clone)]
	/// # struct UserFields;
	/// #
	/// # impl reinhardt_db::orm::FieldSelector for UserFields {
	/// #     fn with_alias(self, _alias: &str) -> Self { self }
	/// # }
	/// #
	/// # impl Model for User {
	/// #     type PrimaryKey = i64;
	/// #     type Fields = UserFields;
	/// #     fn table_name() -> &'static str { "users" }
	/// #     fn primary_key(&self) -> Option<Self::PrimaryKey> { self.id }
	/// #     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
	/// #     fn new_fields() -> Self::Fields { UserFields }
	/// # }
	/// let serializer = Arc::new(ModelSerializer::<User>::new());
	/// let handler = ModelViewSetHandler::<User>::new()
	///     .with_serializer(serializer);
	/// ```
	pub fn with_serializer(
		mut self,
		serializer: Arc<dyn Serializer<Input = T, Output = String> + Send + Sync>,
	) -> Self {
		self.serializer_class = Some(serializer);
		self
	}

	/// Set the database connection pool for this handler
	///
	/// # Examples
	///
	/// ```no_run
	/// # use reinhardt_views::viewsets::ModelViewSetHandler;
	/// # use reinhardt_db::orm::Model;
	/// # use serde::{Serialize, Deserialize};
	/// # use sqlx::AnyPool;
	/// # use std::sync::Arc;
	/// #
	/// # #[derive(Debug, Clone, Serialize, Deserialize)]
	/// # struct User {
	/// #     id: Option<i64>,
	/// #     username: String,
	/// # }
	/// #
	/// # #[derive(Clone)]
	/// # struct UserFields;
	/// #
	/// # impl reinhardt_db::orm::FieldSelector for UserFields {
	/// #     fn with_alias(self, _alias: &str) -> Self { self }
	/// # }
	/// #
	/// # impl Model for User {
	/// #     type PrimaryKey = i64;
	/// #     type Fields = UserFields;
	/// #     fn table_name() -> &'static str { "users" }
	/// #     fn primary_key(&self) -> Option<Self::PrimaryKey> { self.id }
	/// #     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
	/// #     fn new_fields() -> Self::Fields { UserFields }
	/// # }
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// let pool = Arc::new(AnyPool::connect("postgres://localhost/mydb").await?);
	/// let handler = ModelViewSetHandler::<User>::new()
	///     .with_pool(pool);
	/// # Ok(())
	/// # }
	/// ```
	pub fn with_pool(mut self, pool: Arc<sqlx::AnyPool>) -> Self {
		self.pool = Some(pool);
		self
	}

	/// Set the database backend type for this handler
	///
	/// # Examples
	///
	/// ```
	/// # use reinhardt_views::viewsets::ModelViewSetHandler;
	/// # use reinhardt_db::orm::{Model, query_types::DbBackend};
	/// # use serde::{Serialize, Deserialize};
	/// #
	/// # #[derive(Debug, Clone, Serialize, Deserialize)]
	/// # struct User {
	/// #     id: Option<i64>,
	/// #     username: String,
	/// # }
	/// #
	/// # #[derive(Clone)]
	/// # struct UserFields;
	/// #
	/// # impl reinhardt_db::orm::FieldSelector for UserFields {
	/// #     fn with_alias(self, _alias: &str) -> Self { self }
	/// # }
	/// #
	/// # impl Model for User {
	/// #     type PrimaryKey = i64;
	/// #     type Fields = UserFields;
	/// #     fn table_name() -> &'static str { "users" }
	/// #     fn primary_key(&self) -> Option<Self::PrimaryKey> { self.id }
	/// #     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
	/// #     fn new_fields() -> Self::Fields { UserFields }
	/// # }
	/// let handler = ModelViewSetHandler::<User>::new()
	///     .with_db_backend(DbBackend::Sqlite);
	/// ```
	pub fn with_db_backend(mut self, db_backend: DbBackend) -> Self {
		self.db_backend = db_backend;
		self
	}

	/// Add a permission class to this handler
	///
	/// # Examples
	///
	/// ```
	/// # use reinhardt_views::viewsets::ModelViewSetHandler;
	/// # use reinhardt_auth::IsAuthenticated;
	/// # use reinhardt_db::orm::Model;
	/// # use serde::{Serialize, Deserialize};
	/// # use std::sync::Arc;
	/// #
	/// # #[derive(Debug, Clone, Serialize, Deserialize)]
	/// # struct User {
	/// #     id: Option<i64>,
	/// #     username: String,
	/// # }
	/// #
	/// # #[derive(Clone)]
	/// # struct UserFields;
	/// #
	/// # impl reinhardt_db::orm::FieldSelector for UserFields {
	/// #     fn with_alias(self, _alias: &str) -> Self { self }
	/// # }
	/// #
	/// # impl Model for User {
	/// #     type PrimaryKey = i64;
	/// #     type Fields = UserFields;
	/// #     fn table_name() -> &'static str { "users" }
	/// #     fn primary_key(&self) -> Option<Self::PrimaryKey> { self.id }
	/// #     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
	/// #     fn new_fields() -> Self::Fields { UserFields }
	/// # }
	/// let handler = ModelViewSetHandler::<User>::new()
	///     .add_permission(Arc::new(IsAuthenticated));
	/// ```
	pub fn add_permission(mut self, permission: Arc<dyn Permission>) -> Self {
		self.permission_classes.push(permission);
		self
	}

	/// Add a filter backend to this handler
	pub fn add_filter_backend(mut self, backend: Arc<dyn FilterBackend>) -> Self {
		self.filter_backends.push(backend);
		self
	}

	/// Set the pagination class for this handler
	pub fn with_pagination(
		mut self,
		pagination: reinhardt_core::pagination::PaginatorImpl,
	) -> Self {
		self.pagination_class = Some(pagination);
		self
	}

	/// Get the queryset for this handler
	fn get_queryset(&self) -> &[T] {
		self.queryset.as_deref().unwrap_or(&[])
	}

	/// Get the serializer for this handler
	fn get_serializer(&self) -> Arc<dyn Serializer<Input = T, Output = String> + Send + Sync> {
		self.serializer_class
			.clone()
			.unwrap_or_else(|| Arc::new(ModelSerializer::<T>::new()))
	}

	/// Check permissions for the request
	async fn check_permissions(&self, request: &Request) -> std::result::Result<(), ViewError> {
		// Extract authentication information from request extensions
		// The session middleware stores authenticated user_id in extensions
		//
		// Expected usage:
		// 1. Session middleware extracts session from cookie/token
		// 2. Middleware validates session and extracts user_id
		// 3. Middleware stores user_id in request.extensions using a dedicated type
		//
		// Example middleware implementation:
		//   if let Some(user_id) = session.get::<i64>("user_id").ok().flatten() {
		//       request.extensions.insert(AuthenticatedUserId(user_id));
		//   }

		// Try to extract user_id from extensions
		// Support both String and UUID formats
		let user_id_string: Option<String> = request.extensions.get::<String>().or_else(|| {
			request
				.extensions
				.get::<uuid::Uuid>()
				.map(|id| id.to_string())
		});

		// Determine authentication status based on user_id presence
		let is_authenticated = user_id_string.is_some();

		// Load user from database if authenticated and pool is available
		let (is_admin, is_active, user_obj) = if let (Some(user_id_str), Some(_pool)) =
			(user_id_string.as_ref(), self.pool.as_ref())
		{
			// Parse user_id as UUID
			#[cfg(feature = "argon2-hasher")]
			match uuid::Uuid::parse_str(user_id_str) {
				Ok(user_uuid) => {
					// Get database connection
					use reinhardt_db::orm::manager::get_connection;
					match get_connection().await {
						Ok(conn) => {
							// Build SQL query using reinhardt-query for type-safe query construction
							use reinhardt_auth::DefaultUser;
							use reinhardt_db::orm::{
								Alias, ColumnRef, DatabaseBackend, Expr, ExprTrait, Model,
								MySqlQueryBuilder, PostgresQueryBuilder, Query,
								QueryStatementBuilder, SqliteQueryBuilder,
							};

							let table_name = DefaultUser::table_name();
							let pk_field = DefaultUser::primary_key_field();

							// Build SELECT * query using reinhardt-query
							let stmt = Query::select()
								.column(ColumnRef::Asterisk)
								.from(Alias::new(table_name))
								.and_where(
									Expr::col(Alias::new(pk_field))
										.eq(Expr::value(user_uuid.to_string())),
								)
								.to_owned();

							let sql = match conn.backend() {
								DatabaseBackend::Postgres => stmt.to_string(PostgresQueryBuilder),
								DatabaseBackend::MySql => stmt.to_string(MySqlQueryBuilder),
								DatabaseBackend::Sqlite => stmt.to_string(SqliteQueryBuilder),
							};

							match conn.query_optional(&sql, vec![]).await {
								Ok(Some(row)) => {
									// Deserialize user from query result
									match serde_json::from_value::<DefaultUser>(row.data) {
										Ok(user) => {
											use reinhardt_auth::User;
											// Extract admin and active status from loaded user
											let is_admin = user.is_admin();
											let is_active = user.is_active();
											// Box the user object to store in PermissionContext
											let boxed_user: Box<dyn User> = Box::new(user);
											(is_admin, is_active, Some(boxed_user))
										}
										Err(_) => {
											// Deserialization failed, use defaults
											(false, true, None)
										}
									}
								}
								Ok(None) => {
									// User not found, use defaults
									(false, true, None)
								}
								Err(_) => {
									// Database query failed, use defaults
									(false, true, None)
								}
							}
						}
						Err(_) => {
							// Connection failed, use defaults
							(false, true, None)
						}
					}
				}
				Err(_) => {
					// UUID parse failed, use defaults
					(false, true, None)
				}
			}

			// When argon2-hasher feature is disabled, DefaultUser is not available
			// Return default values to indicate user retrieval is not supported
			#[cfg(not(feature = "argon2-hasher"))]
			{
				let _ = user_id_str; // Suppress unused variable warning
				(false, true, None)
			}
		} else {
			// Not authenticated or no pool, use defaults
			(false, true, None)
		};

		let context = PermissionContext {
			request,
			is_authenticated,
			is_admin,
			is_active,
			user: user_obj,
		};

		// Check all registered permission classes
		for permission in &self.permission_classes {
			if !permission.has_permission(&context).await {
				// Permission denied - return specific error
				return Err(ViewError::Permission(format!(
					"Permission denied by {}",
					std::any::type_name_of_val(&**permission)
				)));
			}
		}

		Ok(())
	}

	/// List all objects with optional filtering and pagination
	///
	/// # Examples
	///
	/// ```no_run
	/// # use reinhardt_views::viewsets::ModelViewSetHandler;
	/// # use reinhardt_http::Request;
	/// # use reinhardt_db::orm::Model;
	/// # use serde::{Serialize, Deserialize};
	/// # use bytes::Bytes;
	/// # use hyper::{Method, Version, HeaderMap};
	/// #
	/// # #[derive(Debug, Clone, Serialize, Deserialize)]
	/// # struct User {
	/// #     id: Option<i64>,
	/// #     username: String,
	/// # }
	/// #
	/// # #[derive(Clone)]
	/// # struct UserFields;
	/// #
	/// # impl reinhardt_db::orm::FieldSelector for UserFields {
	/// #     fn with_alias(self, _alias: &str) -> Self { self }
	/// # }
	/// #
	/// # impl Model for User {
	/// #     type PrimaryKey = i64;
	/// #     type Fields = UserFields;
	/// #     fn table_name() -> &'static str { "users" }
	/// #     fn primary_key(&self) -> Option<Self::PrimaryKey> { self.id }
	/// #     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
	/// #     fn new_fields() -> Self::Fields { UserFields }
	/// # }
	/// #
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// let handler = ModelViewSetHandler::<User>::new();
	/// let request = Request::builder()
	///     .method(Method::GET)
	///     .uri("/users/")
	///     .version(Version::HTTP_11)
	///     .headers(HeaderMap::new())
	///     .body(Bytes::new())
	///     .build()?;
	/// let response = handler.list(&request).await?;
	/// # Ok(())
	/// # }
	/// ```
	pub async fn list(&self, request: &Request) -> std::result::Result<Response, ViewError> {
		self.check_permissions(request).await?;

		let serializer = self.get_serializer();

		// Get items from database if pool is available, otherwise use in-memory queryset
		let items: Vec<T> = if let Some(pool) = &self.pool {
			// Query database for all objects
			let session = reinhardt_db::prelude::Session::new(pool.clone(), self.db_backend)
				.await
				.map_err(|e| {
					ViewError::DatabaseError(format!("Failed to create session: {}", e))
				})?;

			session
				.list_all()
				.await
				.map_err(|e| ViewError::DatabaseError(format!("Failed to list objects: {}", e)))?
		} else {
			// Use in-memory queryset
			self.get_queryset().to_vec()
		};

		// Serialize all objects
		let mut serialized_items = Vec::new();
		for item in &items {
			let json = serializer
				.serialize(item)
				.map_err(|e| ViewError::Serialization(e.to_string()))?;
			serialized_items.push(json);
		}

		// Create response body
		let response_body = format!("[{}]", serialized_items.join(","));

		Ok(Response::ok().with_body(response_body))
	}

	/// Retrieve a single object by primary key
	///
	/// # Examples
	///
	/// ```no_run
	/// # use reinhardt_views::viewsets::ModelViewSetHandler;
	/// # use reinhardt_http::Request;
	/// # use reinhardt_db::orm::Model;
	/// # use serde::{Serialize, Deserialize};
	/// # use serde_json::Value;
	/// # use bytes::Bytes;
	/// # use hyper::{Method, Version, HeaderMap};
	/// #
	/// # #[derive(Debug, Clone, Serialize, Deserialize)]
	/// # struct User {
	/// #     id: Option<i64>,
	/// #     username: String,
	/// # }
	/// #
	/// # #[derive(Clone)]
	/// # struct UserFields;
	/// #
	/// # impl reinhardt_db::orm::FieldSelector for UserFields {
	/// #     fn with_alias(self, _alias: &str) -> Self { self }
	/// # }
	/// #
	/// # impl Model for User {
	/// #     type PrimaryKey = i64;
	/// #     type Fields = UserFields;
	/// #     fn table_name() -> &'static str { "users" }
	/// #     fn primary_key(&self) -> Option<Self::PrimaryKey> { self.id }
	/// #     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
	/// #     fn new_fields() -> Self::Fields { UserFields }
	/// # }
	/// #
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// let handler = ModelViewSetHandler::<User>::new();
	/// let request = Request::builder()
	///     .method(Method::GET)
	///     .uri("/users/1/")
	///     .version(Version::HTTP_11)
	///     .headers(HeaderMap::new())
	///     .body(Bytes::new())
	///     .build()?;
	/// let pk = serde_json::json!(1);
	/// let response = handler.retrieve(&request, pk).await?;
	/// # Ok(())
	/// # }
	/// ```
	pub async fn retrieve(
		&self,
		request: &Request,
		pk: serde_json::Value,
	) -> std::result::Result<Response, ViewError> {
		self.check_permissions(request).await?;

		let serializer = self.get_serializer();

		// Get item from database if pool is available, otherwise use in-memory queryset
		let item: T = if let Some(pool) = &self.pool {
			// Query database for all objects and find by pk
			let session = reinhardt_db::prelude::Session::new(pool.clone(), self.db_backend)
				.await
				.map_err(|e| {
					ViewError::DatabaseError(format!("Failed to create session: {}", e))
				})?;

			let items: Vec<T> = session
				.list_all()
				.await
				.map_err(|e| ViewError::DatabaseError(format!("Failed to query objects: {}", e)))?;

			// Compare pk as strings (JSON number vs Display)
			let pk_str = pk.to_string();

			items
				.into_iter()
				.find(|item| {
					if let Some(item_pk) = item.primary_key() {
						item_pk.to_string() == pk_str
					} else {
						false
					}
				})
				.ok_or_else(|| ViewError::NotFound(format!("Object with pk={} not found", pk)))?
		} else {
			// Use in-memory queryset
			let queryset = self.get_queryset();
			queryset
				.iter()
				.find(|item| {
					if let Some(item_pk) = item.primary_key() {
						let item_pk_str = item_pk.to_string();
						let pk_str = pk.to_string();
						item_pk_str == pk_str
					} else {
						false
					}
				})
				.cloned()
				.ok_or_else(|| ViewError::NotFound(format!("Object with pk={} not found", pk)))?
		};

		let json = serializer
			.serialize(&item)
			.map_err(|e| ViewError::Serialization(e.to_string()))?;

		Ok(Response::ok().with_body(json))
	}

	/// Create a new object
	///
	/// # Examples
	///
	/// ```no_run
	/// # use reinhardt_views::viewsets::ModelViewSetHandler;
	/// # use reinhardt_http::Request;
	/// # use reinhardt_db::orm::Model;
	/// # use serde::{Serialize, Deserialize};
	/// # use bytes::Bytes;
	/// # use hyper::{Method, Version, HeaderMap};
	/// #
	/// # #[derive(Debug, Clone, Serialize, Deserialize)]
	/// # struct User {
	/// #     id: Option<i64>,
	/// #     username: String,
	/// # }
	/// #
	/// # #[derive(Clone)]
	/// # struct UserFields;
	/// #
	/// # impl reinhardt_db::orm::FieldSelector for UserFields {
	/// #     fn with_alias(self, _alias: &str) -> Self { self }
	/// # }
	/// #
	/// # impl Model for User {
	/// #     type PrimaryKey = i64;
	/// #     type Fields = UserFields;
	/// #     fn table_name() -> &'static str { "users" }
	/// #     fn primary_key(&self) -> Option<Self::PrimaryKey> { self.id }
	/// #     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
	/// #     fn new_fields() -> Self::Fields { UserFields }
	/// # }
	/// #
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// let handler = ModelViewSetHandler::<User>::new();
	/// let request = Request::builder()
	///     .method(Method::POST)
	///     .uri("/users/")
	///     .version(Version::HTTP_11)
	///     .headers(HeaderMap::new())
	///     .body(Bytes::from(r#"{"username":"alice"}"#))
	///     .build()?;
	/// let response = handler.create(&request).await?;
	/// # Ok(())
	/// # }
	/// ```
	pub async fn create(&self, request: &Request) -> std::result::Result<Response, ViewError> {
		self.check_permissions(request).await?;

		let serializer = self.get_serializer();

		// Parse request body
		let body_str = String::from_utf8(request.body().to_vec())
			.map_err(|e| ViewError::BadRequest(format!("Invalid UTF-8: {}", e)))?;

		// Deserialize into model
		let item = serializer
			.deserialize(&body_str)
			.map_err(|e| ViewError::Serialization(e.to_string()))?;

		// Save to database if pool is available
		if let Some(pool) = &self.pool {
			// Create a new session for this request
			let mut session = reinhardt_db::prelude::Session::new(pool.clone(), self.db_backend)
				.await
				.map_err(|e| {
					ViewError::DatabaseError(format!("Failed to create session: {}", e))
				})?;

			// Begin transaction
			session.begin().await.map_err(|e| {
				ViewError::DatabaseError(format!("Failed to begin transaction: {}", e))
			})?;

			// Add object to session
			session
				.add(item.clone())
				.await
				.map_err(|e| ViewError::DatabaseError(format!("Failed to add object: {}", e)))?;

			// Flush changes to database (generates and executes INSERT)
			session
				.flush()
				.await
				.map_err(|e| ViewError::DatabaseError(format!("Failed to flush: {}", e)))?;

			// Get the generated ID from the session
			let generated_id = session.get_generated_ids().first().map(|(_, id)| *id);

			// Commit transaction
			session
				.commit()
				.await
				.map_err(|e| ViewError::DatabaseError(format!("Failed to commit: {}", e)))?;

			// Re-fetch the created object from the database to get all auto-populated fields
			// (e.g., created_at which is set by database DEFAULT)
			if let Some(id) = generated_id {
				let fetch_session =
					reinhardt_db::prelude::Session::new(pool.clone(), self.db_backend)
						.await
						.map_err(|e| {
							ViewError::DatabaseError(format!("Failed to create session: {}", e))
						})?;

				// Fetch all objects and find the one with matching ID
				let items: Vec<T> = fetch_session.list_all().await.map_err(|e| {
					ViewError::DatabaseError(format!("Failed to fetch objects: {}", e))
				})?;

				let created_item = items
					.into_iter()
					.find(|i| {
						i.primary_key()
							.map(|pk| pk.to_string() == id.to_string())
							.unwrap_or(false)
					})
					.ok_or_else(|| {
						ViewError::DatabaseError("Failed to find created object".to_string())
					})?;

				// Serialize the complete object (including auto-populated fields)
				let response_body = serializer
					.serialize(&created_item)
					.map_err(|e| ViewError::Serialization(e.to_string()))?;

				return Ok(Response::created().with_body(response_body));
			}
		}

		// Fallback: return the original item if no database pool
		let response_body = serializer
			.serialize(&item)
			.map_err(|e| ViewError::Serialization(e.to_string()))?;

		Ok(Response::created().with_body(response_body))
	}

	/// Update an existing object
	///
	/// # Examples
	///
	/// ```no_run
	/// # use reinhardt_views::viewsets::ModelViewSetHandler;
	/// # use reinhardt_http::Request;
	/// # use reinhardt_db::orm::Model;
	/// # use serde::{Serialize, Deserialize};
	/// # use serde_json::Value;
	/// # use bytes::Bytes;
	/// # use hyper::{Method, Version, HeaderMap};
	/// #
	/// # #[derive(Debug, Clone, Serialize, Deserialize)]
	/// # struct User {
	/// #     id: Option<i64>,
	/// #     username: String,
	/// # }
	/// #
	/// # #[derive(Clone)]
	/// # struct UserFields;
	/// #
	/// # impl reinhardt_db::orm::FieldSelector for UserFields {
	/// #     fn with_alias(self, _alias: &str) -> Self { self }
	/// # }
	/// #
	/// # impl Model for User {
	/// #     type PrimaryKey = i64;
	/// #     type Fields = UserFields;
	/// #     fn table_name() -> &'static str { "users" }
	/// #     fn primary_key(&self) -> Option<Self::PrimaryKey> { self.id }
	/// #     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
	/// #     fn new_fields() -> Self::Fields { UserFields }
	/// # }
	/// #
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// let handler = ModelViewSetHandler::<User>::new();
	/// let request = Request::builder()
	///     .method(Method::PUT)
	///     .uri("/users/1/")
	///     .version(Version::HTTP_11)
	///     .headers(HeaderMap::new())
	///     .body(Bytes::from(r#"{"username":"alice_updated"}"#))
	///     .build()?;
	/// let pk = serde_json::json!(1);
	/// let response = handler.update(&request, pk).await?;
	/// # Ok(())
	/// # }
	/// ```
	pub async fn update(
		&self,
		request: &Request,
		pk: serde_json::Value,
	) -> std::result::Result<Response, ViewError> {
		self.check_permissions(request).await?;

		let serializer = self.get_serializer();

		// Get existing object from database
		let existing_obj: T = if let Some(pool) = &self.pool {
			let session = reinhardt_db::prelude::Session::new(pool.clone(), self.db_backend)
				.await
				.map_err(|e| {
					ViewError::DatabaseError(format!("Failed to create session: {}", e))
				})?;

			let items: Vec<T> = session
				.list_all()
				.await
				.map_err(|e| ViewError::DatabaseError(format!("Failed to list objects: {}", e)))?;

			let pk_str = pk.to_string().replace('"', "");
			items
				.into_iter()
				.find(|item| {
					if let Some(item_pk) = item.primary_key() {
						item_pk.to_string() == pk_str
					} else {
						false
					}
				})
				.ok_or_else(|| {
					ViewError::NotFound(format!("Object with pk {} not found", pk_str))
				})?
		} else {
			// Fall back to queryset for non-database mode
			let pk_str = pk.to_string().replace('"', "");
			self.get_queryset()
				.iter()
				.find(|item| {
					if let Some(item_pk) = item.primary_key() {
						item_pk.to_string() == pk_str
					} else {
						false
					}
				})
				.cloned()
				.ok_or_else(|| {
					ViewError::NotFound(format!("Object with pk {} not found", pk_str))
				})?
		};

		// Parse request body as JSON for partial update (PATCH semantics)
		let body_str = String::from_utf8(request.body().to_vec())
			.map_err(|e| ViewError::BadRequest(format!("Invalid UTF-8: {}", e)))?;

		// Parse patch data as JSON
		let patch_data: serde_json::Value = serde_json::from_str(&body_str)
			.map_err(|e| ViewError::Serialization(format!("Invalid JSON: {}", e)))?;

		// Serialize existing object to JSON and merge with patch data
		let existing_json = serializer
			.serialize(&existing_obj)
			.map_err(|e| ViewError::Serialization(e.to_string()))?;
		let mut existing_value: serde_json::Value = serde_json::from_str(&existing_json)
			.map_err(|e| ViewError::Serialization(format!("Failed to parse existing: {}", e)))?;

		// Merge patch data into existing object (only overwrites provided fields)
		if let (Some(existing_obj_map), Some(patch_obj)) =
			(existing_value.as_object_mut(), patch_data.as_object())
		{
			for (key, value) in patch_obj {
				existing_obj_map.insert(key.clone(), value.clone());
			}
		}

		// Deserialize merged object back to model type
		let merged_json = serde_json::to_string(&existing_value)
			.map_err(|e| ViewError::Serialization(format!("Failed to serialize merged: {}", e)))?;
		let updated_item: T = serializer
			.deserialize(&merged_json)
			.map_err(|e| ViewError::Serialization(e.to_string()))?;

		// Update database if pool is available
		if let Some(pool) = &self.pool {
			// Create a new session for this request
			let mut session = reinhardt_db::prelude::Session::new(pool.clone(), self.db_backend)
				.await
				.map_err(|e| {
					ViewError::DatabaseError(format!("Failed to create session: {}", e))
				})?;

			// Begin transaction
			session.begin().await.map_err(|e| {
				ViewError::DatabaseError(format!("Failed to begin transaction: {}", e))
			})?;

			// Add updated object to session (marks as dirty for UPDATE)
			session
				.add(updated_item.clone())
				.await
				.map_err(|e| ViewError::DatabaseError(format!("Failed to add object: {}", e)))?;

			// Flush changes to database (generates and executes UPDATE)
			session
				.flush()
				.await
				.map_err(|e| ViewError::DatabaseError(format!("Failed to flush: {}", e)))?;

			// Commit transaction
			session
				.commit()
				.await
				.map_err(|e| ViewError::DatabaseError(format!("Failed to commit: {}", e)))?;
		}

		// Return the complete merged/updated object
		Ok(Response::ok().with_body(merged_json))
	}

	/// Delete an object
	///
	/// # Examples
	///
	/// ```no_run
	/// # use reinhardt_views::viewsets::ModelViewSetHandler;
	/// # use reinhardt_http::Request;
	/// # use reinhardt_db::orm::Model;
	/// # use serde::{Serialize, Deserialize};
	/// # use serde_json::Value;
	/// # use bytes::Bytes;
	/// # use hyper::{Method, Version, HeaderMap};
	/// #
	/// # #[derive(Debug, Clone, Serialize, Deserialize)]
	/// # struct User {
	/// #     id: Option<i64>,
	/// #     username: String,
	/// # }
	/// #
	/// # #[derive(Clone)]
	/// # struct UserFields;
	/// #
	/// # impl reinhardt_db::orm::FieldSelector for UserFields {
	/// #     fn with_alias(self, _alias: &str) -> Self { self }
	/// # }
	/// #
	/// # impl Model for User {
	/// #     type PrimaryKey = i64;
	/// #     type Fields = UserFields;
	/// #     fn table_name() -> &'static str { "users" }
	/// #     fn primary_key(&self) -> Option<Self::PrimaryKey> { self.id }
	/// #     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
	/// #     fn new_fields() -> Self::Fields { UserFields }
	/// # }
	/// #
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// let handler = ModelViewSetHandler::<User>::new();
	/// let request = Request::builder()
	///     .method(Method::DELETE)
	///     .uri("/users/1/")
	///     .version(Version::HTTP_11)
	///     .headers(HeaderMap::new())
	///     .body(Bytes::new())
	///     .build()?;
	/// let pk = serde_json::json!(1);
	/// let response = handler.destroy(&request, pk).await?;
	/// # Ok(())
	/// # }
	/// ```
	pub async fn destroy(
		&self,
		request: &Request,
		pk: serde_json::Value,
	) -> std::result::Result<Response, ViewError> {
		self.check_permissions(request).await?;

		let serializer = self.get_serializer();

		// Verify object exists and get it for deletion
		let response = self.retrieve(request, pk).await?;

		// Extract the object from response body
		let body_str = String::from_utf8(response.body.to_vec())
			.map_err(|e| ViewError::BadRequest(format!("Invalid UTF-8: {}", e)))?;

		// Deserialize into model
		let item = serializer
			.deserialize(&body_str)
			.map_err(|e| ViewError::Serialization(e.to_string()))?;

		// Delete from database if pool is available
		if let Some(pool) = &self.pool {
			// Create a new session for this request
			let mut session = reinhardt_db::prelude::Session::new(pool.clone(), self.db_backend)
				.await
				.map_err(|e| {
					ViewError::DatabaseError(format!("Failed to create session: {}", e))
				})?;

			// Begin transaction
			session.begin().await.map_err(|e| {
				ViewError::DatabaseError(format!("Failed to begin transaction: {}", e))
			})?;

			// Mark object for deletion
			session.delete(item).await.map_err(|e| {
				ViewError::DatabaseError(format!("Failed to mark object for deletion: {}", e))
			})?;

			// Flush changes to database (generates and executes DELETE)
			session
				.flush()
				.await
				.map_err(|e| ViewError::DatabaseError(format!("Failed to flush: {}", e)))?;

			// Commit transaction
			session
				.commit()
				.await
				.map_err(|e| ViewError::DatabaseError(format!("Failed to commit: {}", e)))?;
		}

		Ok(Response::no_content())
	}
}

impl<T> Default for ModelViewSetHandler<T>
where
	T: Model + Serialize + DeserializeOwned + Clone + Send + Sync + 'static,
{
	fn default() -> Self {
		Self::new()
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;
	use std::thread;

	#[rstest]
	fn test_parking_lot_rwlock_does_not_poison_after_panic() {
		// Arrange
		// parking_lot::RwLock does not poison, so after a thread panics
		// while holding the lock, subsequent access should succeed.
		let lock = RwLock::new(42);

		// Act - panic while holding write lock
		let lock_ref = &lock;
		let result = thread::scope(|s| {
			let handle = s.spawn(|| {
				let mut guard = lock_ref.write();
				*guard = 100;
				panic!("intentional panic while holding write lock");
			});
			let _ = handle.join(); // Thread panicked

			// Assert - lock is still usable (no poisoning)
			let value = *lock_ref.read();
			value
		});

		// parking_lot recovers the lock after panic
		assert!(result == 42 || result == 100);
	}

	#[rstest]
	fn test_rwlock_concurrent_read_access() {
		// Arrange
		let lock = RwLock::new(String::from("test_value"));

		// Act - multiple readers should not block each other
		let guard1 = lock.read();
		let guard2 = lock.read();

		// Assert
		assert_eq!(*guard1, "test_value");
		assert_eq!(*guard2, "test_value");
	}
}
