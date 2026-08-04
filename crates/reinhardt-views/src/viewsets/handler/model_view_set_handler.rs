//! `ModelViewSetHandler` — Django REST Framework-style CRUD handler.
//!
//! Provides the standard list/retrieve/create/update/destroy actions with
//! permission checks, optional pagination, and serialization for `Model`
//! types. The response rendering for each action lives next to the action
//! itself in this module.

use super::error::ViewError;
use reinhardt_auth::{Permission, PermissionContext};
use reinhardt_db::orm::{Model, query_types::DbBackend};
use reinhardt_http::{AuthState, Request, Response};
use reinhardt_rest::filters::FilterBackend;
use reinhardt_rest::serializers::{ModelSerializer, Serializer};
use serde::Serialize;
use serde::de::DeserializeOwned;
use std::marker::PhantomData;
use std::sync::Arc;

/// Django REST Framework-style ViewSet handler for models.
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
/// #     type Objects = reinhardt_db::orm::Manager<Self>;
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
	/// #     type Objects = reinhardt_db::orm::Manager<Self>;
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
	/// #     type Objects = reinhardt_db::orm::Manager<Self>;
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
	/// #     type Objects = reinhardt_db::orm::Manager<Self>;
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
	/// #     type Objects = reinhardt_db::orm::Manager<Self>;
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
	/// #     type Objects = reinhardt_db::orm::Manager<Self>;
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
	/// #     type Objects = reinhardt_db::orm::Manager<Self>;
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

		let auth_state = AuthState::from_extensions(&request.extensions);
		let is_authenticated = auth_state
			.as_ref()
			.map(|state| state.is_authenticated())
			.unwrap_or(false);
		let is_admin = auth_state
			.as_ref()
			.map(|state| state.is_admin())
			.unwrap_or(false);
		let is_active = auth_state
			.as_ref()
			.map(|state| state.is_active())
			.unwrap_or(false);
		let user_obj = None;

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
	/// #     type Objects = reinhardt_db::orm::Manager<Self>;
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
	/// #     type Objects = reinhardt_db::orm::Manager<Self>;
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

			// Normalize pk: strip surrounding quotes from JSON string PKs for comparison
			let pk_str = pk.to_string();
			let pk_str = pk_str.trim_matches('"');

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
			let pk_str = pk.to_string();
			let pk_str = pk_str.trim_matches('"');
			queryset
				.iter()
				.find(|item| {
					if let Some(item_pk) = item.primary_key() {
						item_pk.to_string() == pk_str
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
	/// #     type Objects = reinhardt_db::orm::Manager<Self>;
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
	/// #     type Objects = reinhardt_db::orm::Manager<Self>;
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

			// Normalize pk: strip surrounding quotes only (consistent with retrieve()).
			let pk_str_owned = pk.to_string();
			let pk_str = pk_str_owned.trim_matches('"');
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
			// Normalize pk: strip surrounding quotes only (consistent with retrieve()).
			let pk_str_owned = pk.to_string();
			let pk_str = pk_str_owned.trim_matches('"');
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

		// Validate and merge patch data into existing object (only overwrites provided fields)
		crate::generic::patch_utils::merge_patch_object_into(&mut existing_value, &patch_data)
			.map_err(ViewError::BadRequest)?;

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
	/// #     type Objects = reinhardt_db::orm::Manager<Self>;
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
	use bytes::Bytes;
	use hyper::{HeaderMap, Method, Version};
	use reinhardt_auth::{IsActiveUser, IsAuthenticated};
	use reinhardt_http::Request;
	use rstest::rstest;

	fn build_request(uri: &str) -> Request {
		Request::builder()
			.method(Method::GET)
			.uri(uri)
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap()
	}

	// -----------------------------------------------------------------------
	// Test model for retrieve PK tests
	// -----------------------------------------------------------------------

	#[derive(Debug, Clone, Serialize, serde::Deserialize, PartialEq)]
	struct TestItem {
		id: Option<i64>,
		name: String,
	}

	#[derive(Clone)]
	struct TestItemFields;

	impl reinhardt_db::orm::FieldSelector for TestItemFields {
		fn with_alias(self, _alias: &str) -> Self {
			self
		}
	}

	impl reinhardt_db::orm::Model for TestItem {
		type PrimaryKey = i64;
		type Fields = TestItemFields;
		type Objects = reinhardt_db::orm::Manager<Self>;

		fn table_name() -> &'static str {
			"test_items"
		}

		fn primary_key(&self) -> Option<Self::PrimaryKey> {
			self.id
		}

		fn set_primary_key(&mut self, value: Self::PrimaryKey) {
			self.id = Some(value);
		}

		fn new_fields() -> Self::Fields {
			TestItemFields
		}
	}

	/// Helper to build a ModelViewSetHandler with in-memory queryset
	fn build_model_handler(items: Vec<TestItem>) -> ModelViewSetHandler<TestItem> {
		ModelViewSetHandler::<TestItem>::new().with_queryset(items)
	}

	#[rstest]
	#[tokio::test]
	async fn test_list_denies_bare_user_id_extensions_for_active_permissions() {
		// Arrange
		let handler = build_model_handler(vec![TestItem {
			id: Some(1),
			name: "first".to_string(),
		}])
		.add_permission(Arc::new(IsAuthenticated))
		.add_permission(Arc::new(IsActiveUser));
		let request = build_request("/items/");
		request.extensions.insert("legacy-user".to_string());

		// Act
		let result = handler.list(&request).await;

		// Assert
		let error = result.expect_err("bare user ID extensions must not grant authorization");
		assert!(matches!(error, ViewError::Permission(_)));
	}

	#[rstest]
	#[tokio::test]
	async fn test_retrieve_strips_quotes_from_numeric_pk() {
		// Arrange
		let items = vec![
			TestItem {
				id: Some(1),
				name: "first".to_string(),
			},
			TestItem {
				id: Some(2),
				name: "second".to_string(),
			},
		];
		let handler = build_model_handler(items);
		let request = build_request("/items/1/");

		// Act - pass pk with surrounding quotes (as JSON string value)
		let pk = serde_json::json!("1");
		let result = handler.retrieve(&request, pk).await;

		// Assert - should find the item despite quotes in pk
		assert!(result.is_ok(), "retrieve should succeed with quoted pk");
		let response = result.unwrap();
		assert_eq!(response.status, hyper::StatusCode::OK);
		let body: TestItem =
			serde_json::from_slice(&response.body).expect("response should be valid JSON");
		assert_eq!(body.name, "first");
		assert_eq!(body.id, Some(1));
	}

	#[rstest]
	#[tokio::test]
	async fn test_retrieve_works_with_unquoted_numeric_pk() {
		// Arrange
		let items = vec![TestItem {
			id: Some(42),
			name: "answer".to_string(),
		}];
		let handler = build_model_handler(items);
		let request = build_request("/items/42/");

		// Act - pass pk as JSON number (no quotes)
		let pk = serde_json::json!(42);
		let result = handler.retrieve(&request, pk).await;

		// Assert
		assert!(result.is_ok(), "retrieve should succeed with numeric pk");
		let response = result.unwrap();
		assert_eq!(response.status, hyper::StatusCode::OK);
		let body: TestItem =
			serde_json::from_slice(&response.body).expect("response should be valid JSON");
		assert_eq!(body.name, "answer");
		assert_eq!(body.id, Some(42));
	}

	#[rstest]
	#[tokio::test]
	async fn test_retrieve_returns_not_found_for_nonexistent_pk() {
		// Arrange
		let items = vec![TestItem {
			id: Some(1),
			name: "only".to_string(),
		}];
		let handler = build_model_handler(items);
		let request = build_request("/items/999/");

		// Act
		let pk = serde_json::json!(999);
		let result = handler.retrieve(&request, pk).await;

		// Assert
		assert!(result.is_err(), "retrieve should fail for nonexistent pk");
		let err = result.unwrap_err();
		assert!(
			matches!(err, ViewError::NotFound(_)),
			"error should be NotFound, got: {:?}",
			err
		);
	}
}
