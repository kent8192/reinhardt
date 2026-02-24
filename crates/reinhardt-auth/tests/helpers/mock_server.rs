//! Mock OAuth2/OIDC server for testing

use bytes::Bytes;
use http_body_util::Full;
use hyper::{Method, StatusCode};
use hyper::{Request, Response, body::Incoming};
use hyper_util::rt::TokioIo;
use reinhardt_auth::social::core::{
	claims::{IdToken, StandardClaims},
	token::TokenResponse,
};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use tokio::net::TcpListener;

/// Mock server configuration
#[derive(Clone)]
pub struct MockConfig {
	pub authorization_endpoint: String,
	pub token_endpoint: String,
	pub userinfo_endpoint: Option<String>,
	pub jwks_endpoint: Option<String>,
	pub discovery_endpoint: Option<String>,
	pub redirect_uri: String,
}

impl MockConfig {
	/// Creates a new configuration derived from the server's base URL
	fn from_base_url(base_url: &str) -> Self {
		Self {
			authorization_endpoint: format!("{}/authorize", base_url),
			token_endpoint: format!("{}/token", base_url),
			userinfo_endpoint: Some(format!("{}/userinfo", base_url)),
			jwks_endpoint: Some(format!("{}/jwks", base_url)),
			discovery_endpoint: Some(format!("{}/.well-known/openid-configuration", base_url)),
			redirect_uri: "http://localhost:8080/callback".into(),
		}
	}
}

/// Error simulation mode
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ErrorMode {
	Success,
	NetworkError,
	InvalidResponse,
	Unauthorized,
	ServerError,
}

/// Mock OAuth2/OIDC server state
#[derive(Clone)]
struct MockServerState {
	config: MockConfig,
	error_mode: ErrorMode,
	auth_code: Option<String>,
	token_response: Option<TokenResponse>,
	userinfo_response: Option<StandardClaims>,
	id_token: Option<IdToken>,
	discovery_response: Option<String>,
	jwks_response: Option<String>,
	oidc_enabled: bool,
	userinfo_enabled: bool,
}

/// Mock OAuth2/OIDC server for testing
pub struct MockOAuth2Server {
	state: Arc<Mutex<MockServerState>>,
	local_addr: SocketAddr,
}

impl MockOAuth2Server {
	/// Create a new mock server
	pub async fn new() -> Self {
		// Start the server first to get the actual address
		let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
		let local_addr = listener.local_addr().unwrap();
		let base_url = format!("http://{}", local_addr);

		let state = Arc::new(Mutex::new(MockServerState {
			config: MockConfig::from_base_url(&base_url),
			error_mode: ErrorMode::Success,
			auth_code: None,
			token_response: None,
			userinfo_response: None,
			id_token: None,
			discovery_response: None,
			jwks_response: None,
			oidc_enabled: true,
			userinfo_enabled: true,
		}));

		let state_clone = state.clone();
		tokio::spawn(async move {
			let state = state_clone;
			loop {
				if let Ok((stream, _)) = listener.accept().await {
					let io = TokioIo::new(stream);
					let state = state.clone();

					tokio::spawn(async move {
						let mut service =
							hyper::service::service_fn(move |req: Request<Incoming>| {
								let state = state.clone();
								async move { handle_request(req, state).await }
							});

						let _ = hyper::server::conn::http1::Builder::new()
							.serve_connection(io, &mut service)
							.await;
					});
				}
			}
		});

		// Wait for server to start
		tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

		Self { state, local_addr }
	}

	/// Enable OIDC endpoints (discovery, JWKS)
	pub fn with_oidc(self) -> Self {
		{
			let mut state = self.state.lock().unwrap();
			state.oidc_enabled = true;
		}
		self
	}

	/// Disable OIDC endpoints (discovery, JWKS)
	pub fn without_oidc(self) -> Self {
		{
			let mut state = self.state.lock().unwrap();
			state.oidc_enabled = false;
		}
		self
	}

	/// Enable UserInfo endpoint
	pub fn with_userinfo(self) -> Self {
		{
			let mut state = self.state.lock().unwrap();
			state.userinfo_enabled = true;
		}
		self
	}

	/// Disable UserInfo endpoint
	pub fn without_userinfo(self) -> Self {
		{
			let mut state = self.state.lock().unwrap();
			state.userinfo_enabled = false;
		}
		self
	}

	/// Set authorization response code
	pub fn set_auth_response(&mut self, code: &str) {
		let mut state = self.state.lock().unwrap();
		state.auth_code = Some(code.to_string());
	}

	/// Set token response
	pub fn set_token_response(&mut self, response: TokenResponse) {
		let mut state = self.state.lock().unwrap();
		state.token_response = Some(response);
	}

	/// Set userinfo response
	pub fn set_userinfo_response(&mut self, claims: StandardClaims) {
		let mut state = self.state.lock().unwrap();
		state.userinfo_response = Some(claims);
	}

	/// Set discovery document (OIDC)
	pub fn set_discovery_response(&mut self, discovery: &str) {
		let mut state = self.state.lock().unwrap();
		state.discovery_response = Some(discovery.to_string());
	}

	/// Set JWKS response (OIDC)
	pub fn set_jwks_response(&mut self, jwks: &str) {
		let mut state = self.state.lock().unwrap();
		state.jwks_response = Some(jwks.to_string());
	}

	/// Set error mode
	pub fn set_error_mode(&mut self, mode: ErrorMode) {
		let mut state = self.state.lock().unwrap();
		state.error_mode = mode;
	}

	/// Get the base URL for this server
	pub fn base_url(&self) -> String {
		format!("http://{}", self.local_addr)
	}

	/// Get authorization URL
	pub fn authorization_url(&self) -> String {
		format!("http://{}/authorize", self.local_addr)
	}

	/// Get token URL
	pub fn token_url(&self) -> String {
		format!("http://{}/token", self.local_addr)
	}

	/// Get userinfo URL
	pub fn userinfo_url(&self) -> Option<String> {
		Some(format!("http://{}/userinfo", self.local_addr))
	}

	/// Get JWKS URL
	pub fn jwks_url(&self) -> Option<String> {
		Some(format!("http://{}/jwks", self.local_addr))
	}

	/// Get discovery URL
	pub fn discovery_url(&self) -> Option<String> {
		Some(format!(
			"http://{}/.well-known/openid-configuration",
			self.local_addr
		))
	}

	/// Get the server port
	pub fn port(&self) -> u16 {
		self.local_addr.port()
	}
}

/// Handle incoming requests
async fn handle_request(
	req: Request<Incoming>,
	state: Arc<Mutex<MockServerState>>,
) -> Result<Response<Full<Bytes>>, hyper::Error> {
	let path = req.uri().path();
	let method = req.method();
	let state_guard = state.lock().unwrap();
	let error_mode = state_guard.error_mode;

	// Simulate error mode
	match error_mode {
		ErrorMode::NetworkError => {
			// Return an error response to simulate network issues
			return Ok(Response::builder()
				.status(StatusCode::SERVICE_UNAVAILABLE)
				.body(Full::default())
				.unwrap());
		}
		ErrorMode::InvalidResponse => {
			// Return 200 OK with malformed JSON body
			return Ok(Response::builder()
				.status(StatusCode::OK)
				.header("Content-Type", "application/json")
				.body(Full::from(Bytes::from("{invalid json!!! not valid")))
				.unwrap());
		}
		ErrorMode::Unauthorized => {
			return Ok(Response::builder()
				.status(StatusCode::UNAUTHORIZED)
				.body(Full::default())
				.unwrap());
		}
		ErrorMode::ServerError => {
			return Ok(Response::builder()
				.status(StatusCode::INTERNAL_SERVER_ERROR)
				.body(Full::default())
				.unwrap());
		}
		ErrorMode::Success => {}
	}

	match (method, path) {
		// Authorization endpoint
		(&Method::GET, "/authorize") => {
			let auth_code = state_guard
				.auth_code
				.clone()
				.unwrap_or_else(|| "test_code".to_string());
			let redirect_uri = format!(
				"{}?code={}&state=test_state",
				state_guard.config.redirect_uri, auth_code
			);

			// Return 302 redirect
			Ok(Response::builder()
				.status(StatusCode::FOUND)
				.header("Location", redirect_uri)
				.body(Full::default())
				.unwrap())
		}

		// Token endpoint
		(&Method::POST, "/token") => {
			let token_response =
				state_guard
					.token_response
					.clone()
					.unwrap_or_else(|| TokenResponse {
						access_token: "test_access_token".to_string(),
						token_type: "Bearer".to_string(),
						expires_in: Some(3600),
						refresh_token: Some("test_refresh_token".to_string()),
						scope: Some("openid email profile".to_string()),
						id_token: None,
					});

			let json = serde_json::to_string(&token_response).unwrap();
			Ok(Response::builder()
				.status(StatusCode::OK)
				.header("Content-Type", "application/json")
				.body(Full::from(Bytes::from(json)))
				.unwrap())
		}

		// UserInfo endpoint
		(&Method::GET, "/userinfo") => {
			if !state_guard.userinfo_enabled {
				return Ok(Response::builder()
					.status(StatusCode::NOT_FOUND)
					.body(Full::default())
					.unwrap());
			}

			let userinfo =
				state_guard
					.userinfo_response
					.clone()
					.unwrap_or_else(|| StandardClaims {
						sub: "test_user".to_string(),
						email: Some("test@example.com".to_string()),
						email_verified: Some(true),
						name: Some("Test User".to_string()),
						given_name: Some("Test".to_string()),
						family_name: Some("User".to_string()),
						picture: None,
						locale: None,
						additional_claims: HashMap::new(),
					});

			let json = serde_json::to_string(&userinfo).unwrap();
			Ok(Response::builder()
				.status(StatusCode::OK)
				.header("Content-Type", "application/json")
				.body(Full::from(Bytes::from(json)))
				.unwrap())
		}

		// JWKS endpoint
		(&Method::GET, "/jwks") => {
			if !state_guard.oidc_enabled {
				return Ok(Response::builder()
					.status(StatusCode::NOT_FOUND)
					.body(Full::default())
					.unwrap());
			}

			let jwks = state_guard.jwks_response.clone().unwrap_or_else(|| {
				r#"{"keys":[{"kty":"RSA","kid":"test_key_id","use":"sig","alg":"RS256","n":"test_modulus","e":"AQAB"}]}"#.to_string()
			});

			Ok(Response::builder()
				.status(StatusCode::OK)
				.header("Content-Type", "application/json")
				.body(Full::from(Bytes::from(jwks)))
				.unwrap())
		}

		// Discovery endpoint
		(&Method::GET, "/.well-known/openid-configuration") => {
			if !state_guard.oidc_enabled {
				return Ok(Response::builder()
					.status(StatusCode::NOT_FOUND)
					.body(Full::default())
					.unwrap());
			}

			let port = state_guard
				.config
				.authorization_endpoint
				.rsplit(':')
				.next()
				.and_then(|s| s.split('/').next())
				.unwrap_or("0");

			let discovery = state_guard.discovery_response.clone().unwrap_or_else(|| {
				format!(
					r#"{{
					"issuer": "http://127.0.0.1:{}",
					"authorization_endpoint": "http://127.0.0.1:{}/authorize",
					"token_endpoint": "http://127.0.0.1:{}/token",
					"jwks_uri": "http://127.0.0.1:{}/jwks",
					"userinfo_endpoint": "http://127.0.0.1:{}/userinfo",
					"scopes_supported": ["openid","email","profile"],
					"response_types_supported": ["code"],
					"grant_types_supported": ["authorization_code"],
					"subject_types_supported": ["public"],
					"id_token_signing_alg_values_supported": ["RS256"]
				}}"#,
					port, port, port, port, port
				)
			});

			Ok(Response::builder()
				.status(StatusCode::OK)
				.header("Content-Type", "application/json")
				.body(Full::from(Bytes::from(discovery)))
				.unwrap())
		}

		_ => Ok(Response::builder()
			.status(StatusCode::NOT_FOUND)
			.body(Full::default())
			.unwrap()),
	}
}
