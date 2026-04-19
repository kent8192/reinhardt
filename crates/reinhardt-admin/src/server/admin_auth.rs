//! Type-erased admin user authentication.
//!
//! Provides [`AdminAuthenticatedUser`], a type-erased user extractor for admin
//! server functions. Instead of hardcoding a specific user model, this module
//! uses a registered loader function to query whichever concrete user type the
//! project has configured via [`AdminSite::set_user_type`].
//!
//! [`AdminSite::set_user_type`]: crate::core::AdminSite::set_user_type

use crate::core::AdminUser;
use async_trait::async_trait;
use reinhardt_auth::BaseUser;
use reinhardt_db::orm::{DatabaseConnection, Model};
use reinhardt_di::{DiError, DiResult, Injectable, InjectionContext};
use reinhardt_http::AuthState;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

/// Type-erased async loader that queries a user from the database and returns
/// a boxed [`AdminUser`] trait object.
///
/// The closure captures the concrete user type `U` at registration time via
/// [`create_admin_user_loader`], but the returned signature is fully erased.
pub(crate) type AdminUserLoaderFn = Arc<
	dyn Fn(
			String,
			Arc<DatabaseConnection>,
		) -> Pin<Box<dyn Future<Output = Result<Arc<dyn AdminUser>, DiError>> + Send>>
		+ Send
		+ Sync,
>;

/// Newtype wrapper around [`AdminUserLoaderFn`] for DI registration.
///
/// Stored as a singleton in the DI scope so that [`AdminAuthenticatedUser`]
/// can retrieve it during injection.
#[derive(Clone)]
pub(crate) struct AdminUserLoader(pub(crate) AdminUserLoaderFn);

/// Type-erased authenticated admin user.
///
/// This replaces the hardcoded `AuthUser<AdminDefaultUser>` in admin server
/// functions. It loads the user from the database using whichever concrete
/// user type was registered via [`AdminSite::set_user_type`]. If no custom
/// type was registered, [`AdminDefaultUser`] is used as a fallback.
///
/// The inner `Arc<dyn AdminUser>` provides access to authentication and
/// permission methods without exposing the concrete user type. `Arc` is
/// used instead of `Box` because the `#[server_fn]` macro requires
/// injected types to implement `Clone`.
///
/// # Usage in server functions
///
/// ```rust,ignore
/// use crate::server::admin_auth::AdminAuthenticatedUser;
///
/// #[server_fn]
/// pub async fn my_admin_endpoint(
///     #[inject] AdminAuthenticatedUser(user): AdminAuthenticatedUser,
/// ) -> Result<(), ServerFnError> {
///     // user is Arc<dyn AdminUser>
///     if user.is_superuser() {
///         // ...
///     }
///     Ok(())
/// }
/// ```
///
/// [`AdminSite::set_user_type`]: crate::core::AdminSite::set_user_type
/// [`AdminDefaultUser`]: crate::server::user::AdminDefaultUser
#[derive(Clone)]
pub struct AdminAuthenticatedUser(pub Arc<dyn AdminUser>);

impl std::fmt::Debug for AdminAuthenticatedUser {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("AdminAuthenticatedUser")
			.field("username", &self.0.get_username())
			.finish()
	}
}

#[async_trait]
impl Injectable for AdminAuthenticatedUser {
	async fn inject(ctx: &InjectionContext) -> DiResult<Self> {
		// Get HTTP request from context
		let request = ctx.get_http_request().ok_or_else(|| {
			DiError::Authentication(
				"AdminAuthenticatedUser: No HTTP request available in InjectionContext".to_string(),
			)
		})?;

		// Get AuthState from request extensions
		let auth_state: AuthState = request.extensions.get().ok_or_else(|| {
			DiError::Authentication(
				"AdminAuthenticatedUser: No AuthState found in request extensions".to_string(),
			)
		})?;

		if !auth_state.is_authenticated() {
			return Err(DiError::Authentication(
				"AdminAuthenticatedUser: User is not authenticated".to_string(),
			));
		}

		let user_id = auth_state.user_id().to_string();

		// Get the type-erased loader from DI singleton scope (check early to
		// provide a clear error message if admin routes were not set up)
		let loader: Arc<AdminUserLoader> =
			ctx.get_singleton::<AdminUserLoader>()
				.ok_or_else(|| DiError::NotRegistered {
					type_name: "AdminUserLoader".into(),
					hint: "Call AdminSite::set_user_type::<U>() before building admin routes, \
					       or use the default by calling admin_routes_with_di() which \
					       registers AdminDefaultUser as a fallback."
						.into(),
				})?;

		// Resolve DatabaseConnection from DI (singleton-first, request-scope fallback)
		let db: Arc<DatabaseConnection> = ctx
			.get_singleton::<DatabaseConnection>()
			.or_else(|| ctx.get_request::<DatabaseConnection>())
			.ok_or_else(|| {
				::tracing::warn!(
					"AdminAuthenticatedUser: DatabaseConnection not available for user resolution"
				);
				DiError::Internal {
					message:
						"AdminAuthenticatedUser: DatabaseConnection not registered in DI context"
							.to_string(),
				}
			})?;

		// Call the type-erased loader to query the user from the database
		let user = (loader.0)(user_id, db).await?;

		// Verify user account is active
		if !user.is_active() {
			return Err(DiError::Authentication(
				"User account is not active".to_string(),
			));
		}

		// Verify user has staff privileges
		if !user.is_staff() {
			return Err(DiError::Authentication(
				"User does not have staff privileges".to_string(),
			));
		}

		Ok(AdminAuthenticatedUser(user))
	}
}

/// Creates an [`AdminUserLoader`] that queries user type `U` from the database.
///
/// The returned loader captures the concrete type `U` in a closure, replicating
/// the same database query logic as [`AuthUser<U>::inject`] but returning a
/// type-erased `Arc<dyn AdminUser>`.
///
/// # Type requirements
///
/// `U` must implement `BaseUser`, `AdminUser`, and the ORM trait (`Model`).
/// Types with `FullUser` satisfy `AdminUser` automatically via the blanket impl.
/// Simpler `BaseUser`-only models can manually implement `AdminUser`.
///
/// [`AuthUser<U>::inject`]: reinhardt_auth::AuthUser
pub(crate) fn create_admin_user_loader<U>() -> AdminUserLoader
where
	U: BaseUser + AdminUser + Model + Clone + Send + Sync + 'static,
	<U as BaseUser>::PrimaryKey: std::str::FromStr + ToString + Send + Sync,
	<<U as BaseUser>::PrimaryKey as std::str::FromStr>::Err: std::fmt::Debug,
	<U as Model>::PrimaryKey: From<<U as BaseUser>::PrimaryKey>,
{
	let loader: AdminUserLoaderFn = Arc::new(move |user_id, db| {
		Box::pin(async move {
			// Parse user_id — NO fallback to nil UUID
			let pk = user_id
				.parse::<<U as BaseUser>::PrimaryKey>()
				.map_err(|e| {
					::tracing::warn!(
						user_id = %user_id,
						error = ?e,
						"AdminUserLoader: failed to parse user_id"
					);
					DiError::Authentication("AdminUserLoader: Invalid user_id format".to_string())
				})?;

			let model_pk = <U as Model>::PrimaryKey::from(pk);

			// Query user from database
			let user = U::objects()
				.get(model_pk)
				.first_with_db(&db)
				.await
				.map_err(|e| {
					::tracing::warn!(error = ?e, "AdminUserLoader: Database query failed");
					DiError::Internal {
						message: "AdminUserLoader: Database query failed".to_string(),
					}
				})?
				.ok_or_else(|| {
					::tracing::warn!(
						user_id = %user_id,
						"AdminUserLoader: User not found in database"
					);
					DiError::NotFound("AdminUserLoader: User not found".to_string())
				})?;

			Ok(Arc::new(user) as Arc<dyn AdminUser>)
		})
	});

	AdminUserLoader(loader)
}

/// Authenticated user info returned by [`AdminLoginAuthenticator`].
///
/// Contains the minimal user data needed to generate a JWT token
/// and populate client-side auth state.
pub(crate) struct AuthenticatedUserInfo {
	/// Primary key as string (typically a UUID).
	pub(crate) user_id: String,
	/// Username used for login.
	pub(crate) username: String,
	/// Whether the user is a staff member.
	pub(crate) is_staff: bool,
	/// Whether the user is a superuser.
	pub(crate) is_superuser: bool,
}

/// Type-erased async function that authenticates a user by username and password.
///
/// Returns user info on success, or `None` if credentials are invalid
/// (wrong username, wrong password, or user is not active/staff).
pub(crate) type AdminLoginAuthenticatorFn = Arc<
	dyn Fn(
			String,
			String,
			Arc<DatabaseConnection>,
		)
			-> Pin<Box<dyn Future<Output = Result<Option<AuthenticatedUserInfo>, DiError>> + Send>>
		+ Send
		+ Sync,
>;

/// Newtype wrapper around [`AdminLoginAuthenticatorFn`] for DI registration.
///
/// This type is public because the `#[server_fn]` macro generates a public
/// function signature that references it via `#[inject]`. The inner function
/// pointer is `pub(crate)` to prevent external use.
#[derive(Clone)]
pub struct AdminLoginAuthenticator(pub(crate) AdminLoginAuthenticatorFn);

#[async_trait]
impl Injectable for AdminLoginAuthenticator {
	async fn inject(ctx: &InjectionContext) -> DiResult<Self> {
		ctx.get_singleton::<AdminLoginAuthenticator>()
			.map(|arc| (*arc).clone())
			.ok_or_else(|| DiError::NotRegistered {
				type_name: "AdminLoginAuthenticator".into(),
				hint: "Call AdminSite::set_user_type::<U>() or use admin_routes_with_di() \
				       which registers AdminDefaultUser as a fallback."
					.into(),
			})
	}
}

/// Creates an [`AdminLoginAuthenticator`] for user type `U`.
///
/// The authenticator:
/// 1. Queries the user by username using ORM filter
/// 2. Verifies the password using `BaseUser::check_password()`
/// 3. Checks that the user is active and has staff privileges (via `AdminUser`)
/// 4. Returns user info for JWT token generation
pub(crate) fn create_admin_login_authenticator<U>() -> AdminLoginAuthenticator
where
	U: BaseUser + AdminUser + Model + Clone + Send + Sync + 'static,
	<U as BaseUser>::PrimaryKey: ToString + Send + Sync,
{
	use reinhardt_db::orm::{Filter, FilterOperator, FilterValue};

	let authenticator: AdminLoginAuthenticatorFn = Arc::new(move |username, password, db| {
		Box::pin(async move {
			// Query user by username
			let user: Option<U> = U::objects()
				.filter_by(Filter::new(
					"username",
					FilterOperator::Eq,
					FilterValue::String(username.clone()),
				))
				.first_with_db(&db)
				.await
				.map_err(|e| {
					::tracing::warn!(error = ?e, "AdminLoginAuthenticator: Database query failed");
					DiError::Internal {
						message: "AdminLoginAuthenticator: Database query failed".to_string(),
					}
				})?;

			let Some(user) = user else {
				::tracing::debug!(username = %username, "AdminLoginAuthenticator: User not found");
				return Ok(None);
			};

			// Verify password
			let password_valid = user.check_password(&password).map_err(|e| {
				::tracing::warn!(error = ?e, "AdminLoginAuthenticator: Password check failed");
				DiError::Internal {
					message: "AdminLoginAuthenticator: Password verification error".to_string(),
				}
			})?;

			if !password_valid {
				::tracing::debug!(username = %username, "AdminLoginAuthenticator: Invalid password");
				return Ok(None);
			}

			// Check active and staff status
			if !AdminUser::is_active(&user) {
				::tracing::debug!(username = %username, "AdminLoginAuthenticator: User is not active");
				return Ok(None);
			}

			if !user.is_staff() {
				::tracing::debug!(username = %username, "AdminLoginAuthenticator: User is not staff");
				return Ok(None);
			}

			let user_id = user
				.primary_key()
				.map(|pk| pk.to_string())
				.unwrap_or_default();

			Ok(Some(AuthenticatedUserInfo {
				user_id,
				username: AdminUser::get_username(&user).to_string(),
				is_staff: user.is_staff(),
				is_superuser: user.is_superuser(),
			}))
		})
	});

	AdminLoginAuthenticator(authenticator)
}

#[cfg(test)]
mod tests {
	use super::*;
	use reinhardt_di::SingletonScope;
	use rstest::rstest;

	#[rstest]
	#[tokio::test]
	async fn test_inject_returns_error_when_no_http_request() {
		// Arrange
		let singleton = Arc::new(SingletonScope::new());
		let ctx = InjectionContext::builder(singleton).build();

		// Act
		let result = AdminAuthenticatedUser::inject(&ctx).await;

		// Assert
		assert!(result.is_err());
		let err = result.unwrap_err();
		assert!(
			err.to_string().contains("No HTTP request"),
			"Expected 'No HTTP request' error, got: {}",
			err
		);
	}

	#[rstest]
	#[tokio::test]
	async fn test_inject_returns_error_when_no_auth_state() {
		// Arrange
		let singleton = Arc::new(SingletonScope::new());
		let request = reinhardt_http::Request::builder()
			.uri("/admin/test")
			.build()
			.expect("Failed to build test request");
		let ctx = InjectionContext::builder(singleton)
			.with_request(request)
			.build();

		// Act
		let result = AdminAuthenticatedUser::inject(&ctx).await;

		// Assert
		assert!(result.is_err());
		let err = result.unwrap_err();
		assert!(
			err.to_string().contains("No AuthState"),
			"Expected 'No AuthState' error, got: {}",
			err
		);
	}

	#[rstest]
	#[tokio::test]
	async fn test_inject_returns_error_when_not_authenticated() {
		// Arrange
		let singleton = Arc::new(SingletonScope::new());
		let request = reinhardt_http::Request::builder()
			.uri("/admin/test")
			.build()
			.expect("Failed to build test request");
		// Insert unauthenticated AuthState
		request.extensions.insert(AuthState::anonymous());
		let ctx = InjectionContext::builder(singleton)
			.with_request(request)
			.build();

		// Act
		let result = AdminAuthenticatedUser::inject(&ctx).await;

		// Assert
		assert!(result.is_err());
		let err = result.unwrap_err();
		assert!(
			err.to_string().contains("not authenticated"),
			"Expected 'not authenticated' error, got: {}",
			err
		);
	}

	#[rstest]
	#[tokio::test]
	async fn test_inject_returns_error_when_no_loader_registered() {
		// Arrange
		let singleton = Arc::new(SingletonScope::new());
		let request = reinhardt_http::Request::builder()
			.uri("/admin/test")
			.build()
			.expect("Failed to build test request");
		request
			.extensions
			.insert(AuthState::authenticated("user-123", true, true));
		let ctx = InjectionContext::builder(singleton)
			.with_request(request)
			.build();

		// Act
		let result = AdminAuthenticatedUser::inject(&ctx).await;

		// Assert
		assert!(result.is_err());
		let err = result.unwrap_err();
		assert!(
			err.to_string().contains("AdminUserLoader"),
			"Expected 'AdminUserLoader' error, got: {}",
			err
		);
	}

	#[rstest]
	#[tokio::test]
	async fn test_inject_returns_error_when_no_database_connection() {
		// Arrange: singleton with AdminUserLoader but NO DatabaseConnection
		let singleton = Arc::new(SingletonScope::new());
		let loader = AdminUserLoader(Arc::new(|_user_id, _db| {
			Box::pin(async { Err(DiError::NotFound("should not be called".to_string())) })
		}));
		singleton.set_arc(Arc::new(loader));
		let request = reinhardt_http::Request::builder()
			.uri("/admin/test")
			.build()
			.expect("Failed to build test request");
		request
			.extensions
			.insert(AuthState::authenticated("user-123", true, true));
		let ctx = InjectionContext::builder(singleton)
			.with_request(request)
			.build();

		// Act
		let result = AdminAuthenticatedUser::inject(&ctx).await;

		// Assert
		assert!(result.is_err());
		let err = result.unwrap_err();
		assert!(
			err.to_string().contains("DatabaseConnection"),
			"Expected error mentioning DatabaseConnection, got: {}",
			err
		);
	}

	#[rstest]
	fn test_admin_user_loader_can_be_stored_in_singleton_scope() {
		// Arrange
		let singleton = SingletonScope::new();
		let loader = AdminUserLoader(Arc::new(|_user_id, _db| {
			Box::pin(async { Err(DiError::NotFound("test loader".to_string())) })
		}));

		// Act
		singleton.set_arc(Arc::new(loader));

		// Assert
		let retrieved = singleton.get::<AdminUserLoader>();
		assert!(
			retrieved.is_some(),
			"AdminUserLoader should be retrievable from singleton scope"
		);
	}
}
