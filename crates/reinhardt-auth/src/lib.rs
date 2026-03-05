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
//! - **REST API Authentication**: Multiple authentication backends (JWT, Token, Session, OAuth2)
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
//! - [`current_user`]: Dependency-injectable `CurrentUser` extractor
//! - `social` (feature-gated): OAuth2/OpenID Connect social authentication providers
//! - `user_management`: CRUD operations for users and groups
//!
//! ## Feature Flags
//!
//! | Feature | Default | Description |
//! |---------|---------|-------------|
//! | `params` | enabled | `CurrentUser` parameter extraction via DI |
//! | `jwt` | disabled | JWT-based authentication backend |
//! | `sessions` | disabled | Session-based authentication |
//! | `oauth` | disabled | OAuth2 authorization code flow |
//! | `token` | disabled | Token-based authentication |
//! | `argon2-hasher` | disabled | Argon2 password hashing (alternative to bcrypt) |
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

// CurrentUser injectable for dependency injection
pub mod current_user;
pub use current_user::CurrentUser;

// Re-export core authentication types
pub use core::{
	AllowAny, AnonymousUser, AuthBackend, BaseUser, CompositeAuthBackend, FullUser, IsActiveUser,
	IsAdminUser, IsAuthenticated, IsAuthenticatedOrReadOnly, PasswordHasher, Permission,
	PermissionContext, PermissionsMixin, SimpleUser, User,
};

#[cfg(feature = "argon2-hasher")]
pub use core::Argon2Hasher;

// Re-export permission operators from core
pub use core::permission_operators;

// Implementation-specific modules (kept in contrib)
pub mod advanced_permissions;
pub mod base_user_manager;
pub mod basic;
pub mod default_user;
pub mod default_user_manager;
pub mod group_management;
#[cfg(feature = "sessions")]
pub mod handlers;
pub mod ip_permission;
#[cfg(feature = "jwt")]
pub mod jwt;
pub mod mfa;
pub mod model_permissions;
#[cfg(feature = "oauth")]
pub mod oauth2;
pub mod object_permissions;
#[cfg(feature = "rate-limit")]
pub mod rate_limit_permission;
pub mod remote_user;
pub mod rest_authentication;
#[cfg(feature = "sessions")]
pub mod session;
#[cfg(feature = "social")]
pub mod social;
pub mod time_based_permission;
#[cfg(any(feature = "jwt", feature = "token"))]
pub mod token_blacklist;
#[cfg(any(feature = "jwt", feature = "token"))]
pub mod token_rotation;
#[cfg(any(feature = "jwt", feature = "token"))]
pub mod token_storage;
pub mod user_management;

pub use advanced_permissions::{ObjectPermission as AdvancedObjectPermission, RoleBasedPermission};
pub use base_user_manager::BaseUserManager;
pub use basic::BasicAuthentication as HttpBasicAuth;
#[cfg(feature = "argon2-hasher")]
pub use default_user::DefaultUser;
#[cfg(feature = "argon2-hasher")]
pub use default_user_manager::DefaultUserManager;
pub use group_management::{
	CreateGroupData, Group, GroupManagementError, GroupManagementResult, GroupManager,
};
#[cfg(feature = "sessions")]
pub use handlers::{LoginCredentials, LoginHandler, LogoutHandler, SESSION_COOKIE_NAME};
pub use ip_permission::{CidrRange, IpBlacklistPermission, IpWhitelistPermission};
#[cfg(feature = "jwt")]
pub use jwt::{Claims, JwtAuth};
pub use mfa::MFAAuthentication as MfaManager;
pub use model_permissions::{
	DjangoModelPermissions, DjangoModelPermissionsOrAnonReadOnly, ModelPermission,
};
#[cfg(feature = "oauth")]
pub use oauth2::{
	AccessToken, AuthorizationCode, GrantType, InMemoryOAuth2Store, OAuth2Application,
	OAuth2Authentication, OAuth2TokenStore, SimpleUserRepository, UserRepository,
};
pub use object_permissions::{ObjectPermission, ObjectPermissionChecker, ObjectPermissionManager};
pub use permission_operators::{AndPermission, NotPermission, OrPermission};
#[cfg(feature = "social")]
pub use social::{
	AppleProvider, GitHubProvider, GoogleProvider, IdToken, MicrosoftProvider, OAuthProvider,
	OAuthToken, PkceFlow, ProviderConfig, SocialAuthBackend, SocialAuthError, StandardClaims,
	StateStore, TokenResponse,
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
pub use token_rotation::{AutoTokenRotationManager, TokenRotationConfig, TokenRotationRecord};
#[cfg(feature = "database")]
pub use token_storage::DatabaseTokenStorage;
#[cfg(any(feature = "jwt", feature = "token"))]
pub use token_storage::{
	InMemoryTokenStorage, StoredToken, TokenStorage, TokenStorageError, TokenStorageResult,
};
pub use user_management::{
	CreateUserData, UpdateUserData, UserManagementError, UserManagementResult, UserManager,
};

/// Authentication errors
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthenticationError {
	InvalidCredentials,
	UserNotFound,
	SessionExpired,
	InvalidToken,
	NotAuthenticated,
	DatabaseError(String),
	Unknown(String),
}

impl std::fmt::Display for AuthenticationError {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			AuthenticationError::InvalidCredentials => write!(f, "Invalid credentials"),
			AuthenticationError::UserNotFound => write!(f, "User not found"),
			AuthenticationError::SessionExpired => write!(f, "Session expired"),
			AuthenticationError::InvalidToken => write!(f, "Invalid token"),
			AuthenticationError::NotAuthenticated => write!(f, "User is not authenticated"),
			AuthenticationError::DatabaseError(msg) => write!(f, "Database error: {}", msg),
			AuthenticationError::Unknown(msg) => write!(f, "Authentication error: {}", msg),
		}
	}
}

impl std::error::Error for AuthenticationError {}

/// Authentication backend trait
///
/// All authentication operations are asynchronous to support various backends
/// including database lookups, external API calls, and distributed systems.
#[async_trait::async_trait]
pub trait AuthenticationBackend: Send + Sync {
	/// Authenticate a request and return a user if successful
	///
	/// # Arguments
	///
	/// * `request` - The incoming HTTP request
	///
	/// # Returns
	///
	/// - `Ok(Some(user))` if authentication succeeded
	/// - `Ok(None)` if authentication failed but should try next backend
	/// - `Err(error)` if a fatal error occurred
	async fn authenticate(
		&self,
		request: &reinhardt_http::Request,
	) -> Result<Option<Box<dyn User>>, AuthenticationError>;

	/// Get a user by their ID
	///
	/// # Arguments
	///
	/// * `user_id` - The user's unique identifier
	///
	/// # Returns
	///
	/// - `Ok(Some(user))` if user was found
	/// - `Ok(None)` if user doesn't exist
	/// - `Err(error)` if an error occurred
	async fn get_user(&self, user_id: &str) -> Result<Option<Box<dyn User>>, AuthenticationError>;
}

#[cfg(test)]
mod tests {
	use super::*;
	use uuid::Uuid;

	#[test]
	#[cfg(feature = "jwt")]
	fn test_auth_jwt_generate_unit() {
		let jwt_auth = JwtAuth::new(b"test_secret_key");
		let user_id = "user123".to_string();
		let username = "testuser".to_string();

		let token = jwt_auth.generate_token(user_id, username).unwrap();

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

	#[test]
	fn test_simple_user_implementation() {
		let user = SimpleUser {
			id: Uuid::new_v4(),
			username: "testuser".to_string(),
			email: "test@example.com".to_string(),
			is_active: true,
			is_admin: false,
			is_staff: false,
			is_superuser: false,
		};

		assert!(!user.id().is_empty());
		assert_eq!(user.username(), "testuser");
		assert!(user.is_authenticated());
		assert!(user.is_active());
		assert!(!user.is_admin());
	}

	#[test]
	fn test_anonymous_user() {
		let user = AnonymousUser;

		assert_eq!(user.id(), "");
		assert_eq!(user.username(), "");
		assert!(!user.is_authenticated());
		assert!(!user.is_active());
		assert!(!user.is_admin());
	}
}
