//! DetailView implementation for displaying a single object.

use crate::core::View;
use crate::mixins::SingleObjectMixin;
use async_trait::async_trait;
use reinhardt_core::exception::{Error, Result};
use reinhardt_db::orm::{
	Model, QuerySet,
	query::{Filter, FilterOperator, FilterValue},
};
use reinhardt_http::{Request, Response};
use reinhardt_rest::serializers::{JsonSerializer, Serializer};
use serde::{Deserialize, Serialize};

/// DetailView for displaying a single object
pub struct DetailView<T>
where
	T: Model + Serialize + for<'de> Deserialize<'de> + Send + Sync + Clone,
{
	object: Option<T>,
	queryset: Option<QuerySet<T>>,
	slug_field: String,
	pk_url_kwarg_name: String,
	slug_url_kwarg_name: String,
	context_object_name: Option<String>,
	serializer: Box<dyn Serializer<Input = T, Output = String> + Send + Sync>,
}

impl<T> Default for DetailView<T>
where
	T: Model + Serialize + for<'de> Deserialize<'de> + Send + Sync + Clone + 'static,
{
	fn default() -> Self {
		Self::new()
	}
}

impl<T> DetailView<T>
where
	T: Model + Serialize + for<'de> Deserialize<'de> + Send + Sync + Clone + 'static,
{
	/// Creates a new `DetailView` with default settings.
	///
	/// Uses `JsonSerializer` by default. Use `with_serializer` to provide a custom serializer.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_views::{DetailView, SingleObjectMixin};
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
	/// let view = DetailView::<Article>::new();
	/// assert_eq!(view.get_slug_field(), "slug");
	/// assert_eq!(view.pk_url_kwarg(), "pk");
	/// ```
	pub fn new() -> Self {
		Self {
			object: None,
			queryset: None,
			slug_field: "slug".to_string(),
			pk_url_kwarg_name: "pk".to_string(),
			slug_url_kwarg_name: "slug".to_string(),
			context_object_name: None,
			serializer: Box::new(JsonSerializer::<T>::new()),
		}
	}

	/// Sets a custom serializer for the view.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_views::{DetailView, SingleObjectMixin};
	/// use reinhardt_rest::serializers::JsonSerializer;
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
	/// let view = DetailView::<Article>::new()
	///     .with_serializer(Box::new(JsonSerializer::<Article>::new()));
	/// ```
	pub fn with_serializer(
		mut self,
		serializer: Box<dyn Serializer<Input = T, Output = String> + Send + Sync>,
	) -> Self {
		self.serializer = serializer;
		self
	}
	/// Sets the object to display in the view.
	///
	/// # Examples
	///
	/// ```ignore
	/// use reinhardt_views::{DetailView, SingleObjectMixin};
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
	/// let article = Article { id: Some(1), title: "Hello World".to_string() };
	/// let view = DetailView::<Article>::new()
	///     .with_object(article.clone());
	/// # tokio_test::block_on(async {
	/// use hyper::{Method, Version, HeaderMap};
	/// use bytes::Bytes;
	/// let request = Request::builder()
	///     .method(Method::GET)
	///     .uri("/")
	///     .version(Version::HTTP_11)
	///     .headers(HeaderMap::new())
	///     .body(Bytes::new())
	///     .build()
	///     .unwrap();
	/// let result = view.get_object(&request).await;
	/// assert!(result.is_ok());
	/// # });
	/// ```
	pub fn with_object(mut self, object: T) -> Self {
		self.object = Some(object);
		self
	}
	/// Sets the slug field name for object lookup.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_views::{DetailView, SingleObjectMixin};
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
	/// let view = DetailView::<Article>::new()
	///     .with_slug_field("title");
	/// assert_eq!(view.get_slug_field(), "title");
	/// ```
	pub fn with_slug_field(mut self, slug_field: impl Into<String>) -> Self {
		self.slug_field = slug_field.into();
		self
	}
	/// Sets the primary key URL parameter name.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_views::{DetailView, SingleObjectMixin};
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
	/// let view = DetailView::<Article>::new()
	///     .with_pk_url_kwarg("article_id");
	/// assert_eq!(view.pk_url_kwarg(), "article_id");
	/// ```
	pub fn with_pk_url_kwarg(mut self, kwarg: impl Into<String>) -> Self {
		self.pk_url_kwarg_name = kwarg.into();
		self
	}
	/// Sets the slug URL parameter name.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_views::{DetailView, SingleObjectMixin};
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
	/// let view = DetailView::<Article>::new()
	///     .with_slug_url_kwarg("article_slug");
	/// assert_eq!(view.slug_url_kwarg(), "article_slug");
	/// ```
	pub fn with_slug_url_kwarg(mut self, kwarg: impl Into<String>) -> Self {
		self.slug_url_kwarg_name = kwarg.into();
		self
	}
	/// Sets a custom name for the object in the context.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_views::{DetailView, SingleObjectMixin};
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
	/// let view = DetailView::<Article>::new()
	///     .with_context_object_name("article");
	/// assert_eq!(view.get_context_object_name(), Some("article"));
	/// ```
	pub fn with_context_object_name(mut self, name: impl Into<String>) -> Self {
		self.context_object_name = Some(name.into());
		self
	}
	/// Sets the QuerySet for database lookup.
	///
	/// When a QuerySet is configured, the view will use it to fetch objects from the database
	/// based on the primary key or slug URL parameter.
	///
	/// # Examples
	///
	/// ```rust,no_run
	/// use reinhardt_views::DetailView;
	/// use reinhardt_db::orm::{Model, QuerySet};
	/// use serde::{Serialize, Deserialize};
	///
	/// #[derive(Debug, Clone, Serialize, Deserialize)]
	/// struct Article {
	///     id: Option<i64>,
	///     title: String,
	///     slug: String,
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
	/// let queryset = QuerySet::<Article>::new();
	/// let view = DetailView::<Article>::new()
	///     .with_queryset(queryset);
	/// ```
	pub fn with_queryset(mut self, queryset: QuerySet<T>) -> Self {
		self.queryset = Some(queryset);
		self
	}
}

#[async_trait]
impl<T> SingleObjectMixin<T> for DetailView<T>
where
	T: Model + Serialize + for<'de> Deserialize<'de> + Send + Sync + Clone + 'static,
{
	async fn get_object(&self, request: &Request) -> Result<T> {
		// If object is already set, return it
		if let Some(ref object) = self.object {
			return Ok(object.clone());
		}

		// Get queryset or return error if not configured
		let queryset = self.queryset.as_ref().ok_or_else(|| {
			Error::NotFound(
				"No queryset configured. Set object directly with with_object() \
                 or configure queryset with with_queryset()."
					.to_string(),
			)
		})?;

		// Try to extract pk or slug from URL parameters
		let pk_kwarg = self.pk_url_kwarg();
		let slug_kwarg = self.slug_url_kwarg();

		// Try to get pk from URL parameters
		if let Some(pk_value) = request.path_params.get(pk_kwarg) {
			let pk_field = T::primary_key_field();

			// Try to parse as i64 first (most common primary key type)
			let filter_value = if let Ok(int_pk) = pk_value.parse::<i64>() {
				FilterValue::Integer(int_pk)
			} else {
				// Fallback to string if not a valid integer
				FilterValue::String(pk_value.clone())
			};

			let results = queryset
				.clone()
				.filter(Filter {
					field: pk_field.to_string(),
					operator: FilterOperator::Eq,
					value: filter_value,
				})
				.all()
				.await?;

			return results.into_iter().next().ok_or_else(|| {
				Error::NotFound(format!("Object with pk='{}' not found", pk_value))
			});
		}

		// Try to get slug from URL parameters
		if let Some(slug_value) = request.path_params.get(slug_kwarg) {
			let results = queryset
				.clone()
				.filter(Filter {
					field: self.get_slug_field().to_string(),
					operator: FilterOperator::Eq,
					value: FilterValue::String(slug_value.clone()),
				})
				.all()
				.await?;

			return results.into_iter().next().ok_or_else(|| {
				Error::NotFound(format!("Object with slug='{}' not found", slug_value))
			});
		}

		Err(Error::NotFound("No pk or slug provided in URL".to_string()))
	}

	fn get_slug_field(&self) -> &str {
		&self.slug_field
	}

	fn pk_url_kwarg(&self) -> &str {
		&self.pk_url_kwarg_name
	}

	fn slug_url_kwarg(&self) -> &str {
		&self.slug_url_kwarg_name
	}

	fn get_context_object_name(&self) -> Option<&str> {
		self.context_object_name.as_deref()
	}
}

#[async_trait]
impl<T> View for DetailView<T>
where
	T: Model + Serialize + for<'de> Deserialize<'de> + Send + Sync + Clone + 'static,
{
	async fn dispatch(&self, request: Request) -> Result<Response> {
		// Handle OPTIONS method
		if request.method == "OPTIONS" {
			let methods = self.allowed_methods().join(", ");
			return Ok(Response::ok()
				.with_header("Allow", &methods)
				.with_header("Content-Type", "application/json"));
		}

		// Support GET and HEAD methods
		let is_head = request.method == "HEAD";
		if !matches!(request.method.as_str(), "GET" | "HEAD") {
			return Err(Error::Validation(format!(
				"Method {} not allowed",
				request.method
			)));
		}

		// Get object - extracts pk/slug from URL parameters
		let object = self.get_object(&request).await?;

		// Serialize object
		let serialized = self.serializer.serialize(&object).map_err(|e| match e {
			reinhardt_rest::serializers::SerializerError::Validation(v) => {
				Error::Validation(v.to_string())
			}
			reinhardt_rest::serializers::SerializerError::Serde { message } => {
				Error::Serialization(message)
			}
			reinhardt_rest::serializers::SerializerError::Other { message } => {
				Error::Serialization(message)
			}
			_ => Error::Serialization(e.to_string()),
		})?;

		// Build response - for HEAD, return same headers but empty body
		if is_head {
			Ok(Response::ok().with_header("Content-Type", "application/json"))
		} else {
			Ok(Response::ok()
				.with_body(serialized)
				.with_header("Content-Type", "application/json"))
		}
	}
}
