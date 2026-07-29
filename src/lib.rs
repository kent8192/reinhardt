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
//! ## Generated Model Form Facade
//!
//! Enable the `forms` feature to use generated model-backed forms through the
//! facade and its prelude exports:
//!
//! ```rust,no_run
//! # #[cfg(feature = "forms")]
//! # mod generated_model_form_facade {
//! use reinhardt::forms::{
//!     FormModel,
//!     ModelForm,
//!     ModelFormError,
//! };
//! use reinhardt::core::model_form::{
//!     ModelFormPolicy,
//!     ModelFormSchema,
//! };
//!
//! fn accepts_generated_model_form<T, P>()
//! where
//!     T: FormModel,
//!     P: ModelFormPolicy,
//!     T::Schema: ModelFormSchema<Model = T>,
//! {
//!     let _ = std::any::type_name::<ModelForm<T, P>>();
//!     let _ = std::any::type_name::<ModelFormError>();
//! }
//! # }
//! # fn main() {}
//! ```
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
//! - `auth-token` - Token authentication
//!
//! #### Database Backends ✅
//! - `db-postgres` - PostgreSQL support
//! - `db-pgvector` - pgvector support, including serializable vector markers for shared WASM models
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

#[cfg(not(native))]
#[doc(hidden)]
pub mod reinhardt_core {
	pub use reinhardt_core::model_form;
	pub use reinhardt_core::model_info;
	#[cfg(feature = "core")]
	pub use reinhardt_core::validators;
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
	pub use reinhardt_db::Json;

	/// Validated pgvector value types.
	#[cfg(feature = "db-pgvector")]
	pub mod pgvector {
		pub use reinhardt_db::orm::{MAX_DENSE_VECTOR_DIMENSIONS, Vector, VectorError};
	}

	/// Low-level backend connections used to register ORM connection leases.
	pub mod backends {
		pub use reinhardt_db::backends::*;
	}

	/// Canonical many-to-many default naming helpers used by generated models.
	pub mod m2m_naming {
		pub use reinhardt_db::m2m_naming::*;
	}

	/// Database migration types and utilities.
	pub mod migrations {
		pub use reinhardt_db::migrations::*;
	}

	/// ORM query building and model operations.
	pub mod orm {
		pub use reinhardt_db::orm::*;

		/// Compatibility path for model-macro generated executor bounds.
		pub mod connection {
			pub use reinhardt_db::orm::OrmExecutor;
			pub use reinhardt_db::orm::connection::*;
		}
	}

	/// Model relationship (association) definitions.
	pub mod associations {
		pub use reinhardt_db::associations::*;
	}

	/// Convenience re-exports for database operations.
	pub mod prelude {
		pub use reinhardt_db::prelude::*;
	}

	#[cfg(test)]
	mod tests {
		#[cfg(feature = "db-pgvector")]
		#[test]
		fn pgvector_types_are_available_through_the_facade() {
			let vector = super::pgvector::Vector::<2>::try_from(vec![1.0, 2.0]).unwrap();

			assert_eq!(vector.as_slice(), &[1.0, 2.0]);
		}

		#[test]
		fn m2m_naming_helpers_are_available_through_facade() {
			assert_eq!(
				super::m2m_naming::default_through_table("posts", "tags"),
				"posts_tags"
			);
		}
	}
}

/// WASM-compatible database marker namespace.
///
/// This is intentionally limited to marker field types required by `#[model]`
/// definitions that only expose generated `{Model}Info` DTOs on WASM. It does
/// not expose ORM, connection, query, or migration APIs.
#[cfg(not(native))]
pub mod db {
	use serde::{Deserialize, Deserializer, Serialize, Serializer, de::DeserializeOwned};
	use std::ops::{Deref, DerefMut};

	/// WASM-compatible pgvector marker types.
	#[cfg(feature = "db-pgvector")]
	pub mod pgvector {
		use serde::{Deserialize, Serialize};

		/// Largest dense-vector dimension accepted by PostgreSQL pgvector.
		pub const MAX_DENSE_VECTOR_DIMENSIONS: usize = 2000;

		/// Dimension or element validation error for a dense vector marker.
		#[derive(Debug, Clone, PartialEq, Eq)]
		pub enum VectorError {
			/// The const-generic dimension is outside pgvector's supported range.
			InvalidDimensions {
				/// Requested dimension.
				dimensions: usize,
			},
			/// The supplied value length does not match the declared dimension.
			DimensionMismatch {
				/// Declared dimension.
				expected: usize,
				/// Supplied value length.
				actual: usize,
			},
			/// A vector element is NaN or infinite.
			NonFiniteElement {
				/// Invalid element position.
				index: usize,
			},
		}

		impl std::fmt::Display for VectorError {
			fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
				match self {
					Self::InvalidDimensions { dimensions } => write!(
						formatter,
						"vector dimensions must be between 1 and {MAX_DENSE_VECTOR_DIMENSIONS}, got {dimensions}"
					),
					Self::DimensionMismatch { expected, actual } => write!(
						formatter,
						"vector dimension mismatch: expected {expected}, got {actual}"
					),
					Self::NonFiniteElement { index } => {
						write!(formatter, "vector element at index {index} must be finite")
					}
				}
			}
		}

		impl std::error::Error for VectorError {}

		/// Serializable dense-vector marker used by shared WASM model definitions.
		#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
		pub struct Vector<const N: usize>(Vec<f32>);

		impl<const N: usize> Vector<N> {
			/// Returns the vector elements.
			pub fn as_slice(&self) -> &[f32] {
				&self.0
			}

			/// Consumes the marker and returns its elements.
			pub fn into_vec(self) -> Vec<f32> {
				self.0
			}
		}

		impl<const N: usize> TryFrom<Vec<f32>> for Vector<N> {
			type Error = VectorError;

			fn try_from(values: Vec<f32>) -> Result<Self, Self::Error> {
				if !(1..=MAX_DENSE_VECTOR_DIMENSIONS).contains(&N) {
					return Err(VectorError::InvalidDimensions { dimensions: N });
				}
				if values.len() != N {
					return Err(VectorError::DimensionMismatch {
						expected: N,
						actual: values.len(),
					});
				}
				if let Some(index) = values.iter().position(|value| !value.is_finite()) {
					return Err(VectorError::NonFiniteElement { index });
				}
				Ok(Self(values))
			}
		}
	}

	/// WASM-compatible typed JSON field wrapper.
	#[repr(transparent)]
	#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
	pub struct Json<T>(pub T);

	impl<T> Json<T> {
		/// Creates a typed JSON wrapper.
		pub const fn new(value: T) -> Self {
			Self(value)
		}

		/// Returns the wrapped value.
		pub fn into_inner(self) -> T {
			self.0
		}

		/// Borrows the wrapped value.
		pub const fn as_inner(&self) -> &T {
			&self.0
		}

		/// Mutably borrows the wrapped value.
		pub fn as_inner_mut(&mut self) -> &mut T {
			&mut self.0
		}

		/// Converts the wrapped value into a JSON value.
		pub fn to_json_value(&self) -> Result<serde_json::Value, serde_json::Error>
		where
			T: Serialize,
		{
			serde_json::to_value(&self.0)
		}

		/// Builds a typed JSON wrapper from a JSON value.
		pub fn from_json_value(value: serde_json::Value) -> Result<Self, serde_json::Error>
		where
			T: DeserializeOwned,
		{
			serde_json::from_value(value).map(Self)
		}
	}

	impl<T> Deref for Json<T> {
		type Target = T;

		fn deref(&self) -> &Self::Target {
			&self.0
		}
	}

	impl<T> DerefMut for Json<T> {
		fn deref_mut(&mut self) -> &mut Self::Target {
			&mut self.0
		}
	}

	impl<T> From<T> for Json<T> {
		fn from(value: T) -> Self {
			Self(value)
		}
	}

	impl<T> Serialize for Json<T>
	where
		T: Serialize,
	{
		fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
		where
			S: Serializer,
		{
			self.0.serialize(serializer)
		}
	}

	impl<'de, T> Deserialize<'de> for Json<T>
	where
		T: Deserialize<'de>,
	{
		fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
		where
			D: Deserializer<'de>,
		{
			T::deserialize(deserializer).map(Self)
		}
	}

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
