//! OAuth2 Flows Integration Tests
//!
//! Comprehensive integration tests for OAuth2 authentication flows.
//! These tests verify the complete OAuth2 authorization code flow,
//! token management, and error handling.
//!
//! # Test Categories
//!
//! - Happy path: Complete authorization code flow, token exchange
//! - Error path: Invalid credentials, expired codes, unauthorized clients
//! - State transition: Token lifecycle, code consumption
//! - Edge cases: Multiple applications, scope handling
//! - Decision table: Various grant conditions
//! - Use case: SSO scenarios, multiple scopes

use reinhardt_auth::AuthenticationBackend;
use reinhardt_auth::oauth2::{
	AccessToken, AuthorizationCode, GrantType, InMemoryOAuth2Store, OAuth2Application,
	OAuth2Authentication, OAuth2TokenStore,
};
use rstest::*;
use std::sync::Arc;

// =============================================================================
// Test Fixtures
// =============================================================================

/// Creates a default OAuth2 authentication instance
#[fixture]
fn oauth2_auth() -> OAuth2Authentication {
	OAuth2Authentication::new()
}

/// Creates a standard test application
#[fixture]
fn test_application() -> OAuth2Application {
	OAuth2Application {
		client_id: "test_client".to_string(),
		client_secret: "test_secret_12345".to_string(),
		redirect_uris: vec![
			"https://example.com/callback".to_string(),
			"https://example.com/oauth/callback".to_string(),
		],
		grant_types: vec![GrantType::AuthorizationCode, GrantType::RefreshToken],
	}
}

/// Creates a minimal application for client credentials flow
#[fixture]
fn client_credentials_app() -> OAuth2Application {
	OAuth2Application {
		client_id: "service_client".to_string(),
		client_secret: "service_secret".to_string(),
		redirect_uris: vec![],
		grant_types: vec![GrantType::ClientCredentials],
	}
}

/// Creates an in-memory token store
#[fixture]
fn token_store() -> Arc<InMemoryOAuth2Store> {
	Arc::new(InMemoryOAuth2Store::new())
}

/// Creates an OAuth2 auth instance with registered test application
#[fixture]
fn oauth2_with_app(
	oauth2_auth: OAuth2Authentication,
	test_application: OAuth2Application,
) -> OAuth2Authentication {
	// Run async registration in a separate thread to avoid blocking the tokio test runtime.
	// rstest 0.23 does not properly handle async fixtures with sub-fixture parameters,
	// so we use a sync fixture with an isolated tokio runtime.
	std::thread::spawn(move || {
		tokio::runtime::Builder::new_current_thread()
			.enable_all()
			.build()
			.expect("failed to build runtime")
			.block_on(async move {
				oauth2_auth.register_application(test_application).await;
				oauth2_auth
			})
	})
	.join()
	.expect("thread panicked")
}

// =============================================================================
// Happy Path Tests
// =============================================================================

#[rstest]
#[tokio::test]
async fn test_complete_authorization_code_flow(oauth2_with_app: OAuth2Authentication) {
	// Step 1: Generate authorization code
	let code = oauth2_with_app
		.generate_authorization_code(
			"test_client",
			"https://example.com/callback",
			"user_123",
			Some("read write".to_string()),
		)
		.await
		.expect("Code generation should succeed");

	// Assert code format
	assert!(code.starts_with("code_"), "Code should have 'code_' prefix");
	assert!(code.len() > 10, "Code should be long enough to be secure");

	// Step 2: Exchange code for token
	let token = oauth2_with_app
		.exchange_code(&code, "test_client", "test_secret_12345")
		.await
		.expect("Token exchange should succeed");

	// Assert token properties
	assert_eq!(token.token_type, "Bearer", "Token type should be Bearer");
	assert!(
		token.token.starts_with("access_"),
		"Access token should have 'access_' prefix"
	);
	assert_eq!(token.expires_in, 3600, "Token should expire in 1 hour");
	assert!(
		token.refresh_token.is_some(),
		"Refresh token should be present"
	);
	assert_eq!(
		token.scope,
		Some("read write".to_string()),
		"Scope should be preserved"
	);
}

#[rstest]
#[tokio::test]
async fn test_token_store_operations(token_store: Arc<InMemoryOAuth2Store>) {
	// Store authorization code
	let code = AuthorizationCode {
		code: "test_code_12345".to_string(),
		client_id: "client_1".to_string(),
		redirect_uri: "https://example.com/callback".to_string(),
		user_id: "user_456".to_string(),
		scope: Some("read".to_string()),
		created_at: std::time::Instant::now(),
	};

	token_store
		.store_code(code)
		.await
		.expect("Code storage should succeed");

	// Consume code
	let retrieved = token_store
		.consume_code("test_code_12345")
		.await
		.expect("Code consumption should not error");

	assert!(retrieved.is_some(), "Code should be retrieved");
	let auth_code = retrieved.unwrap();
	assert_eq!(auth_code.user_id, "user_456");
	assert_eq!(auth_code.scope, Some("read".to_string()));
}

#[rstest]
#[tokio::test]
async fn test_access_token_storage_and_retrieval(token_store: Arc<InMemoryOAuth2Store>) {
	// Store access token
	let token = AccessToken {
		token: "access_token_xyz".to_string(),
		token_type: "Bearer".to_string(),
		expires_in: 3600,
		refresh_token: Some("refresh_abc".to_string()),
		scope: Some("read write".to_string()),
	};

	token_store
		.store_token("user_789", token)
		.await
		.expect("Token storage should succeed");

	// Retrieve token
	let user_id = token_store
		.get_token("access_token_xyz")
		.await
		.expect("Token retrieval should not error");

	assert_eq!(user_id, Some("user_789".to_string()));
}

#[rstest]
#[tokio::test]
async fn test_token_revocation(token_store: Arc<InMemoryOAuth2Store>) {
	// Store token
	let token = AccessToken {
		token: "token_to_revoke".to_string(),
		token_type: "Bearer".to_string(),
		expires_in: 3600,
		refresh_token: None,
		scope: None,
	};

	token_store.store_token("user_1", token).await.unwrap();

	// Verify token exists
	let exists = token_store.get_token("token_to_revoke").await.unwrap();
	assert!(exists.is_some(), "Token should exist before revocation");

	// Revoke token
	token_store
		.revoke_token("token_to_revoke")
		.await
		.expect("Revocation should succeed");

	// Verify token is gone
	let after_revoke = token_store.get_token("token_to_revoke").await.unwrap();
	assert!(after_revoke.is_none(), "Token should be revoked");
}

#[rstest]
#[tokio::test]
async fn test_client_validation(oauth2_with_app: OAuth2Authentication) {
	// Valid credentials
	assert!(
		oauth2_with_app
			.validate_client("test_client", "test_secret_12345")
			.await,
		"Valid credentials should be accepted"
	);

	// Invalid secret
	assert!(
		!oauth2_with_app
			.validate_client("test_client", "wrong_secret")
			.await,
		"Invalid secret should be rejected"
	);

	// Unknown client
	assert!(
		!oauth2_with_app
			.validate_client("unknown_client", "any_secret")
			.await,
		"Unknown client should be rejected"
	);
}

#[rstest]
#[tokio::test]
async fn test_get_user_via_default_repository(oauth2_auth: OAuth2Authentication) {
	// Get user via SimpleUserRepository
	let user = oauth2_auth
		.get_user("test_user_id")
		.await
		.expect("Get user should not error");

	assert!(user.is_some(), "User should be found");
	let user = user.unwrap();
	assert_eq!(user.get_username(), "test_user_id");
	assert!(user.is_active());
	assert!(user.is_authenticated());
}

// =============================================================================
// Error Path Tests
// =============================================================================

#[rstest]
#[tokio::test]
async fn test_exchange_code_with_invalid_credentials(oauth2_with_app: OAuth2Authentication) {
	// Generate valid code
	let code = oauth2_with_app
		.generate_authorization_code(
			"test_client",
			"https://example.com/callback",
			"user_123",
			None,
		)
		.await
		.unwrap();

	// Try to exchange with wrong secret
	let result = oauth2_with_app
		.exchange_code(&code, "test_client", "wrong_secret")
		.await;

	assert!(result.is_err(), "Exchange with wrong secret should fail");
	let error = result.unwrap_err();
	assert!(
		error.contains("Invalid client credentials"),
		"Error should mention invalid credentials, got: {}",
		error
	);
}

#[rstest]
#[tokio::test]
async fn test_exchange_code_with_unknown_client(oauth2_with_app: OAuth2Authentication) {
	// Generate valid code
	let code = oauth2_with_app
		.generate_authorization_code(
			"test_client",
			"https://example.com/callback",
			"user_123",
			None,
		)
		.await
		.unwrap();

	// Try to exchange with unknown client
	let result = oauth2_with_app
		.exchange_code(&code, "unknown_client", "any_secret")
		.await;

	assert!(result.is_err(), "Exchange with unknown client should fail");
}

#[rstest]
#[tokio::test]
async fn test_exchange_invalid_code(oauth2_with_app: OAuth2Authentication) {
	// Try to exchange non-existent code
	let result = oauth2_with_app
		.exchange_code("nonexistent_code", "test_client", "test_secret_12345")
		.await;

	assert!(result.is_err(), "Exchange with invalid code should fail");
	let error = result.unwrap_err();
	assert!(
		error.contains("Invalid or expired"),
		"Error should mention invalid/expired code, got: {}",
		error
	);
}

#[rstest]
#[tokio::test]
async fn test_code_reuse_prevention(oauth2_with_app: OAuth2Authentication) {
	// Generate code
	let code = oauth2_with_app
		.generate_authorization_code(
			"test_client",
			"https://example.com/callback",
			"user_123",
			None,
		)
		.await
		.unwrap();

	// First exchange should succeed
	let first_result = oauth2_with_app
		.exchange_code(&code, "test_client", "test_secret_12345")
		.await;
	assert!(first_result.is_ok(), "First exchange should succeed");

	// Second exchange should fail (code is consumed)
	let second_result = oauth2_with_app
		.exchange_code(&code, "test_client", "test_secret_12345")
		.await;
	assert!(second_result.is_err(), "Code reuse should be prevented");
}

#[rstest]
#[tokio::test]
async fn test_consume_nonexistent_code(token_store: Arc<InMemoryOAuth2Store>) {
	let result = token_store.consume_code("nonexistent_code").await;

	assert!(result.is_ok(), "Consume should not error");
	assert!(
		result.unwrap().is_none(),
		"Should return None for nonexistent code"
	);
}

#[rstest]
#[tokio::test]
async fn test_get_nonexistent_token(token_store: Arc<InMemoryOAuth2Store>) {
	let result = token_store.get_token("nonexistent_token").await;

	assert!(result.is_ok(), "Get should not error");
	assert!(
		result.unwrap().is_none(),
		"Should return None for nonexistent token"
	);
}

#[rstest]
#[tokio::test]
async fn test_revoke_nonexistent_token(token_store: Arc<InMemoryOAuth2Store>) {
	// Should not error when revoking non-existent token
	let result = token_store.revoke_token("nonexistent").await;
	assert!(
		result.is_ok(),
		"Revoking nonexistent token should not error"
	);
}

// =============================================================================
// State Transition Tests
// =============================================================================

#[rstest]
#[tokio::test]
async fn test_token_lifecycle_states(oauth2_with_app: OAuth2Authentication) {
	// State 1: Generate authorization code
	let code = oauth2_with_app
		.generate_authorization_code(
			"test_client",
			"https://example.com/callback",
			"user_lifecycle",
			Some("read".to_string()),
		)
		.await
		.unwrap();

	// State 2: Exchange for token
	let token = oauth2_with_app
		.exchange_code(&code, "test_client", "test_secret_12345")
		.await
		.unwrap();

	// State 3: Code is consumed (cannot reuse)
	let reuse_result = oauth2_with_app
		.exchange_code(&code, "test_client", "test_secret_12345")
		.await;
	assert!(reuse_result.is_err(), "Code should be consumed");

	// State 4: Token is valid (has proper format and fields)
	assert!(token.token.starts_with("access_"));
	assert!(token.refresh_token.is_some());
}

#[rstest]
#[tokio::test]
async fn test_application_registration_state(oauth2_auth: OAuth2Authentication) {
	// State 1: Unregistered client
	assert!(
		!oauth2_auth
			.validate_client("new_client", "new_secret")
			.await,
		"Unregistered client should not validate"
	);

	// Transition: Register application
	let app = OAuth2Application {
		client_id: "new_client".to_string(),
		client_secret: "new_secret".to_string(),
		redirect_uris: vec!["https://new.example.com/callback".to_string()],
		grant_types: vec![GrantType::AuthorizationCode],
	};
	oauth2_auth.register_application(app).await;

	// State 2: Registered client
	assert!(
		oauth2_auth
			.validate_client("new_client", "new_secret")
			.await,
		"Registered client should validate"
	);
}

#[rstest]
#[tokio::test]
async fn test_multiple_codes_same_user(oauth2_with_app: OAuth2Authentication) {
	// Generate multiple codes for same user
	let code1 = oauth2_with_app
		.generate_authorization_code(
			"test_client",
			"https://example.com/callback",
			"same_user",
			Some("read".to_string()),
		)
		.await
		.unwrap();

	let code2 = oauth2_with_app
		.generate_authorization_code(
			"test_client",
			"https://example.com/callback",
			"same_user",
			Some("write".to_string()),
		)
		.await
		.unwrap();

	// Codes should be different
	assert_ne!(code1, code2, "Each code should be unique");

	// Both codes should be exchangeable
	let token1 = oauth2_with_app
		.exchange_code(&code1, "test_client", "test_secret_12345")
		.await;
	let token2 = oauth2_with_app
		.exchange_code(&code2, "test_client", "test_secret_12345")
		.await;

	assert!(token1.is_ok(), "First code should exchange successfully");
	assert!(token2.is_ok(), "Second code should exchange successfully");
}

// =============================================================================
// Edge Cases Tests
// =============================================================================

#[rstest]
#[tokio::test]
async fn test_multiple_applications(oauth2_auth: OAuth2Authentication) {
	// Register multiple applications
	let apps = vec![
		OAuth2Application {
			client_id: "app_1".to_string(),
			client_secret: "secret_1".to_string(),
			redirect_uris: vec!["https://app1.example.com/callback".to_string()],
			grant_types: vec![GrantType::AuthorizationCode],
		},
		OAuth2Application {
			client_id: "app_2".to_string(),
			client_secret: "secret_2".to_string(),
			redirect_uris: vec!["https://app2.example.com/callback".to_string()],
			grant_types: vec![GrantType::ClientCredentials],
		},
		OAuth2Application {
			client_id: "app_3".to_string(),
			client_secret: "secret_3".to_string(),
			redirect_uris: vec!["https://app3.example.com/callback".to_string()],
			grant_types: vec![GrantType::RefreshToken],
		},
	];

	for app in apps {
		oauth2_auth.register_application(app).await;
	}

	// Each app should validate with its own credentials
	assert!(oauth2_auth.validate_client("app_1", "secret_1").await);
	assert!(oauth2_auth.validate_client("app_2", "secret_2").await);
	assert!(oauth2_auth.validate_client("app_3", "secret_3").await);

	// Cross-validation should fail
	assert!(!oauth2_auth.validate_client("app_1", "secret_2").await);
	assert!(!oauth2_auth.validate_client("app_2", "secret_3").await);
}

#[rstest]
#[tokio::test]
async fn test_scope_handling(oauth2_with_app: OAuth2Authentication) {
	// No scope
	let code_no_scope = oauth2_with_app
		.generate_authorization_code("test_client", "https://example.com/callback", "user", None)
		.await
		.unwrap();

	let token_no_scope = oauth2_with_app
		.exchange_code(&code_no_scope, "test_client", "test_secret_12345")
		.await
		.unwrap();
	assert!(
		token_no_scope.scope.is_none(),
		"Scope should be None when not provided"
	);

	// With scope
	let code_with_scope = oauth2_with_app
		.generate_authorization_code(
			"test_client",
			"https://example.com/callback",
			"user",
			Some("profile email openid".to_string()),
		)
		.await
		.unwrap();

	let token_with_scope = oauth2_with_app
		.exchange_code(&code_with_scope, "test_client", "test_secret_12345")
		.await
		.unwrap();
	assert_eq!(
		token_with_scope.scope,
		Some("profile email openid".to_string()),
		"Scope should be preserved"
	);
}

#[rstest]
#[tokio::test]
async fn test_empty_string_scope(oauth2_with_app: OAuth2Authentication) {
	// Empty string scope
	let code = oauth2_with_app
		.generate_authorization_code(
			"test_client",
			"https://example.com/callback",
			"user",
			Some(String::new()),
		)
		.await
		.unwrap();

	let token = oauth2_with_app
		.exchange_code(&code, "test_client", "test_secret_12345")
		.await
		.unwrap();

	// Empty string scope should be preserved
	assert_eq!(token.scope, Some(String::new()));
}

#[rstest]
#[tokio::test]
async fn test_special_characters_in_user_id(oauth2_with_app: OAuth2Authentication) {
	let special_user_ids = vec![
		"user@example.com",
		"user+tag@example.com",
		"user-with-dashes",
		"user.with.dots",
		"用户",
	];

	for user_id in special_user_ids {
		let code = oauth2_with_app
			.generate_authorization_code(
				"test_client",
				"https://example.com/callback",
				user_id,
				None,
			)
			.await
			.expect(&format!("Code generation for '{}' should succeed", user_id));

		let token = oauth2_with_app
			.exchange_code(&code, "test_client", "test_secret_12345")
			.await
			.expect(&format!("Token exchange for '{}' should succeed", user_id));

		assert!(
			token.token.starts_with("access_"),
			"Token for '{}' should be valid",
			user_id
		);
	}
}

// =============================================================================
// Decision Table Tests
// =============================================================================

#[rstest]
#[case(true, true, true, true)] // Valid client + Valid secret + Valid code = Success
#[case(true, false, true, false)] // Valid client + Invalid secret + Valid code = Fail
#[case(false, true, true, false)] // Invalid client + Valid secret + Valid code = Fail
#[case(true, true, false, false)] // Valid client + Valid secret + Invalid code = Fail
#[tokio::test]
async fn test_exchange_decision_table(
	#[case] valid_client: bool,
	#[case] valid_secret: bool,
	#[case] valid_code: bool,
	#[case] expected_success: bool,
) {
	let auth = OAuth2Authentication::new();
	let app = OAuth2Application {
		client_id: "dt_client".to_string(),
		client_secret: "dt_secret".to_string(),
		redirect_uris: vec!["https://dt.example.com/callback".to_string()],
		grant_types: vec![GrantType::AuthorizationCode],
	};
	auth.register_application(app).await;

	// Generate valid code
	let real_code = auth
		.generate_authorization_code(
			"dt_client",
			"https://dt.example.com/callback",
			"dt_user",
			None,
		)
		.await
		.unwrap();

	// Build test inputs based on conditions
	let client_id = if valid_client {
		"dt_client"
	} else {
		"invalid_client"
	};
	let secret = if valid_secret {
		"dt_secret"
	} else {
		"wrong_secret"
	};
	let code = if valid_code {
		&real_code
	} else {
		"invalid_code"
	};

	// Execute
	let result = auth.exchange_code(code, client_id, secret).await;

	// Assert
	assert_eq!(
		result.is_ok(),
		expected_success,
		"Exchange with (client={}, secret={}, code={}) should be {}",
		valid_client,
		valid_secret,
		valid_code,
		if expected_success {
			"successful"
		} else {
			"unsuccessful"
		}
	);
}

// =============================================================================
// Use Case Tests
// =============================================================================

#[rstest]
#[tokio::test]
async fn test_use_case_sso_login_flow(oauth2_with_app: OAuth2Authentication) {
	// Step 1: User clicks "Login with OAuth" - app requests authorization
	let code = oauth2_with_app
		.generate_authorization_code(
			"test_client",
			"https://example.com/callback",
			"sso_user_123",
			Some("openid profile email".to_string()),
		)
		.await
		.expect("Authorization should succeed");

	// Step 2: User is redirected back with code - app exchanges for token
	let token = oauth2_with_app
		.exchange_code(&code, "test_client", "test_secret_12345")
		.await
		.expect("Token exchange should succeed");

	// Step 3: App uses token to get user info
	assert!(token.token.starts_with("access_"), "Valid access token");
	assert!(
		token.refresh_token.is_some(),
		"Refresh token for session maintenance"
	);
	assert_eq!(
		token.scope,
		Some("openid profile email".to_string()),
		"Correct scopes for user info"
	);
}

#[rstest]
#[tokio::test]
async fn test_use_case_multiple_concurrent_authorizations() {
	let auth = OAuth2Authentication::new();

	// Register multiple apps (simulating different third-party services)
	let apps = vec![
		("service_a", "secret_a", "https://service-a.com/callback"),
		("service_b", "secret_b", "https://service-b.com/callback"),
	];

	for (client_id, secret, redirect) in &apps {
		auth.register_application(OAuth2Application {
			client_id: client_id.to_string(),
			client_secret: secret.to_string(),
			redirect_uris: vec![redirect.to_string()],
			grant_types: vec![GrantType::AuthorizationCode],
		})
		.await;
	}

	// User authorizes both services
	let code_a = auth
		.generate_authorization_code(
			"service_a",
			"https://service-a.com/callback",
			"user_1",
			Some("read".to_string()),
		)
		.await
		.unwrap();

	let code_b = auth
		.generate_authorization_code(
			"service_b",
			"https://service-b.com/callback",
			"user_1",
			Some("write".to_string()),
		)
		.await
		.unwrap();

	// Both services exchange their codes
	let token_a = auth
		.exchange_code(&code_a, "service_a", "secret_a")
		.await
		.unwrap();
	let token_b = auth
		.exchange_code(&code_b, "service_b", "secret_b")
		.await
		.unwrap();

	// Each service has its own token
	assert_ne!(
		token_a.token, token_b.token,
		"Different services get different tokens"
	);
	assert_eq!(token_a.scope, Some("read".to_string()));
	assert_eq!(token_b.scope, Some("write".to_string()));
}

#[rstest]
#[tokio::test]
async fn test_use_case_refresh_token_presence() {
	let auth = OAuth2Authentication::new();
	let app = OAuth2Application {
		client_id: "refresh_test_client".to_string(),
		client_secret: "refresh_test_secret".to_string(),
		redirect_uris: vec!["https://refresh.example.com/callback".to_string()],
		grant_types: vec![GrantType::AuthorizationCode, GrantType::RefreshToken],
	};
	auth.register_application(app).await;

	let code = auth
		.generate_authorization_code(
			"refresh_test_client",
			"https://refresh.example.com/callback",
			"user_with_refresh",
			None,
		)
		.await
		.unwrap();

	let token = auth
		.exchange_code(&code, "refresh_test_client", "refresh_test_secret")
		.await
		.unwrap();

	// Refresh token should be provided
	assert!(
		token.refresh_token.is_some(),
		"Refresh token should be provided for auth code flow"
	);

	let refresh_token = token.refresh_token.unwrap();
	assert!(
		refresh_token.starts_with("refresh_"),
		"Refresh token should have proper prefix"
	);
}

// =============================================================================
// Sanity Tests
// =============================================================================

#[rstest]
fn test_grant_type_variants() {
	// Verify all grant types are serializable/deserializable
	let grant_types = vec![
		GrantType::AuthorizationCode,
		GrantType::ClientCredentials,
		GrantType::RefreshToken,
		GrantType::Implicit,
	];

	for grant_type in grant_types {
		let json = serde_json::to_string(&grant_type).expect("GrantType should be serializable");
		let _: GrantType = serde_json::from_str(&json).expect("GrantType should be deserializable");
	}
}

#[rstest]
fn test_access_token_serialization() {
	let token = AccessToken {
		token: "test_token".to_string(),
		token_type: "Bearer".to_string(),
		expires_in: 3600,
		refresh_token: Some("refresh_token".to_string()),
		scope: Some("read write".to_string()),
	};

	let json = serde_json::to_string(&token).expect("AccessToken should serialize");
	let deserialized: AccessToken =
		serde_json::from_str(&json).expect("AccessToken should deserialize");

	assert_eq!(deserialized.token, token.token);
	assert_eq!(deserialized.token_type, token.token_type);
	assert_eq!(deserialized.expires_in, token.expires_in);
	assert_eq!(deserialized.refresh_token, token.refresh_token);
	assert_eq!(deserialized.scope, token.scope);
}

#[rstest]
fn test_oauth2_application_serialization() {
	let app = OAuth2Application {
		client_id: "app_client".to_string(),
		client_secret: "app_secret".to_string(),
		redirect_uris: vec![
			"https://example.com/cb1".to_string(),
			"https://example.com/cb2".to_string(),
		],
		grant_types: vec![GrantType::AuthorizationCode, GrantType::RefreshToken],
	};

	let json = serde_json::to_string(&app).expect("OAuth2Application should serialize");
	let deserialized: OAuth2Application =
		serde_json::from_str(&json).expect("OAuth2Application should deserialize");

	assert_eq!(deserialized.client_id, app.client_id);
	assert_eq!(deserialized.client_secret, app.client_secret);
	assert_eq!(deserialized.redirect_uris.len(), 2);
	assert_eq!(deserialized.grant_types.len(), 2);
}
