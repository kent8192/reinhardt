use thiserror::Error;

/// Errors that can occur during test authentication setup.
#[derive(Debug, Error)]
pub enum TestAuthError {
	/// Failed to create or save a session.
	#[error("session backend error: {0}")]
	SessionError(String),
	/// Failed to sign or create a JWT.
	#[error("JWT signing error: {0}")]
	JwtError(String),
	/// A secondary auth layer (e.g., MFA) failed.
	#[error("secondary auth error: {0}")]
	SecondaryAuthError(String),
	/// No primary auth method was configured before calling apply().
	#[error("no primary auth configured")]
	NoPrimaryAuth,
	/// The user is not registered with the MFA manager.
	#[error("MFA user not registered: {0}")]
	MfaUserNotRegistered(String),
	/// An error occurred on the APIClient (e.g., setting a header or cookie).
	#[error("client error: {0}")]
	ClientError(String),
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::*;

	#[rstest]
	#[case(TestAuthError::SessionError("timeout".into()), "session backend error: timeout")]
	#[case(TestAuthError::JwtError("bad key".into()), "JWT signing error: bad key")]
	#[case(TestAuthError::NoPrimaryAuth, "no primary auth configured")]
	#[case(TestAuthError::MfaUserNotRegistered("alice".into()), "MFA user not registered: alice")]
	#[case(TestAuthError::ClientError("header fail".into()), "client error: header fail")]
	fn display_formats(#[case] error: TestAuthError, #[case] expected: &str) {
		assert_eq!(error.to_string(), expected);
	}
}
