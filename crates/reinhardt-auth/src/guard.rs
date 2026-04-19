//! Guard types for permission-based DI resolution.
//!
//! Provides `Guard<P>`, `Public`, and permission combinators (`All`, `Any`, `Not`)
//! that integrate with the Reinhardt dependency injection system. `Guard<P>` checks
//! a [`Permission`] during injection and returns HTTP 403 on failure.
//!
//! # Examples
//!
//! ```rust,ignore
//! use reinhardt_auth::guard::{Guard, All};
//! use reinhardt_auth::{IsAdminUser, IsActiveUser};
//!
//! // Single permission guard
//! #[get("/admin/")]
//! pub async fn admin_view(
//!     #[inject] _guard: Guard<IsAdminUser>,
//! ) -> ViewResult<Response> {
//!     // Only admin users reach here
//! }
//!
//! // Combined permission guard (AND)
//! #[get("/dashboard/")]
//! pub async fn dashboard(
//!     #[inject] _guard: Guard<All<(IsAdminUser, IsActiveUser)>>,
//! ) -> ViewResult<Response> {
//!     // Only active admin users reach here
//! }
//! ```

pub mod combinators;

use std::marker::PhantomData;

use async_trait::async_trait;
use reinhardt_di::{DiError, DiResult, Injectable, InjectionContext};
use reinhardt_http::AuthState;

use crate::core::{Permission, PermissionContext};

/// Permission guard that checks a [`Permission`] during DI resolution.
///
/// When injected, `Guard<P>` extracts the `AuthState` from the HTTP request
/// extensions, constructs a [`PermissionContext`], and calls
/// `P::has_permission()`. If the check fails, injection returns
/// `DiError::Authorization` which maps to HTTP 403 Forbidden.
///
/// # Type Parameters
///
/// * `P` - A [`Permission`] type that implements `Default + Send + Sync + 'static`
///
/// # Examples
///
/// ```rust,ignore
/// use reinhardt_auth::guard::Guard;
/// use reinhardt_auth::IsAdminUser;
///
/// #[get("/admin/")]
/// pub async fn admin_only(
///     #[inject] _guard: Guard<IsAdminUser>,
/// ) -> ViewResult<Response> {
///     // Permission already verified by Guard
/// }
/// ```
pub struct Guard<P: Permission>(PhantomData<P>);

impl<P: Permission> std::fmt::Debug for Guard<P> {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("Guard").finish()
	}
}

impl<P: Permission> Clone for Guard<P> {
	fn clone(&self) -> Self {
		Guard(PhantomData)
	}
}

impl<P: Permission> Default for Guard<P> {
	fn default() -> Self {
		Guard(PhantomData)
	}
}

#[cfg(feature = "params")]
#[async_trait]
impl<P> Injectable for Guard<P>
where
	P: Permission + Default + Send + Sync + 'static,
{
	async fn inject(ctx: &InjectionContext) -> DiResult<Self> {
		let request = ctx.get_http_request().ok_or_else(|| {
			DiError::NotFound(
				"Guard: No HTTP request available in InjectionContext. \
				 Ensure the router is configured with .with_di_context()"
					.to_string(),
			)
		})?;

		let auth_state: AuthState = request
			.extensions
			.get::<AuthState>()
			.unwrap_or_else(AuthState::anonymous);

		let perm_ctx = PermissionContext {
			request,
			is_authenticated: auth_state.is_authenticated(),
			is_admin: auth_state.is_admin(),
			is_active: auth_state.is_active(),
			user: None,
		};

		if P::default().has_permission(&perm_ctx).await {
			Ok(Guard(PhantomData))
		} else {
			Err(DiError::Authorization("Permission denied".to_string()))
		}
	}
}

#[cfg(not(feature = "params"))]
#[async_trait]
impl<P> Injectable for Guard<P>
where
	P: Permission + Default + Send + Sync + 'static,
{
	async fn inject(_ctx: &InjectionContext) -> DiResult<Self> {
		Err(DiError::NotFound(
			"Guard requires the 'params' feature to be enabled".to_string(),
		))
	}
}

/// Public guard that always succeeds during DI resolution.
///
/// Use this as a no-op permission for endpoints that should be publicly
/// accessible without any authentication or authorization checks.
///
/// # Examples
///
/// ```rust,ignore
/// use reinhardt_auth::guard::Public;
///
/// #[get("/health/")]
/// pub async fn health(
///     #[inject] _guard: Public,
/// ) -> ViewResult<Response> {
///     // No permission check required
/// }
/// ```
#[derive(Debug, Clone, Default)]
pub struct Public;

#[async_trait]
impl Injectable for Public {
	async fn inject(_ctx: &InjectionContext) -> DiResult<Self> {
		Ok(Public)
	}
}

/// AND combinator for permissions.
///
/// `All<(P1, P2, ...)>` requires ALL permissions in the tuple to be satisfied.
/// Implements [`Permission`] for tuple arities 2 through 8.
///
/// # Examples
///
/// ```rust,ignore
/// use reinhardt_auth::guard::{Guard, All};
/// use reinhardt_auth::{IsAdminUser, IsActiveUser};
///
/// // Both IsAdminUser AND IsActiveUser must pass
/// type AdminAndActive = Guard<All<(IsAdminUser, IsActiveUser)>>;
/// ```
pub struct All<T>(PhantomData<T>);

impl<T> std::fmt::Debug for All<T> {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("All").finish()
	}
}

impl<T> Clone for All<T> {
	fn clone(&self) -> Self {
		All(PhantomData)
	}
}

impl<T> Default for All<T> {
	fn default() -> Self {
		All(PhantomData)
	}
}

/// OR combinator for permissions.
///
/// `Any<(P1, P2, ...)>` requires AT LEAST ONE permission in the tuple to be satisfied.
/// Implements [`Permission`] for tuple arities 2 through 8.
///
/// # Examples
///
/// ```rust,ignore
/// use reinhardt_auth::guard::{Guard, Any};
/// use reinhardt_auth::{IsAdminUser, IsActiveUser};
///
/// // Either IsAdminUser OR IsActiveUser must pass
/// type AdminOrActive = Guard<Any<(IsAdminUser, IsActiveUser)>>;
/// ```
pub struct Any<T>(PhantomData<T>);

impl<T> std::fmt::Debug for Any<T> {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("Any").finish()
	}
}

impl<T> Clone for Any<T> {
	fn clone(&self) -> Self {
		Any(PhantomData)
	}
}

impl<T> Default for Any<T> {
	fn default() -> Self {
		Any(PhantomData)
	}
}

/// NOT combinator for permissions.
///
/// `Not<P>` inverts a permission check. It succeeds when `P` fails
/// and fails when `P` succeeds.
///
/// # Examples
///
/// ```rust,ignore
/// use reinhardt_auth::guard::{Guard, Not};
/// use reinhardt_auth::IsAdminUser;
///
/// // Denies admin users (only non-admins allowed)
/// type NonAdmin = Guard<Not<IsAdminUser>>;
/// ```
pub struct Not<P>(PhantomData<P>);

impl<P> std::fmt::Debug for Not<P> {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("Not").finish()
	}
}

impl<P> Clone for Not<P> {
	fn clone(&self) -> Self {
		Not(PhantomData)
	}
}

impl<P> Default for Not<P> {
	fn default() -> Self {
		Not(PhantomData)
	}
}
