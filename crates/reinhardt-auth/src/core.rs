//! # reinhardt-core-auth
//!
//! Core authentication types and traits for the Reinhardt framework.
//!
//! This crate provides the fundamental authentication abstractions used throughout
//! Reinhardt. It includes:
//!
//! - **User traits**: `User`, `BaseUser`, `FullUser` for representing authenticated users
//! - **Permission system**: `Permission` trait and common permission classes
//! - **Authentication backends**: `AuthBackend` trait for custom authentication
//! - **Password hashing**: `PasswordHasher` trait and Argon2 implementation
//!
//! ## Features
//!
//! - `argon2-hasher` (default): Enables Argon2id password hashing
//!
//! ## Examples
//!
//! ### Basic User Implementation
//!
//! ```
//! use reinhardt_auth::core::{User, SimpleUser};
//! use uuid::Uuid;
//!
//! let user = SimpleUser {
//!     id: Uuid::now_v7(),
//!     username: "alice".to_string(),
//!     email: "alice@example.com".to_string(),
//!     is_active: true,
//!     is_admin: false,
//!     is_staff: false,
//!     is_superuser: false,
//! };
//!
//! assert!(user.is_authenticated());
//! assert_eq!(user.username(), "alice");
//! ```
//!
//! ### Custom User with Password Hashing
//!
//! ```
//! use reinhardt_auth::core::{BaseUser, PasswordHasher};
//! #[cfg(feature = "argon2-hasher")]
//! use reinhardt_auth::core::Argon2Hasher;
//! use uuid::Uuid;
//! use chrono::{DateTime, Utc};
//! use serde::{Serialize, Deserialize};
//!
//! #[derive(Serialize, Deserialize)]
//! struct MyUser {
//!     id: Uuid,
//!     email: String,
//!     password_hash: Option<String>,
//!     last_login: Option<DateTime<Utc>>,
//!     is_active: bool,
//! }
//!
//! #[cfg(feature = "argon2-hasher")]
//! impl BaseUser for MyUser {
//!     type PrimaryKey = Uuid;
//!     type Hasher = Argon2Hasher;
//!
//!     fn get_username_field() -> &'static str { "email" }
//!     fn get_username(&self) -> &str { &self.email }
//!     fn password_hash(&self) -> Option<&str> { self.password_hash.as_deref() }
//!     fn set_password_hash(&mut self, hash: String) { self.password_hash = Some(hash); }
//!     fn last_login(&self) -> Option<DateTime<Utc>> { self.last_login }
//!     fn set_last_login(&mut self, time: DateTime<Utc>) { self.last_login = Some(time); }
//!     fn is_active(&self) -> bool { self.is_active }
//! }
//!
//! # #[cfg(feature = "argon2-hasher")]
//! # {
//! let mut user = MyUser {
//!     id: Uuid::now_v7(),
//!     email: "user@example.com".to_string(),
//!     password_hash: None,
//!     last_login: None,
//!     is_active: true,
//! };
//!
//! user.set_password("secure_password123").unwrap();
//! assert!(user.check_password("secure_password123").unwrap());
//! # }
//! ```
//!
//! ### Permission Checking
//!
//! ```
//! use reinhardt_auth::core::{Permission, IsAuthenticated, PermissionContext};
//! use reinhardt_http::Request;
//! use hyper::{Method, Uri, Version, header::HeaderMap};
//! use bytes::Bytes;
//!
//! # tokio::runtime::Runtime::new().unwrap().block_on(async {
//! let permission = IsAuthenticated;
//! let request = Request::builder()
//!     .method(Method::GET)
//!     .uri("/")
//!     .version(Version::HTTP_11)
//!     .headers(HeaderMap::new())
//!     .body(Bytes::new())
//!     .build()
//!     .unwrap();
//!
//! let context = PermissionContext {
//!     request: &request,
//!     is_authenticated: true,
//!     is_admin: false,
//!     is_active: true,
//!     user: None,
//! };
//!
//! assert!(permission.has_permission(&context).await);
//! # });
//! ```

// Re-exports the deprecated User trait for backward compatibility.
#![allow(deprecated)]
// Rust 2024 module system: use module.rs + module/ pattern
/// Authentication identity trait (replaces deprecated User trait).
#[path = "core/auth_identity.rs"]
pub mod auth_identity;
/// Authentication backend trait and composite backend.
#[path = "core/backend.rs"]
pub mod backend;
/// Base user trait defining minimal user identity.
#[path = "core/base_user.rs"]
pub mod base_user;
/// Full user trait with extended profile fields.
#[path = "core/full_user.rs"]
pub mod full_user;
/// Password hashing interface and implementations.
#[path = "core/hasher.rs"]
pub mod hasher;
/// Permission trait and built-in permission classes.
#[path = "core/permission.rs"]
pub mod permission;
/// Permission combinators (AND, OR, NOT).
#[path = "core/permission_operators.rs"]
pub mod permission_operators;
/// Mixin for adding permission fields to user models.
#[path = "core/permissions_mixin.rs"]
pub mod permissions_mixin;
/// Superuser creation support for management commands.
#[path = "core/superuser_creator.rs"]
pub mod superuser_creator;
/// Core user types (SimpleUser, AnonymousUser).
#[path = "core/user.rs"]
pub mod user;

// Re-export main types
pub use auth_identity::AuthIdentity;
pub use backend::{AuthBackend, CompositeAuthBackend};
pub use base_user::BaseUser;
pub use full_user::FullUser;
pub use hasher::PasswordHasher;
pub use permission::{
	AllowAny, IsActiveUser, IsAdminUser, IsAuthenticated, IsAuthenticatedOrReadOnly, Permission,
	PermissionContext,
};
pub use permissions_mixin::PermissionsMixin;
pub use superuser_creator::{
	SuperuserCreator, SuperuserCreatorRegistration, SuperuserInit, TypedSuperuserCreator,
	auto_register_superuser_creator, get_superuser_creator, register_superuser_creator,
	superuser_creator_for,
};
pub use user::{AnonymousUser, SimpleUser, User};

// Re-export Argon2Hasher when feature is enabled
#[cfg(feature = "argon2-hasher")]
pub use hasher::Argon2Hasher;
