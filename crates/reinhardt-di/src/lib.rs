//! # Reinhardt Dependency Injection
//!
//! FastAPI-inspired dependency injection system for Reinhardt.
//!
//! ## Features
//!
//! - **Type-safe**: Full compile-time type checking
//! - **Async-first**: Built for async/await
//! - **Scoped**: Request-scoped and singleton dependencies
//! - **Composable**: Dependencies can depend on other dependencies
//! - **Cache**: Automatic caching within request scope
//! - **Circular Dependency Detection**: Automatic runtime detection with optimized performance
//!
//! ## Development Tools (dev-tools feature)
//!
//! When the `dev-tools` feature is enabled, additional debugging and profiling tools are available:
//!
//! - **Visualization**: Generate dependency graphs in DOT format for Graphviz
//! - **Profiling**: Track dependency resolution performance and identify bottlenecks
//! - **Advanced Caching**: LRU and TTL-based caching strategies
//!
//! ## Generator Support (generator feature) ✅
//!
//! Generator-based dependency resolution for lazy, streaming dependency injection.
//!
//! **Note**: Uses `genawaiter` crate as a workaround for unstable native async yield.
//! Will be migrated to native syntax when Rust stabilizes async generators.
//!
//! ```rust,no_run
//! # #[cfg(feature = "generator")]
//! # use reinhardt_di::generator::DependencyGenerator;
//! # #[cfg(feature = "generator")]
//! # async fn example() {
//! // let gen = DependencyGenerator::new(|co| async move {
//! //     let db = resolve_database().await;
//! //     co.yield_(db).await;
//! //
//! //     let cache = resolve_cache().await;
//! //     co.yield_(cache).await;
//! // });
//! # }
//! ```
//!
//! ## Example
//!
//! ```rust,no_run
//! # use reinhardt_di::{Depends, Injectable};
//! # #[tokio::main]
//! # async fn main() {
//! // Define a dependency
//! // struct Database {
//! //     pool: DbPool,
//! // }
//! //
//! // #[async_trait]
//! // impl Injectable for Database {
//! //     async fn inject(ctx: &InjectionContext) -> Result<Self> {
//! //         Ok(Database {
//! //             pool: get_pool().await?,
//! //         })
//! //     }
//! // }
//! //
//! // Use in endpoint
//! // #[endpoint(GET "/users")]
//! // async fn list_users(
//! //     db: Depends<Database>,
//! // ) -> Result<Vec<User>> {
//! //     db.query("SELECT * FROM users").await
//! // }
//! # }
//! ```
//!
//! ## InjectionContext Construction
//!
//! InjectionContext is constructed using the builder pattern with a required singleton scope:
//!
//! ```rust
//! use reinhardt_di::{InjectionContext, SingletonScope};
//! use std::sync::Arc;
//!
//! // Create singleton scope
//! let singleton = Arc::new(SingletonScope::new());
//!
//! // Build injection context with singleton scope
//! let ctx = InjectionContext::builder(singleton).build();
//! ```
//!
//! Optional request and param context can be added:
//!
//! ```no_run
//! use reinhardt_di::{InjectionContext, SingletonScope};
//! use reinhardt_http::Request;
//! use std::sync::Arc;
//!
//! let singleton = Arc::new(SingletonScope::new());
//!
//! // Create a dummy request for demonstration
//! let request = Request::builder()
//!     .method(hyper::Method::GET)
//!     .uri("/")
//!     .version(hyper::Version::HTTP_11)
//!     .headers(hyper::HeaderMap::new())
//!     .body(bytes::Bytes::new())
//!     .build()
//!     .unwrap();
//!
//! let ctx = InjectionContext::builder(singleton)
//!     .with_request(request)
//!     .build();
//! ```
//!
//! ## Circular Dependency Detection
//!
//! The DI system automatically detects circular dependencies at runtime using an optimized
//! thread-local mechanism:
//!
//! ```no_run
//! # use reinhardt_di::{Injectable, InjectionContext, SingletonScope, DiResult};
//! # use async_trait::async_trait;
//! # use std::sync::Arc;
//! #[derive(Clone)]
//! struct ServiceA {
//!     b: Arc<ServiceB>,
//! }
//!
//! #[derive(Clone)]
//! struct ServiceB {
//!     a: Arc<ServiceA>,  // Circular dependency!
//! }
//!
//! #[async_trait]
//! impl Injectable for ServiceA {
//!     async fn inject(ctx: &InjectionContext) -> DiResult<Self> {
//!         let b = ctx.resolve::<ServiceB>().await?;
//!         Ok(ServiceA { b })
//!     }
//! }
//!
//! #[async_trait]
//! impl Injectable for ServiceB {
//!     async fn inject(ctx: &InjectionContext) -> DiResult<Self> {
//!         let a = ctx.resolve::<ServiceA>().await?;
//!         Ok(ServiceB { a })
//!     }
//! }
//!
//! let singleton = Arc::new(SingletonScope::new());
//! let ctx = InjectionContext::builder(singleton).build();
//!
//! // This will return Err with DiError::CircularDependency
//! let result = ctx.resolve::<ServiceA>().await;
//! assert!(result.is_err());
//! ```
//!
//! ### Performance Characteristics
//!
//! - **Cache Hit**: < 5% overhead (cycle detection completely skipped)
//! - **Cache Miss**: 10-20% overhead (O(1) detection using HashSet)
//! - **Deep Chains**: Sampling reduces linear cost (checks every 10th at depth 50+)
//! - **Thread Safety**: Thread-local storage eliminates lock contention
//!
//! ## Development Tools Example
//!
//! ```no_run
//! # #[cfg(feature = "dev-tools")]
//! # use reinhardt_di::{visualization::DependencyGraph, profiling::DependencyProfiler};
//! # #[cfg(feature = "dev-tools")]
//! # fn main() {
//! // fn visualize_dependencies() {
//! //     let mut graph = DependencyGraph::new();
//! //     graph.add_node("Database", "singleton");
//! //     graph.add_node("UserService", "request");
//! //     graph.add_dependency("UserService", "Database");
//! //
//! //     println!("{}", graph.to_dot());
//! // }
//! //
//! // fn profile_resolution() {
//! //     let mut profiler = DependencyProfiler::new();
//! //     profiler.start_resolve("Database");
//! //     // ... perform resolution ...
//! //     profiler.end_resolve("Database");
//! //
//! //     let report = profiler.generate_report();
//! //     println!("{}", report.to_string());
//! // }
//! # }
//! ```
//!
//! ## Auth Extractor DI Context Requirements
//!
//! The `reinhardt-auth` crate provides injectable auth extractors that depend on
//! specific DI context configuration. Understanding these requirements is essential
//! for proper authentication integration.
//!
//! ### `AuthUser<U>` (recommended)
//!
//! Loads the full user model from the database. Requires:
//!
//! - **`DatabaseConnection`** registered as a singleton in `InjectionContext`
//! - **`AuthState`** present in request extensions (set by authentication middleware)
//! - Feature `params` enabled on `reinhardt-auth`
//!
//! Returns an injection error if any requirement is missing (fail-fast behavior).
//!
//! ```no_run
//! use reinhardt_auth::AuthUser;
//! use reinhardt_auth::DefaultUser;
//!
//! #[get("/profile/")]
//! pub async fn profile(
//!     #[inject] AuthUser(user): AuthUser<DefaultUser>,
//! ) -> ViewResult<Response> {
//!     let username = user.get_username();
//!     // ...
//! }
//! ```
//!
//! ### `AuthInfo` (lightweight alternative)
//!
//! Extracts authentication metadata without a database query. Requires:
//!
//! - **`AuthState`** present in request extensions (set by authentication middleware)
//! - No `DatabaseConnection` needed
//!
//! ### `CurrentUser<U>` (deprecated)
//!
//! Deprecated in favor of `AuthUser<U>`. Unlike `AuthUser<U>`, missing context
//! causes silent fallback to anonymous instead of returning an error.
//!
//! ### Startup Validation
//!
//! Call `reinhardt_auth::validate_auth_extractors()` during application startup
//! to verify that required dependencies (e.g., `DatabaseConnection`) are registered
//! before the first request arrives.

#![warn(missing_docs)]

pub mod params;

pub mod context;
pub mod cycle_detection;
pub mod depends;
pub mod function_handle;
pub mod graph;
pub mod injectable;
pub mod injected;
pub mod override_registry;
pub mod provider;
pub mod registration;
pub mod registry;
pub mod scope;

use thiserror::Error;

pub use context::{InjectionContext, InjectionContextBuilder, RequestContext};
pub use cycle_detection::{
	CycleError, ResolutionGuard, begin_resolution, register_type_name, with_cycle_detection_scope,
};
pub use function_handle::FunctionHandle;
pub use override_registry::OverrideRegistry;

#[cfg(feature = "params")]
pub use context::{ParamContext, Request};
pub use depends::{Depends, DependsBuilder};
pub use injectable::Injectable;
pub use injected::{
	DependencyScope as InjectedScope, Injected, InjectionMetadata, OptionalInjected,
};
pub use provider::{Provider, ProviderFn};
pub use registration::DiRegistrationList;
pub use registry::{
	DependencyRegistration, DependencyRegistry, DependencyScope, FactoryTrait, global_registry,
};
pub use scope::{RequestScope, Scope, SingletonScope};

// Re-export inventory for macro use
pub use inventory;

// Re-export macros
#[cfg(feature = "macros")]
pub use reinhardt_di_macros::{injectable, injectable_factory};

/// Errors that can occur during dependency injection resolution.
#[derive(Debug, Error)]
pub enum DiError {
	/// The requested dependency was not found in the container.
	#[error("Dependency not found: {0}")]
	NotFound(String),

	/// A circular dependency chain was detected during resolution.
	#[error("Circular dependency detected: {0}")]
	CircularDependency(String),

	/// An error occurred in a dependency provider function.
	#[error("Provider error: {0}")]
	ProviderError(String),

	/// The resolved type did not match the expected type.
	#[error("Type mismatch: expected {expected}, got {actual}")]
	TypeMismatch {
		/// The type that was expected.
		expected: String,
		/// The type that was actually resolved.
		actual: String,
	},

	/// An error related to dependency scoping (request vs singleton).
	#[error("Scope error: {0}")]
	ScopeError(String),

	/// The requested type was not registered in the dependency registry.
	#[error("Type '{type_name}' not registered. {hint}")]
	NotRegistered {
		/// The name of the unregistered type.
		type_name: String,
		/// A hint message suggesting how to register the type.
		hint: String,
	},

	/// A required dependency was not registered.
	#[error("Dependency not registered: {type_name}")]
	DependencyNotRegistered {
		/// The name of the unregistered dependency type.
		type_name: String,
	},

	/// An internal error in the DI system.
	#[error("Internal error: {message}")]
	Internal {
		/// A description of the internal error.
		message: String,
	},
}

impl From<DiError> for reinhardt_core::exception::Error {
	fn from(err: DiError) -> Self {
		match &err {
			DiError::NotFound(_)
			| DiError::NotRegistered { .. }
			| DiError::DependencyNotRegistered { .. } => reinhardt_core::exception::Error::NotFound(
				format!("Dependency injection error: {}", err),
			),
			_ => reinhardt_core::exception::Error::Internal(format!(
				"Dependency injection error: {}",
				err
			)),
		}
	}
}

/// A specialized `Result` type for dependency injection operations.
pub type DiResult<T> = std::result::Result<T, DiError>;

// Generator support
#[cfg(feature = "generator")]
pub mod generator;

// Development tools
#[cfg(feature = "dev-tools")]
pub mod visualization;

#[cfg(feature = "dev-tools")]
pub mod profiling;

#[cfg(feature = "dev-tools")]
pub mod advanced_cache;
