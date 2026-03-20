//! RetrieveAPIView implementation for retrieving a single object

use async_trait::async_trait;
use hyper::Method;
use reinhardt_core::exception::{Error, Result};
use reinhardt_db::orm::{Filter, FilterOperator, FilterValue, Model, QuerySet};
use reinhardt_http::{Request, Response};
use reinhardt_rest::serializers::Serializer;
use serde::{Deserialize, Serialize};
use std::marker::PhantomData;

use crate::core::View;

/// RetrieveAPIView for retrieving a single object
///
/// Similar to Django REST Framework's RetrieveAPIView, this view provides
/// read-only access to a single model instance.
///
/// # Type Parameters
///
/// * `M` - The model type (must implement `Model`, `Serialize`, `Deserialize`)
/// * `S` - The serializer type (must implement `Serializer`)
///
/// # Examples
///
/// ```rust,no_run
/// use reinhardt_views::RetrieveAPIView;
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
/// let view = RetrieveAPIView::<Article, JsonSerializer<Article>>::new()
///     .with_lookup_field("id".to_string());
/// ```
pub struct RetrieveAPIView<M, S>
where
	M: Model + Serialize + for<'de> Deserialize<'de> + Send + Sync + Clone,
	S: Serializer<Input = M, Output = String> + Send + Sync,
{
	queryset: Option<QuerySet<M>>,
	lookup_field: String,
	_serializer: PhantomData<S>,
}

impl<M, S> RetrieveAPIView<M, S>
where
	M: Model + Serialize + for<'de> Deserialize<'de> + Send + Sync + Clone + 'static,
	S: Serializer<Input = M, Output = String> + Send + Sync + 'static,
{
	/// Creates a new `RetrieveAPIView` with default settings
	///
	/// # Examples
	///
	/// ```rust,no_run
	/// use reinhardt_views::RetrieveAPIView;
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
	/// let view = RetrieveAPIView::<Article, JsonSerializer<Article>>::new();
	/// ```
	pub fn new() -> Self {
		Self {
			queryset: None,
			lookup_field: "pk".to_string(),
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
	/// # use reinhardt_views::RetrieveAPIView;
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
	/// let view = RetrieveAPIView::<Article, JsonSerializer<Article>>::new()
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

impl<M, S> Default for RetrieveAPIView<M, S>
where
	M: Model + Serialize + for<'de> Deserialize<'de> + Send + Sync + Clone + 'static,
	S: Serializer<Input = M, Output = String> + Send + Sync + 'static,
{
	fn default() -> Self {
		Self::new()
	}
}

#[async_trait]
impl<M, S> View for RetrieveAPIView<M, S>
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
			_ => Err(Error::Http("Method not allowed".to_string())),
		}
	}

	fn allowed_methods(&self) -> Vec<&'static str> {
		vec!["GET", "HEAD", "OPTIONS"]
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;

	#[rstest]
	#[case("42", true)]
	#[case("0", true)]
	#[case("-1", true)]
	#[case("9999999999", true)]
	#[case("abc", false)]
	#[case("12abc", false)]
	#[case("", false)]
	#[case("3.14", false)]
	fn test_lookup_value_integer_parsing(#[case] input: &str, #[case] should_be_integer: bool) {
		// Arrange
		let lookup_value = input.to_string();

		// Act
		let filter_value = if let Ok(int_value) = lookup_value.parse::<i64>() {
			FilterValue::Integer(int_value)
		} else {
			FilterValue::String(lookup_value.clone())
		};

		// Assert
		match filter_value {
			FilterValue::Integer(_) => assert!(should_be_integer, "Expected String variant for input '{}'", input),
			FilterValue::String(_) => assert!(!should_be_integer, "Expected Integer variant for input '{}'", input),
			_ => panic!("Unexpected FilterValue variant"),
		}
	}
}
