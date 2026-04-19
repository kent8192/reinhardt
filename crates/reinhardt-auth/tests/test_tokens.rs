#![cfg(feature = "jwt")]

use reinhardt_auth::JwtAuth;

// === JWT Token Tests ===

#[test]
fn test_auth_tokens_jwt_generate() {
	let jwt_auth = JwtAuth::new(b"test_secret_key");
	let user_id = "user123".to_string();
	let username = "testuser".to_string();

	let token = jwt_auth
		.generate_token(user_id, username, false, false)
		.unwrap();

	assert!(!token.is_empty());
	// JWT tokens should have 3 parts separated by dots
	assert_eq!(token.split('.').count(), 3);
}

#[test]
fn test_jwt_verify_valid_token() {
	let jwt_auth = JwtAuth::new(b"test_secret_key");
	let user_id = "user123".to_string();
	let username = "testuser".to_string();

	let token = jwt_auth
		.generate_token(user_id.clone(), username.clone(), false, false)
		.unwrap();
	let claims = jwt_auth.verify_token(&token).unwrap();

	assert_eq!(claims.sub, user_id);
	assert_eq!(claims.username, username);
}

#[test]
fn test_jwt_verify_invalid_token() {
	let jwt_auth = JwtAuth::new(b"test_secret_key");
	let invalid_token = "invalid.token.here";

	assert!(jwt_auth.verify_token(invalid_token).is_err());
}

#[test]
fn test_jwt_verify_token_with_wrong_secret() {
	// A valid token can be created with a secret other than the original
	// But verification with different secret should fail
	let jwt_auth1 = JwtAuth::new(b"secret1");
	let jwt_auth2 = JwtAuth::new(b"secret2");
	let user_id = "user123".to_string();
	let username = "testuser".to_string();

	let token = jwt_auth1
		.generate_token(user_id, username, false, false)
		.unwrap();

	// Token should not verify with different secret
	assert!(jwt_auth2.verify_token(&token).is_err());
}

#[test]
fn test_jwt_token_with_different_secret() {
	// Test that tokens generated with different secrets are different
	let jwt_auth1 = JwtAuth::new(b"secret1");
	let jwt_auth2 = JwtAuth::new(b"secret2");
	let user_id = "user123".to_string();
	let username = "testuser".to_string();

	let token1 = jwt_auth1
		.generate_token(user_id.clone(), username.clone(), false, false)
		.unwrap();
	let token2 = jwt_auth2
		.generate_token(user_id, username, false, false)
		.unwrap();

	assert_ne!(token1, token2);
}

#[test]
fn test_jwt_multiple_tokens_same_user() {
	// The token generated for a user created in the same request will work correctly
	let jwt_auth = JwtAuth::new(b"test_secret_key");
	let user_id = "user123".to_string();
	let username = "testuser".to_string();

	let token1 = jwt_auth
		.generate_token(user_id.clone(), username.clone(), false, false)
		.unwrap();
	let token2 = jwt_auth
		.generate_token(user_id, username, false, false)
		.unwrap();

	// Both tokens should verify successfully
	assert!(jwt_auth.verify_token(&token1).is_ok());
	assert!(jwt_auth.verify_token(&token2).is_ok());
}

#[test]
fn test_jwt_claims_structure() {
	let jwt_auth = JwtAuth::new(b"test_secret_key");
	let user_id = "user456".to_string();
	let username = "anotheruser".to_string();

	let token = jwt_auth
		.generate_token(user_id.clone(), username.clone(), false, false)
		.unwrap();
	let claims = jwt_auth.verify_token(&token).unwrap();

	// Verify claim fields
	assert_eq!(claims.sub, user_id);
	assert_eq!(claims.username, username);
	assert!(claims.exp > 0); // Expiration should be set
}

#[test]
fn test_jwt_token_expiration_is_set() {
	let jwt_auth = JwtAuth::new(b"test_secret_key");
	let user_id = "user123".to_string();
	let username = "testuser".to_string();

	let token = jwt_auth
		.generate_token(user_id, username, false, false)
		.unwrap();
	let claims = jwt_auth.verify_token(&token).unwrap();

	// Token should have an expiration timestamp
	let now = std::time::SystemTime::now()
		.duration_since(std::time::UNIX_EPOCH)
		.unwrap()
		.as_secs() as i64;

	// Expiration should be in the future
	assert!(claims.exp > now);
}

#[test]
fn test_jwt_empty_secret_handling() {
	// Test behavior with empty secret (should still work but not recommended)
	let jwt_auth = JwtAuth::new(b"");
	let user_id = "user123".to_string();
	let username = "testuser".to_string();

	let token_result = jwt_auth.generate_token(user_id, username, false, false);
	// Should still generate a token (implementation dependent)
	assert!(token_result.is_ok());
}

#[test]
fn test_jwt_special_characters_in_username() {
	let jwt_auth = JwtAuth::new(b"test_secret_key");
	let user_id = "user123".to_string();
	let username = "user@example.com".to_string(); // Email as username

	let token = jwt_auth
		.generate_token(user_id.clone(), username.clone(), false, false)
		.unwrap();
	let claims = jwt_auth.verify_token(&token).unwrap();

	assert_eq!(claims.sub, user_id);
	assert_eq!(claims.username, username);
}

#[test]
fn test_jwt_unicode_username() {
	let jwt_auth = JwtAuth::new(b"test_secret_key");
	let user_id = "user123".to_string();
	let username = "ユーザー名".to_string(); // Japanese username (test string)

	let token = jwt_auth
		.generate_token(user_id.clone(), username.clone(), false, false)
		.unwrap();
	let claims = jwt_auth.verify_token(&token).unwrap();

	assert_eq!(claims.sub, user_id);
	assert_eq!(claims.username, username);
}

#[test]
fn test_jwt_long_secret_key() {
	// Test with a very long secret key
	let long_secret = b"this_is_a_very_long_secret_key_that_should_still_work_correctly_with_jwt_authentication_system";
	let jwt_auth = JwtAuth::new(long_secret);
	let user_id = "user123".to_string();
	let username = "testuser".to_string();

	let token = jwt_auth
		.generate_token(user_id.clone(), username.clone(), false, false)
		.unwrap();
	let claims = jwt_auth.verify_token(&token).unwrap();

	assert_eq!(claims.sub, user_id);
	assert_eq!(claims.username, username);
}

#[test]
fn test_jwt_malformed_token_parts() {
	let jwt_auth = JwtAuth::new(b"test_secret_key");

	// Token with wrong number of parts
	assert!(jwt_auth.verify_token("only.two").is_err());
	assert!(jwt_auth.verify_token("one.two.three.four").is_err());
	assert!(jwt_auth.verify_token("single").is_err());
	assert!(jwt_auth.verify_token("").is_err());
}

#[test]
fn test_jwt_tampered_token() {
	let jwt_auth = JwtAuth::new(b"test_secret_key");
	let user_id = "user123".to_string();
	let username = "testuser".to_string();

	let token = jwt_auth
		.generate_token(user_id, username, false, false)
		.unwrap();

	// Tamper with the token by changing a character
	let mut tampered = token.clone();
	if let Some(c) = tampered.pop() {
		tampered.push(if c == 'a' { 'b' } else { 'a' });
	}

	// Tampered token should not verify
	assert!(jwt_auth.verify_token(&tampered).is_err());
}

#[test]
fn test_jwt_token_reuse() {
	// Test that the same token can be verified multiple times
	let jwt_auth = JwtAuth::new(b"test_secret_key");
	let user_id = "user123".to_string();
	let username = "testuser".to_string();

	let token = jwt_auth
		.generate_token(user_id, username, false, false)
		.unwrap();

	// Verify multiple times
	for _ in 0..5 {
		assert!(jwt_auth.verify_token(&token).is_ok());
	}
}

// === Token Security Tests ===

#[test]
fn test_jwt_different_users_different_tokens() {
	let jwt_auth = JwtAuth::new(b"test_secret_key");

	let token1 = jwt_auth
		.generate_token("user1".to_string(), "alice".to_string(), false, false)
		.unwrap();
	let token2 = jwt_auth
		.generate_token("user2".to_string(), "bob".to_string(), false, false)
		.unwrap();

	assert_ne!(token1, token2);

	let claims1 = jwt_auth.verify_token(&token1).unwrap();
	let claims2 = jwt_auth.verify_token(&token2).unwrap();

	assert_eq!(claims1.sub, "user1");
	assert_eq!(claims2.sub, "user2");
}

#[test]
fn test_jwt_secret_key_matters() {
	// Tokens generated with one key should not verify with another
	let jwt_auth1 = JwtAuth::new(b"secret_key_1");
	let jwt_auth2 = JwtAuth::new(b"secret_key_2");

	let token = jwt_auth1
		.generate_token("user123".to_string(), "testuser".to_string(), false, false)
		.unwrap();

	// Should verify with same key
	assert!(jwt_auth1.verify_token(&token).is_ok());

	// Should NOT verify with different key
	assert!(jwt_auth2.verify_token(&token).is_err());
}
