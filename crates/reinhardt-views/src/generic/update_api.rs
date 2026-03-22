//! UpdateAPIView implementation for updating objects

use async_trait::async_trait;
use hyper::Method;
use reinhardt_core::exception::{Error, Result};
use reinhardt_db::orm::{Filter, FilterOperator, FilterValue, Manager, Model, QuerySet};
use reinhardt_http::{Request, Response};
use reinhardt_rest::serializers::Serializer;
use serde::{Deserialize, Serialize};
use std::marker::PhantomData;

use crate::core::View;

/// UpdateAPIView for updating existing objects
///
/// Similar to Django REST Framework's UpdateAPIView, this view provides
/// update-only access with support for both full updates (PUT) and
/// partial updates (PATCH).
///
/// # Type Parameters
///
/// * `M` - The model type (must implement `Model`, `Serialize`, `Deserialize`)
/// * `S` - The serializer type (must implement `Serializer`)
///
/// # Examples
///
/// ```rust,no_run
/// use reinhardt_views::UpdateAPIView;
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
/// let view = UpdateAPIView::<Article, JsonSerializer<Article>>::new()
///     .with_lookup_field("id".to_string());
/// ```
pub struct UpdateAPIView<M, S>
where
	M: Model + Serialize + for<'de> Deserialize<'de> + Send + Sync + Clone,
	S: Serializer<Input = M, Output = String> + Send + Sync,
{
	queryset: Option<QuerySet<M>>,
	lookup_field: String,
	partial: bool,
	_serializer: PhantomData<S>,
}

impl<M, S> UpdateAPIView<M, S>
where
	M: Model + Serialize + for<'de> Deserialize<'de> + Send + Sync + Clone + 'static,
	S: Serializer<Input = M, Output = String> + Send + Sync + 'static + Default,
{
	/// Creates a new `UpdateAPIView` with default settings
	///
	/// # Examples
	///
	/// ```rust,no_run
	/// use reinhardt_views::UpdateAPIView;
	/// use reinhardt_rest::serializers::JsonSerializer;
	/// # use reinhardt_db::orm::Model;
	/// # use serde::{Serialize, Deserialize};
	/// # #[derive(Debug, Clone, Serialize, Deserialize)]
	/// # struct Article { id: Option<i64>, title: String }
	/// # #[derive(Clone)]
	/// # struct ArticleFields;
	/// # impl reinhardt_db::orm::FieldSelector for ArticleFields {
	/// #     fn with_alias(self, _alias: &str) -> Self { self }
	/// # }
	/// # impl Model for Article {
	/// #     type PrimaryKey = i64;
	/// #     type Fields = ArticleFields;
	/// #     fn table_name() -> &'static str { "articles" }
	/// #     fn primary_key(&self) -> Option<Self::PrimaryKey> { self.id }
	/// #     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
	/// #     fn new_fields() -> Self::Fields { ArticleFields }
	/// # }
	///
	/// let view = UpdateAPIView::<Article, JsonSerializer<Article>>::new();
	/// ```
	pub fn new() -> Self {
		Self {
			queryset: None,
			lookup_field: "pk".to_string(),
			partial: false,
			_serializer: PhantomData,
		}
	}

	/// Sets the queryset for this view
	pub fn with_queryset(mut self, queryset: QuerySet<M>) -> Self {
		self.queryset = Some(queryset);
		self
	}

	/// Sets the lookup field for object retrieval
	///
	/// # Examples
	///
	/// ```rust,no_run
	/// # use reinhardt_views::UpdateAPIView;
	/// # use reinhardt_rest::serializers::JsonSerializer;
	/// # use reinhardt_db::orm::Model;
	/// # use serde::{Serialize, Deserialize};
	/// # #[derive(Debug, Clone, Serialize, Deserialize)]
	/// # struct Article { id: Option<i64>, title: String }
	/// # #[derive(Clone)]
	/// # struct ArticleFields;
	/// # impl reinhardt_db::orm::FieldSelector for ArticleFields {
	/// #     fn with_alias(self, _alias: &str) -> Self { self }
	/// # }
	/// # impl Model for Article {
	/// #     type PrimaryKey = i64;
	/// #     type Fields = ArticleFields;
	/// #     fn table_name() -> &'static str { "articles" }
	/// #     fn primary_key(&self) -> Option<Self::PrimaryKey> { self.id }
	/// #     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
	/// #     fn new_fields() -> Self::Fields { ArticleFields }
	/// # }
	///
	/// let view = UpdateAPIView::<Article, JsonSerializer<Article>>::new()
	///     .with_lookup_field("slug".to_string());
	/// ```
	pub fn with_lookup_field(mut self, field: String) -> Self {
		self.lookup_field = field;
		self
	}

	/// Enables partial updates (PATCH)
	pub fn with_partial(mut self, partial: bool) -> Self {
		self.partial = partial;
		self
	}

	/// Gets the queryset, creating a default one if not set
	fn get_queryset(&self) -> QuerySet<M> {
		self.queryset.clone().unwrap_or_default()
	}

	/// Retrieves the object to update by lookup field value from request path params
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

	/// Performs the object update
	///
	/// Determines partial vs full update based on the actual HTTP method:
	/// PATCH always performs partial update, PUT always performs full replacement.
	async fn perform_update(&self, request: &Request) -> Result<M>
	where
		M: serde::de::DeserializeOwned,
	{
		let mut object = self.get_object(request).await?;

		// Determine partial update from the HTTP method, not the `self.partial` field.
		// PATCH = partial update, PUT = full replacement.
		let is_partial = request.method == Method::PATCH;

		if is_partial {
			// PATCH: partial update - merge provided fields into existing object
			let serializer = S::default();
			let current_json = serializer
				.serialize(&object)
				.map_err(|e| Error::Http(e.to_string()))?;

			let mut current: serde_json::Value = serde_json::from_str(&current_json)
				.map_err(|e| Error::Http(format!("Serialization error: {}", e)))?;

			let patch_data: serde_json::Value = request
				.json()
				.map_err(|e| Error::Http(format!("Invalid request body: {}", e)))?;

			// Validate that patch data is a JSON object
			let patch_obj = patch_data.as_object().ok_or_else(|| {
				Error::Http("PATCH request body must be a JSON object".to_string())
			})?;

			// Merge patch data into current object
			if let Some(current_obj) = current.as_object_mut() {
				for (key, value) in patch_obj {
					current_obj.insert(key.clone(), value.clone());
				}
			}

			object = serde_json::from_value(current)
				.map_err(|e| Error::Http(format!("Failed to merge patch: {}", e)))?;
		} else {
			// PUT: full update - replace all fields but keep the same PK
			let update_data: M = request
				.json()
				.map_err(|e| Error::Http(format!("Invalid request body: {}", e)))?;

			let pk = object
				.primary_key()
				.ok_or_else(|| Error::Http("Object has no primary key".to_string()))?;

			object = update_data;
			object.set_primary_key(pk);
		}

		// Update using Manager
		let manager = Manager::<M>::new();
		manager
			.update(&object)
			.await
			.map_err(|e| Error::Http(format!("Failed to update: {}", e)))
	}
}

impl<M, S> Default for UpdateAPIView<M, S>
where
	M: Model + Serialize + for<'de> Deserialize<'de> + Send + Sync + Clone + 'static,
	S: Serializer<Input = M, Output = String> + Send + Sync + 'static + Default,
{
	fn default() -> Self {
		Self::new()
	}
}

#[async_trait]
impl<M, S> View for UpdateAPIView<M, S>
where
	M: Model + Serialize + for<'de> Deserialize<'de> + Send + Sync + Clone + 'static,
	S: Serializer<Input = M, Output = String> + Send + Sync + 'static + Default,
{
	async fn dispatch(&self, request: Request) -> Result<Response> {
		match request.method {
			Method::PUT | Method::PATCH => {
				let obj = self.perform_update(&request).await?;

				// Serialize the updated object
				let serializer = S::default();
				let serialized = serializer
					.serialize(&obj)
					.map_err(|e| Error::Http(e.to_string()))?;

				// Parse to JSON value for response
				let json_value: serde_json::Value = serde_json::from_str(&serialized)
					.map_err(|e| Error::Http(format!("Failed to parse serialized data: {}", e)))?;

				Response::ok()
					.with_json(&json_value)
					.map_err(|e| Error::Http(e.to_string()))
			}
			_ => Err(Error::MethodNotAllowed(format!(
				"Method {} not allowed",
				request.method
			))),
		}
	}

	fn allowed_methods(&self) -> Vec<&'static str> {
		vec!["PUT", "PATCH", "OPTIONS"]
	}
}
