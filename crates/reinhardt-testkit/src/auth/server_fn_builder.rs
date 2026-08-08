use super::identity::SessionIdentity;
use super::traits::ForceLoginUser;
use crate::server_fn::MockSession;
use crate::server_fn::ServerFnTestContext;

/// Builder for auth configuration in server_fn test contexts.
///
/// Mirrors the [`crate::client::APIClient`] auth builder API for primary auth.
/// Uses [`MockSession`] instead of a real `AsyncSessionBackend`.
///
/// Secondary auth layers (MFA, etc.) are not supported in server_fn contexts
/// because `MockSession` does not process HTTP headers. Use
/// [`crate::auth::SessionAuthBuilder`] with an `APIClient` for MFA testing.
pub struct ServerFnAuthBuilder {
	ctx: ServerFnTestContext,
	identity: Option<SessionIdentity>,
}

impl ServerFnAuthBuilder {
	pub(crate) fn new(ctx: ServerFnTestContext) -> Self {
		Self {
			ctx,
			identity: None,
		}
	}

	/// Authenticate as the given user via session.
	///
	/// No `AsyncSessionBackend` is required — uses [`MockSession`] internally.
	pub fn session(mut self, user: &impl ForceLoginUser) -> Self {
		self.identity = Some(SessionIdentity::from_user(user));
		self
	}

	/// Authenticate via JWT (sets identity for mock session).
	#[cfg(native)]
	pub fn jwt(
		mut self,
		user: &impl ForceLoginUser,
		_config: &super::builder::JwtTestConfig,
	) -> Self {
		self.identity = Some(SessionIdentity::from_user(user));
		self
	}

	/// Override the `is_staff` flag.
	pub fn with_staff(mut self, is_staff: bool) -> Self {
		if let Some(ref mut id) = self.identity {
			id.is_staff = is_staff;
		}
		self
	}

	/// Override the `is_superuser` flag.
	pub fn with_superuser(mut self, is_superuser: bool) -> Self {
		if let Some(ref mut id) = self.identity {
			id.is_superuser = is_superuser;
		}
		self
	}

	/// Finalize auth configuration and return the configured [`ServerFnTestContext`].
	///
	/// Call `.build()` or `.build_context()` on the result to get the test environment.
	pub fn done(mut self) -> ServerFnTestContext {
		if let Some(identity) = &self.identity {
			let mock_session = MockSession::from_identity(identity);
			self.ctx = self.ctx.with_session(mock_session);
		}
		self.ctx
	}
}

#[cfg(test)]
mod tests {
	use std::sync::Arc;

	use reinhardt_di::SingletonScope;
	use serde_json::json;
	use uuid::Uuid;

	use super::*;
	use crate::auth::JwtTestConfig;

	struct FixtureUser(Uuid);

	impl ForceLoginUser for FixtureUser {
		fn session_user_id(&self) -> String {
			self.0.to_string()
		}
	}

	#[test]
	fn server_fn_auth_builder_preserves_session_and_jwt_identity_overrides() {
		// Arrange
		let user = FixtureUser(Uuid::from_u128(0x8a5d));
		let scope = Arc::new(SingletonScope::new());

		// Act
		let session_env = ServerFnTestContext::new(scope.clone())
			.auth()
			.session(&user)
			.with_staff(true)
			.with_superuser(true)
			.done()
			.build();
		let jwt_env = ServerFnTestContext::new(scope)
			.auth()
			.jwt(&user, &JwtTestConfig::default())
			.with_staff(true)
			.with_superuser(true)
			.done()
			.build();

		// Assert
		for env in [session_env, jwt_env] {
			let session = env.mock_session.unwrap();
			assert_eq!(session.user.as_ref().unwrap().id, user.0);
			assert_eq!(session.get_raw("user_id"), Some(&json!(user.0.to_string())));
			assert_eq!(session.get_raw("is_staff"), Some(&json!(true)));
			assert_eq!(session.get_raw("is_superuser"), Some(&json!(true)));
		}
	}
}
