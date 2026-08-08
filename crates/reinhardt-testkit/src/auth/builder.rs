use std::sync::Arc;
use std::time::Duration;

use uuid::Uuid;

use super::error::TestAuthError;
use super::identity::SessionIdentity;
use super::secondary::SecondaryAuth;
use super::secondary::TotpSecondaryAuth;
use super::traits::ForceLoginUser;
use crate::client::APIClient;

/// Entry point for building auth configurations on an [`APIClient`].
///
/// Returned by [`APIClient::auth()`].
pub struct AuthBuilder<'a> {
	client: &'a APIClient,
}

impl<'a> AuthBuilder<'a> {
	/// Create a new `AuthBuilder` for the given client.
	pub(crate) fn new(client: &'a APIClient) -> Self {
		Self { client }
	}

	/// Configure session-based authentication.
	///
	/// Creates a real session in the `AsyncSessionBackend` and sets the
	/// `sessionid` cookie on the client. The session will be validated by
	/// `CookieSessionAuthMiddleware` on subsequent requests.
	pub fn session(
		self,
		user: &impl ForceLoginUser,
		backend: Arc<dyn reinhardt_middleware::session::AsyncSessionBackend>,
	) -> SessionAuthBuilder<'a> {
		SessionAuthBuilder {
			client: self.client,
			identity: SessionIdentity::from_user(user),
			backend,
			ttl: Duration::from_secs(30 * 60),
			secondary: vec![],
		}
	}

	/// Configure JWT Bearer token authentication.
	pub fn jwt(self, user: &impl ForceLoginUser, config: JwtTestConfig) -> JwtAuthBuilder<'a> {
		JwtAuthBuilder {
			client: self.client,
			identity: SessionIdentity::from_user(user),
			config,
			secondary: vec![],
		}
	}
}

/// Builder for session-based test authentication.
///
/// Created by [`AuthBuilder::session()`].
pub struct SessionAuthBuilder<'a> {
	client: &'a APIClient,
	identity: SessionIdentity,
	backend: Arc<dyn reinhardt_middleware::session::AsyncSessionBackend>,
	ttl: Duration,
	secondary: Vec<Box<dyn SecondaryAuth>>,
}

impl<'a> SessionAuthBuilder<'a> {
	/// Override the `is_staff` flag.
	///
	/// Use this when the user's `ForceLoginUser` impl defaults `is_staff` to `false`
	/// but the test requires staff access.
	pub fn with_staff(mut self, is_staff: bool) -> Self {
		self.identity.is_staff = is_staff;
		self
	}

	/// Override the `is_superuser` flag.
	pub fn with_superuser(mut self, is_superuser: bool) -> Self {
		self.identity.is_superuser = is_superuser;
		self
	}

	/// Set the session TTL. Default: 30 minutes.
	pub fn with_ttl(mut self, ttl: Duration) -> Self {
		self.ttl = ttl;
		self
	}

	/// Add TOTP MFA as a secondary auth layer.
	///
	/// Uses a pre-generated TOTP code. Generate it from the user's registered
	/// secret before calling this method.
	pub fn with_mfa_code(self, code: impl Into<String>) -> Self {
		self.with_secondary(TotpSecondaryAuth::with_code_only(code))
	}

	/// Add a custom secondary auth layer.
	pub fn with_secondary(mut self, auth: impl SecondaryAuth + 'static) -> Self {
		self.secondary.push(Box::new(auth));
		self
	}

	/// Apply the authentication configuration to the client.
	///
	/// This creates a real session in the backend, sets the `sessionid` cookie,
	/// and applies any secondary auth layers.
	pub async fn apply(self) -> Result<(), TestAuthError> {
		// 1. Generate session ID
		let session_id = Uuid::now_v7().to_string();

		// 2. Build SessionData from identity
		let session_data = self.identity.to_session_data(&session_id, self.ttl);

		// 3. Save to AsyncSessionBackend
		self.backend
			.save(&session_data)
			.await
			.map_err(|e| TestAuthError::SessionError(e.to_string()))?;

		// 4. Set sessionid cookie on client
		self.client
			.set_cookie("sessionid", &session_id)
			.await
			.map_err(|e| TestAuthError::ClientError(e.to_string()))?;

		// 5. Apply secondary auth layers
		for secondary in &self.secondary {
			secondary
				.apply_to_client(self.client, &self.identity)
				.await?;
		}

		Ok(())
	}
}

/// Builder for JWT test authentication.
///
/// Created by [`AuthBuilder::jwt()`].
pub struct JwtAuthBuilder<'a> {
	client: &'a APIClient,
	identity: SessionIdentity,
	config: JwtTestConfig,
	secondary: Vec<Box<dyn SecondaryAuth>>,
}

impl<'a> JwtAuthBuilder<'a> {
	/// Add a custom secondary auth layer.
	pub fn with_secondary(mut self, auth: impl SecondaryAuth + 'static) -> Self {
		self.secondary.push(Box::new(auth));
		self
	}

	/// Apply JWT authentication to the client.
	///
	/// Signs a JWT with the configured secret and sets the `Authorization: Bearer` header.
	pub async fn apply(self) -> Result<(), TestAuthError> {
		use reinhardt_auth::jwt::{Claims, JwtAuth};

		// Build claims
		let claims = Claims::new(
			self.identity.user_id.clone(),
			self.identity.user_id.clone(),
			chrono::Duration::seconds(self.config.expiry.as_secs() as i64),
			self.identity.is_staff,
			self.identity.is_superuser,
		);

		// Sign JWT
		let jwt_auth = JwtAuth::new(self.config.secret.as_bytes());
		let token = jwt_auth
			.encode(&claims)
			.map_err(|e| TestAuthError::JwtError(e.to_string()))?;

		// Set Authorization header
		self.client
			.set_header("Authorization", &format!("Bearer {token}"))
			.await
			.map_err(|e| TestAuthError::ClientError(e.to_string()))?;

		// Apply secondary auth layers
		for secondary in &self.secondary {
			secondary
				.apply_to_client(self.client, &self.identity)
				.await?;
		}

		Ok(())
	}
}

/// JWT configuration for test contexts.
#[derive(Clone, Debug)]
pub struct JwtTestConfig {
	/// Secret key for signing JWTs.
	pub secret: String,
	/// Token expiry duration. Default: 1 hour.
	pub expiry: Duration,
}

impl Default for JwtTestConfig {
	fn default() -> Self {
		Self {
			secret: "test-secret-key-for-testing-only".into(),
			expiry: Duration::from_secs(3600),
		}
	}
}

#[cfg(test)]
mod tests {
	use std::sync::{Arc, Mutex};
	use std::time::Duration;

	use async_trait::async_trait;
	use bytes::Bytes;
	use http::{Request, Response, StatusCode};
	use http_body_util::Full;
	use reinhardt_auth::jwt::JwtAuth;
	use reinhardt_middleware::session::{AsyncSessionBackend, SessionData};
	use serde_json::json;

	use super::*;

	#[derive(Clone)]
	struct FixtureUser;

	impl ForceLoginUser for FixtureUser {
		fn session_user_id(&self) -> String {
			"user-42".into()
		}

		fn session_is_staff(&self) -> bool {
			false
		}
	}

	struct RecordingBackend {
		saved: Arc<Mutex<Vec<SessionData>>>,
		failure: Option<&'static str>,
	}

	#[async_trait]
	impl AsyncSessionBackend for RecordingBackend {
		async fn load(&self, _id: &str) -> reinhardt_http::Result<Option<SessionData>> {
			Ok(None)
		}

		async fn save(&self, session: &SessionData) -> reinhardt_http::Result<()> {
			if let Some(message) = self.failure {
				return Err(reinhardt_http::Error::Internal(message.into()));
			}
			self.saved.lock().unwrap().push(session.clone());
			Ok(())
		}

		async fn destroy(&self, _id: &str) -> reinhardt_http::Result<()> {
			Ok(())
		}

		async fn touch(&self, _id: &str, _ttl: Duration) -> reinhardt_http::Result<()> {
			Ok(())
		}
	}

	struct HeaderSecondary;

	#[async_trait]
	impl SecondaryAuth for HeaderSecondary {
		async fn apply_to_client(
			&self,
			client: &APIClient,
			primary: &SessionIdentity,
		) -> Result<(), TestAuthError> {
			client
				.set_header("X-Secondary-User", &primary.user_id)
				.await
				.map_err(|error| TestAuthError::ClientError(error.to_string()))
		}
	}

	fn recording_client(captured: Arc<Mutex<Vec<http::HeaderMap>>>) -> APIClient {
		let mut client = APIClient::new();
		client.set_handler(move |request: Request<Full<Bytes>>| {
			captured.lock().unwrap().push(request.headers().clone());
			Response::builder()
				.status(StatusCode::NO_CONTENT)
				.body(Full::new(Bytes::new()))
				.unwrap()
		});
		client
	}

	#[tokio::test]
	async fn api_client_auth_builders_apply_session_jwt_and_secondary_auth() {
		// Arrange
		let saved = Arc::new(Mutex::new(Vec::new()));
		let session_headers = Arc::new(Mutex::new(Vec::new()));
		let jwt_headers = Arc::new(Mutex::new(Vec::new()));
		let session_client = recording_client(Arc::clone(&session_headers));
		let jwt_client = recording_client(Arc::clone(&jwt_headers));
		let backend = Arc::new(RecordingBackend {
			saved: Arc::clone(&saved),
			failure: None,
		});
		let config = JwtTestConfig {
			secret: "coverage-test-secret".into(),
			expiry: Duration::from_secs(120),
		};

		// Act
		session_client
			.auth()
			.session(&FixtureUser, backend)
			.with_staff(true)
			.with_superuser(true)
			.with_ttl(Duration::from_secs(90))
			.with_secondary(HeaderSecondary)
			.apply()
			.await
			.unwrap();
		let session_response = session_client.get("/session-check").await.unwrap();
		jwt_client
			.auth()
			.jwt(&FixtureUser, config.clone())
			.with_secondary(HeaderSecondary)
			.apply()
			.await
			.unwrap();
		let jwt_response = jwt_client.get("/jwt-check").await.unwrap();
		let failed = APIClient::new()
			.auth()
			.session(
				&FixtureUser,
				Arc::new(RecordingBackend {
					saved: Arc::new(Mutex::new(Vec::new())),
					failure: Some("backend unavailable"),
				}),
			)
			.apply()
			.await
			.unwrap_err();

		// Assert
		let saved_session = saved.lock().unwrap().pop().unwrap();
		assert_eq!(saved_session.data.get("user_id"), Some(&json!("user-42")));
		assert_eq!(saved_session.data.get("is_staff"), Some(&json!(true)));
		assert_eq!(saved_session.data.get("is_superuser"), Some(&json!(true)));
		assert_eq!(
			saved_session
				.expires_at
				.duration_since(saved_session.created_at)
				.unwrap(),
			Duration::from_secs(90)
		);
		assert_eq!(session_response.status(), StatusCode::NO_CONTENT);
		let session_request = session_headers.lock().unwrap().pop().unwrap();
		assert_eq!(
			session_request.get("cookie").unwrap().to_str().unwrap(),
			format!("sessionid={}", saved_session.id)
		);
		assert_eq!(
			session_request
				.get("x-secondary-user")
				.unwrap()
				.to_str()
				.unwrap(),
			"user-42"
		);
		assert_eq!(jwt_response.status(), StatusCode::NO_CONTENT);
		let jwt_request = jwt_headers.lock().unwrap().pop().unwrap();
		let authorization = jwt_request.get("authorization").unwrap().to_str().unwrap();
		let token = authorization.strip_prefix("Bearer ").unwrap();
		let claims = JwtAuth::new(config.secret.as_bytes())
			.decode(token)
			.unwrap();
		assert_eq!(claims.sub, "user-42");
		assert_eq!(claims.username, "user-42");
		assert_eq!(claims.is_staff, false);
		assert_eq!(claims.is_superuser, false);
		assert_eq!(claims.exp - claims.iat, 120);
		assert_eq!(
			jwt_request
				.get("x-secondary-user")
				.unwrap()
				.to_str()
				.unwrap(),
			"user-42"
		);
		assert_eq!(
			failed.to_string(),
			"session backend error: Internal server error: backend unavailable"
		);
	}
}
