//! Lightweight authentication extractor that reads from request extensions.
//!
//! Does NOT perform a database query. Use [`AuthUser`](crate::AuthUser) when the full
//! user model object is needed.

use async_trait::async_trait;
use reinhardt_di::{DiError, DiResult, Injectable, InjectionContext};
use reinhardt_http::AuthState;

/// Lightweight authentication extractor that reads from request extensions.
///
/// Wraps `AuthState` as a tuple struct for destructuring, consistent
/// with `Path<T>`, `Json<T>`, and other Reinhardt extractors.
///
/// Requires `feature = "params"` to access request data from `InjectionContext`.
///
/// # Usage
///
/// ```rust,ignore
/// use reinhardt_auth::AuthInfo;
///
/// #[get("/admin/")]
/// pub async fn admin(
///     #[inject] AuthInfo(state): AuthInfo,
/// ) -> ViewResult<Response> {
///     if !state.is_admin() {
///         return Err(Error::forbidden("Admin access required"));
///     }
///     // ...
/// }
/// ```
///
/// # Failure
///
/// Returns an injection error (maps to HTTP 401) when:
/// - No `AuthState` is present in request extensions
/// - `AuthState` indicates the user is not authenticated
#[derive(Debug, Clone)]
pub struct AuthInfo(pub AuthState);

#[cfg(feature = "params")]
#[async_trait]
impl Injectable for AuthInfo {
	async fn inject(ctx: &InjectionContext) -> DiResult<Self> {
		let request = ctx.get_http_request().ok_or_else(|| {
			DiError::NotFound(
				"AuthInfo: No HTTP request available in InjectionContext. \
				 Ensure the router is configured with .with_di_context()"
					.to_string(),
			)
		})?;

		let auth_state: AuthState = request.extensions.get().ok_or_else(|| {
			DiError::NotFound(
				"AuthInfo: No AuthState found in request extensions. \
				 Ensure authentication middleware is configured."
					.to_string(),
			)
		})?;

		if !auth_state.is_authenticated() {
			return Err(DiError::Authentication(
				"AuthInfo: User is not authenticated".to_string(),
			));
		}

		Ok(AuthInfo(auth_state))
	}
}

#[cfg(not(feature = "params"))]
#[async_trait]
impl Injectable for AuthInfo {
	async fn inject(_ctx: &InjectionContext) -> DiResult<Self> {
		Err(DiError::NotFound(
			"AuthInfo requires the 'params' feature to be enabled".to_string(),
		))
	}
}
