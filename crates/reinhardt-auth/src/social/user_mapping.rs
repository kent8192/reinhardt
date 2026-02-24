//! User data mapping from OAuth/OIDC claims

use std::collections::HashMap;

use serde_json::Value;

use crate::social::core::{SocialAuthError, StandardClaims};

/// Mapped user data from social provider claims
#[derive(Debug, Clone)]
pub struct MappedUser {
	/// Provider-specific user ID
	pub provider_user_id: String,
	/// Email address
	pub email: Option<String>,
	/// Whether email is verified
	pub email_verified: bool,
	/// Display name
	pub display_name: Option<String>,
	/// First name
	pub first_name: Option<String>,
	/// Last name
	pub last_name: Option<String>,
	/// Profile picture URL
	pub picture: Option<String>,
	/// Locale
	pub locale: Option<String>,
	/// Additional provider-specific data
	pub extra_data: HashMap<String, Value>,
}

/// User mapper trait for transforming claims into application user data
pub trait UserMapper: Send + Sync {
	/// Maps standard claims to application user data
	fn map_claims_to_user(&self, claims: &StandardClaims) -> Result<MappedUser, SocialAuthError>;
}

/// Default user mapper that maps `StandardClaims` fields directly
pub struct DefaultUserMapper;

impl UserMapper for DefaultUserMapper {
	fn map_claims_to_user(&self, claims: &StandardClaims) -> Result<MappedUser, SocialAuthError> {
		Ok(MappedUser {
			provider_user_id: claims.sub.clone(),
			email: claims.email.clone(),
			email_verified: claims.email_verified.unwrap_or(false),
			display_name: claims.name.clone(),
			first_name: claims.given_name.clone(),
			last_name: claims.family_name.clone(),
			picture: claims.picture.clone(),
			locale: claims.locale.clone(),
			extra_data: claims.additional_claims.clone(),
		})
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_default_mapper_full_claims() {
		// Arrange
		let claims = StandardClaims {
			sub: "user_123".to_string(),
			email: Some("user@example.com".to_string()),
			email_verified: Some(true),
			name: Some("Test User".to_string()),
			given_name: Some("Test".to_string()),
			family_name: Some("User".to_string()),
			picture: Some("https://example.com/photo.jpg".to_string()),
			locale: Some("en".to_string()),
			additional_claims: HashMap::new(),
		};
		let mapper = DefaultUserMapper;

		// Act
		let mapped = mapper.map_claims_to_user(&claims).unwrap();

		// Assert
		assert_eq!(mapped.provider_user_id, "user_123");
		assert_eq!(mapped.email, Some("user@example.com".to_string()));
		assert!(mapped.email_verified);
		assert_eq!(mapped.display_name, Some("Test User".to_string()));
		assert_eq!(mapped.first_name, Some("Test".to_string()));
		assert_eq!(mapped.last_name, Some("User".to_string()));
		assert_eq!(
			mapped.picture,
			Some("https://example.com/photo.jpg".to_string())
		);
	}

	#[test]
	fn test_default_mapper_minimal_claims() {
		// Arrange
		let claims = StandardClaims {
			sub: "user_456".to_string(),
			email: None,
			email_verified: None,
			name: None,
			given_name: None,
			family_name: None,
			picture: None,
			locale: None,
			additional_claims: HashMap::new(),
		};
		let mapper = DefaultUserMapper;

		// Act
		let mapped = mapper.map_claims_to_user(&claims).unwrap();

		// Assert
		assert_eq!(mapped.provider_user_id, "user_456");
		assert!(mapped.email.is_none());
		assert!(!mapped.email_verified);
		assert!(mapped.display_name.is_none());
	}

	#[test]
	fn test_default_mapper_extra_data() {
		// Arrange
		let mut additional = HashMap::new();
		additional.insert("login".to_string(), Value::String("testuser".to_string()));

		let claims = StandardClaims {
			sub: "user_789".to_string(),
			email: None,
			email_verified: None,
			name: None,
			given_name: None,
			family_name: None,
			picture: None,
			locale: None,
			additional_claims: additional,
		};
		let mapper = DefaultUserMapper;

		// Act
		let mapped = mapper.map_claims_to_user(&claims).unwrap();

		// Assert
		assert_eq!(
			mapped.extra_data.get("login"),
			Some(&Value::String("testuser".to_string()))
		);
	}
}
