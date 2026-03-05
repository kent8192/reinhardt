//! CurrentUser Injectable for dependency injection
//!
//! Provides access to the authenticated user in endpoint handlers.
//!
//! This module integrates with the authentication middleware to provide
//! type-safe access to the currently authenticated user via dependency injection.
//!
//! # How it works
//!
//! 1. The authentication middleware (e.g., `AuthenticationMiddleware`) validates
//!    the user and stores an `AuthState` in the request extensions.
//! 2. When a handler requests `CurrentUser<U>`, the injectable implementation:
//!    - Extracts `AuthState` from request extensions
//!    - Parses the user_id to the model's primary key type
//!    - Loads the user from the database using `Model::objects().get(pk)`
//! 3. Returns `CurrentUser::authenticated(user, user_id)` or `CurrentUser::anonymous()`

use crate::AuthenticationError;
use crate::BaseUser;
use async_trait::async_trait;
use reinhardt_db::orm::{DatabaseConnection, Model};
use reinhardt_di::{DiResult, Injectable, InjectionContext};
use reinhardt_http::AuthState;
use std::sync::Arc;
use uuid::Uuid;

/// Wrapper type representing the currently authenticated user for DI.
///
/// This type provides access to the authenticated user within endpoint handlers
/// through dependency injection. It wraps an optional user instance and user ID,
/// allowing handlers to check authentication status and access user data.
///
/// # Example
///
/// ```rust,ignore
/// use reinhardt_auth::CurrentUser;
/// use reinhardt_auth::DefaultUser;
/// use reinhardt_http::Response;
///
/// async fn my_handler(
///     current_user: CurrentUser<DefaultUser>,
/// ) -> Result<Response, Box<dyn std::error::Error>> {
///     if current_user.is_authenticated() {
///         let user = current_user.user()?;
///         let user_id = current_user.id()?;
///         println!("Authenticated user: {} (ID: {})", user.get_username(), user_id);
///     }
///     Ok(Response::ok())
/// }
/// ```
pub struct CurrentUser<U: BaseUser + Clone> {
	user: Option<U>,
	user_id: Option<Uuid>,
}

impl<U: BaseUser + Clone> Clone for CurrentUser<U> {
	fn clone(&self) -> Self {
		Self {
			user: self.user.clone(),
			user_id: self.user_id,
		}
	}
}

impl<U: BaseUser + Clone> CurrentUser<U> {
	/// Creates a new authenticated CurrentUser.
	///
	/// # Arguments
	///
	/// * `user` - The authenticated user instance
	/// * `user_id` - The user's unique identifier
	pub fn authenticated(user: U, user_id: Uuid) -> Self {
		Self {
			user: Some(user),
			user_id: Some(user_id),
		}
	}

	/// Creates an anonymous (unauthenticated) CurrentUser.
	pub fn anonymous() -> Self {
		Self {
			user: None,
			user_id: None,
		}
	}

	/// Returns whether the current user is authenticated.
	pub fn is_authenticated(&self) -> bool {
		self.user.is_some()
	}

	/// Returns a reference to the user if authenticated.
	///
	/// # Errors
	///
	/// Returns `AuthenticationError::NotAuthenticated` if the user is not authenticated.
	pub fn user(&self) -> Result<&U, AuthenticationError> {
		self.user
			.as_ref()
			.ok_or(AuthenticationError::NotAuthenticated)
	}

	/// Returns the user ID if authenticated.
	///
	/// # Errors
	///
	/// Returns `AuthenticationError::NotAuthenticated` if the user is not authenticated.
	pub fn id(&self) -> Result<Uuid, AuthenticationError> {
		self.user_id.ok_or(AuthenticationError::NotAuthenticated)
	}

	/// Consumes this wrapper and returns the user if authenticated.
	///
	/// # Errors
	///
	/// Returns `AuthenticationError::NotAuthenticated` if the user is not authenticated.
	pub fn into_user(self) -> Result<U, AuthenticationError> {
		self.user.ok_or(AuthenticationError::NotAuthenticated)
	}

	/// Returns the user as a trait object for permission checking.
	///
	/// This method is used to pass the user to `ModelAdmin` permission methods
	/// that accept `&(dyn Any + Send + Sync)`.
	///
	/// # Returns
	///
	/// Returns `Some` with a reference to the user as a trait object if authenticated,
	/// or `None` if the user is anonymous.
	pub fn as_any(&self) -> Option<&(dyn std::any::Any + Send + Sync)>
	where
		U: 'static,
	{
		self.user
			.as_ref()
			.map(|u| u as &(dyn std::any::Any + Send + Sync))
	}
}

#[async_trait]
impl<U> Injectable for CurrentUser<U>
where
	U: BaseUser + Model + Clone + Send + Sync + 'static,
	// Ensure BaseUser::PrimaryKey and Model::PrimaryKey are the same type
	<U as BaseUser>::PrimaryKey: std::str::FromStr + ToString + Send + Sync,
	<<U as BaseUser>::PrimaryKey as std::str::FromStr>::Err: std::fmt::Debug,
	<U as Model>::PrimaryKey: From<<U as BaseUser>::PrimaryKey>,
{
	async fn inject(ctx: &InjectionContext) -> DiResult<Self> {
		// 1. Get HTTP request from context
		#[cfg(feature = "params")]
		let request = match ctx.get_http_request() {
			Some(req) => req,
			None => return Ok(Self::anonymous()),
		};

		#[cfg(not(feature = "params"))]
		return Ok(Self::anonymous());

		// 2. Get AuthState from request extensions
		#[cfg(feature = "params")]
		let auth_state: AuthState = match request.extensions.get() {
			Some(state) => state,
			None => return Ok(Self::anonymous()),
		};

		// 3. Check if authenticated
		#[cfg(feature = "params")]
		if !auth_state.is_authenticated() {
			return Ok(Self::anonymous());
		}

		// 4. Parse user_id to PrimaryKey type
		#[cfg(feature = "params")]
		let base_pk: <U as BaseUser>::PrimaryKey = match auth_state.user_id().parse() {
			Ok(pk) => pk,
			Err(_) => return Ok(Self::anonymous()),
		};

		// Convert BaseUser::PrimaryKey to Model::PrimaryKey
		#[cfg(feature = "params")]
		let model_pk: <U as Model>::PrimaryKey = base_pk.into();

		// 5. Get DatabaseConnection from DI context
		#[cfg(feature = "params")]
		let db: Arc<DatabaseConnection> = match ctx.resolve::<DatabaseConnection>().await {
			Ok(conn) => conn,
			Err(_) => return Ok(Self::anonymous()),
		};

		// 6. Load user from database using Model::objects() (Django-style ORM)
		#[cfg(feature = "params")]
		let user: U = match U::objects().get(model_pk).first_with_db(&db).await {
			Ok(Some(u)) => u,
			Ok(None) | Err(_) => return Ok(Self::anonymous()),
		};

		// 7. Parse UUID for CurrentUser (Uuid is commonly used for user IDs)
		#[cfg(feature = "params")]
		let user_id = match Uuid::parse_str(auth_state.user_id()) {
			Ok(id) => id,
			Err(_) => Uuid::nil(),
		};

		#[cfg(feature = "params")]
		Ok(Self::authenticated(user, user_id))
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::PasswordHasher;
	use chrono::{DateTime, Utc};
	use serde::{Deserialize, Serialize};

	/// Mock password hasher for testing
	#[derive(Default)]
	struct MockHasher;

	impl PasswordHasher for MockHasher {
		fn hash(&self, password: &str) -> Result<String, reinhardt_core::exception::Error> {
			Ok(format!("hashed:{}", password))
		}

		fn verify(
			&self,
			password: &str,
			hash: &str,
		) -> Result<bool, reinhardt_core::exception::Error> {
			Ok(hash == format!("hashed:{}", password))
		}
	}

	// Test user implementation for unit tests
	#[derive(Clone, Serialize, Deserialize)]
	struct TestUser {
		id: Uuid,
		username: String,
		is_active: bool,
	}

	impl BaseUser for TestUser {
		type PrimaryKey = Uuid;
		type Hasher = MockHasher;

		fn get_username_field() -> &'static str {
			"username"
		}

		fn get_username(&self) -> &str {
			&self.username
		}

		fn password_hash(&self) -> Option<&str> {
			None
		}

		fn set_password_hash(&mut self, _hash: String) {}

		fn last_login(&self) -> Option<DateTime<Utc>> {
			None
		}

		fn set_last_login(&mut self, _time: DateTime<Utc>) {}

		fn is_active(&self) -> bool {
			self.is_active
		}
	}

	#[test]
	fn test_authenticated_user() {
		let user_id = Uuid::new_v4();
		let user = TestUser {
			id: user_id,
			username: "testuser".to_string(),
			is_active: true,
		};

		let current_user = CurrentUser::authenticated(user, user_id);

		assert!(current_user.is_authenticated());
		assert_eq!(current_user.id().unwrap(), user_id);
		assert_eq!(current_user.user().unwrap().get_username(), "testuser");
	}

	#[test]
	fn test_anonymous_user() {
		let current_user: CurrentUser<TestUser> = CurrentUser::anonymous();

		assert!(!current_user.is_authenticated());
		assert!(current_user.id().is_err());
		assert!(current_user.user().is_err());
	}

	#[test]
	fn test_into_user_authenticated() {
		let user_id = Uuid::new_v4();
		let user = TestUser {
			id: user_id,
			username: "testuser".to_string(),
			is_active: true,
		};

		let current_user = CurrentUser::authenticated(user, user_id);
		let extracted = current_user.into_user().unwrap();

		assert_eq!(extracted.get_username(), "testuser");
	}

	#[test]
	fn test_into_user_anonymous() {
		let current_user: CurrentUser<TestUser> = CurrentUser::anonymous();
		let result = current_user.into_user();

		assert!(result.is_err());
	}
}
