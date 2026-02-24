//! ListAPIView implementation for displaying lists of objects

use crate::viewsets::{FilterConfig, PaginationConfig};
use async_trait::async_trait;
use hyper::Method;
use reinhardt_core::exception::{Error, Result};
use reinhardt_db::orm::{Filter, FilterOperator, FilterValue, Model, QuerySet};
use reinhardt_http::{Request, Response};
use reinhardt_rest::serializers::Serializer;
use serde::{Deserialize, Serialize};
use std::marker::PhantomData;

use crate::core::View;

/// ListAPIView for displaying paginated lists of objects
///
/// Similar to Django REST Framework's ListAPIView, this view provides
/// read-only access to a list of model instances with support for
/// pagination, filtering, and ordering.
///
/// # Type Parameters
///
/// * `M` - The model type (must implement `Model`, `Serialize`, `Deserialize`)
/// * `S` - The serializer type (must implement `Serializer`)
///
/// # Examples
///
/// ```rust,no_run
/// use reinhardt_views::ListAPIView;
/// use reinhardt_db::orm::{Model, QuerySet};
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
/// let view = ListAPIView::<Article, JsonSerializer<Article>>::new()
///     .with_paginate_by(10)
///     .with_ordering(vec!["-created_at".into()]);
/// ```
pub struct ListAPIView<M, S>
where
	M: Model + Serialize + for<'de> Deserialize<'de> + Send + Sync + Clone,
	S: Serializer<Input = M, Output = String> + Send + Sync,
{
	queryset: Option<QuerySet<M>>,
	pagination_config: Option<PaginationConfig>,
	filter_config: Option<FilterConfig>,
	ordering: Option<Vec<String>>,
	_serializer: PhantomData<S>,
}

impl<M, S> ListAPIView<M, S>
where
	M: Model + Serialize + for<'de> Deserialize<'de> + Send + Sync + Clone + 'static,
	S: Serializer<Input = M, Output = String> + Send + Sync + 'static,
{
	/// Creates a new `ListAPIView` with default settings
	///
	/// # Examples
	///
	/// ```rust,no_run
	/// use reinhardt_views::ListAPIView;
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
	/// let view = ListAPIView::<Article, JsonSerializer<Article>>::new();
	/// ```
	pub fn new() -> Self {
		Self {
			queryset: None,
			pagination_config: None,
			filter_config: None,
			ordering: None,
			_serializer: PhantomData,
		}
	}

	/// Sets the queryset for this view
	///
	/// # Examples
	///
	/// ```rust,no_run
	/// # use reinhardt_views::ListAPIView;
	/// # use reinhardt_db::orm::{Model, QuerySet};
	/// # use reinhardt_rest::serializers::JsonSerializer;
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
	/// let queryset = QuerySet::<Article>::new();
	/// let view = ListAPIView::<Article, JsonSerializer<Article>>::new()
	///     .with_queryset(queryset);
	/// ```
	pub fn with_queryset(mut self, queryset: QuerySet<M>) -> Self {
		self.queryset = Some(queryset);
		self
	}

	/// Sets the number of items per page for pagination
	///
	/// # Examples
	///
	/// ```rust,no_run
	/// # use reinhardt_views::ListAPIView;
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
	/// let view = ListAPIView::<Article, JsonSerializer<Article>>::new()
	///     .with_paginate_by(20);
	/// ```
	pub fn with_paginate_by(mut self, page_size: usize) -> Self {
		self.pagination_config = Some(PaginationConfig::page_number(page_size, Some(100)));
		self
	}

	/// Sets the pagination configuration for the view
	///
	/// This method allows setting any pagination type (PageNumber, LimitOffset, Cursor, or None).
	///
	/// # Examples
	///
	/// ```rust,no_run
	/// # use reinhardt_views::ListAPIView;
	/// # use reinhardt_views::viewsets::PaginationConfig;
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
	/// let view = ListAPIView::<Article, JsonSerializer<Article>>::new()
	///     .with_pagination(PaginationConfig::limit_offset(10, Some(100)));
	/// ```
	pub fn with_pagination(mut self, config: PaginationConfig) -> Self {
		self.pagination_config = Some(config);
		self
	}

	/// Sets the ordering for the queryset
	///
	/// Fields can be prefixed with `-` for descending order.
	///
	/// # Examples
	///
	/// ```rust,no_run
	/// # use reinhardt_views::ListAPIView;
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
	/// let view = ListAPIView::<Article, JsonSerializer<Article>>::new()
	///     .with_ordering(vec!["-created_at".into(), "title".into()]);
	/// ```
	pub fn with_ordering(mut self, ordering: Vec<String>) -> Self {
		self.ordering = Some(ordering);
		self
	}

	/// Sets the filter configuration
	pub fn with_filter_config(mut self, filter_config: FilterConfig) -> Self {
		self.filter_config = Some(filter_config);
		self
	}

	/// Gets the queryset, creating a default one if not set
	fn get_queryset(&self) -> QuerySet<M> {
		self.queryset.clone().unwrap_or_default()
	}

	/// Gets the objects to display
	async fn get_objects(&self, request: &Request) -> Result<Vec<M>> {
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

impl<M, S> Default for ListAPIView<M, S>
where
	M: Model + Serialize + for<'de> Deserialize<'de> + Send + Sync + Clone + 'static,
	S: Serializer<Input = M, Output = String> + Send + Sync + 'static,
{
	fn default() -> Self {
		Self::new()
	}
}

#[async_trait]
impl<M, S> View for ListAPIView<M, S>
where
	M: Model + Serialize + for<'de> Deserialize<'de> + Send + Sync + Clone + 'static,
	S: Serializer<Input = M, Output = String> + Send + Sync + 'static + Default,
{
	async fn dispatch(&self, request: Request) -> Result<Response> {
		match request.method {
			Method::GET | Method::HEAD => {
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
					match pagination {
						PaginationConfig::PageNumber { page_size, .. } => {
							let page = request
								.query_params
								.get("page")
								.and_then(|p| p.parse::<usize>().ok())
								.unwrap_or(1);
							let count = results.len();
							serde_json::json!({
								"count": count,
								"page": page,
								"page_size": page_size,
								"next": if count == *page_size { Some(format!("?page={}", page + 1)) } else { None::<String> },
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
							let count = results.len();
							serde_json::json!({
								"count": count,
								"offset": offset,
								"limit": limit,
								"next": if count == limit { Some(format!("?offset={}&limit={}", offset + limit, limit)) } else { None::<String> },
								"previous": if offset > 0 { Some(format!("?offset={}&limit={}", offset.saturating_sub(limit), limit)) } else { None::<String> },
								"results": results
							})
						}
						_ => {
							serde_json::json!({
								"count": results.len(),
								"results": results
							})
						}
					}
				} else {
					serde_json::json!(results)
				};

				Response::ok().with_json(&response_body)
			}
			_ => Err(Error::Http("Method not allowed".to_string())),
		}
	}

	fn allowed_methods(&self) -> Vec<&'static str> {
		vec!["GET", "HEAD", "OPTIONS"]
	}
}
