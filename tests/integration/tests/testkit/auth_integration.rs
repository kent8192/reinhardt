use async_trait::async_trait;
use http::{HeaderMap, StatusCode};
use reinhardt_auth::jwt::JwtAuth;
use reinhardt_http::{Handler, Request, Response};
use reinhardt_middleware::session::{AsyncSessionBackend, SessionData};
use reinhardt_test::APIClient;
use reinhardt_test::auth::{
	ForceLoginUser, JwtTestConfig, SecondaryAuth, SessionIdentity, TestAuthError,
};
use rstest::rstest;
use serde_json::json;
use std::sync::{Arc, Mutex};
use std::time::Duration;

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

struct RecordingHandler {
	captured: Arc<Mutex<Vec<HeaderMap>>>,
}

#[async_trait]
impl Handler for RecordingHandler {
	async fn handle(&self, request: Request) -> reinhardt_http::Result<Response> {
		self.captured.lock().unwrap().push(request.headers.clone());
		Ok(Response::no_content())
	}
}

fn recording_client(captured: Arc<Mutex<Vec<HeaderMap>>>) -> APIClient {
	APIClient::from_handler(RecordingHandler { captured })
}

#[rstest]
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
	let user = FixtureUser;

	// Act
	session_client
		.auth()
		.session(&user, backend)
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
		.jwt(&user, config.clone())
		.with_secondary(HeaderSecondary)
		.apply()
		.await
		.unwrap();
	let jwt_response = jwt_client.get("/jwt-check").await.unwrap();
	let failed = APIClient::new()
		.auth()
		.session(
			&user,
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
	assert!(!claims.is_staff);
	assert!(!claims.is_superuser);
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
