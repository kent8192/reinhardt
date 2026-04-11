//! CSRF protection integration with sessions
//!
//! This module provides integration between session management and CSRF protection
//! from reinhardt-forms. CSRF tokens are stored in sessions for validation.
//!
//! ## Example
//!
//! ```rust
//! use reinhardt_auth::sessions::csrf::CsrfSessionManager;
//! use reinhardt_auth::sessions::Session;
//! use reinhardt_auth::sessions::backends::InMemorySessionBackend;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let backend = InMemorySessionBackend::new();
//! let mut session = Session::new(backend);
//!
//! // Create CSRF manager
//! let csrf_manager = CsrfSessionManager::new();
//!
//! // Generate and store CSRF token in session
//! let token = csrf_manager.generate_token(&mut session)?;
//! println!("CSRF token: {}", token);
//!
//! // Validate token from session
//! let is_valid = csrf_manager.validate_token(&mut session, &token)?;
//! assert!(is_valid);
//! # Ok(())
//! # }
//! ```

use super::backends::SessionBackend;
use super::session::Session;
use serde::{Deserialize, Serialize};
use std::time::SystemTime;
use subtle::ConstantTimeEq;
use uuid::Uuid;

/// CSRF token data stored in session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CsrfTokenData {
	/// The token value
	pub token: String,
	/// When the token was created
	pub created_at: SystemTime,
}

/// CSRF session manager
///
/// Manages CSRF tokens in sessions, integrating with reinhardt-forms.
///
/// # Example
///
/// ```rust
/// use reinhardt_auth::sessions::csrf::CsrfSessionManager;
/// use reinhardt_auth::sessions::Session;
/// use reinhardt_auth::sessions::backends::InMemorySessionBackend;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let backend = InMemorySessionBackend::new();
/// let mut session = Session::new(backend);
///
/// let csrf = CsrfSessionManager::new();
///
/// // Generate token
/// let token = csrf.generate_token(&mut session)?;
///
/// // Validate token
/// assert!(csrf.validate_token(&mut session, &token)?);
/// # Ok(())
/// # }
/// ```
pub struct CsrfSessionManager {
	/// Session key for storing CSRF token
	session_key: String,
}

impl CsrfSessionManager {
	/// Create a new CSRF session manager
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_auth::sessions::csrf::CsrfSessionManager;
	///
	/// let csrf = CsrfSessionManager::new();
	/// ```
	pub fn new() -> Self {
		Self {
			session_key: "_csrf_token".to_string(),
		}
	}

	/// Create a new CSRF session manager with custom session key
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_auth::sessions::csrf::CsrfSessionManager;
	///
	/// let csrf = CsrfSessionManager::with_key("my_csrf_token".to_string());
	/// ```
	pub fn with_key(session_key: String) -> Self {
		Self { session_key }
	}

	/// Generate a new CSRF token and store it in the session
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_auth::sessions::csrf::CsrfSessionManager;
	/// use reinhardt_auth::sessions::Session;
	/// use reinhardt_auth::sessions::backends::InMemorySessionBackend;
	///
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// let backend = InMemorySessionBackend::new();
	/// let mut session = Session::new(backend);
	///
	/// let csrf = CsrfSessionManager::new();
	/// let token = csrf.generate_token(&mut session)?;
	///
	/// assert!(!token.is_empty());
	/// # Ok(())
	/// # }
	/// ```
	pub fn generate_token<B: SessionBackend>(
		&self,
		session: &mut Session<B>,
	) -> Result<String, serde_json::Error> {
		let token = Uuid::new_v4().to_string();
		let token_data = CsrfTokenData {
			token: token.clone(),
			created_at: SystemTime::now(),
		};

		session.set(&self.session_key, token_data)?;
		Ok(token)
	}

	/// Get the current CSRF token from the session
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_auth::sessions::csrf::CsrfSessionManager;
	/// use reinhardt_auth::sessions::Session;
	/// use reinhardt_auth::sessions::backends::InMemorySessionBackend;
	///
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// let backend = InMemorySessionBackend::new();
	/// let mut session = Session::new(backend);
	///
	/// let csrf = CsrfSessionManager::new();
	///
	/// // Generate token first
	/// let generated = csrf.generate_token(&mut session)?;
	///
	/// // Get the stored token
	/// let stored = csrf.get_token(&mut session)?;
	/// assert_eq!(stored, Some(generated));
	/// # Ok(())
	/// # }
	/// ```
	pub fn get_token<B: SessionBackend>(
		&self,
		session: &mut Session<B>,
	) -> Result<Option<String>, serde_json::Error> {
		let token_data: Option<CsrfTokenData> = session.get(&self.session_key)?;
		Ok(token_data.map(|data| data.token))
	}

	/// Validate a CSRF token against the session
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_auth::sessions::csrf::CsrfSessionManager;
	/// use reinhardt_auth::sessions::Session;
	/// use reinhardt_auth::sessions::backends::InMemorySessionBackend;
	///
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// let backend = InMemorySessionBackend::new();
	/// let mut session = Session::new(backend);
	///
	/// let csrf = CsrfSessionManager::new();
	/// let token = csrf.generate_token(&mut session)?;
	///
	/// // Valid token
	/// assert!(csrf.validate_token(&mut session, &token)?);
	///
	/// // Invalid token
	/// assert!(!csrf.validate_token(&mut session, "wrong_token")?);
	/// # Ok(())
	/// # }
	/// ```
	pub fn validate_token<B: SessionBackend>(
		&self,
		session: &mut Session<B>,
		submitted_token: &str,
	) -> Result<bool, serde_json::Error> {
		let stored_token = self.get_token(session)?;

		match stored_token {
			Some(token) => {
				// Use constant-time comparison to prevent timing attacks
				Ok(token.as_bytes().ct_eq(submitted_token.as_bytes()).into())
			}
			None => Ok(false),
		}
	}

	/// Rotate the CSRF token (generate a new one)
	///
	/// This is useful after login or privilege escalation.
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_auth::sessions::csrf::CsrfSessionManager;
	/// use reinhardt_auth::sessions::Session;
	/// use reinhardt_auth::sessions::backends::InMemorySessionBackend;
	///
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// let backend = InMemorySessionBackend::new();
	/// let mut session = Session::new(backend);
	///
	/// let csrf = CsrfSessionManager::new();
	///
	/// let old_token = csrf.generate_token(&mut session)?;
	/// let new_token = csrf.rotate_token(&mut session)?;
	///
	/// assert_ne!(old_token, new_token);
	///
	/// // Old token should no longer be valid
	/// assert!(!csrf.validate_token(&mut session, &old_token)?);
	/// // New token should be valid
	/// assert!(csrf.validate_token(&mut session, &new_token)?);
	/// # Ok(())
	/// # }
	/// ```
	pub fn rotate_token<B: SessionBackend>(
		&self,
		session: &mut Session<B>,
	) -> Result<String, serde_json::Error> {
		// Simply generate a new token, which overwrites the old one
		self.generate_token(session)
	}

	/// Clear the CSRF token from the session
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_auth::sessions::csrf::CsrfSessionManager;
	/// use reinhardt_auth::sessions::Session;
	/// use reinhardt_auth::sessions::backends::InMemorySessionBackend;
	///
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// let backend = InMemorySessionBackend::new();
	/// let mut session = Session::new(backend);
	///
	/// let csrf = CsrfSessionManager::new();
	/// csrf.generate_token(&mut session)?;
	///
	/// csrf.clear_token(&mut session);
	///
	/// assert!(csrf.get_token(&mut session)?.is_none());
	/// # Ok(())
	/// # }
	/// ```
	pub fn clear_token<B: SessionBackend>(&self, session: &mut Session<B>) {
		session.delete(&self.session_key);
	}

	/// Get or create a CSRF token
	///
	/// Returns the existing token if available, otherwise generates a new one.
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_auth::sessions::csrf::CsrfSessionManager;
	/// use reinhardt_auth::sessions::Session;
	/// use reinhardt_auth::sessions::backends::InMemorySessionBackend;
	///
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// let backend = InMemorySessionBackend::new();
	/// let mut session = Session::new(backend);
	///
	/// let csrf = CsrfSessionManager::new();
	///
	/// let token1 = csrf.get_or_create_token(&mut session)?;
	/// let token2 = csrf.get_or_create_token(&mut session)?;
	///
	/// // Should be the same token
	/// assert_eq!(token1, token2);
	/// # Ok(())
	/// # }
	/// ```
	pub fn get_or_create_token<B: SessionBackend>(
		&self,
		session: &mut Session<B>,
	) -> Result<String, serde_json::Error> {
		if let Some(token) = self.get_token(session)? {
			Ok(token)
		} else {
			self.generate_token(session)
		}
	}
}

impl Default for CsrfSessionManager {
	/// Create default CSRF session manager
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_auth::sessions::csrf::CsrfSessionManager;
	///
	/// let csrf = CsrfSessionManager::default();
	/// ```
	fn default() -> Self {
		Self::new()
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::sessions::InMemorySessionBackend;

	#[tokio::test]
	async fn test_csrf_manager_new() {
		let _csrf = CsrfSessionManager::new();
	}

	#[tokio::test]
	async fn test_csrf_manager_with_key() {
		let csrf = CsrfSessionManager::with_key("custom_key".to_string());
		assert_eq!(csrf.session_key, "custom_key");
	}

	#[tokio::test]
	async fn test_generate_token() {
		let backend = InMemorySessionBackend::new();
		let mut session = Session::new(backend);

		let csrf = CsrfSessionManager::new();
		let token = csrf.generate_token(&mut session).unwrap();

		assert!(!token.is_empty());
	}

	#[tokio::test]
	async fn test_get_token() {
		let backend = InMemorySessionBackend::new();
		let mut session = Session::new(backend);

		let csrf = CsrfSessionManager::new();

		// No token initially
		assert!(csrf.get_token(&mut session).unwrap().is_none());

		// Generate token
		let generated = csrf.generate_token(&mut session).unwrap();

		// Get the stored token
		let stored = csrf.get_token(&mut session).unwrap();
		assert_eq!(stored, Some(generated));
	}

	#[tokio::test]
	async fn test_validate_token() {
		let backend = InMemorySessionBackend::new();
		let mut session = Session::new(backend);

		let csrf = CsrfSessionManager::new();
		let token = csrf.generate_token(&mut session).unwrap();

		// Valid token
		assert!(csrf.validate_token(&mut session, &token).unwrap());

		// Invalid token
		assert!(!csrf.validate_token(&mut session, "wrong_token").unwrap());
	}

	#[tokio::test]
	async fn test_validate_token_no_token_in_session() {
		let backend = InMemorySessionBackend::new();
		let mut session = Session::new(backend);

		let csrf = CsrfSessionManager::new();

		// No token in session
		assert!(!csrf.validate_token(&mut session, "any_token").unwrap());
	}

	#[tokio::test]
	async fn test_rotate_token() {
		let backend = InMemorySessionBackend::new();
		let mut session = Session::new(backend);

		let csrf = CsrfSessionManager::new();

		let old_token = csrf.generate_token(&mut session).unwrap();
		let new_token = csrf.rotate_token(&mut session).unwrap();

		assert_ne!(old_token, new_token);

		// Old token should no longer be valid
		assert!(!csrf.validate_token(&mut session, &old_token).unwrap());

		// New token should be valid
		assert!(csrf.validate_token(&mut session, &new_token).unwrap());
	}

	#[tokio::test]
	async fn test_clear_token() {
		let backend = InMemorySessionBackend::new();
		let mut session = Session::new(backend);

		let csrf = CsrfSessionManager::new();
		csrf.generate_token(&mut session).unwrap();

		csrf.clear_token(&mut session);

		assert!(csrf.get_token(&mut session).unwrap().is_none());
	}

	#[tokio::test]
	async fn test_get_or_create_token() {
		let backend = InMemorySessionBackend::new();
		let mut session = Session::new(backend);

		let csrf = CsrfSessionManager::new();

		let token1 = csrf.get_or_create_token(&mut session).unwrap();
		let token2 = csrf.get_or_create_token(&mut session).unwrap();

		// Should be the same token
		assert_eq!(token1, token2);
	}

	#[tokio::test]
	async fn test_get_or_create_token_creates_if_missing() {
		let backend = InMemorySessionBackend::new();
		let mut session = Session::new(backend);

		let csrf = CsrfSessionManager::new();

		// No token initially
		assert!(csrf.get_token(&mut session).unwrap().is_none());

		// get_or_create should create one
		let token = csrf.get_or_create_token(&mut session).unwrap();
		assert!(!token.is_empty());

		// Should now exist
		assert!(csrf.get_token(&mut session).unwrap().is_some());
	}
}
