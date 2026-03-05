//! Dependency injection support for gRPC handlers
//!
//! This module provides extensions to integrate Reinhardt's DI system with gRPC handlers.
//!
//! # Overview
//!
//! The DI system for gRPC works by storing an `InjectionContext` in the request extensions,
//! which can then be extracted and used to resolve dependencies.
//!
//! # Security
//!
//! DI error messages are sanitized to prevent leaking internal type names
//! and implementation details to clients. In production, generic error
//! messages are returned while detailed information is logged server-side.
//!
//! # Example
//!
//! ```rust,no_run,ignore
//! # use reinhardt_grpc::{GrpcRequestExt, grpc_handler};
//! # use reinhardt_di::InjectionContext;
//! # use tonic::{Request, Response, Status};
//! # use std::sync::Arc;
//! # struct GetUserRequest { id: i64 }
//! # struct User;
//! # struct DatabaseConnection;
//! # impl DatabaseConnection {
//! #     async fn fetch_user(&self, _id: i64) -> Result<User, Status> { Ok(User) }
//! # }
//! # #[tonic::async_trait]
//! # trait UserService {
//! #     async fn get_user(&self, request: Request<GetUserRequest>) -> Result<Response<User>, Status>;
//! # }
//! pub struct UserServiceImpl {
//!     injection_context: Arc<InjectionContext>,
//! }
//!
//! #[tonic::async_trait]
//! impl UserService for UserServiceImpl {
//!     async fn get_user(&self, mut request: Request<GetUserRequest>)
//!         -> Result<Response<User>, Status>
//!     {
//!         request.extensions_mut().insert(self.injection_context.clone());
//!         self.get_user_impl(request).await
//!     }
//! }
//!
//! impl UserServiceImpl {
//!     #[grpc_handler]
//!     async fn get_user_impl(
//!         &self,
//!         request: Request<GetUserRequest>,
//!         #[inject] db: DatabaseConnection,
//!     ) -> Result<Response<User>, Status> {
//!         let user_id = request.into_inner().id;
//!         let user = db.fetch_user(user_id).await?;
//!         Ok(Response::new(user))
//!     }
//! }
//! ```

/// Extension trait for `tonic::Request` to support DI context extraction
///
/// This trait adds methods to `tonic::Request` for working with Reinhardt's
/// dependency injection system.
pub trait GrpcRequestExt {
	/// Extract DI context from request extensions
	///
	/// Returns `Some(T)` if the context exists, `None` otherwise.
	///
	/// # Example
	///
	/// ```rust,no_run
	/// # use reinhardt_grpc::GrpcRequestExt;
	/// # use reinhardt_di::InjectionContext;
	/// # use std::sync::Arc;
	/// # use tonic::Request;
	/// # fn example<T>(request: &Request<T>) -> Option<Arc<InjectionContext>> {
	/// let di_ctx = request.get_di_context::<Arc<InjectionContext>>()?;
	/// # Some(di_ctx)
	/// # }
	/// ```
	fn get_di_context<T: Clone + Send + Sync + 'static>(&self) -> Option<T>;
}

impl<T> GrpcRequestExt for tonic::Request<T> {
	fn get_di_context<C: Clone + Send + Sync + 'static>(&self) -> Option<C> {
		self.extensions().get::<C>().cloned()
	}
}

/// Sanitize a DI error into a safe tonic::Status for client responses.
///
/// Logs the detailed error server-side and returns a generic message
/// to the client to prevent type information leakage.
///
/// # Security
///
/// DI errors often contain internal type names (e.g., fully-qualified
/// Rust type paths like `my_app::services::DatabasePool`). Exposing
/// these to clients reveals internal architecture details. This function
/// ensures only generic error messages reach the client.
pub fn sanitize_di_error(error: &reinhardt_di::DiError) -> tonic::Status {
	tracing::error!(
		error = %error,
		"DI resolution failed"
	);
	tonic::Status::internal("Internal server error")
}

/// Sanitize a missing DI context error into a safe tonic::Status.
///
/// Returns a generic error message instead of exposing that the
/// DI context is missing from request extensions.
pub fn sanitize_missing_context() -> tonic::Status {
	tracing::error!("DI context not found in request extensions");
	tonic::Status::internal("Internal server error")
}

#[cfg(test)]
mod tests {
	use super::*;
	use reinhardt_di::InjectionContext;
	use rstest::rstest;
	use std::sync::Arc;
	use tonic::Request;

	#[rstest]
	fn grpc_request_ext_extracts_di_context() {
		// Arrange
		let singleton_scope = reinhardt_di::SingletonScope::new();
		let injection_ctx = Arc::new(InjectionContext::builder(singleton_scope).build());
		let mut request = Request::new(());
		request.extensions_mut().insert(injection_ctx.clone());

		// Act
		let extracted = request
			.get_di_context::<Arc<InjectionContext>>()
			.expect("DI context should be extractable from request extensions after insertion");

		// Assert
		assert!(Arc::ptr_eq(&injection_ctx, &extracted));
	}

	#[rstest]
	fn grpc_request_ext_returns_none_for_missing_context() {
		// Arrange
		let request = Request::new(());

		// Act
		let extracted = request.get_di_context::<Arc<InjectionContext>>();

		// Assert
		assert!(extracted.is_none());
	}

	#[rstest]
	fn sanitize_di_error_returns_generic_message() {
		// Arrange
		let error = reinhardt_di::DiError::NotFound("my_app::services::DatabasePool".to_string());

		// Act
		let status = sanitize_di_error(&error);

		// Assert
		assert_eq!(status.code(), tonic::Code::Internal);
		assert_eq!(status.message(), "Internal server error");
		// Ensure the type name is NOT in the client-facing message
		assert!(
			!status.message().contains("DatabasePool"),
			"Type name should not be exposed in client error"
		);
		assert!(
			!status.message().contains("my_app"),
			"Module path should not be exposed in client error"
		);
	}

	#[rstest]
	fn sanitize_di_error_hides_type_mismatch_details() {
		// Arrange
		let error = reinhardt_di::DiError::TypeMismatch {
			expected: "my_app::db::PostgresPool".to_string(),
			actual: "my_app::db::SqlitePool".to_string(),
		};

		// Act
		let status = sanitize_di_error(&error);

		// Assert
		assert_eq!(status.code(), tonic::Code::Internal);
		assert_eq!(status.message(), "Internal server error");
		assert!(!status.message().contains("PostgresPool"));
		assert!(!status.message().contains("SqlitePool"));
	}

	#[rstest]
	fn sanitize_di_error_hides_circular_dependency_details() {
		// Arrange
		let error = reinhardt_di::DiError::CircularDependency(
			"my_app::ServiceA -> my_app::ServiceB -> my_app::ServiceA".to_string(),
		);

		// Act
		let status = sanitize_di_error(&error);

		// Assert
		assert_eq!(status.code(), tonic::Code::Internal);
		assert_eq!(status.message(), "Internal server error");
		assert!(!status.message().contains("ServiceA"));
		assert!(!status.message().contains("ServiceB"));
	}

	#[rstest]
	fn sanitize_missing_context_returns_generic_message() {
		// Act
		let status = sanitize_missing_context();

		// Assert
		assert_eq!(status.code(), tonic::Code::Internal);
		assert_eq!(status.message(), "Internal server error");
		// Ensure no DI-specific details leak
		assert!(!status.message().contains("DI"));
		assert!(!status.message().contains("context"));
		assert!(!status.message().contains("extensions"));
	}
}
