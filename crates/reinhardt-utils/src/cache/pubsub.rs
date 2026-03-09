//! Redis Pub/Sub support for cache invalidation.
//!
//! This module provides publish/subscribe functionality for coordinating
//! cache invalidation across multiple application instances.
//!
//! # Examples
//!
//! ```rust,no_run
//! use reinhardt_utils::cache::{CacheInvalidationChannel, CacheInvalidationMessage};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let channel = CacheInvalidationChannel::new("redis://127.0.0.1:6379").await?;
//!
//!     // Publisher
//!     channel.invalidate("user:123").await?;
//!     channel.invalidate_pattern("user:*").await?;
//!
//!     // Subscriber
//!     let mut subscriber = channel.subscribe().await?;
//!     while let Some(msg) = subscriber.next_message().await? {
//!         println!("Invalidate: {:?}", msg);
//!     }
//!
//!     Ok(())
//! }
//! ```

use futures::StreamExt;
use redis::{AsyncCommands, Client, aio::PubSub};
use reinhardt_core::exception::{Error, Result};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::Mutex;

/// Cache invalidation message types.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CacheInvalidationMessage {
	/// Invalidate a specific key
	InvalidateKey {
		/// The cache key to invalidate.
		key: String,
	},
	/// Invalidate all keys matching a pattern
	InvalidatePattern {
		/// The glob pattern to match cache keys for invalidation.
		pattern: String,
	},
	/// Clear all cache
	ClearAll,
}

/// Cache invalidation pub/sub channel.
pub struct CacheInvalidationChannel {
	client: Client,
	channel_name: String,
}

impl CacheInvalidationChannel {
	/// Create a new pub/sub channel.
	pub async fn new(redis_url: &str) -> Result<Self> {
		let client =
			Client::open(redis_url).map_err(|e| Error::Http(format!("Redis error: {}", e)))?;

		Ok(Self {
			client,
			channel_name: "cache:invalidation".to_string(),
		})
	}

	/// Create with custom channel name.
	pub async fn with_channel_name(redis_url: &str, channel_name: String) -> Result<Self> {
		let client =
			Client::open(redis_url).map_err(|e| Error::Http(format!("Redis error: {}", e)))?;

		Ok(Self {
			client,
			channel_name,
		})
	}

	/// Publish a cache invalidation message for a specific key.
	pub async fn invalidate(&self, key: &str) -> Result<()> {
		let msg = CacheInvalidationMessage::InvalidateKey {
			key: key.to_string(),
		};

		self.publish(msg).await
	}

	/// Publish a cache invalidation message for a pattern.
	pub async fn invalidate_pattern(&self, pattern: &str) -> Result<()> {
		let msg = CacheInvalidationMessage::InvalidatePattern {
			pattern: pattern.to_string(),
		};

		self.publish(msg).await
	}

	/// Publish a clear all message.
	pub async fn clear_all(&self) -> Result<()> {
		let msg = CacheInvalidationMessage::ClearAll;
		self.publish(msg).await
	}

	/// Publish a message to the channel.
	async fn publish(&self, message: CacheInvalidationMessage) -> Result<()> {
		let mut conn = self
			.client
			.get_multiplexed_async_connection()
			.await
			.map_err(|e| Error::Http(format!("Redis error: {}", e)))?;

		let json = serde_json::to_string(&message)
			.map_err(|e| Error::Serialization(format!("Serialization error: {}", e)))?;

		let _: () = conn
			.publish(&self.channel_name, json)
			.await
			.map_err(|e| Error::Http(format!("Redis error: {}", e)))?;

		Ok(())
	}

	/// Subscribe to invalidation messages.
	pub async fn subscribe(&self) -> Result<CacheInvalidationSubscriber> {
		let mut pubsub = self
			.client
			.get_async_pubsub()
			.await
			.map_err(|e| Error::Http(format!("Redis error: {}", e)))?;

		pubsub
			.subscribe(&self.channel_name)
			.await
			.map_err(|e| Error::Http(format!("Redis error: {}", e)))?;

		Ok(CacheInvalidationSubscriber {
			pubsub: Arc::new(Mutex::new(pubsub)),
		})
	}
}

/// Subscriber for cache invalidation messages.
pub struct CacheInvalidationSubscriber {
	pubsub: Arc<Mutex<PubSub>>,
}

impl CacheInvalidationSubscriber {
	/// Get the next invalidation message.
	pub async fn next_message(&mut self) -> Result<Option<CacheInvalidationMessage>> {
		let mut pubsub = self.pubsub.lock().await;

		match pubsub.on_message().next().await {
			Some(msg) => {
				let payload: String = msg
					.get_payload()
					.map_err(|e| Error::Http(format!("Redis error: {}", e)))?;

				let message: CacheInvalidationMessage = serde_json::from_str(&payload)
					.map_err(|e| Error::Serialization(format!("Deserialization error: {}", e)))?;

				Ok(Some(message))
			}
			None => Ok(None),
		}
	}
}
