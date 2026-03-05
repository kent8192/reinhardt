//! gRPC support for Reinhardt framework
//!
//! This crate provides gRPC infrastructure features for the Reinhardt framework.
//!
//! # Features
//!
//! - Common Protobuf types (Empty, Timestamp, Error, PageInfo, BatchResult)
//! - GraphQL over gRPC types (GraphQLRequest, GraphQLResponse, SubscriptionEvent)
//! - gRPC error handling with production-safe error sanitization
//! - gRPC service adapter trait
//! - Server configuration with message size limits, request timeouts, and connection limits
//! - Protobuf message nesting depth limits
//! - Protobuf field constraint validation
//! - Dependency injection support (with `di` feature)
//!
//! # Security
//!
//! This crate includes several security features:
//!
//! - **Depth-limited decoding**: Prevents stack overflow from deeply nested
//!   protobuf messages via [`depth_limit::DepthLimitedDecoder`].
//! - **Field validation**: Enforces protobuf field constraints (required
//!   fields, value ranges) via [`validation::ProtoValidator`].
//! - **Error sanitization**: Prevents information leakage through error
//!   messages via [`error::ErrorSanitizer`].
//! - **DI error sanitization**: Prevents type name leakage through DI
//!   error messages (with `di` feature).
//!
//! # Usage
//!
//! Users can define their own .proto files in their projects,
//! and utilize the common types and adapter traits from this crate.
//!
//! ## Dependency Injection
//!
//! Enable the `di` feature to use dependency injection in gRPC handlers:
//!
//! ```toml
//! [dependencies]
//! reinhardt-grpc = { version = "0.1", features = ["di"] }
//! ```
//!
//! Then use the `#[grpc_handler]` macro:
//!
//! ```rust,ignore
//! # use reinhardt_grpc::{GrpcRequestExt, grpc_handler};
//! # use tonic::{Request, Response, Status};
//! # struct GetUserRequest;
//! # struct User;
//! # struct DatabaseConnection;
//! # struct Handler;
//! # impl Handler {
//! #[grpc_handler]
//! async fn get_user_impl(
//!     &self,
//!     request: Request<GetUserRequest>,
//!     #[inject] db: DatabaseConnection,
//! ) -> Result<Response<User>, Status> {
//!     // db is automatically resolved
//! #   Ok(Response::new(User))
//! }
//! # }
//! ```

pub mod adapter;
pub mod depth_limit;
pub mod error;
pub mod server;
pub mod validation;

#[cfg(feature = "di")]
pub mod di;

// Generated Protobuf code (common types provided by the framework)
pub mod proto {
	pub mod common {
		tonic::include_proto!("reinhardt.common");
	}

	pub mod graphql {
		tonic::include_proto!("reinhardt.graphql");
	}
}

pub use adapter::{GrpcServiceAdapter, GrpcSubscriptionAdapter};
pub use depth_limit::{DepthLimitError, DepthLimitedDecoder};
pub use error::{ErrorSanitizer, GrpcError, GrpcResult};
pub use server::{GrpcServerConfig, GrpcServerConfigBuilder, MessageSizeLimiter};
pub use validation::{FieldRule, ProtoValidator, ValidationError, ValidationRuleSet};

#[cfg(feature = "di")]
pub use di::GrpcRequestExt;

#[cfg(feature = "di")]
pub use reinhardt_grpc_macros::grpc_handler;
