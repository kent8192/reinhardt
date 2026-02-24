//! # Reinhardt
//!
//! A full-stack API framework for Rust, inspired by Django and Django REST Framework.
//!
//! Reinhardt provides a complete, batteries-included solution for building production-ready
//! REST APIs with Rust. It follows Rust's composition patterns instead of Python's inheritance
//! model, making full use of traits, generics, and zero-cost abstractions.
//!
//! ## Core Principles
//!
//! - **Composition over Inheritance**: Uses Rust's trait system for composable behavior
//! - **Type Safety**: Leverages Rust's type system for compile-time guarantees
//! - **Zero-Cost Abstractions**: High-level ergonomics without runtime overhead
//! - **Async-First**: Built on tokio and async/await from the ground up
//!
//! ## Feature Flags
//!
//! Reinhardt provides flexible feature flags to control compilation and reduce binary size.
//!
//! ### Presets
//!
//! - `minimal` - Core functionality only (routing, DI, params)
//! - `standard` (default) - Balanced for most projects
//! - `full` - All features enabled
//! - `api-only` - REST API without templates/forms
//! - `graphql-server` - GraphQL-focused setup
//! - `websocket-server` - WebSocket-centric setup
//! - `cli-tools` - CLI and background jobs
//! - `test-utils` - Testing utilities
//!
//! ### Fine-grained Control
//!
//! Fine-grained feature flags for precise control over included functionality:
//!
//! #### Authentication ✅
//! - `auth-jwt` - JWT authentication
//! - `auth-session` - Session-based authentication
//! - `auth-oauth` - OAuth2 support
//! - `auth-token` - Token authentication
//!
//! #### Database Backends ✅
//! - `db-postgres` - PostgreSQL support
//! - `db-mysql` - MySQL support
//! - `db-sqlite` - SQLite support
//! - `db-cockroachdb` - CockroachDB support (distributed transactions)
//!
//! #### Middleware ✅
//! - `middleware-cors` - CORS (Cross-Origin Resource Sharing) middleware
//! - `middleware-compression` - Response compression (Gzip, Brotli)
//! - `middleware-security` - Security headers (HSTS, XSS Protection, etc.)
//! - `middleware-rate-limit` - Rate limiting and throttling
//!
//! See [docs/FEATURE_FLAGS.md](../docs/FEATURE_FLAGS.md) for detailed documentation.
//!
//! ## Quick Example
//!
//! ```rust,ignore
//! use reinhardt::prelude::*;
//! use serde::{Serialize, Deserialize};
//! use std::sync::Arc;
//!
//! // Define your model (using composition, not inheritance)
//! #[derive(Debug, Clone, Serialize, Deserialize)]
//! struct User {
//!     id: Option<i64>,
//!     username: String,
//!     email: String,
//! }
//!
//! // Implement Model trait
//! impl Model for User {
//!     type PrimaryKey = i64;
//!     fn table_name() -> &'static str { "users" }
//!     fn primary_key(&self) -> Option<&Self::PrimaryKey> { self.id.as_ref() }
//!     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
//! }
//!
//! // Create a ViewSet (no inheritance needed!)
//! let users_viewset = ModelViewSet::<User, JsonSerializer<User>>::new("users");
//!
//! // Set up routing
//! let mut router = DefaultRouter::new();
//! router.register_viewset("users", users_viewset);
//!
//! // Add middleware using composition
//! let app = MiddlewareChain::new(Arc::new(router))
//!     .with_middleware(Arc::new(LoggingMiddleware::new()))
//!     .with_middleware(Arc::new(CorsMiddleware::permissive()));
//! ```

// Re-export external crates for macro support
// Macro-generated code uses paths like `::reinhardt::reinhardt_apps::AppConfig`
// These wrapper modules provide the namespace structure that macros expect.
// Note: These are marked #[doc(hidden)] because they are internal dependencies
// used by macro-generated code. Users should not use these directly.

// WASM-compatible re-exports (always available)
#[cfg(feature = "pages")]
#[doc(hidden)]
pub mod reinhardt_pages {
	pub use reinhardt_pages::*;
}

#[doc(hidden)]
pub mod reinhardt_types {
	// Public API surface glob re-export requires allowing unused imports and unreachable pub
	#[allow(unused_imports, unreachable_pub)]
	pub use reinhardt_core::types::*;
}

// Server-side only re-exports (NOT for WASM)
#[cfg(not(target_arch = "wasm32"))]
#[doc(hidden)]
pub mod reinhardt_apps {
	pub use reinhardt_apps::*;
}

#[cfg(not(target_arch = "wasm32"))]
#[doc(hidden)]
pub mod reinhardt_di {
	pub use reinhardt_di::*;
}

#[cfg(not(target_arch = "wasm32"))]
#[doc(hidden)]
pub mod reinhardt_core {
	pub use reinhardt_core::*;
	// For macro compatibility: Re-export EndpointMetadata at module level
	pub use reinhardt_core::endpoint::EndpointMetadata;
}

#[cfg(not(target_arch = "wasm32"))]
#[doc(hidden)]
pub mod reinhardt_http {
	pub use reinhardt_http::*;
}

#[cfg(not(target_arch = "wasm32"))]
#[doc(hidden)]
pub mod reinhardt_params {
	pub use reinhardt_di::params::*;
}

#[cfg(not(target_arch = "wasm32"))]
#[doc(hidden)]
pub mod async_trait {
	pub use async_trait::*;
}

#[cfg(all(feature = "database", not(target_arch = "wasm32")))]
#[doc(hidden)]
pub mod linkme {
	pub use linkme::*;
}

#[cfg(all(feature = "database", not(target_arch = "wasm32")))]
#[doc(hidden)]
pub mod reinhardt_orm {
	pub use reinhardt_db::orm::*;
}

// Module re-exports following Django's structure
// WASM-compatible modules (always available)
#[cfg(feature = "pages")]
pub mod pages;

// Server-side only modules (NOT for WASM)
#[cfg(all(feature = "admin", not(target_arch = "wasm32")))]
pub mod admin;
#[cfg(all(feature = "core", not(target_arch = "wasm32")))]
pub mod apps;
#[cfg(all(feature = "commands", not(target_arch = "wasm32")))]
pub mod commands;
#[cfg(all(feature = "conf", not(target_arch = "wasm32")))]
pub mod conf;
#[cfg(all(feature = "core", not(target_arch = "wasm32")))]
pub mod core;
#[cfg(all(feature = "dentdelion", not(target_arch = "wasm32")))]
pub mod dentdelion;
#[cfg(all(feature = "di", not(target_arch = "wasm32")))]
pub mod di;
#[cfg(all(feature = "forms", not(target_arch = "wasm32")))]
pub mod forms;
#[cfg(not(target_arch = "wasm32"))]
pub mod http;
#[cfg(all(
	any(feature = "standard", feature = "middleware"),
	not(target_arch = "wasm32")
))]
pub mod middleware;
#[cfg(all(feature = "rest", not(target_arch = "wasm32")))]
pub mod rest;
#[cfg(all(feature = "server", not(target_arch = "wasm32")))]
pub mod server;
#[cfg(all(feature = "shortcuts", not(target_arch = "wasm32")))]
pub mod shortcuts;
#[cfg(all(feature = "tasks", not(target_arch = "wasm32")))]
pub mod tasks;
#[cfg(all(feature = "templates", not(target_arch = "wasm32")))]
pub mod template;
#[cfg(all(feature = "test", not(target_arch = "wasm32")))]
pub mod test;
#[cfg(not(target_arch = "wasm32"))]
pub mod urls;
#[cfg(not(target_arch = "wasm32"))]
pub mod utils;
#[cfg(not(target_arch = "wasm32"))]
pub mod views;

// Server-side only re-exports (NOT for WASM)
// Re-export app types from reinhardt-apps
#[cfg(all(feature = "core", not(target_arch = "wasm32")))]
pub use reinhardt_apps::{AppConfig, AppError, AppResult, Apps};

// Re-export macros
#[cfg(all(feature = "core", not(target_arch = "wasm32")))]
pub use reinhardt_macros::{AppConfig, app_config, installed_apps};

// Re-export Model derive macro and model attribute macro (requires database feature)
#[cfg(all(feature = "database", not(target_arch = "wasm32")))]
pub use reinhardt_macros::{Model, model};

// Re-export collect_migrations macro (requires database feature)
#[cfg(all(feature = "database", not(target_arch = "wasm32")))]
pub use reinhardt_macros::collect_migrations;

// Re-export reinhardt_migrations crate (used by collect_migrations! macro)
#[cfg(all(feature = "database", not(target_arch = "wasm32")))]
pub use reinhardt_db::migrations;

// Alias for macro compatibility
#[cfg(all(feature = "database", not(target_arch = "wasm32")))]
#[doc(hidden)]
pub use migrations as reinhardt_migrations;

// Re-export reinhardt_macros as a module for hierarchical imports
// This allows macro-generated code to use ::reinhardt::macros::Model
#[cfg(not(target_arch = "wasm32"))]
#[doc(hidden)]
pub mod macros {
	pub use reinhardt_macros::*;
}

// Re-export HTTP method macros
#[cfg(not(target_arch = "wasm32"))]
pub use reinhardt_macros::{api_view, delete, get, patch, post, put};

// Re-export routes attribute macro for URL pattern registration
#[cfg(not(target_arch = "wasm32"))]
pub use reinhardt_macros::routes;

// Re-export admin attribute macro (requires admin feature)
#[cfg(all(feature = "admin", not(target_arch = "wasm32")))]
pub use reinhardt_macros::admin;

// Re-export settings from dedicated crate
#[cfg(all(feature = "conf", not(target_arch = "wasm32")))]
pub use reinhardt_conf::settings::{
	AdvancedSettings, CacheSettings, CorsSettings, DatabaseConfig, EmailSettings, LoggingSettings,
	MediaSettings, MiddlewareConfig, SessionSettings, Settings, SettingsError, StaticSettings,
	TemplateConfig,
};

#[cfg(all(feature = "conf", not(target_arch = "wasm32")))]
pub use reinhardt_conf::settings::builder::SettingsBuilder;

#[cfg(all(feature = "conf", not(target_arch = "wasm32")))]
pub use reinhardt_conf::settings::profile::Profile;

#[cfg(all(feature = "conf", not(target_arch = "wasm32")))]
pub use reinhardt_conf::settings::sources::{
	DefaultSource, EnvSource, LowPriorityEnvSource, TomlFileSource,
};

// Re-export core types
#[cfg(all(feature = "core", not(target_arch = "wasm32")))]
pub use reinhardt_core::{
	endpoint::EndpointMetadata,
	exception::{Error, Result},
};

// Re-export HTTP types
#[cfg(all(feature = "core", not(target_arch = "wasm32")))]
pub use reinhardt_http::{Handler, Middleware, MiddlewareChain, Request, Response, ViewResult};

// Re-export inventory crate (used by HTTP method macros for endpoint registration)
#[cfg(not(target_arch = "wasm32"))]
#[doc(hidden)]
pub use inventory;

// Re-export ORM
#[cfg(all(feature = "database", not(target_arch = "wasm32")))]
pub use reinhardt_db::orm::{
	DatabaseBackend, DatabaseConnection, Model, QuerySet, SoftDeletable, SoftDelete, Timestamped,
	Timestamps,
};

// Re-export ORM query expressions (Django-style F/Q objects)
//
// # Availability
//
// Requires `database` feature.
//
// # Examples
//
// ```rust,no_run
// # use reinhardt::{F, Q};
// // Reference a field (like Django's F object)
// let price_expr = F::field("price");
//
// // Build complex queries (like Django's Q object)
// let filter = Q::and(vec![
//     Q::field("status").equals("active"),
//     Q::field("price").gt(100),
// ]);
// ```
#[cfg(all(feature = "database", not(target_arch = "wasm32")))]
pub use reinhardt_db::orm::{
	// Query expressions (equivalent to Django's F and Q)
	Exists,
	F,
	FieldRef,
	// Query filter types
	Filter,
	FilterOperator,
	FilterValue,
	OuterRef,
	Q,
	QOperator,
	Subquery,
};

// Re-export ORM annotations and aggregations
//
// # Availability
//
// Requires `database` feature.
//
// # Examples
//
// ```rust,no_run
// # use reinhardt::{Annotation, Aggregate, F};
// # struct User;
// # impl User { fn objects() -> QueryBuilder { QueryBuilder } }
// # struct Product;
// # impl Product { fn objects() -> QueryBuilder { QueryBuilder } }
// # struct QueryBuilder;
// # impl QueryBuilder {
// #     fn annotate(self, _name: &str, _val: Annotation) -> Self { self }
// #     fn aggregate(self, _name: &str, _val: Aggregate) -> Self { self }
// # }
// // Annotate query results with computed values
// let query = User::objects()
//     .annotate("full_name", Annotation::concat(vec![
//         F::field("first_name"),
//         F::value(" "),
//         F::field("last_name"),
//     ]));
//
// // Aggregate data
// let stats = Product::objects()
//     .aggregate("avg_price", Aggregate::avg("price"));
// ```
#[cfg(all(feature = "database", not(target_arch = "wasm32")))]
pub use reinhardt_db::orm::{
	// Aggregations
	Aggregate,
	AggregateFunc,
	AggregateValue,
	// Annotations
	Annotation,
	AnnotationValue,
};

// Re-export ORM transactions
//
// # Availability
//
// Requires `database` feature.
//
// # Examples
//
// ```rust,no_run
// # use reinhardt::{atomic, IsolationLevel, atomic_with_isolation};
// # #[tokio::main]
// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
// # struct User;
// # struct Profile;
// # let data = ();
// // Use atomic decorator for transactions
// // let result = atomic(|| async {
// //     let user = User::create(data).await?;
// //     let profile = Profile::create(user.id).await?;
// //     Ok((user, profile))
// // }).await?;
//
// // Or with specific isolation level
// // let result = atomic_with_isolation(IsolationLevel::Serializable, || async {
// //     // Your transaction code
// // }).await?;
// # Ok(())
// # }
// ```
#[cfg(all(feature = "database", not(target_arch = "wasm32")))]
pub use reinhardt_db::orm::{
	// Transaction management
	IsolationLevel,
	QueryValue,
	Savepoint,
	Transaction,
	TransactionExecutor,
	TransactionScope,
	atomic,
	atomic_with_isolation,
};

// Re-export ORM database functions
//
// # Availability
//
// Requires `database` feature.
//
// # Examples
//
// ```rust,no_run
// # use reinhardt::{Concat, Upper, Lower, Now, F, Q};
// # struct User;
// # impl User { fn objects() -> QueryBuilder { QueryBuilder } }
// # struct QueryBuilder;
// # impl QueryBuilder {
// #     fn annotate(self, _name: &str, _val: impl std::any::Any) -> Self { self }
// #     fn filter(self, _q: impl std::any::Any) -> Self { self }
// # }
// // String functions
// let query = User::objects()
//     .annotate("full_name", Concat::new(vec![
//         F::field("first_name"),
//         F::value(" "),
//         F::field("last_name"),
//     ]))
//     .annotate("email_upper", Upper::new(F::field("email")));
//
// // Date/time functions
// let recent_users = User::objects()
//     .filter(Q::field("created_at").gte(Now::new()));
// ```
#[cfg(all(feature = "database", not(target_arch = "wasm32")))]
pub use reinhardt_db::orm::{
	// Math functions
	Abs,
	// Utility functions
	Cast,
	Ceil,
	// String functions
	Concat,
	// Date/time functions
	CurrentDate,
	CurrentTime,
	Extract,
	ExtractComponent,
	Floor,
	Greatest,
	Least,
	Length,
	Lower,
	Mod,
	Now,
	NullIf,
	Power,
	Round,
	SqlType,
	Sqrt,
	Substr,
	Trim,
	TrimType,
	Upper,
};

// Re-export ORM window functions
//
// # Availability
//
// Requires `database` feature.
//
// # Examples
//
// ```rust,no_run
// # use reinhardt::{Window, RowNumber, Rank};
// # struct Product;
// # impl Product { fn objects() -> QueryBuilder { QueryBuilder } }
// # struct Sale;
// # impl Sale { fn objects() -> QueryBuilder { QueryBuilder } }
// # struct QueryBuilder;
// # impl QueryBuilder {
// #     fn annotate(self, _name: &str, _val: impl std::any::Any) -> Self { self }
// # }
// // Add row numbers to query results
// let query = Product::objects()
//     .annotate("row_num", RowNumber::new()
//         .over(Window::new().order_by("price")));
//
// // Ranking within partitions
// let query = Sale::objects()
//     .annotate("rank", Rank::new()
//         .over(Window::new()
//             .partition_by("category")
//             .order_by("-amount")));
// ```
#[cfg(all(feature = "database", not(target_arch = "wasm32")))]
pub use reinhardt_db::orm::{
	// Ranking functions
	DenseRank,
	// Value functions
	FirstValue,
	// Window specification
	Frame,
	FrameBoundary,
	FrameType,
	Lag,
	LastValue,
	Lead,
	NTile,
	NthValue,
	Rank,
	RowNumber,
	Window,
	WindowFunction,
};

// Re-export ORM constraints and indexes
//
// # Availability
//
// Requires `database` feature.
//
// # Examples
//
// ```rust,no_run
// # use reinhardt::{UniqueConstraint, BTreeIndex};
// // Define constraints programmatically
// let constraint = UniqueConstraint::new(vec!["email"]);
//
// // Create indexes
// let index = BTreeIndex::new("user_email_idx", vec!["email"]);
// ```
#[cfg(all(feature = "database", not(target_arch = "wasm32")))]
pub use reinhardt_db::orm::{
	// Indexes
	BTreeIndex,
	// Constraints
	CheckConstraint,
	Constraint,
	ForeignKeyConstraint,
	GinIndex,
	GistIndex,
	HashIndex,
	Index,
	OnDelete,
	OnUpdate,
	UniqueConstraint,
};

// Re-export database pool
#[cfg(all(feature = "database", not(target_arch = "wasm32")))]
pub use reinhardt_db::pool::{ConnectionPool, PoolConfig, PoolError};

// Re-export serializers
#[cfg(all(feature = "rest", not(target_arch = "wasm32")))]
pub use reinhardt_rest::serializers::{Deserializer, JsonSerializer, Serializer};

// Re-export viewsets
#[cfg(not(target_arch = "wasm32"))]
pub use reinhardt_views::viewsets::{
	Action, ActionType, CreateMixin, DestroyMixin, GenericViewSet, ListMixin, ModelViewSet,
	ReadOnlyModelViewSet, RetrieveMixin, UpdateMixin, ViewSet,
};

// Re-export routers
#[cfg(not(target_arch = "wasm32"))]
pub use reinhardt_urls::routers::{
	DefaultRouter, PathMatcher, PathPattern, Route, Router, ServerRouter, UnifiedRouter,
	UrlPatternsRegistration, clear_router, get_router, is_router_registered, register_router,
	register_router_arc,
};

// Re-export client-router types (requires client-router feature)
// These types enable UnifiedRouter<V> with both .server() and .client() methods
#[cfg(feature = "client-router")]
pub use reinhardt_urls::routers::{
	ClientPathPattern, ClientRoute, ClientRouteMatch, ClientRouter, FromPath, HistoryState,
	NavigationType, ParamContext, SingleFromPath,
};
// Path extractor for client-side routing (separate from server-side Path from reinhardt-di)
#[cfg(feature = "client-router")]
pub use reinhardt_urls::routers::Path as ClientPath;

// Re-export auth
#[cfg(all(feature = "auth", not(target_arch = "wasm32")))]
pub use reinhardt_auth::{
	AllowAny, AnonymousUser, AuthBackend, BaseUser, CurrentUser, DefaultUser, FullUser,
	IsAdminUser, IsAuthenticated, PasswordHasher, Permission, PermissionsMixin, SimpleUser, User,
};

#[cfg(all(feature = "auth", not(target_arch = "wasm32")))]
#[cfg_attr(docsrs, doc(cfg(feature = "argon2-hasher")))]
pub use reinhardt_auth::Argon2Hasher;

#[cfg(all(feature = "auth-jwt", not(target_arch = "wasm32")))]
pub use reinhardt_auth::{Claims, JwtAuth};

// Re-export auth management
//
// # Availability
//
// Requires `auth` feature.
//
// # Examples
//
// ```rust,no_run
// # use reinhardt::{UserManager, GroupManager, ObjectPermission, CreateUserData, CreateGroupData};
// # #[tokio::main]
// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
// // User management
// let user_manager = UserManager::new();
// // let user = user_manager.create_user(CreateUserData {
// //     username: "alice".to_string(),
// //     email: "alice@example.com".to_string(),
// //     password: "secret".to_string(),
// // }).await?;
//
// // Group management
// let group_manager = GroupManager::new();
// // let group = group_manager.create_group(CreateGroupData {
// //     name: "editors".to_string(),
// // }).await?;
//
// // Object-level permissions
// // let perm = ObjectPermission::new("edit", user, article);
// # Ok(())
// # }
// ```
#[cfg(all(feature = "auth", not(target_arch = "wasm32")))]
pub use reinhardt_auth::{
	// Group management
	CreateGroupData,
	// User management
	CreateUserData,
	Group,
	GroupManagementError,
	GroupManagementResult,
	GroupManager,
	// Object-level permissions
	ObjectPermission,
	ObjectPermissionChecker,
	ObjectPermissionManager,
	UpdateUserData,
	UserManagementError,
	UserManagementResult,
	UserManager,
};

// Re-export middleware
#[cfg(all(feature = "sessions", not(target_arch = "wasm32")))]
pub use reinhardt_middleware::AuthenticationMiddleware;

#[cfg(all(
	any(feature = "standard", feature = "middleware"),
	not(target_arch = "wasm32")
))]
pub use reinhardt_middleware::LoggingMiddleware;

#[cfg(all(feature = "middleware-cors", not(target_arch = "wasm32")))]
pub use reinhardt_middleware::CorsMiddleware;

// Re-export HTTP types (additional commonly used types)
#[cfg(all(feature = "core", not(target_arch = "wasm32")))]
pub use reinhardt_http::Extensions;

// Re-export HTTP types from hyper (already used in reinhardt_http)
#[cfg(not(target_arch = "wasm32"))]
pub use hyper::{Method, StatusCode};

// Re-export pagination
#[cfg(all(feature = "rest", not(target_arch = "wasm32")))]
pub use reinhardt_rest::pagination::{
	CursorPagination, LimitOffsetPagination, PageNumberPagination, PaginatedResponse, Paginator,
};

// Re-export filters
#[cfg(all(feature = "rest", not(target_arch = "wasm32")))]
pub use reinhardt_rest::filters::{
	FieldOrderingExt, FilterBackend, FilterError, FilterResult, MultiTermSearch,
};

// Re-export throttling
#[cfg(all(feature = "rest", not(target_arch = "wasm32")))]
pub use reinhardt_rest::throttling::{
	AnonRateThrottle, ScopedRateThrottle, Throttle, UserRateThrottle,
};

// Re-export signals
#[cfg(all(feature = "core", not(target_arch = "wasm32")))]
pub use reinhardt_core::signals::{
	M2MAction, M2MChangeEvent, Signal, m2m_changed, post_delete, post_save, pre_delete, pre_save,
};

// Re-export core utilities
// Note: reinhardt_types provides Handler, Middleware, etc. which are already re-exported via reinhardt_apps

// Re-export validators
#[cfg(all(feature = "core", not(target_arch = "wasm32")))]
pub use reinhardt_core::validators::{
	CreditCardValidator, EmailValidator, IBANValidator, IPAddressValidator, PhoneNumberValidator,
	UrlValidator, ValidationError as ValidatorError, ValidationResult, Validator,
};

// Re-export views
#[cfg(not(target_arch = "wasm32"))]
pub use reinhardt_views::{
	Context, DetailView, ListView, MultipleObjectMixin, SingleObjectMixin, View,
};

// Re-export parsers
#[cfg(all(feature = "rest", not(target_arch = "wasm32")))]
pub use reinhardt_rest::parsers::{
	FileUploadParser, FormParser, JSONParser, MediaType, MultiPartParser, ParseError, ParseResult,
	Parser,
};

// Re-export versioning
#[cfg(all(feature = "rest", not(target_arch = "wasm32")))]
pub use reinhardt_rest::versioning::{
	AcceptHeaderVersioning, BaseVersioning, HostNameVersioning, NamespaceVersioning,
	QueryParameterVersioning, RequestVersionExt, URLPathVersioning, VersioningError,
	VersioningMiddleware,
};

// Re-export metadata
#[cfg(all(feature = "rest", not(target_arch = "wasm32")))]
pub use reinhardt_rest::metadata::{
	ActionMetadata, BaseMetadata, ChoiceInfo, FieldInfo, FieldInfoBuilder, FieldType,
	MetadataOptions, MetadataResponse, SimpleMetadata,
};

// Re-export negotiation
#[cfg(all(feature = "rest", not(target_arch = "wasm32")))]
pub use reinhardt_rest::negotiation::*;

// Re-export REST integration modules
#[cfg(all(feature = "rest", not(target_arch = "wasm32")))]
pub use reinhardt_rest::{
	filters, metadata, negotiation, pagination, parsers, serializers, throttling, versioning,
};

// Re-export browsable API (from reinhardt-browsable-api via reinhardt-rest)
#[cfg(all(feature = "rest", not(target_arch = "wasm32")))]
pub use reinhardt_rest::browsable_api;

// Re-export OpenAPI types
//
// # Availability
//
// Requires `openapi` feature.
//
// # Examples
//
// ```rust,no_run
// # // Note: This example requires the openapi feature
// // use reinhardt::{OpenApi, ApiDoc};
// //
// // // Define API documentation
// // #[derive(OpenApi)]
// // #[openapi(paths(get_users, create_user))]
// // struct ApiDoc;
// //
// // // Generate OpenAPI schema
// // let openapi = ApiDoc::openapi();
// // let json = serde_json::to_string_pretty(&openapi)?;
// ```
#[cfg(all(feature = "openapi", not(target_arch = "wasm32")))]
pub use reinhardt_rest::openapi::*;

// Re-export shortcuts (Django-style convenience functions)
#[cfg(all(feature = "shortcuts", not(target_arch = "wasm32")))]
pub use reinhardt_shortcuts::{
	get_list_or_404, get_object_or_404, redirect, render_html, render_json, render_text,
};

// Re-export URL utilities
#[cfg(not(target_arch = "wasm32"))]
pub use reinhardt_urls::routers::{
	UrlPattern, UrlPatternWithParams, UrlReverser, include_routes as include, path, re_path,
	reverse,
};

// Admin functionality is available through reinhardt-admin-api crate
// See reinhardt-admin-types for type definitions

// Re-export database related (database feature)
#[cfg(all(feature = "database", not(target_arch = "wasm32")))]
pub use reinhardt_db::contenttypes::{
	CONTENT_TYPE_REGISTRY, ContentType, ContentTypeRegistry, GenericForeignKey, GenericRelatable,
	GenericRelationQuery, ModelType,
};
#[cfg(all(feature = "database", not(target_arch = "wasm32")))]
pub use reinhardt_db::migrations::{
	FieldState, Migration, MigrationAutodetector, MigrationError, MigrationPlan, MigrationRecorder,
	ModelState, ProjectState,
};

// Re-export cache (cache feature)
#[cfg(all(feature = "cache", not(target_arch = "wasm32")))]
pub use reinhardt_utils::cache::{Cache, CacheKeyBuilder, InMemoryCache};

// Cache middleware is in reinhardt-middleware
#[cfg(all(feature = "middleware", not(target_arch = "wasm32")))]
pub use reinhardt_middleware::CacheMiddleware;

#[cfg(all(
	feature = "cache",
	feature = "redis-backend",
	not(target_arch = "wasm32")
))]
pub use reinhardt_utils::cache::RedisCache;

// Re-export sessions (sessions feature)
#[cfg(all(feature = "sessions", not(target_arch = "wasm32")))]
pub use reinhardt_auth::sessions::{
	CacheSessionBackend, InMemorySessionBackend, Session, SessionBackend, SessionError,
};

#[cfg(all(
	feature = "sessions",
	feature = "middleware",
	not(target_arch = "wasm32")
))]
pub use reinhardt_auth::sessions::{HttpSessionConfig, SameSite, SessionMiddleware};

// Re-export contrib modules (contrib feature)
// Note: reinhardt_contrib exports individual modules (auth, sessions, etc.)
// rather than a single "contrib" module

// Re-export forms (forms feature)
#[cfg(all(feature = "forms", not(target_arch = "wasm32")))]
pub use reinhardt_forms::{
	BoundField, CharField, EmailField, FieldError, FileField, Form, FormError, FormResult,
	IntegerField, ModelForm,
};

// Re-export DI and parameters (FastAPI-style parameter extraction)
#[cfg(all(feature = "di", not(target_arch = "wasm32")))]
pub use reinhardt_di::{Depends, DiError, DiResult, InjectionContext, RequestContext};

// Re-export DI params - available in minimal, standard, and di features
#[cfg(all(
	any(feature = "minimal", feature = "standard", feature = "di"),
	not(target_arch = "wasm32")
))]
pub use reinhardt_di::params::{Body, Cookie, Header, Json, Path, Query};

// Re-export template/rendering functionality from reinhardt-pages
// Note: TemplateError was removed as Tera templating was replaced with reinhardt-pages SSR

// Re-export tasks
#[cfg(all(feature = "tasks", not(target_arch = "wasm32")))]
pub use reinhardt_tasks::{Scheduler, Task, TaskExecutor, TaskQueue};

// Re-export test utilities
#[cfg(all(feature = "test", not(target_arch = "wasm32")))]
pub use reinhardt_test::{APIClient, APIRequestFactory, APITestCase, TestResponse};

// Re-export storage
#[cfg(all(feature = "storage", not(target_arch = "wasm32")))]
pub use reinhardt_utils::storage::{InMemoryStorage, LocalStorage, Storage};

// Server-side only prelude (NOT for WASM)
#[cfg(not(target_arch = "wasm32"))]
pub mod prelude {
	// Core types - always available
	pub use crate::{
		Action,
		DefaultRouter,
		DetailView,
		ListView,
		ModelViewSet,
		MultipleObjectMixin,
		ReadOnlyModelViewSet,
		Route,
		Router,
		ServerRouter,
		SingleObjectMixin,
		StatusCode,
		UnifiedRouter,
		View,
		ViewResult,
		ViewSet,
		// Routers
		clear_router,
		get_router,
		is_router_registered,
		register_router,
	};

	// External dependencies (via core)
	#[cfg(feature = "core")]
	pub use crate::core::async_trait;
	#[cfg(feature = "core")]
	pub use crate::core::serde::{Deserialize, Serialize};

	// Core feature - types, signals, etc.
	#[cfg(feature = "core")]
	pub use crate::{
		Error, Handler, Middleware, MiddlewareChain, Request, Response, Result, Signal,
		m2m_changed, post_delete, post_save, pre_delete, pre_save,
	};

	// HTTP method macros - always available
	pub use crate::{api_view, delete, get, patch, post, put};

	// Database feature - ORM and Model macros
	#[cfg(feature = "database")]
	pub use crate::{
		Aggregate,
		// Annotations and aggregations
		Annotation,
		CheckConstraint,
		// Common database functions
		Concat,
		CurrentDate,
		DatabaseConnection,
		DenseRank,
		// Query expressions (Django-style F/Q objects)
		F,
		ForeignKeyConstraint,
		Lower,
		Now,
		Q,
		QOperator,
		Rank,
		RowNumber,
		SoftDeletable,
		Timestamped,
		// Transaction management
		Transaction,
		// Constraints
		UniqueConstraint,
		Upper,
		// Window functions (commonly used)
		Window,
		atomic,
		// Model attribute macro for struct-level model definition
		model,
	};

	// Import Model trait directly from reinhardt_db to avoid name shadowing
	// (crate::Model refers to the derive macro, not the trait)
	#[cfg(feature = "database")]
	pub use reinhardt_db::orm::Model;

	// Auth feature
	#[cfg(feature = "auth")]
	pub use crate::{
		AuthBackend,
		Group,
		GroupManager,
		// Object-level permissions
		ObjectPermission,
		ObjectPermissionChecker,
		PasswordHasher,
		Permission,
		SimpleUser,
		User,
		// User and group management
		UserManager,
	};

	// OpenAPI feature - schema generation and documentation
	// Note: When 'openapi' feature is enabled, types are available at top level
	// Example: use reinhardt::prelude::*; or use reinhardt::{OpenApi, ApiDoc, Schema};

	// DI params - FastAPI-style parameter extraction
	#[cfg(any(feature = "minimal", feature = "standard", feature = "di"))]
	pub use crate::{Body, Cookie, Header, Json, Path, Query};

	// REST feature - serializers, parsers, pagination, throttling, versioning, metadata
	#[cfg(feature = "rest")]
	pub use crate::{
		// Versioning
		AcceptHeaderVersioning,
		// Throttling
		AnonRateThrottle,
		CursorPagination,
		FormParser,
		// Parsers
		JSONParser,
		JsonSerializer,
		LimitOffsetPagination,
		MultiPartParser,
		// Filters
		MultiTermSearch,
		// Pagination
		PageNumberPagination,
		Paginator,
		Parser,
		QueryParameterVersioning,
		ScopedRateThrottle,
		// Serializers
		Serializer,
		// Metadata
		SimpleMetadata,
		Throttle,
		URLPathVersioning,
		UserRateThrottle,
		VersioningMiddleware,
	};

	// Settings feature
	#[cfg(feature = "conf")]
	pub use crate::Settings;

	// Middleware
	#[cfg(any(feature = "standard", feature = "middleware"))]
	pub use crate::LoggingMiddleware;

	// Sessions feature
	#[cfg(feature = "sessions")]
	pub use crate::{AuthenticationMiddleware, Session};

	// Cache feature
	#[cfg(feature = "cache")]
	pub use crate::{Cache, InMemoryCache};

	// Admin feature - use reinhardt-admin-api crate directly for admin functionality
}

// Re-export database modules for Model derive macro generated code
// These must be available at `::reinhardt::db::*` for the macro to work correctly
#[cfg(all(feature = "database", not(target_arch = "wasm32")))]
pub mod db {
	// Re-export commonly used types at module level for easier access
	pub use reinhardt_db::DatabaseConnection;
	pub use reinhardt_db::DatabaseError as Error;

	// Explicitly re-export modules used by Model derive macro
	pub mod migrations {
		pub use reinhardt_db::migrations::*;
	}

	pub mod orm {
		pub use reinhardt_db::orm::*;
	}

	// Re-export associations module for relationship definitions
	pub mod associations {
		pub use reinhardt_db::associations::*;
	}

	// Re-export prelude for convenience
	pub mod prelude {
		pub use reinhardt_db::prelude::*;
	}
}
