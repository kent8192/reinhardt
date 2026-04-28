//! CreateAPIView implementation for creating objects

use async_trait::async_trait;
use hyper::Method;
use reinhardt_core::exception::{Error, Result};
use reinhardt_db::orm::{Model, QuerySet};
use reinhardt_http::{Request, Response};
use reinhardt_rest::serializers::{Serializer, ValidatorConfig};
use serde::{Deserialize, Serialize};
use std::marker::PhantomData;

use crate::core::View;

/// CreateAPIView for creating new objects
///
/// Similar to Django REST Framework's CreateAPIView, this view provides
/// create-only access with support for validation and serialization.
///
/// # Type Parameters
///
/// * `M` - The model type (must implement `Model`, `Serialize`, `Deserialize`)
/// * `S` - The serializer type (must implement `Serializer`)
///
/// # Examples
///
/// ```rust,no_run
/// use reinhardt_views::CreateAPIView;
/// use reinhardt_db::orm::Model;
/// use reinhardt_rest::serializers::JsonSerializer;
/// use serde::{Serialize, Deserialize};
///
/// #[derive(Serialize, Deserialize, Clone, Debug)]
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
/// let view = CreateAPIView::<Article, JsonSerializer<Article>>::new();
/// ```
pub struct CreateAPIView<M, S>
where
	M: Model + Serialize + for<'de> Deserialize<'de> + Send + Sync + Clone,
	S: Serializer<Input = M, Output = String> + Send + Sync,
{
	queryset: Option<QuerySet<M>>,
	validation_config: Option<ValidatorConfig<M>>,
	_serializer: PhantomData<S>,
}

impl<M, S> CreateAPIView<M, S>
where
	M: Model + Serialize + for<'de> Deserialize<'de> + Send + Sync + Clone + 'static,
	S: Serializer<Input = M, Output = String> + Send + Sync + 'static,
{
	/// Creates a new `CreateAPIView` with default settings
	///
	/// # Examples
	///
	/// ```rust,no_run
	/// use reinhardt_views::CreateAPIView;
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
	/// let view = CreateAPIView::<Article, JsonSerializer<Article>>::new();
	/// ```
	pub fn new() -> Self {
		Self {
			queryset: None,
			validation_config: None,
			_serializer: PhantomData,
		}
	}

	/// Sets the queryset for this view
	pub fn with_queryset(mut self, queryset: QuerySet<M>) -> Self {
		self.queryset = Some(queryset);
		self
	}

	/// Gets the queryset, creating a default one if not set
	fn get_queryset(&self) -> QuerySet<M> {
		self.queryset.clone().unwrap_or_default()
	}

	/// Sets the validation configuration
	///
	/// # Examples
	///
	/// ```rust,no_run
	/// # use reinhardt_views::CreateAPIView;
	/// # use reinhardt_rest::serializers::{JsonSerializer, ValidatorConfig};
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
	/// let config = ValidatorConfig::<Article>::new();
	/// let view = CreateAPIView::<Article, JsonSerializer<Article>>::new()
	///     .with_validation_config(config);
	/// ```
	pub fn with_validation_config(mut self, config: ValidatorConfig<M>) -> Self {
		self.validation_config = Some(config);
		self
	}

	/// Performs the object creation
	async fn perform_create(&self, request: &Request) -> Result<M> {
		// Parse request body and deserialize to model
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

		// Create via QuerySet
		let queryset = self.get_queryset();
		queryset
			.create(data)
			.await
			.map_err(|e| Error::Http(format!("Failed to create: {}", e)))
	}
}

impl<M, S> Default for CreateAPIView<M, S>
where
	M: Model + Serialize + for<'de> Deserialize<'de> + Send + Sync + Clone + 'static,
	S: Serializer<Input = M, Output = String> + Send + Sync + 'static,
{
	fn default() -> Self {
		Self::new()
	}
}

#[async_trait]
impl<M, S> View for CreateAPIView<M, S>
where
	M: Model + Serialize + for<'de> Deserialize<'de> + Send + Sync + Clone + 'static,
	S: Serializer<Input = M, Output = String> + Send + Sync + 'static + Default,
{
	async fn dispatch(&self, request: Request) -> Result<Response> {
		match request.method {
			Method::POST => {
				let obj = self.perform_create(&request).await?;

				// Serialize the created object
				let serializer = S::default();
				let serialized = serializer
					.serialize(&obj)
					.map_err(|e| Error::Http(e.to_string()))?;

				// Parse to JSON value for response
				let json_value: serde_json::Value = serde_json::from_str(&serialized)
					.map_err(|e| Error::Http(format!("Failed to parse serialized data: {}", e)))?;

				Response::created()
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
		vec!["POST", "OPTIONS"]
	}
}

// Manually re-assert `UnwindSafe` / `RefUnwindSafe`. `CreateAPIView` holds an
// `Option<ValidatorConfig<M>>`, whose auto-trait state was lost when
// `Vec<Arc<dyn ModelLevelValidator<M>>>` was added to `ValidatorConfig`.
// Without these impls, cargo-semver-checks reports `auto_trait_impl_removed`
// during the RC phase. See the matching block in viewset.rs for soundness
// rationale.
impl<M, S> std::panic::UnwindSafe for CreateAPIView<M, S>
where
	M: Model + Serialize + for<'de> Deserialize<'de> + Send + Sync + Clone,
	S: Serializer<Input = M, Output = String> + Send + Sync,
{
}
impl<M, S> std::panic::RefUnwindSafe for CreateAPIView<M, S>
where
	M: Model + Serialize + for<'de> Deserialize<'de> + Send + Sync + Clone,
	S: Serializer<Input = M, Output = String> + Send + Sync,
{
}
