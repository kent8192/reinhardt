//! DestroyAPIView implementation for deleting objects

use async_trait::async_trait;
use hyper::Method;
use reinhardt_core::exception::{Error, Result};
use reinhardt_db::orm::{Filter, FilterOperator, FilterValue, Manager, Model, QuerySet};
use reinhardt_http::{Request, Response};
use serde::{Deserialize, Serialize};
use std::marker::PhantomData;

use crate::core::View;

/// DestroyAPIView for deleting objects
///
/// Similar to Django REST Framework's DestroyAPIView, this view provides
/// delete-only access to model instances.
///
/// # Type Parameters
///
/// * `M` - The model type (must implement `Model`, `Serialize`, `Deserialize`)
///
/// # Examples
///
/// ```rust,no_run
/// use reinhardt_views::DestroyAPIView;
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
/// let view = DestroyAPIView::<Article>::new()
///     .with_lookup_field("id".to_string());
/// ```
pub struct DestroyAPIView<M>
where
	M: Model + Serialize + for<'de> Deserialize<'de> + Send + Sync + Clone,
{
	queryset: Option<QuerySet<M>>,
	lookup_field: String,
	_model: PhantomData<M>,
}

impl<M> DestroyAPIView<M>
where
	M: Model + Serialize + for<'de> Deserialize<'de> + Send + Sync + Clone + 'static,
{
	/// Creates a new `DestroyAPIView` with default settings
	///
	/// # Examples
	///
	/// ```rust,no_run
	/// use reinhardt_views::DestroyAPIView;
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
	/// let view = DestroyAPIView::<Article>::new();
	/// ```
	pub fn new() -> Self {
		Self {
			queryset: None,
			lookup_field: "pk".to_string(),
			_model: PhantomData,
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
	/// # use reinhardt_views::DestroyAPIView;
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
	/// let view = DestroyAPIView::<Article>::new()
	///     .with_lookup_field("slug".to_string());
	/// ```
	pub fn with_lookup_field(mut self, field: String) -> Self {
		self.lookup_field = field;
		self
	}

	/// Gets the queryset, creating a default one if not set
	fn get_queryset(&self) -> QuerySet<M> {
		self.queryset.clone().unwrap_or_default()
	}

	/// Retrieves the object to delete by lookup field value from request path params
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

	/// Performs the object deletion
	async fn perform_destroy(&self, request: &Request) -> Result<()>
	where
		M: serde::de::DeserializeOwned,
	{
		// Get the object first to ensure it exists
		let object = self.get_object(request).await?;

		// Get the primary key for deletion
		let pk = object
			.primary_key()
			.ok_or_else(|| Error::Http("Object has no primary key".to_string()))?;

		// Delete using Manager
		let manager = Manager::<M>::new();
		manager
			.delete(pk)
			.await
			.map_err(|e| Error::Http(format!("Failed to delete: {}", e)))
	}
}

impl<M> Default for DestroyAPIView<M>
where
	M: Model + Serialize + for<'de> Deserialize<'de> + Send + Sync + Clone + 'static,
{
	fn default() -> Self {
		Self::new()
	}
}

#[async_trait]
impl<M> View for DestroyAPIView<M>
where
	M: Model + Serialize + for<'de> Deserialize<'de> + Send + Sync + Clone + 'static,
{
	async fn dispatch(&self, request: Request) -> Result<Response> {
		match request.method {
			Method::DELETE => {
				self.perform_destroy(&request).await?;

				// Return 204 No Content for successful deletion
				Ok(Response::no_content())
			}
			_ => Err(Error::MethodNotAllowed(format!(
				"Method {} not allowed",
				request.method
			))),
		}
	}

	fn allowed_methods(&self) -> Vec<&'static str> {
		vec!["DELETE", "OPTIONS"]
	}
}
