//! `ModelViewSetHandler` — Django REST Framework-style CRUD handler.
//!
//! Provides the standard list/retrieve/create/update/destroy actions with
//! permission checks, optional pagination, and serialization for `Model`
//! types. The response rendering for each action lives next to the action
//! itself in this module.

use super::error::ViewError;
use reinhardt_auth::{Permission, PermissionContext};
use reinhardt_db::orm::model::filter_value_from_field_type;
use reinhardt_db::orm::{
	CustomManager, Filter, FilterCondition, FilterOperator, FilterValue, Model, QuerySet,
	query_types::DbBackend,
};
use reinhardt_http::{AuthState, Request, Response};
use reinhardt_rest::filters::FilterBackend;
use reinhardt_rest::serializers::{ModelSerializer, Serializer};
use serde::Serialize;
use serde::de::DeserializeOwned;
use std::marker::PhantomData;
use std::sync::Arc;

type QuerysetFn =
	dyn Fn(&Request) -> std::result::Result<FilterCondition, ViewError> + Send + Sync + 'static;

fn parse_length_prefixed_composite_parts<'a>(
	inner: &'a str,
	fields: &[String],
) -> Option<Vec<&'a str>> {
	if fields.is_empty() {
		return None;
	}

	let mut cursor = inner;
	let mut parts = Vec::with_capacity(fields.len());
	for (index, field_name) in fields.iter().enumerate() {
		let value_start = cursor.strip_prefix(&format!("{field_name}="))?;
		let length_separator = value_start.find(':')?;
		let length = value_start[..length_separator].parse::<usize>().ok()?;
		let content_start = length_separator + 1;
		let content_end = content_start.checked_add(length)?;
		let value = value_start.get(content_start..content_end)?;
		let remainder = value_start.get(content_end..)?;

		if index + 1 == fields.len() {
			if !remainder.is_empty() {
				return None;
			}
		} else {
			cursor = remainder.strip_prefix(", ")?;
		}
		parts.push(value);
	}

	Some(parts)
}

fn parse_legacy_composite_parts<'a, F>(
	cursor: &'a str,
	fields: &[String],
	index: usize,
	is_valid_part: &F,
) -> Option<Vec<&'a str>>
where
	F: Fn(usize, &str) -> bool,
{
	let field_name = fields.get(index)?;
	let value_start = cursor.strip_prefix(&format!("{field_name}="))?;
	if index + 1 == fields.len() {
		return is_valid_part(index, value_start).then(|| vec![value_start]);
	}

	let delimiter = format!(", {}=", fields[index + 1]);
	for (position, _) in value_start.match_indices(&delimiter) {
		let part = &value_start[..position];
		if !is_valid_part(index, part) {
			continue;
		}
		let next_cursor = &value_start[position + 2..];
		if let Some(mut tail) =
			parse_legacy_composite_parts(next_cursor, fields, index + 1, is_valid_part)
		{
			tail.insert(0, part);
			return Some(tail);
		}
	}

	None
}

fn primary_key_filter_for_model<T: Model>(
	pk: &serde_json::Value,
) -> std::result::Result<FilterCondition, ViewError> {
	let pk_string = pk
		.as_str()
		.map(str::to_owned)
		.unwrap_or_else(|| pk.to_string());
	let pk_string = urlencoding::decode(&pk_string)
		.map_err(|_| ViewError::NotFound(format!("Object with pk={} not found", pk_string)))?
		.into_owned();
	let Some(composite) = T::composite_primary_key() else {
		let value = T::primary_key_filter_value_from_str(&pk_string)
			.map_err(|_| ViewError::NotFound(format!("Object with pk={} not found", pk_string)))?;
		let column = T::field_metadata()
			.into_iter()
			.find(|field| field.name == T::primary_key_field())
			.map(|field| field.db_column_name().to_owned())
			.unwrap_or_else(|| T::primary_key_field().to_owned());
		return Ok(Filter::new(column, FilterOperator::Eq, value).into());
	};

	let inner = pk_string
		.strip_prefix('(')
		.and_then(|value| value.strip_suffix(')'))
		.ok_or_else(|| ViewError::NotFound(format!("Object with pk={} not found", pk_string)))?;
	let fields = composite.fields();
	let metadata = T::field_metadata();
	let is_valid_part = |index: usize, part: &str| {
		let field_name = &fields[index];
		match metadata.iter().find(|field| field.name == *field_name) {
			Some(field) => filter_value_from_field_type(&field.field_type, part).is_ok(),
			None => true,
		}
	};
	let parts = parse_length_prefixed_composite_parts(inner, fields)
		.or_else(|| parse_legacy_composite_parts(inner, fields, 0, &is_valid_part));
	let parts = parts
		.ok_or_else(|| ViewError::NotFound(format!("Object with pk={} not found", pk_string)))?;
	let filters = fields
		.iter()
		.zip(parts)
		.map(|(field_name, part)| {
			let field = metadata.iter().find(|field| field.name == *field_name);
			let filter_value = field
				.map(|field| filter_value_from_field_type(&field.field_type, part))
				.transpose()
				.map_err(|_| {
					ViewError::NotFound(format!("Object with pk={} not found", pk_string))
				})?
				.unwrap_or_else(|| FilterValue::String(part.to_owned()));
			let column = field
				.map(|field| field.db_column_name().to_owned())
				.unwrap_or_else(|| field_name.clone());
			Ok(Filter::new(column, FilterOperator::Eq, filter_value))
		})
		.collect::<std::result::Result<Vec<_>, _>>()?;

	Ok(FilterCondition::and(
		filters.into_iter().map(FilterCondition::from).collect(),
	))
}

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
	queryset_fn: Option<Arc<QuerysetFn>>,
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
			queryset_fn: None,
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

	/// Scope database queries using the current request.
	///
	/// The synchronous, fallible hook returns one [`FilterCondition`] and requires
	/// a database pool. It applies to list, retrieve, update, and destroy; create
	/// deliberately does not call it, so create ownership belongs in the
	/// serializer, permission layer, or database. Resolve asynchronous scope data
	/// in middleware before dispatch and read its application-defined identity
	/// from request extensions in this hook. Static `Vec` data supplied through
	/// [`Self::with_queryset`] is separate and is never filtered by this hook.
	/// A scoped-out object or a malformed detail primary key is reported as 404.
	/// Custom lookup fields are outside this primary-key scope boundary and are
	/// tracked by #6091.
	pub fn with_queryset_fn<F>(mut self, queryset_fn: F) -> Self
	where
		F: Fn(&Request) -> std::result::Result<FilterCondition, ViewError> + Send + Sync + 'static,
	{
		self.queryset_fn = Some(Arc::new(queryset_fn));
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

	fn scoped_queryset(&self, request: &Request) -> std::result::Result<QuerySet<T>, ViewError> {
		let queryset = T::objects().all();
		match &self.queryset_fn {
			Some(queryset_fn) => Ok(queryset.filter(queryset_fn(request)?)),
			None => Ok(queryset),
		}
	}

	fn primary_key_filter(
		pk: &serde_json::Value,
	) -> std::result::Result<FilterCondition, ViewError> {
		primary_key_filter_for_model::<T>(pk)
	}

	async fn get_object(
		&self,
		request: &Request,
		pk: &serde_json::Value,
	) -> std::result::Result<T, ViewError> {
		let pool = self.pool.as_ref().ok_or_else(|| {
			ViewError::Internal("with_queryset_fn requires a database pool".to_owned())
		})?;
		let session = reinhardt_db::prelude::Session::new(pool.clone(), self.db_backend)
			.await
			.map_err(|error| {
				ViewError::DatabaseError(format!("Failed to create session: {error}"))
			})?;
		let queryset = self
			.scoped_queryset(request)?
			.filter(Self::primary_key_filter(pk)?)
			.limit(1);
		session
			.list(&queryset)
			.await
			.map_err(|error| ViewError::DatabaseError(format!("Failed to query objects: {error}")))?
			.into_iter()
			.next()
			.ok_or_else(|| ViewError::NotFound(format!("Object with pk={pk} not found")))
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

		let items = if let Some(pool) = &self.pool {
			let session = reinhardt_db::prelude::Session::new(pool.clone(), self.db_backend)
				.await
				.map_err(|error| {
					ViewError::DatabaseError(format!("Failed to create session: {error}"))
				})?;
			let queryset = self.scoped_queryset(request)?;
			session.list(&queryset).await.map_err(|error| {
				ViewError::DatabaseError(format!("Failed to list objects: {error}"))
			})?
		} else if self.queryset_fn.is_some() {
			return Err(ViewError::Internal(
				"with_queryset_fn requires a database pool".to_owned(),
			));
		} else {
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

		let item = if self.pool.is_some() {
			self.get_object(request, &pk).await?
		} else if self.queryset_fn.is_some() {
			return Err(ViewError::Internal(
				"with_queryset_fn requires a database pool".to_owned(),
			));
		} else {
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

				let pk = serde_json::json!(id);
				let queryset = QuerySet::<T>::new()
					.filter(Self::primary_key_filter(&pk)?)
					.limit(1);
				let created_item = fetch_session
					.list(&queryset)
					.await
					.map_err(|error| {
						ViewError::DatabaseError(format!(
							"Failed to refresh created object: {error}"
						))
					})?
					.into_iter()
					.next()
					.ok_or_else(|| {
						ViewError::DatabaseError("Failed to find created object".to_owned())
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

		let existing_obj = if self.pool.is_some() {
			self.get_object(request, &pk).await?
		} else if self.queryset_fn.is_some() {
			return Err(ViewError::Internal(
				"with_queryset_fn requires a database pool".to_owned(),
			));
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
		let mut updated_item: T = serializer
			.deserialize(&merged_json)
			.map_err(|e| ViewError::Serialization(e.to_string()))?;
		let primary_key = existing_obj
			.primary_key()
			.ok_or_else(|| ViewError::Internal("Object has no primary key".to_owned()))?;
		updated_item.set_primary_key(primary_key);
		let response_json = serializer
			.serialize(&updated_item)
			.map_err(|e| ViewError::Serialization(e.to_string()))?;

		// Update database if pool is available
		if let Some(pool) = &self.pool {
			// Create a new session for this request
			let mut session = reinhardt_db::prelude::Session::new(pool.clone(), self.db_backend)
				.await
				.map_err(|e| {
					ViewError::DatabaseError(format!("Failed to create session: {}", e))
				})?;

			// Recheck and mutate through one dedicated transaction connection.
			let mut transaction = pool.begin().await.map_err(|e| {
				ViewError::DatabaseError(format!("Failed to begin transaction: {}", e))
			})?;

			// Recheck the request-scoped predicate and lock the row before writing.
			let mutation_queryset = self
				.scoped_queryset(request)?
				.filter(Self::primary_key_filter(&pk)?)
				.limit(1)
				.without_distinct();
			if session
				.list_with_connection_for_update(&mutation_queryset, &mut *transaction)
				.await
				.map_err(|e| ViewError::DatabaseError(format!("Failed to recheck object: {}", e)))?
				.into_iter()
				.next()
				.is_none()
			{
				return Err(ViewError::NotFound(format!(
					"Object with pk={} not found",
					pk
				)));
			}

			// Add updated object to session (marks as dirty for UPDATE)
			session
				.add(updated_item.clone())
				.await
				.map_err(|e| ViewError::DatabaseError(format!("Failed to add object: {}", e)))?;

			// Flush changes to database (generates and executes UPDATE)
			session
				.flush_with_connection(&mut *transaction)
				.await
				.map_err(|e| ViewError::DatabaseError(format!("Failed to flush: {}", e)))?;

			// Commit transaction
			transaction
				.commit()
				.await
				.map_err(|e| ViewError::DatabaseError(format!("Failed to commit: {}", e)))?;
		}

		// Return the complete merged/updated object
		Ok(Response::ok().with_body(response_json))
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

		if self.pool.is_none() {
			if self.queryset_fn.is_some() {
				return Err(ViewError::Internal(
					"with_queryset_fn requires a database pool".to_owned(),
				));
			}
			let pk_str_owned = pk.to_string();
			let pk_str = pk_str_owned.trim_matches('"');
			self.get_queryset()
				.iter()
				.find(|item| {
					item.primary_key()
						.map(|item_pk| item_pk.to_string() == pk_str)
						.unwrap_or(false)
				})
				.cloned()
				.ok_or_else(|| {
					ViewError::NotFound(format!("Object with pk {} not found", pk_str))
				})?;
		}

		// Delete from database if pool is available
		if let Some(pool) = &self.pool {
			// Create a new session for this request
			let mut session = reinhardt_db::prelude::Session::new(pool.clone(), self.db_backend)
				.await
				.map_err(|e| {
					ViewError::DatabaseError(format!("Failed to create session: {}", e))
				})?;

			// Recheck and mutate through one dedicated transaction connection.
			let mut transaction = pool.begin().await.map_err(|e| {
				ViewError::DatabaseError(format!("Failed to begin transaction: {}", e))
			})?;

			// Recheck the request-scoped predicate and lock the row before deleting.
			let mutation_queryset = self
				.scoped_queryset(request)?
				.filter(Self::primary_key_filter(&pk)?)
				.limit(1)
				.without_distinct();
			let item = session
				.list_with_connection_for_update(&mutation_queryset, &mut *transaction)
				.await
				.map_err(|e| ViewError::DatabaseError(format!("Failed to recheck object: {}", e)))?
				.into_iter()
				.next()
				.ok_or_else(|| ViewError::NotFound(format!("Object with pk={} not found", pk)))?;

			// Mark object for deletion
			session.delete(item).await.map_err(|e| {
				ViewError::DatabaseError(format!("Failed to mark object for deletion: {}", e))
			})?;

			// Flush changes to database (generates and executes DELETE)
			session
				.flush_with_connection(&mut *transaction)
				.await
				.map_err(|e| ViewError::DatabaseError(format!("Failed to flush: {}", e)))?;

			// Commit transaction
			transaction
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
	use reinhardt_db::orm::{Filter, FilterOperator, FilterValue};
	use reinhardt_http::Request;
	use rstest::rstest;
	use std::sync::atomic::{AtomicUsize, Ordering};

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

	#[rstest]
	fn composite_pk_parser_preserves_delimiters_in_length_prefixed_values() {
		let fields = vec!["namespace".to_owned(), "id".to_owned()];
		let parts =
			parse_length_prefixed_composite_parts("namespace=9:a, id=999, id=3:123", &fields)
				.expect("length-prefixed composite keys should parse");

		assert_eq!(parts, vec!["a, id=999", "123"]);
	}

	#[rstest]
	fn legacy_composite_pk_parser_uses_typed_boundaries() {
		let fields = vec!["namespace".to_owned(), "id".to_owned()];
		let is_valid = |index: usize, value: &str| index == 0 || value.parse::<i64>().is_ok();
		let parts =
			parse_legacy_composite_parts("namespace=a, id=999, id=1", &fields, 0, &is_valid)
				.expect("legacy composite keys should parse");

		assert_eq!(parts, vec!["a, id=999", "1"]);
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

	#[derive(Clone, Copy)]
	struct OrganizationId(i64);

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

	#[test]
	fn scoped_queryset_fn_reads_request_extensions() {
		let request = build_request("/items/");
		request.extensions.insert(OrganizationId(7));
		let handler = ModelViewSetHandler::<TestItem>::new().with_queryset_fn(|request| {
			let organization = request
				.extensions
				.get::<OrganizationId>()
				.ok_or_else(|| ViewError::Permission("organization scope is missing".to_owned()))?;
			Ok(Filter::new(
				"organization_id",
				FilterOperator::Eq,
				FilterValue::Integer(organization.0),
			)
			.into())
		});

		let queryset = handler.scoped_queryset(&request).unwrap();

		assert_eq!(queryset.filters().len(), 1);
		assert_eq!(queryset.filters()[0].field, "organization_id");
	}

	#[test]
	fn scoped_queryset_propagates_hook_errors() {
		let handler = ModelViewSetHandler::<TestItem>::new().with_queryset_fn(|_| {
			Err(ViewError::Permission(
				"organization scope is missing".to_owned(),
			))
		});

		let error = match handler.scoped_queryset(&build_request("/items/")) {
			Ok(_) => panic!("queryset hook error must propagate"),
			Err(error) => error,
		};

		assert!(
			matches!(error, ViewError::Permission(message) if message == "organization scope is missing")
		);
	}

	#[test]
	fn get_object_primary_key_filter_preserves_integer_type() {
		let filter =
			ModelViewSetHandler::<TestItem>::primary_key_filter(&serde_json::json!(42)).unwrap();
		let FilterCondition::Single(filter) = filter else {
			panic!("a scalar primary key should produce one filter");
		};

		assert_eq!(filter.field, "id");
		assert!(matches!(filter.value, FilterValue::Integer(42)));
	}

	#[tokio::test]
	async fn queryset_fn_without_pool_fails_closed() {
		let request = build_request("/items/");
		let handler = ModelViewSetHandler::<TestItem>::new()
			.with_queryset(vec![TestItem {
				id: Some(1),
				name: "visible".to_owned(),
			}])
			.with_queryset_fn(|_| {
				Ok(Filter::new("organization_id", FilterOperator::Eq, 1_i64.into()).into())
			});

		let error = handler.list(&request).await.unwrap_err();

		assert!(matches!(error, ViewError::Internal(_)));
	}

	#[tokio::test]
	async fn permission_denial_does_not_call_queryset_fn() {
		let hook_calls = Arc::new(AtomicUsize::new(0));
		let hook_calls_for_queryset = Arc::clone(&hook_calls);
		let handler = ModelViewSetHandler::<TestItem>::new()
			.add_permission(Arc::new(IsAuthenticated))
			.with_queryset_fn(move |_| {
				hook_calls_for_queryset.fetch_add(1, Ordering::SeqCst);
				Ok(Filter::new("organization_id", FilterOperator::Eq, 1_i64.into()).into())
			});

		let error = handler.list(&build_request("/items/")).await.unwrap_err();

		assert!(matches!(error, ViewError::Permission(_)));
		assert_eq!(hook_calls.load(Ordering::SeqCst), 0);
	}

	#[tokio::test]
	async fn create_does_not_call_queryset_fn() {
		let hook_calls = Arc::new(AtomicUsize::new(0));
		let hook_calls_for_queryset = Arc::clone(&hook_calls);
		let handler = ModelViewSetHandler::<TestItem>::new().with_queryset_fn(move |_| {
			hook_calls_for_queryset.fetch_add(1, Ordering::SeqCst);
			Ok(Filter::new("organization_id", FilterOperator::Eq, 1_i64.into()).into())
		});
		let request = Request::builder()
			.method(Method::POST)
			.uri("/items/")
			.body(Bytes::from_static(br#"{"id":null,"name":"created"}"#))
			.build()
			.unwrap();

		let response = handler.create(&request).await.unwrap();

		assert_eq!(response.status, hyper::StatusCode::CREATED);
		assert_eq!(hook_calls.load(Ordering::SeqCst), 0);
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
