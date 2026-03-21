use async_trait::async_trait;
use reinhardt_http::{Request, Response, Result};

use crate::viewsets::{BatchRequest, BatchResponse};

/// Mixin traits for ViewSet functionality
/// These use composition instead of multiple inheritance
/// List mixin - provides list() action
#[async_trait]
pub trait ListMixin: Send + Sync {
	/// List all resources matching the request parameters.
	async fn list(&self, request: Request) -> Result<Response>;
}

/// Retrieve mixin - provides retrieve() action
#[async_trait]
pub trait RetrieveMixin: Send + Sync {
	/// Retrieve a single resource by its identifier.
	async fn retrieve(&self, request: Request, id: String) -> Result<Response>;
}

/// Create mixin - provides create() action
#[async_trait]
pub trait CreateMixin: Send + Sync {
	/// Create a new resource from the request data.
	async fn create(&self, request: Request) -> Result<Response>;
}

/// Update mixin - provides update() action
#[async_trait]
pub trait UpdateMixin: Send + Sync {
	/// Fully update an existing resource by its identifier.
	async fn update(&self, request: Request, id: String) -> Result<Response>;
}

/// Destroy mixin - provides destroy() action
#[async_trait]
pub trait DestroyMixin: Send + Sync {
	/// Delete a resource by its identifier.
	async fn destroy(&self, request: Request, id: String) -> Result<Response>;
}

/// Composite trait for all CRUD operations
/// This demonstrates trait composition in Rust
#[async_trait]
pub trait CrudMixin: ListMixin + RetrieveMixin + CreateMixin + UpdateMixin + DestroyMixin {}

// Blanket implementation for any type that implements all mixins
impl<T> CrudMixin for T where T: ListMixin + RetrieveMixin + CreateMixin + UpdateMixin + DestroyMixin
{}

/// Bulk create mixin - provides bulk_create() action
///
/// # Example
///
/// ```
/// use reinhardt_views::viewsets::{BulkCreateMixin, BatchRequest, BatchResponse, BatchOperation};
/// use reinhardt_http::{Request, Response, Result};
/// use async_trait::async_trait;
/// use serde::{Serialize, Deserialize};
///
/// #[derive(Debug, Clone, Serialize, Deserialize)]
/// struct User {
///     id: i64,
///     name: String,
/// }
///
/// struct UserViewSet;
///
/// #[async_trait]
/// impl BulkCreateMixin for UserViewSet {
///     type Item = User;
///
///     async fn bulk_create(&self, request: BatchRequest<Self::Item>) -> Result<BatchResponse<Self::Item>> {
///         // Example implementation: Create users from batch request
///         use reinhardt_views::viewsets::BatchOperationResult;
///
///         let results: Vec<BatchOperationResult<User>> = request.operations
///             .iter()
///             .enumerate()
///             .map(|(index, _op)| {
///                 // Simplified: Return success with created user
///                 BatchOperationResult::success(index, Some(User {
///                     id: (index + 1) as i64,
///                     name: format!("User {}", index + 1),
///                 }))
///             })
///             .collect();
///
///         Ok(BatchResponse::new(results))
///     }
/// }
/// ```
#[async_trait]
pub trait BulkCreateMixin: Send + Sync {
	/// The type of item to create
	type Item: Send + Sync;

	/// Create multiple items in a single request
	///
	/// Implementations should use database transactions to ensure atomicity.
	async fn bulk_create(
		&self,
		request: BatchRequest<Self::Item>,
	) -> Result<BatchResponse<Self::Item>>;
}

/// Bulk update mixin - provides bulk_update() action
///
/// # Example
///
/// ```
/// use reinhardt_views::viewsets::{BulkUpdateMixin, BatchRequest, BatchResponse, BatchOperation};
/// use reinhardt_http::{Request, Response, Result};
/// use async_trait::async_trait;
/// use serde::{Serialize, Deserialize};
///
/// #[derive(Debug, Clone, Serialize, Deserialize)]
/// struct User {
///     id: i64,
///     name: String,
/// }
///
/// struct UserViewSet;
///
/// #[async_trait]
/// impl BulkUpdateMixin for UserViewSet {
///     type Item = User;
///
///     async fn bulk_update(&self, request: BatchRequest<Self::Item>) -> Result<BatchResponse<Self::Item>> {
///         // Example implementation: Update users from batch request
///         use reinhardt_views::viewsets::BatchOperationResult;
///
///         let results: Vec<BatchOperationResult<User>> = request.operations
///             .iter()
///             .enumerate()
///             .map(|(index, _op)| {
///                 // Simplified: Return success with updated user
///                 BatchOperationResult::success(index, Some(User {
///                     id: (index + 1) as i64,
///                     name: format!("Updated User {}", index + 1),
///                 }))
///             })
///             .collect();
///
///         Ok(BatchResponse::new(results))
///     }
/// }
/// ```
#[async_trait]
pub trait BulkUpdateMixin: Send + Sync {
	/// The type of item to update
	type Item: Send + Sync;

	/// Update multiple items in a single request
	///
	/// Implementations should use database transactions to ensure atomicity.
	async fn bulk_update(
		&self,
		request: BatchRequest<Self::Item>,
	) -> Result<BatchResponse<Self::Item>>;
}

/// Bulk delete mixin - provides bulk_delete() action
///
/// # Example
///
/// ```
/// use reinhardt_views::viewsets::{BulkDeleteMixin, BatchRequest, BatchResponse, BatchOperation};
/// use reinhardt_http::{Request, Response, Result};
/// use async_trait::async_trait;
/// use serde::{Serialize, Deserialize};
///
/// #[derive(Debug, Clone, Serialize, Deserialize)]
/// struct User {
///     id: i64,
///     name: String,
/// }
///
/// struct UserViewSet;
///
/// #[async_trait]
/// impl BulkDeleteMixin for UserViewSet {
///     type Item = User;
///
///     async fn bulk_delete(&self, request: BatchRequest<Self::Item>) -> Result<BatchResponse<Self::Item>> {
///         // Example implementation: Delete users from batch request
///         use reinhardt_views::viewsets::BatchOperationResult;
///
///         let results: Vec<BatchOperationResult<User>> = request.operations
///             .iter()
///             .enumerate()
///             .map(|(index, _op)| {
///                 // Simplified: Return success without data (deletion)
///                 BatchOperationResult::success(index, None)
///             })
///             .collect();
///
///         Ok(BatchResponse::new(results))
///     }
/// }
/// ```
#[async_trait]
pub trait BulkDeleteMixin: Send + Sync {
	/// The type of item to delete
	type Item: Send + Sync;

	/// Delete multiple items in a single request
	///
	/// Implementations should use database transactions to ensure atomicity.
	async fn bulk_delete(
		&self,
		request: BatchRequest<Self::Item>,
	) -> Result<BatchResponse<Self::Item>>;
}

/// Composite trait for all bulk operations
#[async_trait]
pub trait BulkOperationsMixin: BulkCreateMixin + BulkUpdateMixin + BulkDeleteMixin {}

// Blanket implementation for any type that implements all bulk mixins
impl<T> BulkOperationsMixin for T where T: BulkCreateMixin + BulkUpdateMixin + BulkDeleteMixin {}
