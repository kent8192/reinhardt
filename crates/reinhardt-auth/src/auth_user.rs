//! Authenticated user extractor that loads the full user model from database.
//!
//! Wraps the user model `U` as a tuple struct for destructuring, consistent
//! with `Path`, `Json`, and other Reinhardt extractors.

use crate::BaseUser;
use async_trait::async_trait;
use reinhardt_db::orm::{DatabaseConnection, Model};
use reinhardt_di::{DiError, DiResult, Injectable, InjectionContext};
use reinhardt_http::AuthState;
use std::sync::Arc;

/// Authenticated user extractor that loads the full user model from database.
///
/// Wraps the user model `U` as a tuple struct for destructuring, consistent
/// with `Path<T>`, `Json<T>`, and other Reinhardt extractors.
///
/// Requires `feature = "params"` to access request data from `InjectionContext`.
///
/// # Usage
///
/// ```rust,ignore
/// use reinhardt_auth::AuthUser;
/// use reinhardt_auth::DefaultUser;
///
/// #[get("/profile/")]
/// pub async fn profile(
///     #[inject] AuthUser(user): AuthUser<DefaultUser>,
/// ) -> ViewResult<Response> {
///     let username = user.get_username();
///     // ...
/// }
/// ```
///
/// # Failure
///
/// Returns an injection error when:
/// - No `AuthState` in request extensions (HTTP 401)
/// - `user_id` parse failure (HTTP 401, not nil UUID fallback)
/// - `DatabaseConnection` not registered in DI (HTTP 503)
/// - Database query failure (HTTP 500)
#[derive(Debug, Clone)]
pub struct AuthUser<U: BaseUser>(pub U);

#[cfg(feature = "params")]
#[async_trait]
impl<U> Injectable for AuthUser<U>
where
	U: BaseUser + Model + Clone + Send + Sync + 'static,
	<U as BaseUser>::PrimaryKey: std::str::FromStr + ToString + Send + Sync,
	<<U as BaseUser>::PrimaryKey as std::str::FromStr>::Err: std::fmt::Debug,
	<U as Model>::PrimaryKey: From<<U as BaseUser>::PrimaryKey>,
{
	async fn inject(ctx: &InjectionContext) -> DiResult<Self> {
		// Get HTTP request from context
		let request = ctx.get_http_request().ok_or_else(|| {
			DiError::NotFound("AuthUser: No HTTP request available in InjectionContext".to_string())
		})?;

		// Get AuthState from request extensions
		let auth_state: AuthState = request.extensions.get().ok_or_else(|| {
			DiError::NotFound("AuthUser: No AuthState found in request extensions".to_string())
		})?;

		if !auth_state.is_authenticated() {
			return Err(DiError::Authentication(
				"AuthUser: User is not authenticated".to_string(),
			));
		}

		// Parse user_id — NO fallback to nil UUID (#2430)
		let user_pk = auth_state
			.user_id()
			.parse::<<U as BaseUser>::PrimaryKey>()
			.map_err(|e| {
				::tracing::warn!(
					user_id = %auth_state.user_id(),
					error = ?e,
					"AuthUser: failed to parse user_id from AuthState"
				);
				DiError::Authentication("AuthUser: Invalid user_id format in AuthState".to_string())
			})?;

		let model_pk = <U as Model>::PrimaryKey::from(user_pk);

		// Resolve DatabaseConnection from DI (singleton-first, request-scope fallback)
		// Uses get_singleton/get_request directly instead of ctx.resolve() because
		// DatabaseConnection is pre-seeded into the singleton scope at server startup,
		// not registered in the global DependencyRegistry.
		let db: Arc<DatabaseConnection> = ctx
			.get_singleton::<DatabaseConnection>()
			.or_else(|| ctx.get_request::<DatabaseConnection>())
			.ok_or_else(|| {
				::tracing::warn!("AuthUser: DatabaseConnection not available for user resolution");
				DiError::Internal {
					message: "AuthUser: DatabaseConnection not registered in DI context"
						.to_string(),
				}
			})?;

		// Query user from database
		let user = U::objects()
			.get(model_pk)
			.first_with_db(&db)
			.await
			.map_err(|e| {
				::tracing::warn!(error = ?e, "AuthUser: Failed to load user from database");
				DiError::Internal {
					message: "AuthUser: Database query failed".to_string(),
				}
			})?
			.ok_or_else(|| {
				::tracing::warn!(
					user_id = %auth_state.user_id(),
					"AuthUser: User not found in database"
				);
				DiError::NotFound("AuthUser: User not found".to_string())
			})?;

		Ok(AuthUser(user))
	}
}

#[cfg(not(feature = "params"))]
#[async_trait]
impl<U> Injectable for AuthUser<U>
where
	U: BaseUser + Model + Clone + Send + Sync + 'static,
	<U as BaseUser>::PrimaryKey: std::str::FromStr + ToString + Send + Sync,
	<<U as BaseUser>::PrimaryKey as std::str::FromStr>::Err: std::fmt::Debug,
	<U as Model>::PrimaryKey: From<<U as BaseUser>::PrimaryKey>,
{
	async fn inject(_ctx: &InjectionContext) -> DiResult<Self> {
		Err(DiError::NotFound(
			"AuthUser requires the 'params' feature to be enabled".to_string(),
		))
	}
}
