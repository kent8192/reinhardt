//! Authenticated user extractor that loads the full user model from database.
//!
//! Wraps the user model `U` as a tuple struct for destructuring, consistent
//! with `Path`, `Json`, and other Reinhardt extractors.

use crate::BaseUser;
use async_trait::async_trait;
use reinhardt_db::orm::{CustomManager, DatabaseConnection, Model};
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
/// use reinhardt_auth::CurrentUser;
/// use reinhardt_auth::DefaultUser;
///
/// #[get("/profile/")]
/// pub async fn profile(
///     #[inject] CurrentUser(user): CurrentUser<DefaultUser>,
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
pub struct CurrentUser<U: BaseUser>(pub U);

#[cfg(feature = "params")]
async fn resolve_current_user<U>(ctx: &InjectionContext) -> DiResult<U>
where
	U: BaseUser + Model + Clone + Send + Sync + 'static,
	<U as BaseUser>::PrimaryKey: std::str::FromStr + ToString + Send + Sync,
	<<U as BaseUser>::PrimaryKey as std::str::FromStr>::Err: std::fmt::Debug,
	<U as Model>::PrimaryKey: From<<U as BaseUser>::PrimaryKey>,
{
	// Get HTTP request from context.
	let request = ctx.get_http_request().ok_or_else(|| {
		DiError::NotFound("CurrentUser: No HTTP request available in InjectionContext".to_string())
	})?;

	// Get AuthState from request extensions.
	let auth_state: AuthState = request.extensions.get().ok_or_else(|| {
		DiError::NotFound("CurrentUser: No AuthState found in request extensions".to_string())
	})?;

	if !auth_state.is_authenticated() {
		return Err(DiError::Authentication(
			"CurrentUser: User is not authenticated".to_string(),
		));
	}

	// Parse user_id — NO fallback to nil UUID (#2430).
	let user_pk = auth_state
		.user_id()
		.parse::<<U as BaseUser>::PrimaryKey>()
		.map_err(|e| {
			::tracing::warn!(
				user_id = %auth_state.user_id(),
				error = ?e,
				"CurrentUser: failed to parse user_id from AuthState"
			);
			DiError::Authentication("CurrentUser: Invalid user_id format in AuthState".to_string())
		})?;

	let model_pk = <U as Model>::PrimaryKey::from(user_pk);

	// Resolve DatabaseConnection from DI (singleton-first, request-scope fallback)
	// using get_singleton/get_request directly because DatabaseConnection is
	// pre-seeded into the singleton scope at server startup, not registered in
	// the global DependencyRegistry.
	let db: Arc<DatabaseConnection> = ctx
		.get_singleton::<DatabaseConnection>()
		.or_else(|| ctx.get_request::<DatabaseConnection>())
		.ok_or_else(|| {
			::tracing::warn!("CurrentUser: DatabaseConnection not available for user resolution");
			DiError::Internal {
				message: "CurrentUser: DatabaseConnection not registered in DI context".to_string(),
			}
		})?;

	U::objects()
		.get(model_pk)
		.first_with_db(&db)
		.await
		.map_err(|e| {
			::tracing::warn!(error = ?e, "CurrentUser: Failed to load user from database");
			DiError::Internal {
				message: "CurrentUser: Database query failed".to_string(),
			}
		})?
		.ok_or_else(|| {
			::tracing::warn!(
				user_id = %auth_state.user_id(),
				"CurrentUser: User not found in database"
			);
			DiError::NotFound("CurrentUser: User not found".to_string())
		})
}

#[cfg(feature = "params")]
#[async_trait]
impl<U> Injectable for CurrentUser<U>
where
	U: BaseUser + Model + Clone + Send + Sync + 'static,
	<U as BaseUser>::PrimaryKey: std::str::FromStr + ToString + Send + Sync,
	<<U as BaseUser>::PrimaryKey as std::str::FromStr>::Err: std::fmt::Debug,
	<U as Model>::PrimaryKey: From<<U as BaseUser>::PrimaryKey>,
{
	async fn inject(ctx: &InjectionContext) -> DiResult<Self> {
		resolve_current_user(ctx).await.map(CurrentUser)
	}
}

#[cfg(not(feature = "params"))]
#[async_trait]
impl<U> Injectable for CurrentUser<U>
where
	U: BaseUser + Model + Clone + Send + Sync + 'static,
	<U as BaseUser>::PrimaryKey: std::str::FromStr + ToString + Send + Sync,
	<<U as BaseUser>::PrimaryKey as std::str::FromStr>::Err: std::fmt::Debug,
	<U as Model>::PrimaryKey: From<<U as BaseUser>::PrimaryKey>,
{
	async fn inject(_ctx: &InjectionContext) -> DiResult<Self> {
		Err(DiError::NotFound(
			"CurrentUser requires the 'params' feature to be enabled".to_string(),
		))
	}
}

#[cfg(test)]
mod tests {
	use super::CurrentUser;
	use crate::{BaseUser, PasswordHasher};
	use chrono::{DateTime, Utc};
	use serde::{Deserialize, Serialize};

	#[derive(Default)]
	struct TestHasher;

	impl PasswordHasher for TestHasher {
		fn hash(&self, password: &str) -> Result<String, reinhardt_core::exception::Error> {
			Ok(password.to_string())
		}

		fn verify(
			&self,
			password: &str,
			hash: &str,
		) -> Result<bool, reinhardt_core::exception::Error> {
			Ok(password == hash)
		}
	}

	#[derive(Clone, Serialize, Deserialize)]
	struct TestUser {
		username: String,
		password_hash: Option<String>,
		last_login: Option<DateTime<Utc>>,
		is_active: bool,
	}

	impl BaseUser for TestUser {
		type PrimaryKey = String;
		type Hasher = TestHasher;

		fn get_username_field() -> &'static str {
			"username"
		}

		fn get_username(&self) -> &str {
			&self.username
		}

		fn password_hash(&self) -> Option<&str> {
			self.password_hash.as_deref()
		}

		fn set_password_hash(&mut self, hash: String) {
			self.password_hash = Some(hash);
		}

		fn last_login(&self) -> Option<DateTime<Utc>> {
			self.last_login
		}

		fn set_last_login(&mut self, time: DateTime<Utc>) {
			self.last_login = Some(time);
		}

		fn is_active(&self) -> bool {
			self.is_active
		}
	}

	fn test_user(username: &str) -> TestUser {
		TestUser {
			username: username.to_string(),
			password_hash: None,
			last_login: None,
			is_active: true,
		}
	}

	#[test]
	fn current_user_supports_tuple_struct_destructuring() {
		let CurrentUser(user): CurrentUser<TestUser> = CurrentUser(test_user("alice"));

		assert_eq!(user.get_username(), "alice");
	}
}
