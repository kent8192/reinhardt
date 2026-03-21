//! ORM models for session storage
//!
//! This module provides ORM model definitions for database-backed session storage.
//! The SessionModel can be used with reinhardt-orm to manage sessions in the database.
//!
//! ## Features
//!
//! - **SessionModel**: ORM model for database session storage
//! - **Model trait implementation**: Full ORM integration
//!
//! ## Example
//!
//! ```rust
//! use reinhardt_auth::sessions::models::SessionModel;
//! use chrono::Utc;
//! use serde_json::json;
//!
//! let session = SessionModel::new(
//!     "session_key_123".to_string(),
//!     json!({"user_id": 42}),
//!     3600, // TTL in seconds
//! );
//!
//! assert_eq!(session.session_key(), "session_key_123");
//! assert!(session.expire_date() > &Utc::now());
//! ```

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};

#[cfg(feature = "database")]
use reinhardt_db::orm::Model;

/// Session model for database storage
///
/// Represents a session stored in the database with expiration information.
/// This model implements the ORM Model trait when the `database` feature is enabled.
///
/// ## Example
///
/// ```rust
/// use reinhardt_auth::sessions::models::SessionModel;
/// use chrono::Utc;
/// use serde_json::json;
///
/// // Create a new session model
/// let session = SessionModel::new(
///     "abc123".to_string(),
///     json!({"user_id": 42, "authenticated": true}),
///     3600,
/// );
///
/// assert_eq!(session.session_key(), "abc123");
/// assert!(session.is_valid());
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionModel {
	/// Unique session key (primary key)
	session_key: String,
	/// Session data stored as JSON
	session_data: serde_json::Value,
	/// Session expiration timestamp
	expire_date: DateTime<Utc>,
}

impl SessionModel {
	/// Create a new session model
	///
	/// # Arguments
	///
	/// * `session_key` - Unique session identifier
	/// * `session_data` - Session data as JSON value
	/// * `ttl_seconds` - Time to live in seconds
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_auth::sessions::models::SessionModel;
	/// use serde_json::json;
	///
	/// let session = SessionModel::new(
	///     "session_123".to_string(),
	///     json!({"cart_total": 99.99}),
	///     7200,
	/// );
	/// ```
	pub fn new(session_key: String, session_data: serde_json::Value, ttl_seconds: i64) -> Self {
		let expire_date = Utc::now() + Duration::seconds(ttl_seconds);
		Self {
			session_key,
			session_data,
			expire_date,
		}
	}

	/// Create a session model with a specific expiration date
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_auth::sessions::models::SessionModel;
	/// use chrono::{Utc, Duration};
	/// use serde_json::json;
	///
	/// let expire_date = Utc::now() + Duration::hours(2);
	/// let session = SessionModel::with_expire_date(
	///     "session_xyz".to_string(),
	///     json!({"preferences": {"theme": "dark"}}),
	///     expire_date,
	/// );
	/// ```
	pub fn with_expire_date(
		session_key: String,
		session_data: serde_json::Value,
		expire_date: DateTime<Utc>,
	) -> Self {
		Self {
			session_key,
			session_data,
			expire_date,
		}
	}

	/// Get the session key
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_auth::sessions::models::SessionModel;
	/// use serde_json::json;
	///
	/// let session = SessionModel::new("key_123".to_string(), json!({}), 3600);
	/// assert_eq!(session.session_key(), "key_123");
	/// ```
	pub fn session_key(&self) -> &str {
		&self.session_key
	}

	/// Get the session data
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_auth::sessions::models::SessionModel;
	/// use serde_json::json;
	///
	/// let data = json!({"user": "alice"});
	/// let session = SessionModel::new("key".to_string(), data.clone(), 3600);
	/// assert_eq!(session.session_data(), &data);
	/// ```
	pub fn session_data(&self) -> &serde_json::Value {
		&self.session_data
	}

	/// Get the expiration date
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_auth::sessions::models::SessionModel;
	/// use chrono::Utc;
	/// use serde_json::json;
	///
	/// let session = SessionModel::new("key".to_string(), json!({}), 3600);
	/// assert!(session.expire_date() > &Utc::now());
	/// ```
	pub fn expire_date(&self) -> &DateTime<Utc> {
		&self.expire_date
	}

	/// Set the session data
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_auth::sessions::models::SessionModel;
	/// use serde_json::json;
	///
	/// let mut session = SessionModel::new("key".to_string(), json!({}), 3600);
	/// session.set_session_data(json!({"updated": true}));
	/// assert_eq!(session.session_data()["updated"], true);
	/// ```
	pub fn set_session_data(&mut self, data: serde_json::Value) {
		self.session_data = data;
	}

	/// Set the expiration date
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_auth::sessions::models::SessionModel;
	/// use chrono::{Utc, Duration};
	/// use serde_json::json;
	///
	/// let mut session = SessionModel::new("key".to_string(), json!({}), 3600);
	/// let new_expire = Utc::now() + Duration::hours(24);
	/// session.set_expire_date(new_expire);
	/// ```
	pub fn set_expire_date(&mut self, expire_date: DateTime<Utc>) {
		self.expire_date = expire_date;
	}

	/// Check if the session is still valid (not expired)
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_auth::sessions::models::SessionModel;
	/// use serde_json::json;
	///
	/// let session = SessionModel::new("key".to_string(), json!({}), 3600);
	/// assert!(session.is_valid());
	///
	/// let expired = SessionModel::new("old".to_string(), json!({}), -100);
	/// assert!(!expired.is_valid());
	/// ```
	pub fn is_valid(&self) -> bool {
		self.expire_date > Utc::now()
	}

	/// Check if the session has expired
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_auth::sessions::models::SessionModel;
	/// use serde_json::json;
	///
	/// let session = SessionModel::new("key".to_string(), json!({}), 3600);
	/// assert!(!session.is_expired());
	///
	/// let expired = SessionModel::new("old".to_string(), json!({}), -100);
	/// assert!(expired.is_expired());
	/// ```
	pub fn is_expired(&self) -> bool {
		self.expire_date <= Utc::now()
	}

	/// Extend the session expiration by the given number of seconds
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_auth::sessions::models::SessionModel;
	/// use chrono::Utc;
	/// use serde_json::json;
	///
	/// let mut session = SessionModel::new("key".to_string(), json!({}), 3600);
	/// let old_expire = *session.expire_date();
	///
	/// session.extend(1800);
	/// assert!(session.expire_date() > &old_expire);
	/// ```
	pub fn extend(&mut self, seconds: i64) {
		self.expire_date += Duration::seconds(seconds);
	}

	/// Refresh the session with a new TTL from now
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_auth::sessions::models::SessionModel;
	/// use chrono::Utc;
	/// use serde_json::json;
	///
	/// let mut session = SessionModel::new("key".to_string(), json!({}), 3600);
	/// session.refresh(7200);
	///
	/// // New expiration should be approximately 7200 seconds from now
	/// let expected = Utc::now() + chrono::Duration::seconds(7200);
	/// assert!(session.expire_date().signed_duration_since(expected).num_seconds().abs() < 2);
	/// ```
	pub fn refresh(&mut self, ttl_seconds: i64) {
		self.expire_date = Utc::now() + Duration::seconds(ttl_seconds);
	}
}

/// Query field descriptors for the `SessionModel`.
#[cfg(feature = "database")]
#[derive(Debug, Clone)]
pub struct SessionModelFields {
	/// The session key field.
	pub session_key: reinhardt_db::orm::query_fields::Field<SessionModel, String>,
	/// The session data field.
	pub session_data: reinhardt_db::orm::query_fields::Field<SessionModel, serde_json::Value>,
	/// The expiration date field.
	pub expire_date: reinhardt_db::orm::query_fields::Field<SessionModel, DateTime<Utc>>,
}

#[cfg(feature = "database")]
impl Default for SessionModelFields {
	fn default() -> Self {
		Self::new()
	}
}

#[cfg(feature = "database")]
impl SessionModelFields {
	/// Creates a new `SessionModelFields` with default field mappings.
	pub fn new() -> Self {
		Self {
			session_key: reinhardt_db::orm::query_fields::Field::new(vec!["session_key"]),
			session_data: reinhardt_db::orm::query_fields::Field::new(vec!["session_data"]),
			expire_date: reinhardt_db::orm::query_fields::Field::new(vec!["expire_date"]),
		}
	}
}

#[cfg(feature = "database")]
impl reinhardt_db::orm::FieldSelector for SessionModelFields {
	fn with_alias(mut self, alias: &str) -> Self {
		self.session_key = self.session_key.with_alias(alias);
		self.session_data = self.session_data.with_alias(alias);
		self.expire_date = self.expire_date.with_alias(alias);
		self
	}
}

#[cfg(feature = "database")]
impl Model for SessionModel {
	type PrimaryKey = String;
	type Fields = SessionModelFields;

	fn table_name() -> &'static str {
		"sessions"
	}

	fn new_fields() -> Self::Fields {
		SessionModelFields::new()
	}

	fn primary_key_field() -> &'static str {
		"session_key"
	}

	fn primary_key(&self) -> Option<Self::PrimaryKey> {
		Some(self.session_key.clone())
	}

	fn set_primary_key(&mut self, value: Self::PrimaryKey) {
		self.session_key = value;
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use serde_json::json;

	#[test]
	fn test_session_model_new() {
		let session = SessionModel::new("test_key".to_string(), json!({"test": "data"}), 3600);

		assert_eq!(session.session_key(), "test_key");
		assert_eq!(session.session_data(), &json!({"test": "data"}));
		assert!(session.expire_date() > &Utc::now());
	}

	#[test]
	fn test_session_model_with_expire_date() {
		let expire_date = Utc::now() + Duration::hours(2);
		let session = SessionModel::with_expire_date(
			"custom_key".to_string(),
			json!({"custom": "data"}),
			expire_date,
		);

		assert_eq!(session.session_key(), "custom_key");
		assert_eq!(session.expire_date(), &expire_date);
	}

	#[test]
	fn test_session_model_getters() {
		let data = json!({"user_id": 42});
		let session = SessionModel::new("key_123".to_string(), data.clone(), 3600);

		assert_eq!(session.session_key(), "key_123");
		assert_eq!(session.session_data(), &data);
		assert!(session.expire_date() > &Utc::now());
	}

	#[test]
	fn test_session_model_set_session_data() {
		let mut session = SessionModel::new("key".to_string(), json!({}), 3600);

		let new_data = json!({"updated": true, "count": 42});
		session.set_session_data(new_data.clone());

		assert_eq!(session.session_data(), &new_data);
	}

	#[test]
	fn test_session_model_set_expire_date() {
		let mut session = SessionModel::new("key".to_string(), json!({}), 3600);

		let new_expire = Utc::now() + Duration::days(7);
		session.set_expire_date(new_expire);

		assert_eq!(session.expire_date(), &new_expire);
	}

	#[test]
	fn test_session_model_is_valid() {
		let valid_session = SessionModel::new("valid".to_string(), json!({}), 3600);
		assert!(valid_session.is_valid());

		let expired_session = SessionModel::new("expired".to_string(), json!({}), -100);
		assert!(!expired_session.is_valid());
	}

	#[test]
	fn test_session_model_is_expired() {
		let active_session = SessionModel::new("active".to_string(), json!({}), 3600);
		assert!(!active_session.is_expired());

		let expired_session = SessionModel::new("expired".to_string(), json!({}), -100);
		assert!(expired_session.is_expired());
	}

	#[test]
	fn test_session_model_extend() {
		let mut session = SessionModel::new("key".to_string(), json!({}), 3600);
		let original_expire = *session.expire_date();

		session.extend(1800);

		let expected_expire = original_expire + Duration::seconds(1800);
		assert_eq!(session.expire_date(), &expected_expire);
	}

	#[test]
	fn test_session_model_refresh() {
		let mut session = SessionModel::new("key".to_string(), json!({}), 3600);

		// Wait a moment to ensure time difference
		std::thread::sleep(std::time::Duration::from_millis(10));

		session.refresh(7200);

		// New expiration should be approximately 7200 seconds from now
		let expected = Utc::now() + Duration::seconds(7200);
		let diff = session
			.expire_date()
			.signed_duration_since(expected)
			.num_seconds()
			.abs();
		assert!(diff < 2);
	}

	#[test]
	#[cfg(feature = "database")]
	fn test_session_model_implements_model_trait() {
		let session = SessionModel::new("model_key".to_string(), json!({}), 3600);

		assert_eq!(SessionModel::table_name(), "sessions");
		assert_eq!(SessionModel::primary_key_field(), "session_key");
		assert_eq!(session.primary_key(), Some("model_key".to_string()));
	}

	#[test]
	#[cfg(feature = "database")]
	fn test_session_model_set_primary_key() {
		let mut session = SessionModel::new("old_key".to_string(), json!({}), 3600);

		session.set_primary_key("new_key".to_string());
		assert_eq!(session.session_key(), "new_key");
		assert_eq!(session.primary_key(), Some("new_key".to_string()));
	}

	#[test]
	fn test_session_model_serialization() {
		let session =
			SessionModel::new("serialize_test".to_string(), json!({"data": "value"}), 3600);

		// Serialize
		let serialized = serde_json::to_string(&session).unwrap();
		assert!(serialized.contains("serialize_test"));

		// Deserialize
		let deserialized: SessionModel = serde_json::from_str(&serialized).unwrap();
		assert_eq!(deserialized.session_key(), "serialize_test");
		assert_eq!(deserialized.session_data(), &json!({"data": "value"}));
	}

	#[test]
	fn test_session_model_edge_cases() {
		// Very short TTL
		let short_ttl = SessionModel::new("short".to_string(), json!({}), 1);
		assert!(short_ttl.is_valid());

		// Very long TTL
		let long_ttl = SessionModel::new("long".to_string(), json!({}), 86400 * 365);
		assert!(long_ttl.is_valid());

		// Zero TTL (immediately expired)
		let zero_ttl = SessionModel::new("zero".to_string(), json!({}), 0);
		assert!(!zero_ttl.is_valid());
	}
}
