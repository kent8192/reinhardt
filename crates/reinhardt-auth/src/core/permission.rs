// This module uses the deprecated User trait for backward compatibility.
// PermissionContext holds an optional Box<dyn User> for existing permission APIs.
#![allow(deprecated)]
use async_trait::async_trait;

use crate::core::user::User;

/// Permission context - contains request information for permission checking
///
/// This struct provides the context needed for permission classes to make
/// authorization decisions.
///
/// # Examples
///
/// ```
/// use reinhardt_auth::{PermissionContext, AnonymousUser, User};
/// use reinhardt_http::Request;
/// use hyper::{Method, Uri, Version, header::HeaderMap};
/// use bytes::Bytes;
///
/// let request = Request::builder()
///     .method(Method::GET)
///     .uri("/")
///     .version(Version::HTTP_11)
///     .headers(HeaderMap::new())
///     .body(Bytes::new())
///     .build()
///     .unwrap();
/// let context = PermissionContext {
///     request: &request,
///     is_authenticated: false,
///     is_admin: false,
///     is_active: false,
///     user: None,
/// };
///
/// assert!(!context.is_authenticated);
/// ```
pub struct PermissionContext<'a> {
	/// The HTTP request
	pub request: &'a reinhardt_http::Request,
	/// Whether the user is authenticated
	pub is_authenticated: bool,
	/// Whether the user is an admin
	pub is_admin: bool,
	/// Whether the user account is active
	pub is_active: bool,
	/// The authenticated user, if any
	pub user: Option<Box<dyn User>>,
}

/// Permission trait - defines permission checking interface
///
/// Implement this trait to create custom permission classes for your API.
///
/// # Examples
///
/// Custom permission class:
///
/// ```
/// use reinhardt_auth::{Permission, PermissionContext};
/// use async_trait::async_trait;
///
/// struct IsOwner;
///
/// #[async_trait]
/// impl Permission for IsOwner {
///     async fn has_permission(&self, context: &PermissionContext<'_>) -> bool {
///         // Check if user is authenticated and owns the resource
///         if !context.is_authenticated {
///             return false;
///         }
///
///         // Extract owner_id from request and compare with user.id()
///         // This is a simplified example
///         true
///     }
/// }
/// ```
#[async_trait]
pub trait Permission: Send + Sync {
	/// Checks if the user has permission to perform the action
	///
	/// Returns `true` if permission is granted, `false` otherwise.
	async fn has_permission(&self, context: &PermissionContext<'_>) -> bool;
}

/// AllowAny - grants permission to all requests
///
/// This permission class allows unrestricted access. Use with caution.
///
/// # Examples
///
/// ```
/// use reinhardt_auth::{Permission, AllowAny, PermissionContext};
/// use reinhardt_http::Request;
/// use hyper::{Method, Uri, Version, header::HeaderMap};
/// use bytes::Bytes;
///
/// # tokio::runtime::Runtime::new().unwrap().block_on(async {
/// let permission = AllowAny;
/// let request = Request::builder()
///     .method(Method::GET)
///     .uri("/")
///     .version(Version::HTTP_11)
///     .headers(HeaderMap::new())
///     .body(Bytes::new())
///     .build()
///     .unwrap();
/// let context = PermissionContext {
///     request: &request,
///     is_authenticated: false,
///     is_admin: false,
///     is_active: false,
///     user: None,
/// };
///
/// assert!(permission.has_permission(&context).await);
/// # });
/// ```
#[derive(Clone, Copy, Default)]
pub struct AllowAny;

#[async_trait]
impl Permission for AllowAny {
	async fn has_permission(&self, _context: &PermissionContext<'_>) -> bool {
		true
	}
}

/// IsAuthenticated - requires the user to be authenticated
///
/// # Examples
///
/// ```
/// use reinhardt_auth::{Permission, IsAuthenticated, PermissionContext, SimpleUser, User};
/// use reinhardt_http::Request;
/// use hyper::{Method, Uri, Version, header::HeaderMap};
/// use bytes::Bytes;
/// use uuid::Uuid;
///
/// # tokio::runtime::Runtime::new().unwrap().block_on(async {
/// let permission = IsAuthenticated;
/// let request = Request::builder()
///     .method(Method::GET)
///     .uri("/")
///     .version(Version::HTTP_11)
///     .headers(HeaderMap::new())
///     .body(Bytes::new())
///     .build()
///     .unwrap();
///
/// // Anonymous user - permission denied
/// let context = PermissionContext {
///     request: &request,
///     is_authenticated: false,
///     is_admin: false,
///     is_active: false,
///     user: None,
/// };
/// assert!(!permission.has_permission(&context).await);
///
/// // Authenticated user - permission granted
/// let user = SimpleUser {
///     id: Uuid::now_v7(),
///     username: "alice".to_string(),
///     email: "alice@example.com".to_string(),
///     is_active: true,
///     is_admin: false,
///     is_staff: false,
///     is_superuser: false,
/// };
/// let context = PermissionContext {
///     request: &request,
///     is_authenticated: true,
///     is_admin: false,
///     is_active: true,
///     user: Some(Box::new(user)),
/// };
/// assert!(permission.has_permission(&context).await);
/// # });
/// ```
#[derive(Clone, Copy, Default)]
pub struct IsAuthenticated;

#[async_trait]
impl Permission for IsAuthenticated {
	async fn has_permission(&self, context: &PermissionContext<'_>) -> bool {
		context.is_authenticated
	}
}

/// IsAdminUser - requires the user to be an admin
///
/// # Examples
///
/// ```
/// use reinhardt_auth::{Permission, IsAdminUser, PermissionContext, SimpleUser, User};
/// use reinhardt_http::Request;
/// use hyper::{Method, Uri, Version, header::HeaderMap};
/// use bytes::Bytes;
/// use uuid::Uuid;
///
/// # tokio::runtime::Runtime::new().unwrap().block_on(async {
/// let permission = IsAdminUser;
/// let request = Request::builder()
///     .method(Method::GET)
///     .uri("/")
///     .version(Version::HTTP_11)
///     .headers(HeaderMap::new())
///     .body(Bytes::new())
///     .build()
///     .unwrap();
///
/// // Non-admin user - permission denied
/// let user = SimpleUser {
///     id: Uuid::now_v7(),
///     username: "alice".to_string(),
///     email: "alice@example.com".to_string(),
///     is_active: true,
///     is_admin: false,
///     is_staff: false,
///     is_superuser: false,
/// };
/// let context = PermissionContext {
///     request: &request,
///     is_authenticated: true,
///     is_admin: false,
///     is_active: true,
///     user: Some(Box::new(user)),
/// };
/// assert!(!permission.has_permission(&context).await);
///
/// // Admin user - permission granted
/// let admin = SimpleUser {
///     id: Uuid::now_v7(),
///     username: "admin".to_string(),
///     email: "admin@example.com".to_string(),
///     is_active: true,
///     is_admin: true,
///     is_staff: true,
///     is_superuser: true,
/// };
/// let context = PermissionContext {
///     request: &request,
///     is_authenticated: true,
///     is_admin: true,
///     is_active: true,
///     user: Some(Box::new(admin)),
/// };
/// assert!(permission.has_permission(&context).await);
/// # });
/// ```
pub struct IsAdminUser;

#[async_trait]
impl Permission for IsAdminUser {
	async fn has_permission(&self, context: &PermissionContext<'_>) -> bool {
		context.is_authenticated && context.is_admin
	}
}

/// IsActiveUser - requires the user to be active
///
/// # Examples
///
/// ```
/// use reinhardt_auth::{Permission, IsActiveUser, PermissionContext};
/// use reinhardt_http::Request;
/// use hyper::{Method, Uri, Version, header::HeaderMap};
/// use bytes::Bytes;
///
/// # tokio::runtime::Runtime::new().unwrap().block_on(async {
/// let permission = IsActiveUser;
/// let request = Request::builder()
///     .method(Method::GET)
///     .uri("/")
///     .version(Version::HTTP_11)
///     .headers(HeaderMap::new())
///     .body(Bytes::new())
///     .build()
///     .unwrap();
///
/// // Inactive user - permission denied
/// let context = PermissionContext {
///     request: &request,
///     is_authenticated: true,
///     is_admin: false,
///     is_active: false,
///     user: None,
/// };
/// assert!(!permission.has_permission(&context).await);
///
/// // Active user - permission granted
/// let context = PermissionContext {
///     request: &request,
///     is_authenticated: true,
///     is_admin: false,
///     is_active: true,
///     user: None,
///     };
/// assert!(permission.has_permission(&context).await);
/// # });
/// ```
pub struct IsActiveUser;

#[async_trait]
impl Permission for IsActiveUser {
	async fn has_permission(&self, context: &PermissionContext<'_>) -> bool {
		context.is_authenticated && context.is_active
	}
}

/// IsAuthenticatedOrReadOnly - authenticated for writes, allow reads
///
/// This permission allows read-only access (GET, HEAD, OPTIONS) to all users,
/// but requires authentication for write operations (POST, PUT, PATCH, DELETE).
///
/// # Examples
///
/// ```
/// use reinhardt_auth::{Permission, IsAuthenticatedOrReadOnly, PermissionContext};
/// use reinhardt_http::Request;
/// use hyper::{Method, Uri, Version, header::HeaderMap};
/// use bytes::Bytes;
///
/// # tokio::runtime::Runtime::new().unwrap().block_on(async {
/// let permission = IsAuthenticatedOrReadOnly;
///
/// // GET request from anonymous user - allowed
/// let mut request = Request::builder()
///     .method(Method::GET)
///     .uri("/")
///     .version(Version::HTTP_11)
///     .headers(HeaderMap::new())
///     .body(Bytes::new())
///     .build()
///     .unwrap();
/// request.method = Method::GET;
/// let context = PermissionContext {
///     request: &request,
///     is_authenticated: false,
///     is_admin: false,
///     is_active: false,
///     user: None,
/// };
/// assert!(permission.has_permission(&context).await);
///
/// // POST request from anonymous user - denied
/// request.method = Method::POST;
/// let context = PermissionContext {
///     request: &request,
///     is_authenticated: false,
///     is_admin: false,
///     is_active: false,
///     user: None,
/// };
/// assert!(!permission.has_permission(&context).await);
///
/// // POST request from authenticated user - allowed
/// let context = PermissionContext {
///     request: &request,
///     is_authenticated: true,
///     is_admin: false,
///     is_active: true,
///     user: None,
/// };
/// assert!(permission.has_permission(&context).await);
/// # });
/// ```
#[derive(Clone, Copy, Default)]
pub struct IsAuthenticatedOrReadOnly;

#[async_trait]
impl Permission for IsAuthenticatedOrReadOnly {
	async fn has_permission(&self, context: &PermissionContext<'_>) -> bool {
		use hyper::Method;

		// Allow read-only methods
		matches!(
			context.request.method,
			Method::GET | Method::HEAD | Method::OPTIONS
		) || context.is_authenticated
	}
}
