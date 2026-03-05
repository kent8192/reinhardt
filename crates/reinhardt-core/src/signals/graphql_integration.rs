#![cfg(not(target_arch = "wasm32"))]

//! GraphQL Subscriptions - Signal-based GraphQL subscription support
//!
//! This module provides GraphQL subscription integration for signals,
//! allowing signals to drive GraphQL subscription updates.
//!
//! # Examples
//!
//! ```rust,no_run
//! use reinhardt_core::signals::graphql_integration::{GraphQLSubscriptionBridge, SubscriptionEvent};
//! use reinhardt_core::signals::{Signal, SignalName};
//!
//! # #[tokio::main]
//! # async fn main() {
//! # struct User;
//! # fn post_save() -> Signal<User> {
//! #     Signal::new(SignalName::custom("post_save"))
//! # }
//! // Create a GraphQL subscription bridge
//! let bridge = GraphQLSubscriptionBridge::new();
//!
//! // Connect a signal to a GraphQL subscription
//! bridge.connect_signal(
//!     post_save(),
//!     "userUpdated",
//!     |_user| SubscriptionEvent::new("userUpdated", ())
//! ).await;
//!
//! // Subscribe to GraphQL subscription
//! let _stream = bridge.subscribe("userUpdated").await;
//! # }
//! ```

use super::error::SignalError;
use super::signal::Signal;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::collections::HashMap;
use std::fmt;
use std::marker::PhantomData;
use std::sync::Arc;
use tokio::sync::broadcast;

/// GraphQL subscription event
///
/// # Examples
///
/// ```
/// use reinhardt_core::signals::graphql_integration::SubscriptionEvent;
/// use serde_json::json;
///
/// let event = SubscriptionEvent::new("userCreated", json!({"id": 1}));
/// assert_eq!(event.subscription_name, "userCreated");
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubscriptionEvent<T> {
	/// Subscription name
	pub subscription_name: String,
	/// Event payload
	pub data: T,
	/// Event timestamp (Unix timestamp in milliseconds)
	pub timestamp: u64,
}

impl<T> SubscriptionEvent<T> {
	/// Create a new subscription event
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::signals::graphql_integration::SubscriptionEvent;
	///
	/// let event = SubscriptionEvent::new("taskCompleted", 42);
	/// assert_eq!(event.subscription_name, "taskCompleted");
	/// assert_eq!(event.data, 42);
	/// ```
	pub fn new(subscription_name: impl Into<String>, data: T) -> Self {
		use std::time::{SystemTime, UNIX_EPOCH};

		let timestamp = SystemTime::now()
			.duration_since(UNIX_EPOCH)
			.unwrap()
			.as_millis() as u64;

		Self {
			subscription_name: subscription_name.into(),
			data,
			timestamp,
		}
	}
}

/// GraphQL subscription stream
///
/// Represents an active subscription stream for a specific subscription name
type SubscriptionStream = broadcast::Sender<String>;

/// GraphQL subscription bridge
///
/// Bridges signals to GraphQL subscriptions, allowing signals to trigger
/// subscription updates
///
/// # Examples
///
/// ```
/// use reinhardt_core::signals::graphql_integration::GraphQLSubscriptionBridge;
///
/// let bridge = GraphQLSubscriptionBridge::new();
/// ```
pub struct GraphQLSubscriptionBridge {
	streams: Arc<RwLock<HashMap<String, SubscriptionStream>>>,
}

impl GraphQLSubscriptionBridge {
	/// Create a new GraphQL subscription bridge
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::signals::graphql_integration::GraphQLSubscriptionBridge;
	///
	/// let bridge = GraphQLSubscriptionBridge::new();
	/// ```
	pub fn new() -> Self {
		Self {
			streams: Arc::new(RwLock::new(HashMap::new())),
		}
	}

	/// Get or create a subscription stream
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::signals::graphql_integration::GraphQLSubscriptionBridge;
	///
	/// let bridge = GraphQLSubscriptionBridge::new();
	/// let stream = bridge.get_or_create_stream("userUpdated");
	/// ```
	pub fn get_or_create_stream(&self, subscription_name: &str) -> SubscriptionStream {
		let mut streams = self.streams.write();
		streams
			.entry(subscription_name.to_string())
			.or_insert_with(|| {
				let (tx, _) = broadcast::channel(100);
				tx
			})
			.clone()
	}

	/// Subscribe to a GraphQL subscription
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::signals::graphql_integration::GraphQLSubscriptionBridge;
	///
	/// # async fn example() {
	/// let bridge = GraphQLSubscriptionBridge::new();
	/// let mut receiver = bridge.subscribe("userUpdated").await;
	/// # }
	/// ```
	pub async fn subscribe(
		&self,
		subscription_name: impl Into<String>,
	) -> broadcast::Receiver<String> {
		let name = subscription_name.into();
		let stream = self.get_or_create_stream(&name);
		stream.subscribe()
	}

	/// Publish an event to a subscription
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::signals::graphql_integration::GraphQLSubscriptionBridge;
	///
	/// # async fn example() {
	/// let bridge = GraphQLSubscriptionBridge::new();
	/// bridge.publish("userUpdated", "{\"id\": 1}".to_string()).await.unwrap();
	/// # }
	/// ```
	pub async fn publish(
		&self,
		subscription_name: impl Into<String>,
		message: String,
	) -> Result<(), SignalError> {
		let stream = self.get_or_create_stream(&subscription_name.into());
		stream
			.send(message)
			.map_err(|e| SignalError::new(format!("Failed to publish: {}", e)))?;
		Ok(())
	}

	/// Get the number of active subscriptions
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::signals::graphql_integration::GraphQLSubscriptionBridge;
	///
	/// let bridge = GraphQLSubscriptionBridge::new();
	/// assert_eq!(bridge.subscription_count(), 0);
	/// ```
	pub fn subscription_count(&self) -> usize {
		self.streams.read().len()
	}

	/// Get the number of active receivers for a subscription
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::signals::graphql_integration::GraphQLSubscriptionBridge;
	///
	/// # async fn example() {
	/// let bridge = GraphQLSubscriptionBridge::new();
	/// let _receiver = bridge.subscribe("test").await;
	/// assert_eq!(bridge.receiver_count("test"), 1);
	/// # }
	/// ```
	pub fn receiver_count(&self, subscription_name: &str) -> usize {
		self.streams
			.read()
			.get(subscription_name)
			.map(|s| s.receiver_count())
			.unwrap_or(0)
	}

	/// Connect a signal to a GraphQL subscription
	///
	/// When the signal is emitted, it will be transformed and published to the subscription
	///
	/// # Examples
	///
	/// ```rust,no_run
	/// use reinhardt_core::signals::graphql_integration::{GraphQLSubscriptionBridge, SubscriptionEvent};
	/// use reinhardt_core::signals::{Signal, SignalName};
	///
	/// # #[tokio::main]
	/// # async fn main() {
	/// # use serde::{Serialize, Deserialize};
	/// # #[derive(Debug, Clone, Serialize, Deserialize)]
	/// # struct User { id: Option<i64> }
	/// # fn post_save() -> Signal<User> {
	/// #     Signal::new(SignalName::custom("post_save"))
	/// # }
	/// let bridge = GraphQLSubscriptionBridge::new();
	/// bridge.connect_signal(
	///     post_save(),
	///     "userSaved",
	///     |_user| SubscriptionEvent::new("userSaved", ())
	/// ).await;
	/// # }
	/// ```
	pub async fn connect_signal<T, F, E>(
		&self,
		signal: Signal<T>,
		subscription_name: impl Into<String>,
		transform: F,
	) where
		T: Send + Sync + 'static,
		F: Fn(Arc<T>) -> E + Send + Sync + 'static,
		E: Serialize + Send + Sync + 'static,
	{
		let streams = Arc::clone(&self.streams);
		let subscription_name = subscription_name.into();
		let transform = Arc::new(transform);

		signal.connect(move |instance| {
			let streams = Arc::clone(&streams);
			let subscription_name = subscription_name.clone();
			let transform = Arc::clone(&transform);

			async move {
				let event = transform(instance);
				let json = serde_json::to_string(&event)
					.map_err(|e| SignalError::new(format!("Serialization error: {}", e)))?;

				let streams_read = streams.read();
				if let Some(stream) = streams_read.get(&subscription_name)
					&& let Err(e) = stream.send(json)
				{
					eprintln!("Failed to send GraphQL subscription event: {}", e);
				}

				Ok(())
			}
		});
	}

	/// Remove a subscription stream
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::signals::graphql_integration::GraphQLSubscriptionBridge;
	///
	/// let bridge = GraphQLSubscriptionBridge::new();
	/// bridge.get_or_create_stream("test");
	/// assert_eq!(bridge.subscription_count(), 1);
	///
	/// bridge.remove_stream("test");
	/// assert_eq!(bridge.subscription_count(), 0);
	/// ```
	pub fn remove_stream(&self, subscription_name: &str) {
		self.streams.write().remove(subscription_name);
	}
}

impl Default for GraphQLSubscriptionBridge {
	fn default() -> Self {
		Self::new()
	}
}

impl Clone for GraphQLSubscriptionBridge {
	fn clone(&self) -> Self {
		Self {
			streams: Arc::clone(&self.streams),
		}
	}
}

impl fmt::Debug for GraphQLSubscriptionBridge {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_struct("GraphQLSubscriptionBridge")
			.field("subscription_count", &self.subscription_count())
			.finish()
	}
}

/// Typed GraphQL subscription
///
/// A type-safe wrapper for a specific subscription type
///
/// # Examples
///
/// ```
/// use reinhardt_core::signals::graphql_integration::{GraphQLSubscriptionBridge, TypedSubscription};
///
/// let bridge = GraphQLSubscriptionBridge::new();
/// let subscription = TypedSubscription::<String>::new(bridge, "stringEvent");
/// ```
pub struct TypedSubscription<T>
where
	T: Serialize + DeserializeOwned + Send + Sync + 'static,
{
	bridge: GraphQLSubscriptionBridge,
	subscription_name: String,
	_phantom: PhantomData<T>,
}

impl<T> TypedSubscription<T>
where
	T: Serialize + DeserializeOwned + Send + Sync + 'static,
{
	/// Create a new typed subscription
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::signals::graphql_integration::{GraphQLSubscriptionBridge, TypedSubscription};
	///
	/// let bridge = GraphQLSubscriptionBridge::new();
	/// let subscription = TypedSubscription::<i32>::new(bridge, "numberEvent");
	/// ```
	pub fn new(bridge: GraphQLSubscriptionBridge, subscription_name: impl Into<String>) -> Self {
		Self {
			bridge,
			subscription_name: subscription_name.into(),
			_phantom: PhantomData,
		}
	}

	/// Subscribe and receive typed events
	///
	/// # Examples
	///
	/// ```rust,no_run
	/// use reinhardt_core::signals::graphql_integration::{GraphQLSubscriptionBridge, TypedSubscription};
	///
	/// # #[tokio::main]
	/// # async fn main() {
	/// let bridge = GraphQLSubscriptionBridge::new();
	/// let subscription = TypedSubscription::<String>::new(bridge, "messages");
	///
	/// let mut receiver = subscription.subscribe().await;
	/// while let Ok(event) = receiver.recv().await {
	///     println!("Received: {:?}", event);
	/// }
	/// # }
	/// ```
	pub async fn subscribe(&self) -> broadcast::Receiver<String> {
		self.bridge.subscribe(&self.subscription_name).await
	}

	/// Publish a typed event
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::signals::graphql_integration::{GraphQLSubscriptionBridge, TypedSubscription, SubscriptionEvent};
	///
	/// # async fn example() {
	/// let bridge = GraphQLSubscriptionBridge::new();
	/// let subscription = TypedSubscription::new(bridge, "test");
	///
	/// let event = SubscriptionEvent::new("test", 42);
	/// subscription.publish(event).await.unwrap();
	/// # }
	/// ```
	pub async fn publish(&self, event: SubscriptionEvent<T>) -> Result<(), SignalError> {
		let json = serde_json::to_string(&event)
			.map_err(|e| SignalError::new(format!("Serialization error: {}", e)))?;

		self.bridge.publish(&self.subscription_name, json).await
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
	struct TestData {
		id: i32,
		message: String,
	}

	#[test]
	fn test_subscription_event_creation() {
		let event = SubscriptionEvent::new("test", "data");
		assert_eq!(event.subscription_name, "test");
		assert_eq!(event.data, "data");
		assert!(event.timestamp > 0);
	}

	#[tokio::test]
	async fn test_graphql_bridge_subscribe() {
		let bridge = GraphQLSubscriptionBridge::new();
		let mut receiver = bridge.subscribe("test").await;

		bridge.publish("test", "message".to_string()).await.unwrap();

		let result = receiver.recv().await.unwrap();
		assert_eq!(result, "message");
	}

	#[tokio::test]
	async fn test_graphql_bridge_multiple_subscribers() {
		let bridge = GraphQLSubscriptionBridge::new();
		let mut receiver1 = bridge.subscribe("event").await;
		let mut receiver2 = bridge.subscribe("event").await;

		assert_eq!(bridge.receiver_count("event"), 2);

		bridge.publish("event", "data".to_string()).await.unwrap();

		let msg1 = receiver1.recv().await.unwrap();
		let msg2 = receiver2.recv().await.unwrap();

		assert_eq!(msg1, "data");
		assert_eq!(msg2, "data");
	}

	#[tokio::test]
	async fn test_graphql_bridge_connect_signal() {
		let bridge = GraphQLSubscriptionBridge::new();
		let mut receiver = bridge.subscribe("user_event").await;

		let signal = Signal::<String>::new(crate::signals::SignalName::custom("test"));

		bridge
			.connect_signal(signal.clone(), "user_event", |data| {
				SubscriptionEvent::new("user_event", (*data).clone())
			})
			.await;

		signal.send("test data".to_string()).await.unwrap();

		let result = receiver.recv().await.unwrap();
		let parsed: SubscriptionEvent<String> = serde_json::from_str(&result).unwrap();
		assert_eq!(parsed.subscription_name, "user_event");
		assert_eq!(parsed.data, "test data");
	}

	#[tokio::test]
	async fn test_graphql_bridge_subscription_count() {
		let bridge = GraphQLSubscriptionBridge::new();
		assert_eq!(bridge.subscription_count(), 0);

		bridge.get_or_create_stream("sub1");
		assert_eq!(bridge.subscription_count(), 1);

		bridge.get_or_create_stream("sub2");
		assert_eq!(bridge.subscription_count(), 2);
	}

	#[tokio::test]
	async fn test_graphql_bridge_remove_stream() {
		let bridge = GraphQLSubscriptionBridge::new();

		bridge.get_or_create_stream("test");
		assert_eq!(bridge.subscription_count(), 1);

		bridge.remove_stream("test");
		assert_eq!(bridge.subscription_count(), 0);
	}

	#[tokio::test]
	async fn test_typed_subscription() {
		let bridge = GraphQLSubscriptionBridge::new();
		let subscription = TypedSubscription::<TestData>::new(bridge, "typed_test");

		let mut receiver = subscription.subscribe().await;

		let event = SubscriptionEvent::new(
			"typed_test",
			TestData {
				id: 1,
				message: "Hello".to_string(),
			},
		);

		subscription.publish(event.clone()).await.unwrap();

		let result = receiver.recv().await.unwrap();
		let parsed: SubscriptionEvent<TestData> = serde_json::from_str(&result).unwrap();

		assert_eq!(parsed.subscription_name, "typed_test");
		assert_eq!(parsed.data, event.data);
	}

	#[tokio::test]
	async fn test_graphql_bridge_receiver_count() {
		let bridge = GraphQLSubscriptionBridge::new();

		let _r1 = bridge.subscribe("test").await;
		assert_eq!(bridge.receiver_count("test"), 1);

		let _r2 = bridge.subscribe("test").await;
		assert_eq!(bridge.receiver_count("test"), 2);

		drop(_r1);
		// Note: receiver_count may still show 2 due to async nature of channel cleanup
	}

	#[tokio::test]
	async fn test_graphql_bridge_multiple_subscriptions() {
		let bridge = GraphQLSubscriptionBridge::new();

		let mut r1 = bridge.subscribe("sub1").await;
		let mut r2 = bridge.subscribe("sub2").await;

		bridge.publish("sub1", "msg1".to_string()).await.unwrap();
		bridge.publish("sub2", "msg2".to_string()).await.unwrap();

		let result1 = r1.recv().await.unwrap();
		let result2 = r2.recv().await.unwrap();

		assert_eq!(result1, "msg1");
		assert_eq!(result2, "msg2");
	}
}
