//! # Reinhardt ViewSets
//!
//! Django REST Framework-inspired ViewSets for building REST APIs.
//!
//! ## Overview
//!
//! ViewSets combine common view patterns into a single class, providing automatic
//! routing, pagination, filtering, and caching support. This crate is the core
//! of Reinhardt's REST API functionality.
//!
//! ## Features
//!
//! - **[`ModelViewSet`]**: Full CRUD operations for a model
//! - **[`ReadOnlyModelViewSet`]**: Read-only operations (list and retrieve)
//! - **[`GenericViewSet`]**: Base viewset for custom implementations
//! - **Mixins**: Composable behaviors ([`ListMixin`], [`CreateMixin`], [`RetrieveMixin`], etc.)
//! - **Custom Actions**: Define custom endpoints with `@action` decorator
//! - **Batch Operations**: Bulk create, update, and delete support
//! - **Pagination**: Built-in pagination with configurable page sizes
//! - **Filtering**: Field filtering and ordering support
//! - **Caching**: Response caching with cache invalidation
//! - **Middleware**: Per-viewset middleware (authentication, permissions)
//! - **Nested Resources**: Parent-child resource relationships
//!
//! ## Quick Start
//!
//! ### ModelViewSet (Full CRUD)
//!
//! ```rust,ignore
//! use reinhardt_views::viewsets::{ModelViewSet, ViewSet};
//!
//! // Create a viewset for User model with UserSerializer
//! let viewset: ModelViewSet<User, UserSerializer> = ModelViewSet::new("users");
//!
//! // Supports: list, create, retrieve, update, partial_update, destroy
//! // GET    /users/      -> list
//! // POST   /users/      -> create
//! // GET    /users/{id}/ -> retrieve
//! // PUT    /users/{id}/ -> update
//! // PATCH  /users/{id}/ -> partial_update
//! // DELETE /users/{id}/ -> destroy
//! ```
//!
//! ### ReadOnlyModelViewSet
//!
//! ```rust,ignore
//! use reinhardt_views::viewsets::{ReadOnlyModelViewSet, ViewSet};
//!
//! // Read-only viewset (list and retrieve only)
//! let viewset: ReadOnlyModelViewSet<Post, PostSerializer> = ReadOnlyModelViewSet::new("posts");
//!
//! // Supports: list, retrieve
//! // GET    /posts/      -> list
//! // GET    /posts/{id}/ -> retrieve
//! ```
//!
//! ## Available Mixins
//!
//! Mixins provide composable behaviors that can be combined:
//!
//! | Mixin | Action | HTTP Method | URL Pattern |
//! |-------|--------|-------------|-------------|
//! | [`ListMixin`] | list | GET | `/resources/` |
//! | [`CreateMixin`] | create | POST | `/resources/` |
//! | [`RetrieveMixin`] | retrieve | GET | `/resources/{id}/` |
//! | [`UpdateMixin`] | update | PUT | `/resources/{id}/` |
//! | [`DestroyMixin`] | destroy | DELETE | `/resources/{id}/` |
//! | [`BulkCreateMixin`] | bulk_create | POST | `/resources/bulk/` |
//! | [`BulkUpdateMixin`] | bulk_update | PUT | `/resources/bulk/` |
//! | [`BulkDeleteMixin`] | bulk_delete | DELETE | `/resources/bulk/` |
//!
//! ## Custom Actions
//!
//! Define custom endpoints using the action registry:
//!
//! ```rust,ignore
//! use reinhardt_views::viewsets::{action, ActionType};
//!
//! // Register a detail action (operates on a single resource)
//! #[action(detail = true, methods = ["POST"])]
//! async fn activate(request: Request) -> Result<Response> {
//!     // Activate a specific user
//!     Ok(Response::ok())
//! }
//!
//! // Register a list action (operates on the collection)
//! #[action(detail = false, methods = ["GET"])]
//! async fn recent(request: Request) -> Result<Response> {
//!     // Get recent items
//!     Ok(Response::ok())
//! }
//! ```
//!
//! ## Pagination
//!
//! Built-in pagination support:
//!
//! ```rust,ignore
//! use reinhardt_views::viewsets::{PaginatedViewSet, PaginationConfig};
//!
//! let config = PaginationConfig {
//!     page_size: 20,
//!     max_page_size: 100,
//!     page_query_param: "page".to_string(),
//!     page_size_query_param: "page_size".to_string(),
//! };
//!
//! let viewset = PaginatedViewSet::new(viewset, config);
//! ```
//!
//! ## Filtering
//!
//! Filter and order query results:
//!
//! ```rust,ignore
//! use reinhardt_views::viewsets::{FilterableViewSet, FilterConfig, OrderingConfig};
//!
//! let filter_config = FilterConfig {
//!     filterable_fields: vec!["status", "category"],
//!     search_fields: vec!["title", "description"],
//! };
//!
//! let ordering_config = OrderingConfig {
//!     ordering_fields: vec!["created_at", "updated_at", "title"],
//!     default_ordering: vec!["-created_at"], // Descending by created_at
//! };
//!
//! let viewset = FilterableViewSet::new(viewset, filter_config, ordering_config);
//! ```
//!
//! ## Caching
//!
//! Response caching with automatic invalidation:
//!
//! ```rust,ignore
//! use reinhardt_views::viewsets::{CachedViewSet, CacheConfig};
//!
//! let config = CacheConfig {
//!     ttl_seconds: 300,           // 5 minutes
//!     vary_headers: vec!["Authorization"],
//!     cache_methods: vec!["GET", "HEAD"],
//! };
//!
//! let viewset = CachedViewSet::new(viewset, config);
//! ```
//!
//! ## Middleware
//!
//! Apply middleware to viewsets:
//!
//! ```rust,ignore
//! use reinhardt_views::viewsets::{AuthenticationMiddleware, PermissionMiddleware};
//!
//! let viewset = viewset
//!     .with_middleware(AuthenticationMiddleware::required())
//!     .with_middleware(PermissionMiddleware::new(&["users.view", "users.edit"]));
//! ```
//!
//! ## Nested Resources
//!
//! Define parent-child resource relationships:
//!
//! ```rust,ignore
//! use reinhardt_views::viewsets::{NestedViewSet, NestedResource};
//!
//! // /users/{user_id}/posts/
//! let nested = NestedViewSet::new(post_viewset)
//!     .parent::<User>("user_id")
//!     .filter_by_parent(|query, user_id| {
//!         query.filter("author_id", user_id)
//!     });
//! ```
//!
//! ## Batch Operations
//!
//! Process multiple records in a single request:
//!
//! ```rust,ignore
//! use reinhardt_views::viewsets::{BatchProcessor, BatchRequest};
//!
//! // POST /users/bulk/
//! // Body: [{"name": "Alice"}, {"name": "Bob"}]
//!
//! let result = BatchProcessor::new(&viewset)
//!     .process_create(batch_request)
//!     .await?;
//!
//! println!("Created: {}, Failed: {}", result.success_count, result.failure_count);
//! ```

pub mod actions;
pub mod batch_operations;
pub mod builder;
pub mod cached;
pub mod filtering_support;
pub mod handler;
pub mod injectable;
pub mod metadata;
pub mod middleware;
pub mod mixins;
pub mod nested_resources;
pub mod pagination_support;
pub mod registry;
pub mod schema_metadata;
pub mod viewset;

pub use actions::{Action, ActionType};
pub use batch_operations::{
	BatchOperation, BatchOperationResult, BatchProcessor, BatchRequest, BatchResponse,
	BatchStatistics,
};
pub use builder::{RegisterViewSet, ViewSetBuilder};
pub use cached::{CacheConfig, CachedResponse, CachedViewSet, CachedViewSetTrait};
pub use filtering_support::{FilterConfig, FilterableViewSet, InMemoryFilter, OrderingConfig};
pub use handler::{ModelViewSetHandler, ViewError, ViewSetHandler};
pub use injectable::InjectableViewSet;
pub use metadata::{ActionHandler, ActionMetadata, ActionRegistryEntry, FunctionActionHandler};
pub use middleware::{
	AuthenticationMiddleware, CompositeMiddleware, PermissionMiddleware, ViewSetMiddleware,
};
pub use mixins::{
	BulkCreateMixin, BulkDeleteMixin, BulkOperationsMixin, BulkUpdateMixin, CreateMixin,
	DestroyMixin, ListMixin, RetrieveMixin, UpdateMixin,
};
pub use nested_resources::{
	NestedResource, NestedResourcePath, NestedViewSet, nested_detail_url, nested_url,
};
pub use pagination_support::{PaginatedViewSet, PaginationConfig};
pub use registry::{action, clear_actions, get_registered_actions, register_action};
pub use schema_metadata::{FieldSchema, ModelSchema, RequestSchema, ResponseSchema, ViewSetSchema};
pub use viewset::{GenericViewSet, ModelViewSet, ReadOnlyModelViewSet, ViewSet};

#[cfg(test)]
mod tests {
	use super::*;
	use bytes::Bytes;
	use hyper::{HeaderMap, Method, StatusCode, Version};
	use reinhardt_http::Request;

	#[allow(dead_code)]
	#[derive(Debug, Clone)]
	struct TestModel {
		id: i64,
		name: String,
	}

	#[derive(Debug, Clone)]
	struct TestSerializer;

	#[tokio::test]
	async fn test_viewset_get_basename() {
		let viewset = GenericViewSet::new("test", ());
		assert_eq!(viewset.get_basename(), "test");
	}

	#[tokio::test]
	async fn test_model_viewset_list_action() {
		let viewset: ModelViewSet<TestModel, TestSerializer> = ModelViewSet::new("users");
		let request = Request::builder()
			.method(Method::GET)
			.uri("/users/")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();
		let action = Action::list();

		let response = viewset.dispatch(request, action).await.unwrap();

		assert_eq!(response.status, StatusCode::OK);
	}

	#[tokio::test]
	async fn test_model_viewset_retrieve_action() {
		let viewset: ModelViewSet<TestModel, TestSerializer> = ModelViewSet::new("users");
		let request = Request::builder()
			.method(Method::GET)
			.uri("/users/1/")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();
		let action = Action::retrieve();

		let response = viewset.dispatch(request, action).await.unwrap();

		assert_eq!(response.status, StatusCode::OK);
	}

	#[tokio::test]
	async fn test_model_viewset_create_action() {
		let viewset: ModelViewSet<TestModel, TestSerializer> = ModelViewSet::new("users");
		let request = Request::builder()
			.method(Method::POST)
			.uri("/users/")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::from(r#"{"name": "test"}"#))
			.build()
			.unwrap();
		let action = Action::create();

		let response = viewset.dispatch(request, action).await.unwrap();

		assert_eq!(response.status, StatusCode::CREATED);
	}

	#[tokio::test]
	async fn test_model_viewset_update_action() {
		let viewset: ModelViewSet<TestModel, TestSerializer> = ModelViewSet::new("users");
		let request = Request::builder()
			.method(Method::PUT)
			.uri("/users/1/")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::from(r#"{"name": "updated"}"#))
			.build()
			.unwrap();
		let action = Action::update();

		let response = viewset.dispatch(request, action).await.unwrap();

		assert_eq!(response.status, StatusCode::OK);
	}

	#[tokio::test]
	async fn test_model_viewset_destroy_action() {
		let viewset: ModelViewSet<TestModel, TestSerializer> = ModelViewSet::new("users");
		let request = Request::builder()
			.method(Method::DELETE)
			.uri("/users/1/")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();
		let action = Action::destroy();

		let response = viewset.dispatch(request, action).await.unwrap();

		assert_eq!(response.status, StatusCode::NO_CONTENT);
	}

	#[tokio::test]
	async fn test_readonly_viewset_list_allowed() {
		let viewset: ReadOnlyModelViewSet<TestModel, TestSerializer> =
			ReadOnlyModelViewSet::new("posts");
		let request = Request::builder()
			.method(Method::GET)
			.uri("/posts/")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();
		let action = Action::list();

		let response = viewset.dispatch(request, action).await.unwrap();

		assert_eq!(response.status, StatusCode::OK);
	}

	#[tokio::test]
	async fn test_readonly_viewset_retrieve_allowed() {
		let viewset: ReadOnlyModelViewSet<TestModel, TestSerializer> =
			ReadOnlyModelViewSet::new("posts");
		let request = Request::builder()
			.method(Method::GET)
			.uri("/posts/1/")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();
		let action = Action::retrieve();

		let response = viewset.dispatch(request, action).await.unwrap();

		assert_eq!(response.status, StatusCode::OK);
	}

	#[tokio::test]
	async fn test_readonly_viewset_create_denied() {
		let viewset: ReadOnlyModelViewSet<TestModel, TestSerializer> =
			ReadOnlyModelViewSet::new("posts");
		let request = Request::builder()
			.method(Method::POST)
			.uri("/posts/")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::from(r#"{"title": "test"}"#))
			.build()
			.unwrap();
		let action = Action::create();

		let result = viewset.dispatch(request, action).await;

		assert!(result.is_err());
	}

	#[tokio::test]
	async fn test_readonly_viewset_delete_denied() {
		let viewset: ReadOnlyModelViewSet<TestModel, TestSerializer> =
			ReadOnlyModelViewSet::new("posts");
		let request = Request::builder()
			.method(Method::DELETE)
			.uri("/posts/1/")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();
		let action = Action::destroy();

		let result = viewset.dispatch(request, action).await;

		assert!(result.is_err());
	}
}
