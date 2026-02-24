//! Channel layers for distributed WebSocket systems
//!
//! This module provides channel layer abstractions for distributed WebSocket communication,
//! inspired by Django Channels. Channel layers enable multiple application instances
//! to communicate with each other and share WebSocket connections.

use crate::connection::Message;
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{RwLock, mpsc};

/// Channel layer result type
pub type ChannelResult<T> = Result<T, ChannelError>;

/// Channel layer errors
#[derive(Debug, thiserror::Error)]
pub enum ChannelError {
	#[error("Send error: {0}")]
	SendError(String),
	#[error("Receive error: {0}")]
	ReceiveError(String),
	#[error("Channel not found: {0}")]
	ChannelNotFound(String),
	#[error("Group not found: {0}")]
	GroupNotFound(String),
	#[error("Serialization error: {0}")]
	SerializationError(String),
	#[error("Authentication required for Redis connection")]
	AuthenticationRequired,
}

/// Channel message for distributed communication
///
/// # Examples
///
/// ```
/// use reinhardt_websockets::channels::ChannelMessage;
/// use reinhardt_websockets::Message;
///
/// let msg = ChannelMessage::new(
///     "user_1".to_string(),
///     Message::text("Hello".to_string()),
/// );
///
/// assert_eq!(msg.sender(), "user_1");
/// ```
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ChannelMessage {
	sender: String,
	payload: Message,
	metadata: HashMap<String, String>,
}

impl ChannelMessage {
	/// Create a new channel message
	pub fn new(sender: String, payload: Message) -> Self {
		Self {
			sender,
			payload,
			metadata: HashMap::new(),
		}
	}

	/// Add metadata to the message
	pub fn with_metadata(mut self, key: String, value: String) -> Self {
		self.metadata.insert(key, value);
		self
	}

	/// Get the sender
	pub fn sender(&self) -> &str {
		&self.sender
	}

	/// Get the payload
	pub fn payload(&self) -> &Message {
		&self.payload
	}

	/// Get metadata
	pub fn metadata(&self, key: &str) -> Option<&String> {
		self.metadata.get(key)
	}
}

/// Channel layer trait for distributed messaging
#[async_trait]
pub trait ChannelLayer: Send + Sync {
	/// Send a message to a specific channel
	async fn send(&self, channel: &str, message: ChannelMessage) -> ChannelResult<()>;

	/// Receive a message from a specific channel
	async fn receive(&self, channel: &str) -> ChannelResult<Option<ChannelMessage>>;

	/// Add a channel to a group
	async fn group_add(&self, group: &str, channel: &str) -> ChannelResult<()>;

	/// Remove a channel from a group
	async fn group_discard(&self, group: &str, channel: &str) -> ChannelResult<()>;

	/// Send a message to all channels in a group
	async fn group_send(&self, group: &str, message: ChannelMessage) -> ChannelResult<()>;
}

/// In-memory channel layer implementation
///
/// # Examples
///
/// ```
/// use reinhardt_websockets::channels::{InMemoryChannelLayer, ChannelLayer, ChannelMessage};
/// use reinhardt_websockets::Message;
///
/// # tokio_test::block_on(async {
/// let layer = InMemoryChannelLayer::new();
///
/// let msg = ChannelMessage::new(
///     "user_1".to_string(),
///     Message::text("Hello".to_string()),
/// );
///
/// layer.send("channel_1", msg.clone()).await.unwrap();
/// let received = layer.receive("channel_1").await.unwrap();
///
/// assert!(received.is_some());
/// # });
/// ```
pub struct InMemoryChannelLayer {
	channels: Arc<RwLock<HashMap<String, mpsc::UnboundedSender<ChannelMessage>>>>,
	receivers: Arc<RwLock<HashMap<String, mpsc::UnboundedReceiver<ChannelMessage>>>>,
	groups: Arc<RwLock<HashMap<String, Vec<String>>>>,
}

impl InMemoryChannelLayer {
	/// Create a new in-memory channel layer
	pub fn new() -> Self {
		Self {
			channels: Arc::new(RwLock::new(HashMap::new())),
			receivers: Arc::new(RwLock::new(HashMap::new())),
			groups: Arc::new(RwLock::new(HashMap::new())),
		}
	}

	/// Create or get a channel
	async fn get_or_create_channel(&self, channel: &str) -> mpsc::UnboundedSender<ChannelMessage> {
		let mut channels = self.channels.write().await;

		if let Some(tx) = channels.get(channel) {
			return tx.clone();
		}

		let (tx, rx) = mpsc::unbounded_channel();
		channels.insert(channel.to_string(), tx.clone());

		let mut receivers = self.receivers.write().await;
		receivers.insert(channel.to_string(), rx);

		tx
	}

	/// Get channel count
	pub async fn channel_count(&self) -> usize {
		let channels = self.channels.read().await;
		channels.len()
	}

	/// Get group count
	pub async fn group_count(&self) -> usize {
		let groups = self.groups.read().await;
		groups.len()
	}

	/// Clear all channels and groups
	pub async fn clear(&self) {
		let mut channels = self.channels.write().await;
		let mut receivers = self.receivers.write().await;
		let mut groups = self.groups.write().await;

		channels.clear();
		receivers.clear();
		groups.clear();
	}
}

impl Default for InMemoryChannelLayer {
	fn default() -> Self {
		Self::new()
	}
}

#[async_trait]
impl ChannelLayer for InMemoryChannelLayer {
	async fn send(&self, channel: &str, message: ChannelMessage) -> ChannelResult<()> {
		let tx = self.get_or_create_channel(channel).await;

		tx.send(message)
			.map_err(|e| ChannelError::SendError(e.to_string()))
	}

	async fn receive(&self, channel: &str) -> ChannelResult<Option<ChannelMessage>> {
		let mut receivers = self.receivers.write().await;

		if let Some(rx) = receivers.get_mut(channel) {
			Ok(rx.try_recv().ok())
		} else {
			Ok(None)
		}
	}

	async fn group_add(&self, group: &str, channel: &str) -> ChannelResult<()> {
		let mut groups = self.groups.write().await;

		let channels = groups.entry(group.to_string()).or_insert_with(Vec::new);

		if !channels.contains(&channel.to_string()) {
			channels.push(channel.to_string());
		}

		Ok(())
	}

	async fn group_discard(&self, group: &str, channel: &str) -> ChannelResult<()> {
		let mut groups = self.groups.write().await;

		if let Some(channels) = groups.get_mut(group) {
			channels.retain(|c| c != channel);

			if channels.is_empty() {
				groups.remove(group);
			}
		}

		Ok(())
	}

	async fn group_send(&self, group: &str, message: ChannelMessage) -> ChannelResult<()> {
		let groups = self.groups.read().await;

		let channels = groups
			.get(group)
			.ok_or_else(|| ChannelError::GroupNotFound(group.to_string()))?;

		for channel in channels {
			self.send(channel, message.clone()).await?;
		}

		Ok(())
	}
}

/// Channel layer wrapper with additional features
///
/// # Examples
///
/// ```
/// use reinhardt_websockets::channels::{ChannelLayerWrapper, InMemoryChannelLayer};
///
/// let layer = InMemoryChannelLayer::new();
/// let wrapper = ChannelLayerWrapper::new(Box::new(layer));
/// ```
pub struct ChannelLayerWrapper {
	layer: Box<dyn ChannelLayer>,
}

impl ChannelLayerWrapper {
	/// Create a new channel layer wrapper
	pub fn new(layer: Box<dyn ChannelLayer>) -> Self {
		Self { layer }
	}

	/// Send a message to a channel
	pub async fn send(&self, channel: &str, message: ChannelMessage) -> ChannelResult<()> {
		self.layer.send(channel, message).await
	}

	/// Receive a message from a channel
	pub async fn receive(&self, channel: &str) -> ChannelResult<Option<ChannelMessage>> {
		self.layer.receive(channel).await
	}

	/// Add a channel to a group
	pub async fn group_add(&self, group: &str, channel: &str) -> ChannelResult<()> {
		self.layer.group_add(group, channel).await
	}

	/// Remove a channel from a group
	pub async fn group_discard(&self, group: &str, channel: &str) -> ChannelResult<()> {
		self.layer.group_discard(group, channel).await
	}

	/// Send a message to a group
	pub async fn group_send(&self, group: &str, message: ChannelMessage) -> ChannelResult<()> {
		self.layer.group_send(group, message).await
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_channel_message_creation() {
		let msg = ChannelMessage::new("user_1".to_string(), Message::text("Hello".to_string()));
		assert_eq!(msg.sender(), "user_1");
	}

	#[test]
	fn test_channel_message_metadata() {
		let msg = ChannelMessage::new("user_1".to_string(), Message::text("Hello".to_string()))
			.with_metadata("priority".to_string(), "high".to_string());

		assert_eq!(msg.metadata("priority").unwrap(), "high");
	}

	#[tokio::test]
	async fn test_in_memory_channel_layer_send_receive() {
		let layer = InMemoryChannelLayer::new();
		let msg = ChannelMessage::new("user_1".to_string(), Message::text("Hello".to_string()));

		layer.send("channel_1", msg.clone()).await.unwrap();

		let received = layer.receive("channel_1").await.unwrap();
		assert!(received.is_some());
		assert_eq!(received.unwrap().sender(), "user_1");
	}

	#[tokio::test]
	async fn test_in_memory_channel_layer_group_add() {
		let layer = InMemoryChannelLayer::new();

		layer.group_add("group_1", "channel_1").await.unwrap();
		layer.group_add("group_1", "channel_2").await.unwrap();

		assert_eq!(layer.group_count().await, 1);
	}

	#[tokio::test]
	async fn test_in_memory_channel_layer_group_discard() {
		let layer = InMemoryChannelLayer::new();

		layer.group_add("group_1", "channel_1").await.unwrap();
		layer.group_add("group_1", "channel_2").await.unwrap();

		layer.group_discard("group_1", "channel_1").await.unwrap();

		assert_eq!(layer.group_count().await, 1);
	}

	#[tokio::test]
	async fn test_in_memory_channel_layer_group_send() {
		let layer = InMemoryChannelLayer::new();

		layer.group_add("group_1", "channel_1").await.unwrap();
		layer.group_add("group_1", "channel_2").await.unwrap();

		let msg = ChannelMessage::new("user_1".to_string(), Message::text("Broadcast".to_string()));

		layer.group_send("group_1", msg).await.unwrap();

		let received1 = layer.receive("channel_1").await.unwrap();
		let received2 = layer.receive("channel_2").await.unwrap();

		assert!(received1.is_some());
		assert!(received2.is_some());
	}

	#[tokio::test]
	async fn test_in_memory_channel_layer_clear() {
		let layer = InMemoryChannelLayer::new();
		let msg = ChannelMessage::new("user_1".to_string(), Message::text("Test".to_string()));

		layer.send("channel_1", msg).await.unwrap();
		layer.group_add("group_1", "channel_1").await.unwrap();

		assert_eq!(layer.channel_count().await, 1);
		assert_eq!(layer.group_count().await, 1);

		layer.clear().await;

		assert_eq!(layer.channel_count().await, 0);
		assert_eq!(layer.group_count().await, 0);
	}

	#[tokio::test]
	async fn test_channel_layer_wrapper() {
		let layer = InMemoryChannelLayer::new();
		let wrapper = ChannelLayerWrapper::new(Box::new(layer));

		let msg = ChannelMessage::new("user_1".to_string(), Message::text("Hello".to_string()));

		wrapper.send("channel_1", msg.clone()).await.unwrap();

		let received = wrapper.receive("channel_1").await.unwrap();
		assert!(received.is_some());
	}
}
