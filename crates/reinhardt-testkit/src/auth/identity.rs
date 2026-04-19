use super::traits::ForceLoginUser;

/// Type-erased session identity, decoupled from the user's concrete type.
///
/// Holds the minimal fields that `CookieSessionAuthMiddleware` reads from
/// `SessionData.data`: `user_id`, `is_staff`, `is_superuser`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SessionIdentity {
	/// User ID stored as `"user_id"` in session data.
	pub user_id: String,
	/// Staff flag stored as `"is_staff"` in session data.
	pub is_staff: bool,
	/// Superuser flag stored as `"is_superuser"` in session data.
	pub is_superuser: bool,
}

impl SessionIdentity {
	/// Extract identity from any user implementing [`ForceLoginUser`].
	pub fn from_user(user: &impl ForceLoginUser) -> Self {
		Self {
			user_id: user.session_user_id(),
			is_staff: user.session_is_staff(),
			is_superuser: user.session_is_superuser(),
		}
	}

	/// Convert to `SessionData` for `AsyncSessionBackend`.
	///
	/// Creates a `SessionData` with the fields that `CookieSessionAuthMiddleware`
	/// expects: `user_id`, `is_staff`, `is_superuser`.
	#[cfg(native)]
	pub fn to_session_data(
		&self,
		session_id: &str,
		ttl: std::time::Duration,
	) -> reinhardt_middleware::session::SessionData {
		use std::collections::HashMap;

		use serde_json::json;

		let mut session = reinhardt_middleware::session::SessionData::new(ttl);
		session.id = session_id.to_string();
		session.data = HashMap::from([
			("user_id".into(), json!(self.user_id)),
			("is_staff".into(), json!(self.is_staff)),
			("is_superuser".into(), json!(self.is_superuser)),
		]);
		session
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::*;

	struct StubUser {
		id: String,
		staff: bool,
		superuser: bool,
	}

	impl ForceLoginUser for StubUser {
		fn session_user_id(&self) -> String {
			self.id.clone()
		}
		fn session_is_staff(&self) -> bool {
			self.staff
		}
		fn session_is_superuser(&self) -> bool {
			self.superuser
		}
	}

	#[rstest]
	fn from_user_extracts_all_fields() {
		// Arrange
		let user = StubUser {
			id: "abc-123".into(),
			staff: true,
			superuser: false,
		};

		// Act
		let identity = SessionIdentity::from_user(&user);

		// Assert
		assert_eq!(
			identity,
			SessionIdentity {
				user_id: "abc-123".into(),
				is_staff: true,
				is_superuser: false,
			}
		);
	}

	#[rstest]
	fn from_user_default_flags() {
		let user = StubUser {
			id: "u1".into(),
			staff: false,
			superuser: false,
		};
		let identity = SessionIdentity::from_user(&user);
		assert!(!identity.is_staff);
		assert!(!identity.is_superuser);
	}

	#[cfg(native)]
	mod session_data_tests {
		use std::time::Duration;

		use serde_json::json;

		use super::*;

		#[rstest]
		fn to_session_data_sets_correct_keys() {
			// Arrange
			let identity = SessionIdentity {
				user_id: "user-42".into(),
				is_staff: true,
				is_superuser: false,
			};

			// Act
			let data = identity.to_session_data("sess-001", Duration::from_secs(1800));

			// Assert
			assert_eq!(data.id, "sess-001");
			assert_eq!(data.data.get("user_id").unwrap(), &json!("user-42"));
			assert_eq!(data.data.get("is_staff").unwrap(), &json!(true));
			assert_eq!(data.data.get("is_superuser").unwrap(), &json!(false));
			assert!(data.expires_at > data.created_at);
		}
	}
}
