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
//! - `full` - All features enabled (opt-in for the broadest surface area)
//! - `standard` (default) - Balanced for most projects
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
//! - `auth-social` - Social authentication providers
//! - `social-auth` - Browser-bound social authentication state with session middleware
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
//! See [Cargo.toml feature definitions](https://github.com/kent8192/reinhardt/blob/main/Cargo.toml) for detailed documentation.
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

#![warn(missing_docs)]
#![cfg_attr(docsrs, feature(doc_cfg))]

// ============================================================================
// Macro-support modules (D1: must stay at crate root for path stability)
//
// Macro-generated code uses paths like `::reinhardt::reinhardt_apps::AppConfig`.
// These wrapper modules provide the namespace structure that macros expect.
// Marked #[doc(hidden)] — users should not use these directly.
// ============================================================================

#[cfg(feature = "pages")]
#[doc(hidden)]
pub mod reinhardt_pages {
	pub use reinhardt_pages::*;
}

#[doc(hidden)]
pub mod reinhardt_types {
	#[allow(unused_imports, unreachable_pub)]
	pub use reinhardt_core::types::*;
}

#[cfg(all(feature = "core", native))]
#[doc(hidden)]
pub mod reinhardt_apps {
	pub use reinhardt_apps::*;
}

// WASM shim for `reinhardt_apps` (Issue #4161).
//
// `#[app_config(...)]` expands to code that
// references `::reinhardt::reinhardt_apps::apps::AppLabel` and
// `::reinhardt::reinhardt_apps::AppConfig`. The real `reinhardt-apps`
// crate depends on `tokio` / `reinhardt-server` and is decidedly
// native-only, so on wasm we expose only the surface the macro emits.
//
// These shims compile but never execute: the dashboard-style SPA
// imports them transitively, but only constructs `UnifiedRouter` /
// `WebSocketRouter`, which are themselves wasm-side stubs (see below).
#[cfg(not(native))]
#[doc(hidden)]
pub mod reinhardt_apps {
	/// Application label trait (wasm shim).
	///
	/// Mirrors the trait emitted by `installed_apps!`. The native build
	/// re-exports the real trait from `reinhardt-apps`.
	pub mod apps {
		pub trait AppLabel {
			const LABEL: &'static str;
			fn path(&self) -> &'static str {
				Self::LABEL
			}
		}
	}

	/// Application configuration (wasm shim).
	///
	/// `#[app_config(name = "...", label = "...")]` expands to
	/// `pub fn config() -> AppConfig { AppConfig::new(name, label).with_verbose_name(...) }`.
	/// On wasm we provide a builder-shaped stub with the same signatures so
	/// the expansion compiles. None of these methods are intended to be
	/// invoked at runtime in a wasm consumer.
	pub struct AppConfig {
		_private: (),
	}

	impl AppConfig {
		pub fn new(_name: impl Into<String>, _label: impl Into<String>) -> Self {
			Self { _private: () }
		}

		pub fn with_verbose_name(self, _verbose_name: impl Into<String>) -> Self {
			self
		}
	}
}

pub use reinhardt_core::model_info;

#[cfg(all(feature = "di", native))]
#[doc(hidden)]
pub mod reinhardt_di {
	pub use reinhardt_di::*;
}

#[cfg(all(feature = "auth", native))]
/// Authentication and authorization APIs re-exported by the facade crate.
pub mod auth {
	pub use reinhardt_auth::*;
}

#[cfg(all(feature = "auth", native))]
#[doc(hidden)]
pub mod reinhardt_auth {
	pub use reinhardt_auth::*;
}

#[cfg(all(feature = "commands", native))]
#[doc(hidden)]
pub mod reinhardt_commands {
	pub use reinhardt_commands::*;
}

#[cfg(native)]
#[doc(hidden)]
pub mod reinhardt_core {
	pub use reinhardt_core::endpoint::EndpointMetadata;
	pub use reinhardt_core::*;
}

#[cfg(all(feature = "core", native))]
#[doc(hidden)]
pub mod reinhardt_http {
	pub use reinhardt_http::*;
}

#[cfg(all(feature = "di", native))]
#[doc(hidden)]
pub mod reinhardt_params {
	pub use reinhardt_di::params::*;
}

#[cfg(native)]
#[doc(hidden)]
pub mod async_trait {
	pub use async_trait::*;
}

#[cfg(all(feature = "database", native))]
#[doc(hidden)]
pub mod linkme {
	pub use linkme::*;
}

#[cfg(all(feature = "database", native))]
#[doc(hidden)]
pub mod ctor {
	pub use ctor::*;
}

#[cfg(native)]
#[doc(hidden)]
pub use paste::paste;

#[cfg(all(feature = "database", native))]
#[doc(hidden)]
pub mod reinhardt_orm {
	pub use reinhardt_db::orm::*;
}

// ============================================================================
// Module declarations (D2: define the crate's module tree)
// ============================================================================

#[cfg(feature = "pages")]
pub mod pages;

#[cfg(all(feature = "admin", native))]
pub mod admin;
#[cfg(all(feature = "core", native))]
pub mod apps;
#[cfg(all(feature = "commands", native))]
pub mod commands;
#[cfg(all(feature = "conf", native))]
pub mod conf;
#[cfg(all(feature = "core", native))]
pub mod core;
#[cfg(all(feature = "deeplink", native))]
pub mod deeplink;
#[cfg(all(feature = "dentdelion", native))]
pub mod dentdelion;
#[cfg(all(feature = "di", native))]
pub mod di;
#[cfg(all(feature = "dispatch", native))]
pub mod dispatch;
#[cfg(all(feature = "forms", native))]
pub mod forms;
#[cfg(all(feature = "graphql", native))]
pub mod graphql;
#[cfg(all(feature = "grpc", native))]
pub mod grpc;
#[cfg(all(feature = "core", native))]
pub mod http;
#[cfg(all(feature = "i18n", native))]
pub mod i18n;
#[cfg(all(feature = "mail", native))]
pub mod mail;
#[cfg(all(any(feature = "standard", feature = "middleware"), native))]
pub mod middleware;
#[cfg(all(feature = "rest", native))]
pub mod rest;
#[cfg(all(feature = "server", native))]
pub mod server;
#[cfg(all(feature = "shortcuts", native))]
pub mod shortcuts;
#[cfg(feature = "streaming")]
pub mod streaming;
#[cfg(all(feature = "tasks", native))]
pub mod tasks;
#[cfg(all(feature = "templates", native))]
pub mod template;
#[cfg(feature = "test")]
pub mod test;
#[cfg(all(feature = "routing", native))]
pub mod urls;

/// WASM shim for the `urls` module (Issue #4161).
///
/// Provides the namespace structure that downstream wasm SPAs reference
/// (`reinhardt::urls::prelude::UnifiedRouter`,
/// `reinhardt::urls::proxy`). The real `reinhardt-urls` crate is wasm-safe,
/// but its `prelude` is gated `#[cfg(all(feature = "routers", native))]`.
///
/// When the `client-router` feature is enabled (the realistic configuration
/// for wasm consumers that use `#[routes]`), this re-exports the real
/// wasm-side `UnifiedRouter` from `reinhardt_urls::routers`. That type
/// provides the correct closure signatures
/// (`server: FnOnce(ServerRouter) -> ServerRouter`,
/// `client: FnOnce(ClientRouter) -> ClientRouter`) so user-supplied bodies
/// such as `.client(|c| c.route(...))` type-check on wasm. On wasm the
/// `ServerRouter` is a no-op builder whose result is discarded (issue #4569).
///
/// Without `client-router`, an inert stub is exposed so that the path
/// resolves; user bodies that invoke `.server`/`.client` on the stub are
/// expected to be no-ops in that minimal configuration.
#[cfg(all(feature = "routing", not(native)))]
pub mod urls {
	/// Wasm-side stub mirroring `reinhardt_urls::prelude`.
	pub mod prelude {
		#[cfg(feature = "client-router")]
		pub use reinhardt_urls::routers::{ClientRouter, ServerRouter, UnifiedRouter};

		#[cfg(not(feature = "client-router"))]
		pub use stub::*;

		#[cfg(not(feature = "client-router"))]
		mod stub {
			/// Empty stand-in for `reinhardt_urls::routers::ServerRouter`.
			pub struct ServerRouter;
			/// Empty stand-in for `reinhardt_urls::routers::client_router::ClientRouter`.
			pub struct ClientRouter;

			/// WASM-only no-op stand-in for `reinhardt_urls::routers::UnifiedRouter`.
			pub struct UnifiedRouter {
				_private: (),
			}

			impl UnifiedRouter {
				/// Creates an empty no-op router builder.
				pub fn new() -> Self {
					Self { _private: () }
				}

				/// Accepts a route namespace and returns the unchanged no-op router.
				pub fn with_namespace(self, _namespace: impl Into<String>) -> Self {
					self
				}

				/// Accepts a server router closure and discards its no-op result.
				pub fn server<F>(self, _f: F) -> Self
				where
					F: FnOnce(ServerRouter) -> ServerRouter,
				{
					self
				}

				/// Accepts a client router closure and discards its no-op result.
				pub fn client<F>(self, _f: F) -> Self
				where
					F: FnOnce(ClientRouter) -> ClientRouter,
				{
					self
				}
			}

			impl Default for UnifiedRouter {
				fn default() -> Self {
					Self::new()
				}
			}
		}
	}

	/// Wasm-side stub for the `proxy` submodule referenced by
	/// `crate_paths::get_reinhardt_proxy_crate()`. Empty on wasm.
	pub mod proxy {}
}

#[cfg(all(
	any(feature = "cache", feature = "static-files", feature = "storage"),
	native
))]
pub mod utils;
#[cfg(all(
	any(feature = "api", feature = "standard", feature = "api-only"),
	native
))]
pub mod views;

// ============================================================================
// Organized re-exports (extracted from the former monolithic lib.rs)
// ============================================================================

mod exports;
pub use exports::*;

// ============================================================================
// Additional macro-support re-exports (D1: must stay at crate root)
// ============================================================================

#[cfg(all(feature = "database", native))]
pub use reinhardt_db::migrations;

#[cfg(all(feature = "database", native))]
#[doc(hidden)]
pub use migrations as reinhardt_migrations;

#[doc(hidden)]
pub mod macros {
	pub use reinhardt_macros::*;
}

#[cfg(all(feature = "core", native))]
#[doc(hidden)]
pub use inventory;

#[cfg(all(feature = "routing", target_family = "wasm", target_os = "unknown"))]
#[doc(hidden)]
pub use reinhardt_urls::inventory;

#[cfg(feature = "routing")]
#[doc(hidden)]
pub use ::reinhardt_urls;

// ============================================================================
// Prelude
// ============================================================================

pub mod prelude;

// ============================================================================
// WASM compatibility shims
// ============================================================================

#[cfg(not(native))]
mod compat;
#[cfg(not(native))]
pub use compat::websockets::WebSocketRouter;

// ============================================================================
// Database modules (D2: macro-emitted paths reference `::reinhardt::db::*`)
// ============================================================================

/// SQL query builder module.
///
/// Re-exports [`reinhardt_query`] for building type-safe SQL queries.
/// Requires `database` feature.
#[cfg(all(feature = "database", native))]
pub mod query;

/// Database re-exports for Model derive macro generated code.
///
/// These must be available at `::reinhardt::db::*` for the macro to work correctly.
#[cfg(all(feature = "database", native))]
pub mod db {
	pub use reinhardt_db::DatabaseConnection;
	pub use reinhardt_db::DatabaseError as Error;

	/// Database migration types and utilities.
	pub mod migrations {
		pub use reinhardt_db::migrations::*;
	}

	/// ORM query building and model operations.
	pub mod orm {
		pub use reinhardt_db::orm::*;
	}

	/// Model relationship (association) definitions.
	pub mod associations {
		pub use reinhardt_db::associations::*;
	}

	/// Convenience re-exports for database operations.
	pub mod prelude {
		pub use reinhardt_db::prelude::*;
	}
}

/// WASM-compatible database marker namespace.
///
/// This is intentionally limited to marker field types required by `#[model]`
/// definitions that only expose generated `{Model}Info` DTOs on WASM. It does
/// not expose ORM, connection, query, or migration APIs.
#[cfg(not(native))]
pub mod db {
	/// Relationship marker types accepted by `#[model]` on WASM.
	pub mod associations {
		use std::marker::PhantomData;

		/// Marker type for foreign-key relationship fields.
		#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
		pub struct ForeignKeyField<T>(PhantomData<T>);

		impl<T> Default for ForeignKeyField<T> {
			fn default() -> Self {
				Self(PhantomData)
			}
		}

		impl<T> ForeignKeyField<T> {
			/// Creates a new foreign-key marker.
			pub const fn new() -> Self {
				Self(PhantomData)
			}
		}

		/// Marker type for many-to-many relationship fields.
		#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
		pub struct ManyToManyField<Source, Target, S = ()>(PhantomData<(Source, Target, S)>);

		impl<Source, Target, S> Default for ManyToManyField<Source, Target, S> {
			fn default() -> Self {
				Self(PhantomData)
			}
		}

		impl<Source, Target, S> ManyToManyField<Source, Target, S> {
			/// Creates a new many-to-many marker.
			pub const fn new() -> Self {
				Self(PhantomData)
			}
		}

		/// Marker type for one-to-one relationship fields.
		#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
		pub struct OneToOneField<T>(PhantomData<T>);

		impl<T> Default for OneToOneField<T> {
			fn default() -> Self {
				Self(PhantomData)
			}
		}

		impl<T> OneToOneField<T> {
			/// Creates a new one-to-one marker.
			pub const fn new() -> Self {
				Self(PhantomData)
			}
		}
	}
}
