//! Authentication views demonstrating JWT, Token, Session, and Basic auth
//!
//! Endpoints:
//! - POST /api/auth/register - Register a new user
//! - POST /api/auth/login - Login and receive JWT token
//! - POST /api/auth/token - Generate an API token (requires JWT auth)
//! - GET /api/auth/me - Get current user profile (requires authentication)
//! - POST /api/auth/logout - Logout (session invalidation)

use chrono::Utc;
use reinhardt::{BaseUser, JwtAuth, Request, Response, StatusCode, ViewResult, get, post};
use uuid::Uuid;

use crate::apps::users::models::UserStorage;
use crate::apps::users::serializers::{
	AuthResponse, LoginRequest, LogoutResponse, RegisterRequest, TokenResponse, UserResponse,
};
use crate::config::settings::jwt_secret;

/// Helper to extract UserStorage from request extensions
fn get_storage(req: &Request) -> Result<UserStorage, String> {
	req.extensions
		.get::<UserStorage>()
		.ok_or_else(|| "UserStorage not found in request extensions".to_string())
}

/// Helper to create JwtAuth instance
fn create_jwt_auth() -> JwtAuth {
	JwtAuth::new(&jwt_secret())
}

/// Register a new user
///
/// Accepts JSON body with username, email, password, and optional name fields.
/// Returns a JWT token and user profile on success.
#[post("/api/auth/register", name = "auth_register")]
pub async fn register(req: Request) -> ViewResult<Response> {
	let storage = get_storage(&req)?;

	let body: RegisterRequest = req
		.json()
		.map_err(|e| format!("Invalid request body: {}", e))?;

	// Check if username already exists
	if storage.find_by_username(&body.username).await.is_some() {
		let error = serde_json::json!({"error": "Username already taken"});
		return Ok(Response::new(StatusCode::CONFLICT)
			.with_header("Content-Type", "application/json")
			.with_body(serde_json::to_string(&error).unwrap_or_default()));
	}

	// Create new user
	let mut user = crate::apps::users::models::AppUser {
		id: Uuid::new_v4(),
		username: body.username,
		email: body.email,
		first_name: body.first_name.unwrap_or_default(),
		last_name: body.last_name.unwrap_or_default(),
		date_joined: Utc::now(),
		is_active: true,
		..Default::default()
	};

	// Hash password using BaseUser trait method (Argon2)
	user.set_password(&body.password)
		.map_err(|e| format!("Failed to hash password: {}", e))?;

	// Generate JWT token
	let jwt_auth = create_jwt_auth();
	let token = jwt_auth
		.generate_token(user.id.to_string(), user.username.clone())
		.map_err(|e| format!("Failed to generate token: {}", e))?;

	let response = AuthResponse {
		token,
		user: UserResponse::from(&user),
	};

	storage.add_user(user).await;

	let json = serde_json::to_string(&response)
		.map_err(|e| format!("Serialization error: {}", e))?;

	Ok(Response::new(StatusCode::CREATED)
		.with_header("Content-Type", "application/json")
		.with_body(json))
}

/// Login with username and password
///
/// Verifies credentials and returns a JWT token.
#[post("/api/auth/login", name = "auth_login")]
pub async fn login(req: Request) -> ViewResult<Response> {
	let storage = get_storage(&req)?;

	let body: LoginRequest = req
		.json()
		.map_err(|e| format!("Invalid request body: {}", e))?;

	// Find user by username
	let user = storage
		.find_by_username(&body.username)
		.await
		.ok_or("Invalid credentials")?;

	// Verify password using BaseUser trait method
	let valid = user
		.check_password(&body.password)
		.map_err(|e| format!("Password verification error: {}", e))?;

	if !valid {
		let error = serde_json::json!({"error": "Invalid credentials"});
		return Ok(Response::new(StatusCode::UNAUTHORIZED)
			.with_header("Content-Type", "application/json")
			.with_body(serde_json::to_string(&error).unwrap_or_default()));
	}

	if !user.is_active {
		let error = serde_json::json!({"error": "Account is inactive"});
		return Ok(Response::new(StatusCode::FORBIDDEN)
			.with_header("Content-Type", "application/json")
			.with_body(serde_json::to_string(&error).unwrap_or_default()));
	}

	// Generate JWT token
	let jwt_auth = create_jwt_auth();
	let token = jwt_auth
		.generate_token(user.id.to_string(), user.username.clone())
		.map_err(|e| format!("Failed to generate token: {}", e))?;

	let response = AuthResponse {
		token,
		user: UserResponse::from(&user),
	};

	let json = serde_json::to_string(&response)
		.map_err(|e| format!("Serialization error: {}", e))?;

	Ok(Response::new(StatusCode::OK)
		.with_header("Content-Type", "application/json")
		.with_body(json))
}

/// Generate an API token for the authenticated user
///
/// Requires JWT authentication. Returns a persistent API token
/// that can be used with Token authentication (Authorization: Token <token>).
#[post("/api/auth/token", name = "auth_generate_token")]
pub async fn generate_token(req: Request) -> ViewResult<Response> {
	let storage = get_storage(&req)?;

	// Extract and verify JWT from Authorization header
	let user = extract_authenticated_user(&req, &storage).await?;

	// Generate a random API token
	let api_token = Uuid::new_v4().to_string();

	// Store the token
	storage
		.store_api_token(api_token.clone(), user.id)
		.await;

	let response = TokenResponse { api_token };

	let json = serde_json::to_string(&response)
		.map_err(|e| format!("Serialization error: {}", e))?;

	Ok(Response::new(StatusCode::CREATED)
		.with_header("Content-Type", "application/json")
		.with_body(json))
}

/// Get current user profile
///
/// Demonstrates IsAuthenticated-style permission checking.
/// Accepts JWT Bearer token, API Token, or Basic auth.
#[get("/api/auth/me", name = "auth_me")]
pub async fn me(req: Request) -> ViewResult<Response> {
	let storage = get_storage(&req)?;

	// Try multiple auth methods (composite authentication pattern)
	let user = extract_authenticated_user(&req, &storage).await?;

	let response = UserResponse::from(&user);

	let json = serde_json::to_string(&response)
		.map_err(|e| format!("Serialization error: {}", e))?;

	Ok(Response::new(StatusCode::OK)
		.with_header("Content-Type", "application/json")
		.with_body(json))
}

/// Logout endpoint
///
/// For JWT-based auth, the client simply discards the token.
/// This endpoint confirms the logout action.
#[post("/api/auth/logout", name = "auth_logout")]
pub async fn logout(_req: Request) -> ViewResult<Response> {
	// For stateless JWT auth, logout is handled client-side by discarding the token.
	// For session-based auth, you would invalidate the session here.
	let response = LogoutResponse {
		message: "Successfully logged out".to_string(),
	};

	let json = serde_json::to_string(&response)
		.map_err(|e| format!("Serialization error: {}", e))?;

	Ok(Response::new(StatusCode::OK)
		.with_header("Content-Type", "application/json")
		.with_body(json))
}

/// Extract authenticated user from request using composite authentication
///
/// Public so other app modules can reuse this authentication logic.
///
/// Tries authentication methods in order:
/// 1. JWT Bearer token
/// 2. API Token (Authorization: Token <token>)
/// 3. Basic authentication
pub async fn extract_authenticated_user(
	req: &Request,
	storage: &UserStorage,
) -> Result<crate::apps::users::models::AppUser, String> {
	let auth_header = req
		.headers
		.get("authorization")
		.and_then(|h| h.to_str().ok());

	let Some(header) = auth_header else {
		return Err("Authentication required".to_string());
	};

	// Try JWT Bearer token
	if let Some(token) = header.strip_prefix("Bearer ") {
		let jwt_auth = create_jwt_auth();
		let claims = jwt_auth
			.verify_token(token)
			.map_err(|_| "Invalid or expired JWT token".to_string())?;

		let user_id = Uuid::parse_str(&claims.sub)
			.map_err(|_| "Invalid user ID in token".to_string())?;

		return storage
			.get_user(&user_id)
			.await
			.ok_or_else(|| "User not found".to_string());
	}

	// Try API Token
	if let Some(token) = header.strip_prefix("Token ") {
		return storage
			.get_user_by_token(token)
			.await
			.ok_or_else(|| "Invalid API token".to_string());
	}

	// Try Basic authentication
	if let Some(credentials) = header.strip_prefix("Basic ") {
		use base64::{Engine, engine::general_purpose::STANDARD};

		let decoded = STANDARD
			.decode(credentials)
			.map_err(|_| "Invalid Basic auth encoding".to_string())?;
		let credential_str =
			String::from_utf8(decoded).map_err(|_| "Invalid UTF-8 in credentials".to_string())?;

		let (username, password) = credential_str
			.split_once(':')
			.ok_or_else(|| "Invalid Basic auth format".to_string())?;

		let user = storage
			.find_by_username(username)
			.await
			.ok_or_else(|| "Invalid credentials".to_string())?;

		let valid = user
			.check_password(password)
			.map_err(|e| format!("Password verification error: {}", e))?;

		if !valid {
			return Err("Invalid credentials".to_string());
		}

		return Ok(user);
	}

	Err("Unsupported authentication method".to_string())
}
