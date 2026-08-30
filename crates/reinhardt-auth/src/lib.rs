#![warn(missing_docs)]
#![cfg_attr(docsrs, feature(doc_cfg))]
//! # Reinhardt Auth
//!
//! Authentication and authorization system for Reinhardt framework.
//!
//! ## Features
//!
//! - **DjangoModelPermissions**: Django-style model permissions with `app_label.action_model` format
//! - **DjangoModelPermissionsOrAnonReadOnly**: Anonymous read access for unauthenticated users
//! - **Object-Level Permissions**: Fine-grained access control on individual objects
//! - **User Management**: CRUD operations for users with password hashing
//! - **Group Management**: User groups and permission assignment
//! - **REST API Authentication**: Multiple authentication backends (JWT, Token, Session, OAuth2, social OAuth state)
//! - **Standard Permissions**: Permission classes for common authorization scenarios
//! - **createsuperuser Command**: CLI tool for creating admin users
//!
//! ## Quick Start
//!
//! ```rust
//! use reinhardt_auth::core::{IsAuthenticated, PermissionContext};
//!
//! // Check if a permission is satisfied
//! let permission = IsAuthenticated;
//! // In actual usage, you would pass a real request context
//! let _ = permission; // permission classes implement PermissionClass trait
//! ```
//!
//! ## Architecture
//!
//! Key modules in this crate:
//!
//! - [`core`]: Authentication traits, user types, permission classes, and password hashing
//! - [`sessions`]: Session backends (JWT, database, Redis, cookie, file)
//! - `social` (feature-gated): OAuth2/OpenID Connect social authentication providers
//! - `user_management`: CRUD operations for users and groups
//!
//! ## Feature Flags
//!
//! | Feature | Default | Description |
//! |---------|---------|-------------|
//! | `params` | enabled | Parameter extraction via DI |
//! | `jwt` | disabled | JWT-based authentication backend |
//! | `sessions` | disabled | Session-based authentication |
//! | `oauth` | disabled | OAuth2 authorization code flow |
//! | `token` | disabled | Token-based authentication |
//! | `argon2-hasher` | disabled | Argon2 password hashing |
//! | `bcrypt-hasher` | disabled | bcrypt password hashing for compatibility |
//! | `social` | disabled | Social authentication (OAuth2/OIDC providers) |
//! | `database` | disabled | Database-backed user/group storage via ORM |
//!
//! ## Security Note: Client-Side vs Server-Side Checks
//!
//! Authentication state exposed via `reinhardt_http::AuthState` (e.g.,
//! `is_authenticated()`, `is_admin()`) is populated by server-side
//! middleware and stored in request extensions. When this state is
//! forwarded to client-side code (e.g., via WASM or JSON responses),
//! **it must only be used for UI display purposes** (showing/hiding
//! elements). All authorization decisions must be enforced server-side
//! through middleware and permission classes provided by this crate.

pub mod sessions;

// Core authentication types and traits (migrated from reinhardt-core-auth)
pub mod core;

// AuthInfo lightweight auth extractor
pub mod auth_info;
pub use auth_info::AuthInfo;

// Guard types for permission-based DI resolution
/// Permission guard types and combinators for DI-based authorization.
pub mod guard;
pub use guard::{All, Any, Guard, Not, Public};

// Re-export guard!() macro from reinhardt-auth-macros
pub use reinhardt_auth_macros::guard;

// Authenticated user extractors
pub mod auth_user;
pub use auth_user::CurrentUser;

// Startup validation for auth extractors
pub mod auth_extractors;
pub use auth_extractors::validate_auth_extractors;

/// Project-specific UUID namespace for deterministic user ID generation.
///
/// Computed from `Uuid::new_v5(&Uuid::NAMESPACE_URL, b"https://reinhardt.rs/user-id")`.
pub(crate) const USER_ID_NAMESPACE: uuid::Uuid =
	uuid::uuid!("c7a85537-073f-5092-8d10-774e109477c9");

pub(crate) mod internal_user;

// Re-export core authentication types. The deprecated `User` trait,
// `SimpleUser`, and `AnonymousUser` (which lived in `core::user`) were
// removed in 0.2.0 per Issue #4520 — use `AuthIdentity` + `BaseUser` /
// `FullUser` + `PermissionsMixin` instead.
pub use core::{
	AllowAny, AuthBackend, AuthIdentity, BaseUser, CompositeAuthBackend, FullUser, IsActiveUser,
	IsAdminUser, IsAuthenticated, IsAuthenticatedOrReadOnly, PasswordCheck, PasswordHashPolicy,
	PasswordHasher, PasswordVerification, Permission, PermissionContext, PermissionsMixin,
	SuperuserCreator, SuperuserCreatorRegistration, SuperuserInit, TypedSuperuserCreator,
	auto_register_superuser_creator, get_superuser_creator, register_superuser_creator,
	superuser_creator_for,
};

#[cfg(feature = "argon2-hasher")]
pub use core::Argon2Hasher;

#[cfg(feature = "bcrypt-hasher")]
pub use core::BcryptHasher;

// Re-export permission operators from core
pub use core::permission_operators;

pub mod repository;
pub use repository::{SimpleUserRepository, UserRepository};

/// Advanced permission classes (role-based, object-level).
pub mod advanced_permissions;
/// Base user manager trait for CRUD operations.
pub mod base_user_manager;
/// HTTP Basic authentication backend.
#[cfg_attr(docsrs, doc(cfg(feature = "basic")))]
#[cfg(feature = "basic")]
pub mod basic;
/// Group management (create, delete, assign users).
pub mod group_management;
/// Login/logout HTTP handlers.
#[cfg(feature = "sessions")]
pub mod handlers;
/// IP-based permission classes (whitelist/blacklist with CIDR).
pub mod ip_permission;
/// JWT (JSON Web Token) authentication.
#[cfg(feature = "jwt")]
pub mod jwt;
/// Multi-factor authentication support.
pub mod mfa;
/// Django-compatible model-level permissions.
pub mod model_permissions;
/// OAuth2 authentication provider.
#[cfg(feature = "oauth")]
pub mod oauth2;
/// Object-level permission checking.
pub mod object_permissions;
/// Database-backed permission model.
#[cfg(feature = "database")]
pub mod permission;
/// Rate-limiting permission class.
#[cfg(feature = "rate-limit")]
pub mod rate_limit_permission;
/// Remote user authentication (proxy-based).
pub mod remote_user;
/// REST API authentication backends.
pub mod rest_authentication;
/// Session-based authentication.
#[cfg(feature = "sessions")]
pub mod session;
/// Social authentication providers (Google, GitHub, Apple, Microsoft).
#[cfg(feature = "social")]
pub mod social;
/// Time-based permission class (time windows, date ranges).
pub mod time_based_permission;
/// Token blacklist for revocation.
#[cfg(any(feature = "jwt", feature = "token"))]
pub mod token_blacklist;
/// Automatic token rotation.
#[cfg(any(feature = "jwt", feature = "token"))]
pub mod token_rotation;
/// Token persistence storage backends.
#[cfg(any(feature = "jwt", feature = "token"))]
pub mod token_storage;
/// User CRUD management.
pub mod user_management;

/// Settings fragments for authentication backends.
pub mod settings;

pub use advanced_permissions::{ObjectPermission as AdvancedObjectPermission, RoleBasedPermission};
pub use base_user_manager::BaseUserManager;
#[cfg_attr(docsrs, doc(cfg(feature = "basic")))]
#[cfg(feature = "basic")]
pub use basic::BasicAuthentication as HttpBasicAuth;
pub use group_management::{
	CreateGroupData, Group, GroupManagementError, GroupManagementResult, GroupManager,
	get_group_manager, register_group_manager,
};
#[cfg(feature = "sessions")]
pub use handlers::{LoginCredentials, LoginHandler, LogoutHandler, SESSION_COOKIE_NAME};
pub use ip_permission::{CidrRange, IpBlacklistPermission, IpWhitelistPermission};
#[cfg(feature = "jwt")]
pub use jwt::{Claims, JwtAuth, JwtError};
pub use mfa::MFAAuthentication as MfaManager;
pub use model_permissions::{
	DjangoModelPermissions, DjangoModelPermissionsOrAnonReadOnly, ModelPermission,
};
#[cfg(feature = "oauth")]
pub use oauth2::{
	AccessToken, AuthorizationCode, GrantType, InMemoryOAuth2Store, OAuth2Application,
	OAuth2Authentication, OAuth2TokenStore,
};
pub use object_permissions::{ObjectPermission, ObjectPermissionChecker, ObjectPermissionManager};
#[cfg(feature = "database")]
pub use permission::AuthPermission;
pub use permission_operators::{AndPermission, NotPermission, OrPermission};
// Re-export the error type used by `BaseUserManager` so downstream code (and the
// `#[user]` macro's auto-generated manager impl) can reference it without
// taking a direct dependency on `reinhardt-core`.
pub use reinhardt_core::exception::Error as BaseUserManagerError;

/// Re-export of [`serde_json::Value`] for use in `BaseUserManager` method
/// signatures emitted by the `#[user(...)]` auto-manager generator.
///
/// Consumers of the auto-generated manager get this re-export "for free" via
/// `reinhardt_auth::JsonValue`, so they do not need to add a direct
/// `serde_json` dependency just to satisfy the trait signature.
pub use serde_json::Value as JsonValue;
#[cfg(feature = "social")]
pub use social::{
	AppleProvider, ContextualCallbackResult, ContextualStateData, GenericOidcConfig,
	GenericOidcProvider, GitHubProvider, GoogleProvider, IdToken, MicrosoftProvider, OAuthProvider,
	OAuthToken, PkceFlow, ProviderConfig, SocialAuthBackend, SocialAuthError, StandardClaims,
	StateStore, TokenResponse, UserInfoMapper,
};

#[cfg(feature = "rate-limit")]
pub use rate_limit_permission::{RateLimitPermission, RateLimitPermissionBuilder};
pub use remote_user::RemoteUserAuthentication as RemoteUserAuth;
pub use rest_authentication::{
	BasicAuthConfig, CompositeAuthentication, RemoteUserAuthentication, RestAuthentication,
	SessionAuthConfig, SessionAuthentication, TokenAuthConfig, TokenAuthentication,
};
#[cfg(feature = "sessions")]
pub use session::{InMemorySessionStore, SESSION_KEY_USER_ID, Session, SessionId, SessionStore};
pub use time_based_permission::{DateRange, TimeBasedPermission, TimeWindow};
#[cfg(any(feature = "jwt", feature = "token"))]
pub use token_blacklist::{
	BlacklistReason, BlacklistStats, BlacklistedToken, InMemoryRefreshTokenStore,
	InMemoryTokenBlacklist, RefreshToken, RefreshTokenStore, TokenBlacklist, TokenRotationManager,
};
#[cfg(any(feature = "jwt", feature = "token"))]
#[allow(deprecated)] // Re-export keeps the compatibility API discoverable during the 0.2 line.
pub use token_rotation::{AutoTokenRotationManager, TokenRotationConfig, TokenRotationRecord};

#[cfg(feature = "sessions")]
pub use settings::SessionSettings;

#[cfg(feature = "jwt")]
pub use settings::{JwtSessionSettings, create_jwt_session_backend_from_settings};

#[cfg(feature = "token")]
pub use settings::{TokenRotationSettings, create_token_rotation_manager_from_settings};
#[cfg(all(feature = "database", any(feature = "jwt", feature = "token")))]
pub use token_storage::DatabaseTokenStorage;
#[cfg(any(feature = "jwt", feature = "token"))]
pub use token_storage::{
	InMemoryTokenStorage, StoredToken, TokenStorage, TokenStorageError, TokenStorageResult,
};
pub use user_management::{
	CreateUserData, ManagedUser, UpdateUserData, UserManagementError, UserManagementResult,
	UserManager,
};

/// Authentication errors that can occur during user verification.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthenticationError {
	/// The provided credentials (username/password) are incorrect.
	InvalidCredentials,
	/// The requested user does not exist.
	UserNotFound,
	/// The user's session has expired.
	SessionExpired,
	/// The provided authentication token is invalid or malformed.
	InvalidToken,
	/// The JWT token has expired.
	TokenExpired,
	/// The request lacks valid authentication credentials.
	NotAuthenticated,
	/// A database error occurred during authentication.
	DatabaseError(String),
	/// An unspecified authentication error.
	Unknown(String),
}

impl std::fmt::Display for AuthenticationError {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			AuthenticationError::InvalidCredentials => write!(f, "Invalid credentials"),
			AuthenticationError::UserNotFound => write!(f, "User not found"),
			AuthenticationError::SessionExpired => write!(f, "Session expired"),
			AuthenticationError::InvalidToken => write!(f, "Invalid token"),
			AuthenticationError::TokenExpired => write!(f, "Token expired"),
			AuthenticationError::NotAuthenticated => write!(f, "User is not authenticated"),
			AuthenticationError::DatabaseError(msg) => write!(f, "Database error: {}", msg),
			AuthenticationError::Unknown(msg) => write!(f, "Authentication error: {}", msg),
		}
	}
}

impl std::error::Error for AuthenticationError {}

#[cfg(feature = "jwt")]
impl From<JwtError> for AuthenticationError {
	fn from(err: JwtError) -> Self {
		match err {
			JwtError::TokenExpired => AuthenticationError::TokenExpired,
			JwtError::InvalidSignature(_) | JwtError::InvalidToken(_) => {
				AuthenticationError::InvalidToken
			}
			JwtError::EncodingError(msg) => AuthenticationError::Unknown(msg),
		}
	}
}

// `AuthenticationBackend` trait was removed in 0.2.0 per Issue #4520.
// Use `AuthBackend` (from `core::auth_backend`) instead. The old trait
// depended on the removed `User` trait; `AuthBackend` works with
// `AuthIdentity` and is the canonical authentication backend trait.

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	#[cfg(feature = "jwt")]
	fn test_auth_jwt_generate_unit() {
		let jwt_auth = JwtAuth::new(b"test_secret_key");
		let user_id = "user123".to_string();
		let username = "testuser".to_string();

		let token = jwt_auth
			.generate_token(user_id, username, false, false)
			.unwrap();

		assert!(!token.is_empty());
	}

	#[tokio::test]
	async fn test_permission_allow_any() {
		use bytes::Bytes;
		use hyper::Method;
		use reinhardt_http::Request;

		let permission = AllowAny;
		let request = Request::builder()
			.method(Method::GET)
			.uri("/test")
			.body(Bytes::new())
			.build()
			.unwrap();

		let context = PermissionContext {
			request: &request,
			is_authenticated: false,
			is_admin: false,
			is_active: false,
			user: None,
		};

		assert!(permission.has_permission(&context).await);
	}

	#[tokio::test]
	async fn test_permission_is_authenticated_with_auth() {
		use bytes::Bytes;
		use hyper::Method;
		use reinhardt_http::Request;

		let permission = IsAuthenticated;
		let request = Request::builder()
			.method(Method::GET)
			.uri("/test")
			.body(Bytes::new())
			.build()
			.unwrap();

		let context = PermissionContext {
			request: &request,
			is_authenticated: true,
			is_admin: false,
			is_active: true,
			user: None,
		};

		assert!(permission.has_permission(&context).await);
	}

	#[tokio::test]
	async fn test_permission_is_authenticated_without_auth() {
		use bytes::Bytes;
		use hyper::Method;
		use reinhardt_http::Request;

		let permission = IsAuthenticated;
		let request = Request::builder()
			.method(Method::GET)
			.uri("/test")
			.body(Bytes::new())
			.build()
			.unwrap();

		let context = PermissionContext {
			request: &request,
			is_authenticated: false,
			is_admin: false,
			is_active: false,
			user: None,
		};

		assert!(!permission.has_permission(&context).await);
	}

	#[tokio::test]
	async fn test_permission_is_admin_user() {
		use bytes::Bytes;
		use hyper::Method;
		use reinhardt_http::Request;

		let permission = IsAdminUser;
		let request = Request::builder()
			.method(Method::GET)
			.uri("/test")
			.body(Bytes::new())
			.build()
			.unwrap();

		// Admin user
		let context = PermissionContext {
			request: &request,
			is_authenticated: true,
			is_admin: true,
			is_active: true,
			user: None,
		};
		assert!(permission.has_permission(&context).await);

		// Non-admin user
		let context = PermissionContext {
			request: &request,
			is_authenticated: true,
			is_admin: false,
			is_active: true,
			user: None,
		};
		assert!(!permission.has_permission(&context).await);
	}

	#[tokio::test]
	async fn test_permission_is_active_user() {
		use bytes::Bytes;
		use hyper::Method;
		use reinhardt_http::Request;

		let permission = IsActiveUser;
		let request = Request::builder()
			.method(Method::GET)
			.uri("/test")
			.body(Bytes::new())
			.build()
			.unwrap();

		// Active user
		let context = PermissionContext {
			request: &request,
			is_authenticated: true,
			is_admin: false,
			is_active: true,
			user: None,
		};
		assert!(permission.has_permission(&context).await);

		// Inactive user
		let context = PermissionContext {
			request: &request,
			is_authenticated: true,
			is_admin: false,
			is_active: false,
			user: None,
		};
		assert!(!permission.has_permission(&context).await);
	}

	#[tokio::test]
	async fn test_permission_is_authenticated_or_read_only_get() {
		use bytes::Bytes;
		use hyper::Method;
		use reinhardt_http::Request;

		let permission = IsAuthenticatedOrReadOnly;
		let request = Request::builder()
			.method(Method::GET)
			.uri("/test")
			.body(Bytes::new())
			.build()
			.unwrap();

		// Unauthenticated GET should be allowed
		let context = PermissionContext {
			request: &request,
			is_authenticated: false,
			is_admin: false,
			is_active: false,
			user: None,
		};
		assert!(permission.has_permission(&context).await);
	}

	#[tokio::test]
	async fn test_permission_is_authenticated_or_read_only_post() {
		use bytes::Bytes;
		use hyper::Method;
		use reinhardt_http::Request;

		let permission = IsAuthenticatedOrReadOnly;
		let request = Request::builder()
			.method(Method::POST)
			.uri("/test")
			.body(Bytes::new())
			.build()
			.unwrap();

		// Unauthenticated POST should be denied
		let context = PermissionContext {
			request: &request,
			is_authenticated: false,
			is_admin: false,
			is_active: false,
			user: None,
		};
		assert!(!permission.has_permission(&context).await);

		// Authenticated POST should be allowed
		let context = PermissionContext {
			request: &request,
			is_authenticated: true,
			is_admin: false,
			is_active: true,
			user: None,
		};
		assert!(permission.has_permission(&context).await);
	}
}
