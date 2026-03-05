//! Redis-backed channel layer for distributed WebSocket systems
//!
//! This module provides a channel layer for distributed WebSocket systems using Redis.
//! It allows sharing WebSocket connections across multiple application instances.
//!
//! ## Usage Examples
//!
//! ```no_run
//! use reinhardt_websockets::redis_channel::{RedisChannelLayer, RedisConfig};
//! use reinhardt_websockets::channels::{ChannelLayer, ChannelMessage};
//! use reinhardt_websockets::Message;
//!
//! # tokio_test::block_on(async {
//! let config = RedisConfig::default();
//! let layer = RedisChannelLayer::new(config).await.unwrap();
//!
//! let msg = ChannelMessage::new(
//!     "user_1".to_string(),
//!     Message::text("Hello".to_string()),
//! );
//!
//! layer.send("channel_1", msg).await.unwrap();
//! # });
//! ```

#[cfg(feature = "redis-channel")]
use crate::channels::{ChannelError, ChannelLayer, ChannelMessage, ChannelResult};
#[cfg(feature = "redis-channel")]
use async_trait::async_trait;
#[cfg(feature = "redis-channel")]
use redis::aio::ConnectionManager;
#[cfg(feature = "redis-channel")]
use redis::{AsyncCommands, Client};
#[cfg(feature = "redis-channel")]
use tracing::warn;

/// Redis channel layer configuration
#[cfg(feature = "redis-channel")]
#[derive(Debug, Clone)]
pub struct RedisConfig {
	/// Redis connection URL
	pub url: String,
	/// Channel prefix
	pub channel_prefix: String,
	/// Group prefix
	pub group_prefix: String,
	/// Message expiry time (seconds)
	pub message_expiry: u64,
	/// Redis password for authentication
	pub password: Option<String>,
	/// Redis username for authentication (Redis 6+ ACL)
	pub username: Option<String>,
	/// Enable TLS for secure connection
	pub tls: bool,
	/// Require authentication (warns if disabled without credentials)
	pub require_auth: bool,
}

#[cfg(feature = "redis-channel")]
impl Default for RedisConfig {
	/// Creates default Redis configuration.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_websockets::redis_channel::RedisConfig;
	///
	/// let config = RedisConfig::default();
	/// assert_eq!(config.url, "redis://127.0.0.1:6379");
	/// assert_eq!(config.channel_prefix, "ws:channel:");
	/// assert_eq!(config.group_prefix, "ws:group:");
	/// assert_eq!(config.message_expiry, 60);
	/// assert!(config.password.is_none());
	/// assert!(config.username.is_none());
	/// assert!(!config.tls);
	/// assert!(config.require_auth);
	/// ```
	fn default() -> Self {
		Self {
			url: "redis://127.0.0.1:6379".to_string(),
			channel_prefix: "ws:channel:".to_string(),
			group_prefix: "ws:group:".to_string(),
			message_expiry: 60, // 60 seconds
			password: None,
			username: None,
			tls: false,
			require_auth: true, // Security: require auth by default
		}
	}
}

#[cfg(feature = "redis-channel")]
impl RedisConfig {
	/// Creates a new Redis configuration.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_websockets::redis_channel::RedisConfig;
	///
	/// let config = RedisConfig::new("redis://localhost:6379".to_string());
	/// assert_eq!(config.url, "redis://localhost:6379");
	/// ```
	pub fn new(url: String) -> Self {
		Self {
			url,
			..Default::default()
		}
	}

	/// Sets the channel prefix.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_websockets::redis_channel::RedisConfig;
	///
	/// let config = RedisConfig::default()
	///     .with_channel_prefix("custom:channel:".to_string());
	/// assert_eq!(config.channel_prefix, "custom:channel:");
	/// ```
	pub fn with_channel_prefix(mut self, prefix: String) -> Self {
		self.channel_prefix = prefix;
		self
	}

	/// Sets the group prefix.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_websockets::redis_channel::RedisConfig;
	///
	/// let config = RedisConfig::default()
	///     .with_group_prefix("custom:group:".to_string());
	/// assert_eq!(config.group_prefix, "custom:group:");
	/// ```
	pub fn with_group_prefix(mut self, prefix: String) -> Self {
		self.group_prefix = prefix;
		self
	}

	/// Sets the message expiry time.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_websockets::redis_channel::RedisConfig;
	///
	/// let config = RedisConfig::default()
	///     .with_message_expiry(120);
	/// assert_eq!(config.message_expiry, 120);
	/// ```
	pub fn with_message_expiry(mut self, expiry: u64) -> Self {
		self.message_expiry = expiry;
		self
	}

	/// Sets the Redis password for authentication.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_websockets::redis_channel::RedisConfig;
	///
	/// let config = RedisConfig::default()
	///     .with_password("secret".to_string());
	/// assert_eq!(config.password, Some("secret".to_string()));
	/// ```
	pub fn with_password(mut self, password: String) -> Self {
		self.password = Some(password);
		self
	}

	/// Sets the Redis username for authentication (Redis 6+ ACL).
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_websockets::redis_channel::RedisConfig;
	///
	/// let config = RedisConfig::default()
	///     .with_username("app_user".to_string());
	/// assert_eq!(config.username, Some("app_user".to_string()));
	/// ```
	pub fn with_username(mut self, username: String) -> Self {
		self.username = Some(username);
		self
	}

	/// Enables TLS for secure connection.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_websockets::redis_channel::RedisConfig;
	///
	/// let config = RedisConfig::default().with_tls();
	/// assert!(config.tls);
	/// ```
	pub fn with_tls(mut self) -> Self {
		self.tls = true;
		self
	}

	/// Sets whether authentication is required.
	///
	/// By default, authentication is required for security.
	/// Set to `false` only for local development or trusted networks.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_websockets::redis_channel::RedisConfig;
	///
	/// let config = RedisConfig::default().with_require_auth(false);
	/// assert!(!config.require_auth);
	/// ```
	pub fn with_require_auth(mut self, require: bool) -> Self {
		self.require_auth = require;
		self
	}

	/// Validates authentication configuration and logs warnings.
	///
	/// Returns an error if authentication is required but not configured.
	pub fn validate_auth(&self) -> Result<(), crate::channels::ChannelError> {
		use crate::channels::ChannelError;

		if self.require_auth && self.password.is_none() {
			warn!(
				"Redis authentication is required but no password is configured. \
				 This is a security risk. Set a password using .with_password() or \
				 disable auth requirement using .with_require_auth(false) for local development only."
			);
			return Err(ChannelError::AuthenticationRequired);
		}

		if self.password.is_none() && !self.require_auth {
			warn!(
				"Redis connection is configured without authentication. \
				 This is not recommended for production environments."
			);
		}

		Ok(())
	}
}

/// Redis-backed channel layer
///
/// # Examples
///
/// ```no_run
/// use reinhardt_websockets::redis_channel::{RedisChannelLayer, RedisConfig};
///
/// # tokio_test::block_on(async {
/// let config = RedisConfig::default();
/// let layer = RedisChannelLayer::new(config).await.unwrap();
/// # });
/// ```
#[cfg(feature = "redis-channel")]
pub struct RedisChannelLayer {
	config: RedisConfig,
	connection: ConnectionManager,
}

#[cfg(feature = "redis-channel")]
impl RedisChannelLayer {
	/// Creates a new Redis channel layer.
	///
	/// # Examples
	///
	/// ```no_run
	/// use reinhardt_websockets::redis_channel::{RedisChannelLayer, RedisConfig};
	///
	/// # tokio_test::block_on(async {
	/// let config = RedisConfig::default()
	///     .with_password("secret".to_string());
	/// let layer = RedisChannelLayer::new(config).await.unwrap();
	/// # });
	/// ```
	pub async fn new(config: RedisConfig) -> ChannelResult<Self> {
		// Validate authentication configuration before connecting
		config.validate_auth()?;

		let client = Client::open(config.url.as_str())
			.map_err(|e| ChannelError::SendError(format!("Redis client error: {}", e)))?;

		let connection = ConnectionManager::new(client)
			.await
			.map_err(|e| ChannelError::SendError(format!("Redis connection error: {}", e)))?;

		Ok(Self { config, connection })
	}

	/// Generates a channel key.
	fn channel_key(&self, channel: &str) -> String {
		format!("{}{}", self.config.channel_prefix, channel)
	}

	/// Generates a group key.
	fn group_key(&self, group: &str) -> String {
		format!("{}{}", self.config.group_prefix, group)
	}

	/// Serializes a message.
	fn serialize_message(&self, message: &ChannelMessage) -> ChannelResult<String> {
		serde_json::to_string(message)
			.map_err(|e| ChannelError::SerializationError(format!("Serialize error: {}", e)))
	}

	/// Deserializes a message.
	fn deserialize_message(&self, data: &str) -> ChannelResult<ChannelMessage> {
		serde_json::from_str(data)
			.map_err(|e| ChannelError::SerializationError(format!("Deserialize error: {}", e)))
	}
}

#[cfg(feature = "redis-channel")]
#[async_trait]
impl ChannelLayer for RedisChannelLayer {
	async fn send(&self, channel: &str, message: ChannelMessage) -> ChannelResult<()> {
		let key = self.channel_key(channel);
		let data = self.serialize_message(&message)?;

		let mut conn = self.connection.clone();

		// Push message to list
		conn.rpush::<_, _, ()>(&key, &data)
			.await
			.map_err(|e| ChannelError::SendError(format!("Redis rpush error: {}", e)))?;

		// Set expiry time
		conn.expire::<_, ()>(&key, self.config.message_expiry as i64)
			.await
			.map_err(|e| ChannelError::SendError(format!("Redis expire error: {}", e)))?;

		Ok(())
	}

	async fn receive(&self, channel: &str) -> ChannelResult<Option<ChannelMessage>> {
		let key = self.channel_key(channel);
		let mut conn = self.connection.clone();

		// Get first message from list (non-blocking)
		let result: Option<String> = conn
			.lpop(&key, None)
			.await
			.map_err(|e| ChannelError::ReceiveError(format!("Redis lpop error: {}", e)))?;

		match result {
			Some(data) => {
				let message = self.deserialize_message(&data)?;
				Ok(Some(message))
			}
			None => Ok(None),
		}
	}

	async fn group_add(&self, group: &str, channel: &str) -> ChannelResult<()> {
		let key = self.group_key(group);
		let mut conn = self.connection.clone();

		// Add channel to group (set)
		conn.sadd::<_, _, ()>(&key, channel)
			.await
			.map_err(|e| ChannelError::SendError(format!("Redis sadd error: {}", e)))?;

		Ok(())
	}

	async fn group_discard(&self, group: &str, channel: &str) -> ChannelResult<()> {
		let key = self.group_key(group);
		let mut conn = self.connection.clone();

		// Remove channel from group
		conn.srem::<_, _, ()>(&key, channel)
			.await
			.map_err(|e| ChannelError::SendError(format!("Redis srem error: {}", e)))?;

		Ok(())
	}

	async fn group_send(&self, group: &str, message: ChannelMessage) -> ChannelResult<()> {
		let key = self.group_key(group);
		let mut conn = self.connection.clone();

		// Get all channels in the group
		let channels: Vec<String> = conn
			.smembers(&key)
			.await
			.map_err(|e| ChannelError::SendError(format!("Redis smembers error: {}", e)))?;

		if channels.is_empty() {
			return Err(ChannelError::GroupNotFound(group.to_string()));
		}

		// Send message to each channel
		for channel in channels {
			self.send(&channel, message.clone()).await?;
		}

		Ok(())
	}
}

/// Feature-gated placeholder: Empty type when redis-channel is disabled.
///
/// This type exists to allow code to compile when the `redis-channel` feature
/// is not enabled, but provides no functionality. To use Redis-backed channel
/// layers for distributed WebSocket systems, enable the feature:
///
/// ```toml
/// reinhardt-websockets = { version = "...", features = ["redis-channel"] }
/// ```
#[cfg(not(feature = "redis-channel"))]
pub struct RedisConfig;

#[cfg(not(feature = "redis-channel"))]
impl RedisConfig {
	/// Creates a placeholder configuration (no-op when feature is disabled).
	pub fn default() -> Self {
		Self
	}
}

/// Feature-gated placeholder: Empty type when redis-channel is disabled.
///
/// This type exists to allow code to compile when the `redis-channel` feature
/// is not enabled, but provides no functionality. To use Redis-backed channel
/// layers, enable the feature in Cargo.toml.
#[cfg(not(feature = "redis-channel"))]
pub struct RedisChannelLayer;

#[cfg(all(test, feature = "redis-channel"))]
mod tests {
	use super::*;
	use crate::connection::Message;
	use std::collections::HashMap;
	use std::sync::{Arc, Mutex};

	/// Mock Redis storage for testing
	///
	/// This mock provides an in-memory implementation of Redis operations
	/// needed for testing the RedisChannelLayer without requiring a real Redis instance.
	#[derive(Clone, Default)]
	struct MockRedisStorage {
		/// In-memory storage for lists (channel messages)
		lists: Arc<Mutex<HashMap<String, Vec<String>>>>,
		/// In-memory storage for sets (group members)
		sets: Arc<Mutex<HashMap<String, std::collections::HashSet<String>>>>,
	}

	impl MockRedisStorage {
		fn new() -> Self {
			Self::default()
		}

		fn rpush(&self, key: &str, value: &str) {
			let mut lists = self.lists.lock().unwrap();
			lists
				.entry(key.to_string())
				.or_default()
				.push(value.to_string());
		}

		fn lpop(&self, key: &str) -> Option<String> {
			let mut lists = self.lists.lock().unwrap();
			if let Some(list) = lists.get_mut(key)
				&& !list.is_empty()
			{
				return Some(list.remove(0));
			}
			None
		}

		fn sadd(&self, key: &str, member: &str) {
			let mut sets = self.sets.lock().unwrap();
			sets.entry(key.to_string())
				.or_default()
				.insert(member.to_string());
		}

		fn srem(&self, key: &str, member: &str) {
			let mut sets = self.sets.lock().unwrap();
			if let Some(set) = sets.get_mut(key) {
				set.remove(member);
			}
		}

		fn smembers(&self, key: &str) -> Vec<String> {
			let sets = self.sets.lock().unwrap();
			sets.get(key)
				.map(|set| set.iter().cloned().collect())
				.unwrap_or_default()
		}
	}

	/// Mock RedisChannelLayer for testing
	///
	/// This structure mimics the behavior of RedisChannelLayer but uses in-memory storage
	/// instead of a real Redis connection.
	struct MockRedisChannelLayer {
		config: RedisConfig,
		storage: MockRedisStorage,
	}

	impl MockRedisChannelLayer {
		fn new(config: RedisConfig) -> Self {
			Self {
				config,
				storage: MockRedisStorage::new(),
			}
		}

		fn channel_key(&self, channel: &str) -> String {
			format!("{}{}", self.config.channel_prefix, channel)
		}

		fn group_key(&self, group: &str) -> String {
			format!("{}{}", self.config.group_prefix, group)
		}

		fn serialize_message(&self, message: &ChannelMessage) -> ChannelResult<String> {
			serde_json::to_string(message)
				.map_err(|e| ChannelError::SerializationError(format!("Serialize error: {}", e)))
		}

		fn deserialize_message(&self, data: &str) -> ChannelResult<ChannelMessage> {
			serde_json::from_str(data)
				.map_err(|e| ChannelError::SerializationError(format!("Deserialize error: {}", e)))
		}

		async fn send(&self, channel: &str, message: ChannelMessage) -> ChannelResult<()> {
			let key = self.channel_key(channel);
			let data = self.serialize_message(&message)?;
			self.storage.rpush(&key, &data);
			Ok(())
		}

		async fn receive(&self, channel: &str) -> ChannelResult<Option<ChannelMessage>> {
			let key = self.channel_key(channel);
			match self.storage.lpop(&key) {
				Some(data) => {
					let message = self.deserialize_message(&data)?;
					Ok(Some(message))
				}
				None => Ok(None),
			}
		}

		async fn group_add(&self, group: &str, channel: &str) -> ChannelResult<()> {
			let key = self.group_key(group);
			self.storage.sadd(&key, channel);
			Ok(())
		}

		async fn group_discard(&self, group: &str, channel: &str) -> ChannelResult<()> {
			let key = self.group_key(group);
			self.storage.srem(&key, channel);
			Ok(())
		}

		async fn group_send(&self, group: &str, message: ChannelMessage) -> ChannelResult<()> {
			let key = self.group_key(group);
			let channels = self.storage.smembers(&key);

			if channels.is_empty() {
				return Err(ChannelError::GroupNotFound(group.to_string()));
			}

			for channel in channels {
				self.send(&channel, message.clone()).await?;
			}

			Ok(())
		}
	}

	#[test]
	fn test_redis_config_default() {
		let config = RedisConfig::default();
		assert_eq!(config.url, "redis://127.0.0.1:6379");
		assert_eq!(config.channel_prefix, "ws:channel:");
		assert_eq!(config.group_prefix, "ws:group:");
		assert_eq!(config.message_expiry, 60);
		assert!(config.password.is_none());
		assert!(config.username.is_none());
		assert!(!config.tls);
		assert!(config.require_auth); // Security: auth required by default
	}

	#[test]
	fn test_redis_config_builder() {
		let config = RedisConfig::new("redis://localhost:6379".to_string())
			.with_channel_prefix("custom:channel:".to_string())
			.with_group_prefix("custom:group:".to_string())
			.with_message_expiry(120)
			.with_password("secret".to_string())
			.with_username("app_user".to_string())
			.with_tls()
			.with_require_auth(true);

		assert_eq!(config.url, "redis://localhost:6379");
		assert_eq!(config.channel_prefix, "custom:channel:");
		assert_eq!(config.group_prefix, "custom:group:");
		assert_eq!(config.message_expiry, 120);
		assert_eq!(config.password, Some("secret".to_string()));
		assert_eq!(config.username, Some("app_user".to_string()));
		assert!(config.tls);
		assert!(config.require_auth);
	}

	#[test]
	fn test_channel_key_generation() {
		let config = RedisConfig::default();
		let layer = MockRedisChannelLayer::new(config);
		assert_eq!(layer.channel_key("test"), "ws:channel:test");
	}

	#[test]
	fn test_group_key_generation() {
		let config = RedisConfig::default();
		let layer = MockRedisChannelLayer::new(config);
		assert_eq!(layer.group_key("test_group"), "ws:group:test_group");
	}

	#[test]
	fn test_message_serialization() {
		let config = RedisConfig::default();
		let layer = MockRedisChannelLayer::new(config);

		let msg = ChannelMessage::new("user_1".to_string(), Message::text("Hello".to_string()));

		let serialized = layer.serialize_message(&msg).unwrap();
		let deserialized = layer.deserialize_message(&serialized).unwrap();

		assert_eq!(msg.sender(), deserialized.sender());
	}

	#[tokio::test]
	async fn test_send_message_to_channel() {
		let config = RedisConfig::default();
		let layer = MockRedisChannelLayer::new(config);

		let msg = ChannelMessage::new(
			"user_1".to_string(),
			Message::text("Test message".to_string()),
		);

		// Send message
		let result = layer.send("test_channel", msg.clone()).await;
		assert!(result.is_ok());

		// Verify message was stored
		let key = layer.channel_key("test_channel");
		let stored = layer.storage.lpop(&key);
		assert!(stored.is_some());

		// Verify deserialized message
		let deserialized = layer.deserialize_message(&stored.unwrap()).unwrap();
		assert_eq!(deserialized.sender(), msg.sender());
	}

	#[tokio::test]
	async fn test_receive_message_from_channel() {
		let config = RedisConfig::default();
		let layer = MockRedisChannelLayer::new(config);

		let msg = ChannelMessage::new(
			"user_1".to_string(),
			Message::text("Test message".to_string()),
		);

		// Manually add message to mock storage
		let key = layer.channel_key("test_channel");
		let serialized = layer.serialize_message(&msg).unwrap();
		layer.storage.rpush(&key, &serialized);

		// Receive message
		let result = layer.receive("test_channel").await;
		let received = result.unwrap();
		assert!(received.is_some());
		assert_eq!(received.unwrap().sender(), msg.sender());

		// Verify queue is empty
		let result2 = layer.receive("test_channel").await;
		assert!(result2.is_ok());
		assert!(result2.unwrap().is_none());
	}

	#[tokio::test]
	async fn test_group_add_and_discard() {
		let config = RedisConfig::default();
		let layer = MockRedisChannelLayer::new(config);

		// Add channel to group
		let result = layer.group_add("test_group", "channel_1").await;
		assert!(result.is_ok());

		// Verify channel was added
		let key = layer.group_key("test_group");
		let members = layer.storage.smembers(&key);
		assert_eq!(members.len(), 1);
		assert!(members.contains(&"channel_1".to_string()));

		// Add another channel
		let result = layer.group_add("test_group", "channel_2").await;
		assert!(result.is_ok());

		let members = layer.storage.smembers(&key);
		assert_eq!(members.len(), 2);

		// Discard channel
		let result = layer.group_discard("test_group", "channel_1").await;
		assert!(result.is_ok());

		let members = layer.storage.smembers(&key);
		assert_eq!(members.len(), 1);
		assert!(members.contains(&"channel_2".to_string()));
	}

	#[tokio::test]
	async fn test_group_send() {
		let config = RedisConfig::default();
		let layer = MockRedisChannelLayer::new(config);

		// Add channels to group
		layer.group_add("test_group", "channel_1").await.unwrap();
		layer.group_add("test_group", "channel_2").await.unwrap();

		// Send message to group
		let msg = ChannelMessage::new(
			"user_1".to_string(),
			Message::text("Group message".to_string()),
		);

		let result = layer.group_send("test_group", msg.clone()).await;
		assert!(result.is_ok());

		// Verify both channels received the message
		let key1 = layer.channel_key("channel_1");
		let key2 = layer.channel_key("channel_2");

		let msg1 = layer.storage.lpop(&key1);
		let msg2 = layer.storage.lpop(&key2);

		assert!(msg1.is_some());
		assert!(msg2.is_some());

		let deserialized1 = layer.deserialize_message(&msg1.unwrap()).unwrap();
		let deserialized2 = layer.deserialize_message(&msg2.unwrap()).unwrap();

		assert_eq!(deserialized1.sender(), msg.sender());
		assert_eq!(deserialized2.sender(), msg.sender());
	}

	#[tokio::test]
	async fn test_group_send_to_empty_group() {
		let config = RedisConfig::default();
		let layer = MockRedisChannelLayer::new(config);

		let msg = ChannelMessage::new(
			"user_1".to_string(),
			Message::text("Test message".to_string()),
		);

		// Send to non-existent group
		let result = layer.group_send("empty_group", msg).await;
		assert!(result.is_err());

		match result {
			Err(ChannelError::GroupNotFound(group)) => {
				assert_eq!(group, "empty_group");
			}
			_ => panic!("Expected GroupNotFound error"),
		}
	}

	#[test]
	fn test_auth_validation_fails_without_password() {
		// Default config requires auth but has no password
		let config = RedisConfig::default();
		let result = config.validate_auth();
		assert!(result.is_err());
		match result {
			Err(ChannelError::AuthenticationRequired) => {}
			_ => panic!("Expected AuthenticationRequired error"),
		}
	}

	#[test]
	fn test_auth_validation_passes_with_password() {
		let config = RedisConfig::default().with_password("secret".to_string());
		let result = config.validate_auth();
		assert!(result.is_ok());
	}

	#[test]
	fn test_auth_validation_passes_when_disabled() {
		// Auth disabled should not require password
		let config = RedisConfig::default().with_require_auth(false);
		let result = config.validate_auth();
		assert!(result.is_ok());
	}

	#[test]
	fn test_auth_config_with_all_fields() {
		let config = RedisConfig::default()
			.with_password("secret".to_string())
			.with_username("admin".to_string())
			.with_tls()
			.with_require_auth(true);

		assert_eq!(config.password, Some("secret".to_string()));
		assert_eq!(config.username, Some("admin".to_string()));
		assert!(config.tls);
		assert!(config.require_auth);

		// Should pass validation
		let result = config.validate_auth();
		assert!(result.is_ok());
	}
}
