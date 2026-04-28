//! Composite API Views that combine multiple operations

use crate::viewsets::{FilterConfig, PaginationConfig};
use async_trait::async_trait;
use hyper::Method;
use reinhardt_core::exception::{Error, Result};
use reinhardt_db::orm::{Filter, FilterOperator, FilterValue, Manager, Model, QuerySet};
use reinhardt_http::{Request, Response};
use reinhardt_rest::serializers::{Serializer, ValidatorConfig};
use serde::{Deserialize, Serialize};
use std::marker::PhantomData;

use crate::core::View;

/// ListCreateAPIView combines list and create operations
///
/// This view allows clients to:
/// - GET: List all objects (with pagination, filtering, ordering)
/// - POST: Create a new object
///
/// Similar to Django REST Framework's ListCreateAPIView.
///
/// # Examples
///
/// ```rust,no_run
/// use reinhardt_views::ListCreateAPIView;
/// use reinhardt_db::orm::Model;
/// use reinhardt_rest::serializers::JsonSerializer;
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
/// let view = ListCreateAPIView::<Article, JsonSerializer<Article>>::new()
///     .with_paginate_by(10);
/// ```
pub struct ListCreateAPIView<M, S>
where
	M: Model + Serialize + for<'de> Deserialize<'de> + Send + Sync + Clone,
	S: Serializer<Input = M, Output = String> + Send + Sync,
{
	queryset: Option<QuerySet<M>>,
	pagination_config: Option<PaginationConfig>,
	filter_config: Option<FilterConfig>,
	ordering: Option<Vec<String>>,
	validation_config: Option<ValidatorConfig<M>>,
	_serializer: PhantomData<S>,
}

impl<M, S> ListCreateAPIView<M, S>
where
	M: Model + Serialize + for<'de> Deserialize<'de> + Send + Sync + Clone + 'static,
	S: Serializer<Input = M, Output = String> + Send + Sync + 'static,
{
	/// Create a new `ListCreateAPIView` with default settings.
	pub fn new() -> Self {
		Self {
			queryset: None,
			pagination_config: None,
			filter_config: None,
			ordering: None,
			validation_config: None,
			_serializer: PhantomData,
		}
	}

	/// Set the queryset for this view.
	pub fn with_queryset(mut self, queryset: QuerySet<M>) -> Self {
		self.queryset = Some(queryset);
		self
	}

	/// Set the page size for pagination.
	pub fn with_paginate_by(mut self, page_size: usize) -> Self {
		self.pagination_config = Some(PaginationConfig::page_number(page_size, Some(100)));
		self
	}

	/// Sets the filter configuration
	pub fn with_filter_config(mut self, filter_config: FilterConfig) -> Self {
		self.filter_config = Some(filter_config);
		self
	}

	/// Set the default ordering fields for list results.
	pub fn with_ordering(mut self, ordering: Vec<String>) -> Self {
		self.ordering = Some(ordering);
		self
	}

	/// Gets the queryset, creating a default one if not set
	fn get_queryset(&self) -> QuerySet<M> {
		self.queryset.clone().unwrap_or_default()
	}

	/// Builds a filtered queryset with ordering applied, before pagination.
	fn get_filtered_queryset(&self, request: &Request) -> QuerySet<M> {
		let mut queryset = self.get_queryset();

		// Apply ordering if configured
		if let Some(ref ordering) = self.ordering {
			let order_fields: Vec<&str> = ordering.iter().map(|s| s.as_str()).collect();
			queryset = queryset.order_by(&order_fields);
		}

		// Apply filtering based on request query parameters
		if let Some(ref filter_config) = self.filter_config {
			for field in &filter_config.filterable_fields {
				if let Some(value) = request.query_params.get(field) {
					let filter = Filter::new(
						field.clone(),
						FilterOperator::Eq,
						FilterValue::String(value.clone()),
					);
					queryset = queryset.filter(filter);
				}
			}
		}

		queryset
	}

	/// Gets the objects to display with pagination applied.
	async fn get_objects(&self, request: &Request) -> Result<Vec<M>> {
		let mut queryset = self.get_filtered_queryset(request);

		// Apply pagination based on request parameters
		if let Some(ref pagination) = self.pagination_config {
			match pagination {
				PaginationConfig::PageNumber { page_size, .. } => {
					let page = request
						.query_params
						.get("page")
						.and_then(|p| p.parse::<usize>().ok())
						.unwrap_or(1);
					queryset = queryset.paginate(page, *page_size);
				}
				PaginationConfig::LimitOffset {
					default_limit,
					max_limit,
				} => {
					let limit = request
						.query_params
						.get("limit")
						.and_then(|l| l.parse::<usize>().ok())
						.unwrap_or(*default_limit)
						.min(max_limit.unwrap_or(usize::MAX));
					let offset = request
						.query_params
						.get("offset")
						.and_then(|o| o.parse::<usize>().ok())
						.unwrap_or(0);
					queryset = queryset.offset(offset).limit(limit);
				}
				PaginationConfig::Cursor { page_size, .. } => {
					// For cursor pagination, just apply page_size as limit
					queryset = queryset.limit(*page_size);
				}
				PaginationConfig::None => {
					// No pagination - return all objects
				}
			}
		}

		queryset.all().await.map_err(|e| Error::Http(e.to_string()))
	}
}

impl<M, S> Default for ListCreateAPIView<M, S>
where
	M: Model + Serialize + for<'de> Deserialize<'de> + Send + Sync + Clone + 'static,
	S: Serializer<Input = M, Output = String> + Send + Sync + 'static,
{
	fn default() -> Self {
		Self::new()
	}
}

#[async_trait]
impl<M, S> View for ListCreateAPIView<M, S>
where
	M: Model + Serialize + for<'de> Deserialize<'de> + Send + Sync + Clone + 'static,
	S: Serializer<Input = M, Output = String> + Send + Sync + 'static + Default,
{
	async fn dispatch(&self, request: Request) -> Result<Response> {
		match request.method {
			Method::GET | Method::HEAD => {
				// List logic (from ListAPIView pattern)
				let objects = self.get_objects(&request).await?;

				// Serialize the objects
				let serializer = S::default();
				let serialized = objects
					.iter()
					.map(|obj| {
						serializer
							.serialize(obj)
							.map_err(|e| Error::Http(e.to_string()))
					})
					.collect::<Result<Vec<_>>>()?;

				// Build response with pagination metadata
				let results: Vec<serde_json::Value> = serialized
					.iter()
					.filter_map(|s| serde_json::from_str::<serde_json::Value>(s).ok())
					.collect();

				let response_body = if let Some(ref pagination) = self.pagination_config {
					// Get total count from the filtered queryset (before pagination)
					let total_count = self
						.get_filtered_queryset(&request)
						.count()
						.await
						.map_err(|e| Error::Http(e.to_string()))?;

					match pagination {
						PaginationConfig::PageNumber { page_size, .. } => {
							let page = request
								.query_params
								.get("page")
								.and_then(|p| p.parse::<usize>().ok())
								.unwrap_or(1);
							let has_next = page.saturating_mul(*page_size) < total_count;
							serde_json::json!({
								"count": total_count,
								"page": page,
								"page_size": page_size,
								"next": if has_next { Some(format!("?page={}", page + 1)) } else { None::<String> },
								"previous": if page > 1 { Some(format!("?page={}", page - 1)) } else { None::<String> },
								"results": results
							})
						}
						PaginationConfig::LimitOffset { .. } => {
							let offset = request
								.query_params
								.get("offset")
								.and_then(|o| o.parse::<usize>().ok())
								.unwrap_or(0);
							let limit = request
								.query_params
								.get("limit")
								.and_then(|l| l.parse::<usize>().ok())
								.unwrap_or(10);
							let has_next = offset.saturating_add(limit) < total_count;
							serde_json::json!({
								"count": total_count,
								"offset": offset,
								"limit": limit,
								"next": if has_next { Some(format!("?offset={}&limit={}", offset.saturating_add(limit), limit)) } else { None::<String> },
								"previous": if offset > 0 { Some(format!("?offset={}&limit={}", offset.saturating_sub(limit), limit)) } else { None::<String> },
								"results": results
							})
						}
						_ => {
							serde_json::json!({
								"count": total_count,
								"results": results
							})
						}
					}
				} else {
					serde_json::json!(results)
				};

				Response::ok().with_json(&response_body)
			}
			Method::POST => {
				// Create logic
				let data: M = request
					.json()
					.map_err(|e| Error::Http(format!("Invalid request body: {}", e)))?;

				// Apply validation if configured
				if let Some(ref validators) = self.validation_config
					&& let Some(di_ctx) =
						request.get_di_context::<std::sync::Arc<reinhardt_di::InjectionContext>>()
				{
					use reinhardt_db::DatabaseConnection;
					use reinhardt_di::Depends;

					let conn = Depends::<DatabaseConnection>::resolve(&di_ctx, true)
						.await
						.map_err(|e| Error::Internal(format!("Failed to resolve DB: {:?}", e)))?;

					validators
						.validate_async(conn.into_inner().inner(), &data, None)
						.await?;
				}

				let queryset = self.get_queryset();
				let created = queryset
					.create(data)
					.await
					.map_err(|e| Error::Http(format!("Failed to create: {}", e)))?;

				// Serialize the created object
				let serializer = S::default();
				let serialized = serializer
					.serialize(&created)
					.map_err(|e| Error::Http(e.to_string()))?;

				// Parse to JSON value for response
				let json_value: serde_json::Value = serde_json::from_str(&serialized)
					.map_err(|e| Error::Http(format!("Serialization error: {}", e)))?;

				Response::created().with_json(&json_value)
			}
			_ => Err(Error::MethodNotAllowed(format!(
				"Method {} not allowed",
				request.method
			))),
		}
	}

	fn allowed_methods(&self) -> Vec<&'static str> {
		vec!["GET", "HEAD", "POST", "OPTIONS"]
	}
}

// Manually re-assert `UnwindSafe` / `RefUnwindSafe`. `ListCreateAPIView` holds
// an `Option<ValidatorConfig<M>>`, whose auto-trait state was lost when
// `Vec<Arc<dyn ModelLevelValidator<M>>>` was added to `ValidatorConfig`.
// Without these impls, cargo-semver-checks reports `auto_trait_impl_removed`
// during the RC phase. See the matching block in viewset.rs for soundness
// rationale.
impl<M, S> std::panic::UnwindSafe for ListCreateAPIView<M, S>
where
	M: Model + Serialize + for<'de> Deserialize<'de> + Send + Sync + Clone,
	S: Serializer<Input = M, Output = String> + Send + Sync,
{
}
impl<M, S> std::panic::RefUnwindSafe for ListCreateAPIView<M, S>
where
	M: Model + Serialize + for<'de> Deserialize<'de> + Send + Sync + Clone,
	S: Serializer<Input = M, Output = String> + Send + Sync,
{
}

/// RetrieveUpdateAPIView combines retrieve and update operations
///
/// This view allows clients to:
/// - GET: Retrieve a single object
/// - PUT/PATCH: Update an existing object
///
/// Similar to Django REST Framework's RetrieveUpdateAPIView.
pub struct RetrieveUpdateAPIView<M, S>
where
	M: Model + Serialize + for<'de> Deserialize<'de> + Send + Sync + Clone,
	S: Serializer<Input = M, Output = String> + Send + Sync,
{
	queryset: Option<QuerySet<M>>,
	lookup_field: String,
	_serializer: PhantomData<S>,
}

impl<M, S> RetrieveUpdateAPIView<M, S>
where
	M: Model + Serialize + for<'de> Deserialize<'de> + Send + Sync + Clone + 'static,
	S: Serializer<Input = M, Output = String> + Send + Sync + 'static,
{
	/// Create a new `RetrieveUpdateAPIView` with default settings.
	pub fn new() -> Self {
		Self {
			queryset: None,
			lookup_field: "pk".to_string(),
			_serializer: PhantomData,
		}
	}

	/// Set the queryset for this view.
	pub fn with_queryset(mut self, queryset: QuerySet<M>) -> Self {
		self.queryset = Some(queryset);
		self
	}

	/// Set the field used for object lookup.
	pub fn with_lookup_field(mut self, field: String) -> Self {
		self.lookup_field = field;
		self
	}

	/// Gets the queryset, creating a default one if not set
	fn get_queryset(&self) -> QuerySet<M> {
		self.queryset.clone().unwrap_or_default()
	}

	/// Gets a single object by lookup field value from request path params
	async fn get_object(&self, request: &Request) -> Result<M>
	where
		M: serde::de::DeserializeOwned,
	{
		let lookup_value = request.path_params.get(&self.lookup_field).ok_or_else(|| {
			Error::Http(format!(
				"Missing lookup field '{}' in path parameters",
				self.lookup_field
			))
		})?;

		// Try to parse as i64 first (common for primary keys), fallback to string
		let filter_value = if let Ok(int_value) = lookup_value.parse::<i64>() {
			FilterValue::Integer(int_value)
		} else {
			FilterValue::String(lookup_value.clone())
		};

		let filter = Filter::new(self.lookup_field.clone(), FilterOperator::Eq, filter_value);

		self.get_queryset()
			.filter(filter)
			.get()
			.await
			.map_err(|e| Error::Http(format!("Object not found: {}", e)))
	}
}

impl<M, S> Default for RetrieveUpdateAPIView<M, S>
where
	M: Model + Serialize + for<'de> Deserialize<'de> + Send + Sync + Clone + 'static,
	S: Serializer<Input = M, Output = String> + Send + Sync + 'static,
{
	fn default() -> Self {
		Self::new()
	}
}

#[async_trait]
impl<M, S> View for RetrieveUpdateAPIView<M, S>
where
	M: Model + Serialize + for<'de> Deserialize<'de> + Send + Sync + Clone + 'static,
	S: Serializer<Input = M, Output = String> + Send + Sync + 'static + Default,
{
	async fn dispatch(&self, request: Request) -> Result<Response> {
		match request.method {
			Method::GET | Method::HEAD => {
				// Retrieve logic
				let object = self.get_object(&request).await?;

				// Serialize the object
				let serializer = S::default();
				let serialized = serializer
					.serialize(&object)
					.map_err(|e| Error::Http(e.to_string()))?;

				// Parse to JSON value for response
				let json_value: serde_json::Value = serde_json::from_str(&serialized)
					.map_err(|e| Error::Http(format!("Serialization error: {}", e)))?;

				Response::ok().with_json(&json_value)
			}
			Method::PUT => {
				// Full update logic - replace all fields
				let mut object = self.get_object(&request).await?;
				let update_data: M = request
					.json()
					.map_err(|e| Error::Http(format!("Invalid request body: {}", e)))?;

				// Get the primary key from existing object to preserve identity
				let pk = object
					.primary_key()
					.ok_or_else(|| Error::Http("Object has no primary key".to_string()))?;

				// Replace object with update data but keep the same PK
				object = update_data;
				object.set_primary_key(pk);

				// Update using Manager
				let manager = Manager::<M>::new();
				let updated = manager
					.update(&object)
					.await
					.map_err(|e| Error::Http(format!("Failed to update: {}", e)))?;

				// Serialize the updated object
				let serializer = S::default();
				let serialized = serializer
					.serialize(&updated)
					.map_err(|e| Error::Http(e.to_string()))?;

				let json_value: serde_json::Value = serde_json::from_str(&serialized)
					.map_err(|e| Error::Http(format!("Serialization error: {}", e)))?;

				Response::ok().with_json(&json_value)
			}
			Method::PATCH => {
				// Partial update logic - only update provided fields
				let object = self.get_object(&request).await?;

				// Serialize current object to JSON
				let serializer = S::default();
				let current_json = serializer
					.serialize(&object)
					.map_err(|e| Error::Http(e.to_string()))?;

				// Parse current object as JSON value
				let mut current: serde_json::Value = serde_json::from_str(&current_json)
					.map_err(|e| Error::Http(format!("Serialization error: {}", e)))?;

				// Parse patch data
				let patch_data: serde_json::Value = request
					.json()
					.map_err(|e| Error::Http(format!("Invalid request body: {}", e)))?;

				// Validate and merge patch data into current object
				crate::generic::patch_utils::merge_patch_object_into(&mut current, &patch_data)
					.map_err(Error::Http)?;

				// Deserialize merged object back to model
				let merged: M = serde_json::from_value(current)
					.map_err(|e| Error::Http(format!("Failed to merge patch: {}", e)))?;

				// Update using Manager
				let manager = Manager::<M>::new();
				let updated = manager
					.update(&merged)
					.await
					.map_err(|e| Error::Http(format!("Failed to update: {}", e)))?;

				// Serialize the updated object
				let serialized = serializer
					.serialize(&updated)
					.map_err(|e| Error::Http(e.to_string()))?;

				let json_value: serde_json::Value = serde_json::from_str(&serialized)
					.map_err(|e| Error::Http(format!("Serialization error: {}", e)))?;

				Response::ok().with_json(&json_value)
			}
			_ => Err(Error::MethodNotAllowed(format!(
				"Method {} not allowed",
				request.method
			))),
		}
	}

	fn allowed_methods(&self) -> Vec<&'static str> {
		vec!["GET", "HEAD", "PUT", "PATCH", "OPTIONS"]
	}
}

/// RetrieveDestroyAPIView combines retrieve and destroy operations
///
/// This view allows clients to:
/// - GET: Retrieve a single object
/// - DELETE: Delete an existing object
///
/// Similar to Django REST Framework's RetrieveDestroyAPIView.
pub struct RetrieveDestroyAPIView<M, S>
where
	M: Model + Serialize + for<'de> Deserialize<'de> + Send + Sync + Clone,
	S: Serializer<Input = M, Output = String> + Send + Sync,
{
	queryset: Option<QuerySet<M>>,
	lookup_field: String,
	_serializer: PhantomData<S>,
}

impl<M, S> RetrieveDestroyAPIView<M, S>
where
	M: Model + Serialize + for<'de> Deserialize<'de> + Send + Sync + Clone + 'static,
	S: Serializer<Input = M, Output = String> + Send + Sync + 'static,
{
	/// Create a new `RetrieveDestroyAPIView` with default settings.
	pub fn new() -> Self {
		Self {
			queryset: None,
			lookup_field: "pk".to_string(),
			_serializer: PhantomData,
		}
	}

	/// Set the queryset for this view.
	pub fn with_queryset(mut self, queryset: QuerySet<M>) -> Self {
		self.queryset = Some(queryset);
		self
	}

	/// Set the field used for object lookup.
	pub fn with_lookup_field(mut self, field: String) -> Self {
		self.lookup_field = field;
		self
	}

	/// Gets the queryset, creating a default one if not set
	fn get_queryset(&self) -> QuerySet<M> {
		self.queryset.clone().unwrap_or_default()
	}

	/// Gets a single object by lookup field value from request path params
	async fn get_object(&self, request: &Request) -> Result<M>
	where
		M: serde::de::DeserializeOwned,
	{
		let lookup_value = request.path_params.get(&self.lookup_field).ok_or_else(|| {
			Error::Http(format!(
				"Missing lookup field '{}' in path parameters",
				self.lookup_field
			))
		})?;

		// Try to parse as i64 first (common for primary keys), fallback to string
		let filter_value = if let Ok(int_value) = lookup_value.parse::<i64>() {
			FilterValue::Integer(int_value)
		} else {
			FilterValue::String(lookup_value.clone())
		};

		let filter = Filter::new(self.lookup_field.clone(), FilterOperator::Eq, filter_value);

		self.get_queryset()
			.filter(filter)
			.get()
			.await
			.map_err(|e| Error::Http(format!("Object not found: {}", e)))
	}
}

impl<M, S> Default for RetrieveDestroyAPIView<M, S>
where
	M: Model + Serialize + for<'de> Deserialize<'de> + Send + Sync + Clone + 'static,
	S: Serializer<Input = M, Output = String> + Send + Sync + 'static,
{
	fn default() -> Self {
		Self::new()
	}
}

#[async_trait]
impl<M, S> View for RetrieveDestroyAPIView<M, S>
where
	M: Model + Serialize + for<'de> Deserialize<'de> + Send + Sync + Clone + 'static,
	S: Serializer<Input = M, Output = String> + Send + Sync + 'static + Default,
{
	async fn dispatch(&self, request: Request) -> Result<Response> {
		match request.method {
			Method::GET | Method::HEAD => {
				// Retrieve logic
				let object = self.get_object(&request).await?;

				// Serialize the object
				let serializer = S::default();
				let serialized = serializer
					.serialize(&object)
					.map_err(|e| Error::Http(e.to_string()))?;

				// Parse to JSON value for response
				let json_value: serde_json::Value = serde_json::from_str(&serialized)
					.map_err(|e| Error::Http(format!("Serialization error: {}", e)))?;

				Response::ok().with_json(&json_value)
			}
			Method::DELETE => {
				// Destroy logic - get object first to ensure it exists
				let object = self.get_object(&request).await?;

				// Get the primary key for deletion
				let pk = object
					.primary_key()
					.ok_or_else(|| Error::Http("Object has no primary key".to_string()))?;

				// Delete using Manager
				let manager = Manager::<M>::new();
				manager
					.delete(pk)
					.await
					.map_err(|e| Error::Http(format!("Failed to delete: {}", e)))?;

				// Return 204 No Content
				Ok(Response::no_content())
			}
			_ => Err(Error::MethodNotAllowed(format!(
				"Method {} not allowed",
				request.method
			))),
		}
	}

	fn allowed_methods(&self) -> Vec<&'static str> {
		vec!["GET", "HEAD", "DELETE", "OPTIONS"]
	}
}

/// RetrieveUpdateDestroyAPIView combines retrieve, update, and destroy operations
///
/// This view allows clients to:
/// - GET: Retrieve a single object
/// - PUT/PATCH: Update an existing object
/// - DELETE: Delete an existing object
///
/// Similar to Django REST Framework's RetrieveUpdateDestroyAPIView.
pub struct RetrieveUpdateDestroyAPIView<M, S>
where
	M: Model + Serialize + for<'de> Deserialize<'de> + Send + Sync + Clone,
	S: Serializer<Input = M, Output = String> + Send + Sync,
{
	queryset: Option<QuerySet<M>>,
	lookup_field: String,
	_serializer: PhantomData<S>,
}

impl<M, S> RetrieveUpdateDestroyAPIView<M, S>
where
	M: Model + Serialize + for<'de> Deserialize<'de> + Send + Sync + Clone + 'static,
	S: Serializer<Input = M, Output = String> + Send + Sync + 'static,
{
	/// Create a new `RetrieveUpdateDestroyAPIView` with default settings.
	pub fn new() -> Self {
		Self {
			queryset: None,
			lookup_field: "pk".to_string(),
			_serializer: PhantomData,
		}
	}

	/// Set the queryset for this view.
	pub fn with_queryset(mut self, queryset: QuerySet<M>) -> Self {
		self.queryset = Some(queryset);
		self
	}

	/// Set the field used for object lookup.
	pub fn with_lookup_field(mut self, field: String) -> Self {
		self.lookup_field = field;
		self
	}

	/// Gets the queryset, creating a default one if not set
	fn get_queryset(&self) -> QuerySet<M> {
		self.queryset.clone().unwrap_or_default()
	}

	/// Gets a single object by lookup field value from request path params
	async fn get_object(&self, request: &Request) -> Result<M>
	where
		M: serde::de::DeserializeOwned,
	{
		let lookup_value = request.path_params.get(&self.lookup_field).ok_or_else(|| {
			Error::Http(format!(
				"Missing lookup field '{}' in path parameters",
				self.lookup_field
			))
		})?;

		// Try to parse as i64 first (common for primary keys), fallback to string
		let filter_value = if let Ok(int_value) = lookup_value.parse::<i64>() {
			FilterValue::Integer(int_value)
		} else {
			FilterValue::String(lookup_value.clone())
		};

		let filter = Filter::new(self.lookup_field.clone(), FilterOperator::Eq, filter_value);

		self.get_queryset()
			.filter(filter)
			.get()
			.await
			.map_err(|e| Error::Http(format!("Object not found: {}", e)))
	}
}

impl<M, S> Default for RetrieveUpdateDestroyAPIView<M, S>
where
	M: Model + Serialize + for<'de> Deserialize<'de> + Send + Sync + Clone + 'static,
	S: Serializer<Input = M, Output = String> + Send + Sync + 'static,
{
	fn default() -> Self {
		Self::new()
	}
}

#[async_trait]
impl<M, S> View for RetrieveUpdateDestroyAPIView<M, S>
where
	M: Model + Serialize + for<'de> Deserialize<'de> + Send + Sync + Clone + 'static,
	S: Serializer<Input = M, Output = String> + Send + Sync + 'static + Default,
{
	async fn dispatch(&self, request: Request) -> Result<Response> {
		match request.method {
			Method::GET | Method::HEAD => {
				// Retrieve logic
				let object = self.get_object(&request).await?;

				// Serialize the object
				let serializer = S::default();
				let serialized = serializer
					.serialize(&object)
					.map_err(|e| Error::Http(e.to_string()))?;

				// Parse to JSON value for response
				let json_value: serde_json::Value = serde_json::from_str(&serialized)
					.map_err(|e| Error::Http(format!("Serialization error: {}", e)))?;

				Response::ok().with_json(&json_value)
			}
			Method::PUT => {
				// Full update logic - replace all fields
				let mut object = self.get_object(&request).await?;
				let update_data: M = request
					.json()
					.map_err(|e| Error::Http(format!("Invalid request body: {}", e)))?;

				// Get the primary key from existing object to preserve identity
				let pk = object
					.primary_key()
					.ok_or_else(|| Error::Http("Object has no primary key".to_string()))?;

				// Replace object with update data but keep the same PK
				object = update_data;
				object.set_primary_key(pk);

				// Update using Manager
				let manager = Manager::<M>::new();
				let updated = manager
					.update(&object)
					.await
					.map_err(|e| Error::Http(format!("Failed to update: {}", e)))?;

				// Serialize the updated object
				let serializer = S::default();
				let serialized = serializer
					.serialize(&updated)
					.map_err(|e| Error::Http(e.to_string()))?;

				let json_value: serde_json::Value = serde_json::from_str(&serialized)
					.map_err(|e| Error::Http(format!("Serialization error: {}", e)))?;

				Response::ok().with_json(&json_value)
			}
			Method::PATCH => {
				// Partial update logic - only update provided fields
				let object = self.get_object(&request).await?;

				// Serialize current object to JSON
				let serializer = S::default();
				let current_json = serializer
					.serialize(&object)
					.map_err(|e| Error::Http(e.to_string()))?;

				// Parse current object as JSON value
				let mut current: serde_json::Value = serde_json::from_str(&current_json)
					.map_err(|e| Error::Http(format!("Serialization error: {}", e)))?;

				// Parse patch data
				let patch_data: serde_json::Value = request
					.json()
					.map_err(|e| Error::Http(format!("Invalid request body: {}", e)))?;

				// Validate and merge patch data into current object
				crate::generic::patch_utils::merge_patch_object_into(&mut current, &patch_data)
					.map_err(Error::Http)?;

				// Deserialize merged object back to model
				let merged: M = serde_json::from_value(current)
					.map_err(|e| Error::Http(format!("Failed to merge patch: {}", e)))?;

				// Update using Manager
				let manager = Manager::<M>::new();
				let updated = manager
					.update(&merged)
					.await
					.map_err(|e| Error::Http(format!("Failed to update: {}", e)))?;

				// Serialize the updated object
				let serialized = serializer
					.serialize(&updated)
					.map_err(|e| Error::Http(e.to_string()))?;

				let json_value: serde_json::Value = serde_json::from_str(&serialized)
					.map_err(|e| Error::Http(format!("Serialization error: {}", e)))?;

				Response::ok().with_json(&json_value)
			}
			Method::DELETE => {
				// Destroy logic - get object first to ensure it exists
				let object = self.get_object(&request).await?;

				// Get the primary key for deletion
				let pk = object
					.primary_key()
					.ok_or_else(|| Error::Http("Object has no primary key".to_string()))?;

				// Delete using Manager
				let manager = Manager::<M>::new();
				manager
					.delete(pk)
					.await
					.map_err(|e| Error::Http(format!("Failed to delete: {}", e)))?;

				// Return 204 No Content
				Ok(Response::no_content())
			}
			_ => Err(Error::MethodNotAllowed(format!(
				"Method {} not allowed",
				request.method
			))),
		}
	}

	fn allowed_methods(&self) -> Vec<&'static str> {
		vec!["GET", "HEAD", "PUT", "PATCH", "DELETE", "OPTIONS"]
	}
}
