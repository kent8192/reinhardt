#![cfg(not(target_arch = "wasm32"))]

//! Distributed Signals - Cross-service signal dispatch via message brokers
//!
//! This module provides distributed signal support, allowing signals to be
//! dispatched across multiple service instances via message brokers like
//! Redis Pub/Sub, RabbitMQ, or Kafka.
//!
//! # Examples
//!
//! ```rust,no_run
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! use reinhardt_core::signals::distributed::{DistributedSignal, InMemoryBroker};
//!
//! // Create a distributed signal with an in-memory broker
//! let broker = InMemoryBroker::new();
//! let signal = DistributedSignal::<serde_json::Value, _>::new("user_created", broker, "service-1");
//!
//! // Subscribe to signals from other services
//! signal.subscribe(|event| {
//!     println!("Received distributed signal: {:?}", event);
//!     Ok(())
//! }).await?;
//!
//! // Publish signals to other services
//! let user_event = serde_json::json!({"user_id": 123});
//! signal.publish(user_event).await?;
//! # Ok(())
//! # }

use super::error::SignalError;
use async_trait::async_trait;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::fmt;
use std::marker::PhantomData;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Distributed signal event wrapper
///
/// # Examples
///
/// ```
/// use reinhardt_core::signals::distributed::DistributedEvent;
/// use serde::{Deserialize, Serialize};
///
/// #[derive(Serialize, Deserialize, Clone)]
/// struct UserEvent { user_id: i64 }
///
/// let event = DistributedEvent::new(
///     "user_created",
///     UserEvent { user_id: 123 },
///     "service-1"
/// );
/// assert_eq!(event.signal_name, "user_created");
/// assert_eq!(event.source_service, "service-1");
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DistributedEvent<T> {
	/// Signal name
	pub signal_name: String,
	/// Event payload
	pub payload: T,
	/// Source service identifier
	pub source_service: String,
	/// Event timestamp (Unix timestamp in milliseconds)
	pub timestamp: u64,
	/// Unique event ID
	pub event_id: String,
}

impl<T> DistributedEvent<T> {
	/// Create a new distributed event
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::signals::distributed::DistributedEvent;
	///
	/// let event = DistributedEvent::new("test_signal", "payload", "service-1");
	/// assert_eq!(event.signal_name, "test_signal");
	/// ```
	pub fn new(
		signal_name: impl Into<String>,
		payload: T,
		source_service: impl Into<String>,
	) -> Self {
		use std::time::{SystemTime, UNIX_EPOCH};

		let timestamp = SystemTime::now()
			.duration_since(UNIX_EPOCH)
			.unwrap()
			.as_millis() as u64;

		Self {
			signal_name: signal_name.into(),
			payload,
			source_service: source_service.into(),
			timestamp,
			event_id: uuid::Uuid::new_v4().to_string(),
		}
	}
}

/// Message broker trait for distributed signals
///
/// Implement this trait to support different message brokers
#[async_trait]
pub trait MessageBroker: Send + Sync {
	/// Publish a message to a channel
	async fn publish(&self, channel: &str, message: &[u8]) -> Result<(), SignalError>;

	/// Subscribe to a channel
	async fn subscribe(
		&self,
		channel: &str,
		handler: Arc<dyn Fn(Vec<u8>) -> Result<(), SignalError> + Send + Sync>,
	) -> Result<(), SignalError>;

	/// Unsubscribe from a channel
	async fn unsubscribe(&self, channel: &str) -> Result<(), SignalError>;
}

/// In-memory message broker for testing and local development
///
/// Type alias for channel subscriber function
type SubscriberFn = Arc<dyn Fn(Vec<u8>) -> Result<(), SignalError> + Send + Sync>;

/// Type alias for channels map
type ChannelsMap = std::collections::HashMap<String, Vec<SubscriberFn>>;

/// # Examples
///
/// ```
/// use reinhardt_core::signals::distributed::InMemoryBroker;
///
/// let broker = InMemoryBroker::new();
/// ```
pub struct InMemoryBroker {
	channels: Arc<RwLock<ChannelsMap>>,
}

impl InMemoryBroker {
	/// Create a new in-memory broker
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::signals::distributed::InMemoryBroker;
	///
	/// let broker = InMemoryBroker::new();
	/// ```
	pub fn new() -> Self {
		Self {
			channels: Arc::new(RwLock::new(std::collections::HashMap::new())),
		}
	}
}

impl Default for InMemoryBroker {
	fn default() -> Self {
		Self::new()
	}
}

#[async_trait]
impl MessageBroker for InMemoryBroker {
	async fn publish(&self, channel: &str, message: &[u8]) -> Result<(), SignalError> {
		let channels = self.channels.read().await;
		if let Some(handlers) = channels.get(channel) {
			for handler in handlers {
				handler(message.to_vec())?;
			}
		}
		Ok(())
	}

	async fn subscribe(
		&self,
		channel: &str,
		handler: Arc<dyn Fn(Vec<u8>) -> Result<(), SignalError> + Send + Sync>,
	) -> Result<(), SignalError> {
		let mut channels = self.channels.write().await;
		channels
			.entry(channel.to_string())
			.or_insert_with(Vec::new)
			.push(handler);
		Ok(())
	}

	async fn unsubscribe(&self, channel: &str) -> Result<(), SignalError> {
		let mut channels = self.channels.write().await;
		channels.remove(channel);
		Ok(())
	}
}

/// Distributed signal with message broker integration
///
/// # Examples
///
/// ```
/// use reinhardt_core::signals::distributed::{DistributedSignal, InMemoryBroker};
///
/// # async fn example() {
/// let broker = InMemoryBroker::new();
/// let signal = DistributedSignal::<String, _>::new("my_signal", broker, "service-1");
/// # }
/// ```
pub struct DistributedSignal<T, B>
where
	T: Serialize + DeserializeOwned + Send + Sync + 'static,
	B: MessageBroker + 'static,
{
	signal_name: String,
	broker: Arc<B>,
	service_id: String,
	_phantom: PhantomData<T>,
}

impl<T, B> DistributedSignal<T, B>
where
	T: Serialize + DeserializeOwned + Send + Sync + 'static,
	B: MessageBroker + 'static,
{
	/// Create a new distributed signal
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::signals::distributed::{DistributedSignal, InMemoryBroker};
	///
	/// let broker = InMemoryBroker::new();
	/// let signal = DistributedSignal::<String, _>::new("test", broker, "svc-1");
	/// ```
	pub fn new(signal_name: impl Into<String>, broker: B, service_id: impl Into<String>) -> Self {
		Self {
			signal_name: signal_name.into(),
			broker: Arc::new(broker),
			service_id: service_id.into(),
			_phantom: PhantomData,
		}
	}

	/// Get the signal name
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::signals::distributed::{DistributedSignal, InMemoryBroker};
	///
	/// let broker = InMemoryBroker::new();
	/// let signal = DistributedSignal::<String, _>::new("my_signal", broker, "svc-1");
	/// assert_eq!(signal.name(), "my_signal");
	/// ```
	pub fn name(&self) -> &str {
		&self.signal_name
	}

	/// Get the service ID
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::signals::distributed::{DistributedSignal, InMemoryBroker};
	///
	/// let broker = InMemoryBroker::new();
	/// let signal = DistributedSignal::<String, _>::new("test", broker, "service-1");
	/// assert_eq!(signal.service_id(), "service-1");
	/// ```
	pub fn service_id(&self) -> &str {
		&self.service_id
	}

	/// Publish an event to the distributed signal
	///
	/// # Examples
	///
	/// ```rust,no_run
	/// use reinhardt_core::signals::distributed::{DistributedSignal, InMemoryBroker};
	///
	/// # #[tokio::main]
	/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
	/// let broker = InMemoryBroker::new();
	/// let signal = DistributedSignal::new("user_created", broker, "api-service");
	///
	/// signal.publish("User data".to_string()).await?;
	/// # Ok(())
	/// # }
	/// ```
	pub async fn publish(&self, payload: T) -> Result<(), SignalError> {
		let event = DistributedEvent::new(&self.signal_name, payload, &self.service_id);
		let message = serde_json::to_vec(&event)
			.map_err(|e| SignalError::new(format!("Serialization error: {}", e)))?;

		self.broker.publish(&self.signal_name, &message).await
	}

	/// Subscribe to distributed signal events
	///
	/// # Examples
	///
	/// ```rust,no_run
	/// use reinhardt_core::signals::distributed::{DistributedSignal, InMemoryBroker};
	///
	/// # #[tokio::main]
	/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
	/// let broker = InMemoryBroker::new();
	/// let signal = DistributedSignal::<String, _>::new("user_created", broker, "worker-service");
	///
	/// signal.subscribe(|event| {
	///     println!("Received: {:?}", event);
	///     Ok(())
	/// }).await?;
	/// # Ok(())
	/// # }
	/// ```
	pub async fn subscribe<F>(&self, handler: F) -> Result<(), SignalError>
	where
		F: Fn(DistributedEvent<T>) -> Result<(), SignalError> + Send + Sync + 'static,
	{
		let handler = Arc::new(handler);
		let wrapped_handler = Arc::new(move |message: Vec<u8>| {
			let event: DistributedEvent<T> = serde_json::from_slice(&message)
				.map_err(|e| SignalError::new(format!("Deserialization error: {}", e)))?;
			handler(event)
		});

		self.broker
			.subscribe(&self.signal_name, wrapped_handler)
			.await
	}

	/// Unsubscribe from distributed signal events
	///
	/// # Examples
	///
	/// ```rust,no_run
	/// use reinhardt_core::signals::distributed::{DistributedSignal, InMemoryBroker};
	///
	/// # #[tokio::main]
	/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
	/// let broker = InMemoryBroker::new();
	/// let signal = DistributedSignal::<String, _>::new("user_created", broker, "worker-service");
	///
	/// signal.unsubscribe().await?;
	/// # Ok(())
	/// # }
	/// ```
	pub async fn unsubscribe(&self) -> Result<(), SignalError> {
		self.broker.unsubscribe(&self.signal_name).await
	}
}

impl<T, B> Clone for DistributedSignal<T, B>
where
	T: Serialize + DeserializeOwned + Send + Sync + 'static,
	B: MessageBroker + 'static,
{
	fn clone(&self) -> Self {
		Self {
			signal_name: self.signal_name.clone(),
			broker: Arc::clone(&self.broker),
			service_id: self.service_id.clone(),
			_phantom: PhantomData,
		}
	}
}

impl<T, B> fmt::Debug for DistributedSignal<T, B>
where
	T: Serialize + DeserializeOwned + Send + Sync + 'static,
	B: MessageBroker + 'static,
{
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_struct("DistributedSignal")
			.field("signal_name", &self.signal_name)
			.field("service_id", &self.service_id)
			.finish()
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use parking_lot::Mutex;
	use std::sync::Arc;
	use std::sync::atomic::{AtomicUsize, Ordering};

	#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
	struct TestPayload {
		message: String,
		count: i32,
	}

	#[test]
	fn test_distributed_event_creation() {
		let event = DistributedEvent::new("test_signal", "test_payload", "service-1");
		assert_eq!(event.signal_name, "test_signal");
		assert_eq!(event.payload, "test_payload");
		assert_eq!(event.source_service, "service-1");
		assert!(event.timestamp > 0);
		assert!(!event.event_id.is_empty());
	}

	#[tokio::test]
	async fn test_in_memory_broker() {
		let broker = InMemoryBroker::new();
		let received = Arc::new(Mutex::new(Vec::new()));

		let r = received.clone();
		let handler = Arc::new(move |msg: Vec<u8>| {
			r.lock().push(msg);
			Ok(())
		});

		broker.subscribe("test_channel", handler).await.unwrap();
		broker.publish("test_channel", b"hello").await.unwrap();

		let messages = received.lock();
		assert_eq!(messages.len(), 1);
		assert_eq!(messages[0], b"hello");
	}

	#[tokio::test]
	async fn test_distributed_signal_publish_subscribe() {
		let broker = InMemoryBroker::new();
		let signal = DistributedSignal::new("user_event", broker, "service-1");

		let received = Arc::new(Mutex::new(Vec::new()));
		let r = received.clone();

		signal
			.subscribe(move |event| {
				r.lock().push(event.payload);
				Ok(())
			})
			.await
			.unwrap();

		let payload = TestPayload {
			message: "Hello".to_string(),
			count: 42,
		};

		signal.publish(payload.clone()).await.unwrap();

		let events = received.lock();
		assert_eq!(events.len(), 1);
		assert_eq!(events[0], payload);
	}

	#[tokio::test]
	async fn test_distributed_signal_multiple_subscribers() {
		let broker = InMemoryBroker::new();
		let signal = DistributedSignal::new("broadcast", broker, "svc-1");

		let counter = Arc::new(AtomicUsize::new(0));

		let c1 = counter.clone();
		signal
			.subscribe(move |_event: DistributedEvent<String>| {
				c1.fetch_add(1, Ordering::SeqCst);
				Ok(())
			})
			.await
			.unwrap();

		let c2 = counter.clone();
		signal
			.subscribe(move |_event| {
				c2.fetch_add(1, Ordering::SeqCst);
				Ok(())
			})
			.await
			.unwrap();

		signal.publish("test".to_string()).await.unwrap();

		assert_eq!(counter.load(Ordering::SeqCst), 2);
	}

	#[tokio::test]
	async fn test_distributed_signal_name_and_service_id() {
		let broker = InMemoryBroker::new();
		let signal = DistributedSignal::<String, _>::new("my_signal", broker, "my_service");

		assert_eq!(signal.name(), "my_signal");
		assert_eq!(signal.service_id(), "my_service");
	}

	#[tokio::test]
	async fn test_distributed_signal_unsubscribe() {
		let broker = InMemoryBroker::new();
		let signal = DistributedSignal::new("temp_signal", broker, "svc-1");

		let counter = Arc::new(AtomicUsize::new(0));
		let c = counter.clone();

		signal
			.subscribe(move |_event: DistributedEvent<String>| {
				c.fetch_add(1, Ordering::SeqCst);
				Ok(())
			})
			.await
			.unwrap();

		signal.publish("first".to_string()).await.unwrap();
		assert_eq!(counter.load(Ordering::SeqCst), 1);

		signal.unsubscribe().await.unwrap();
		signal.publish("second".to_string()).await.unwrap();

		// Still 1 because we unsubscribed
		assert_eq!(counter.load(Ordering::SeqCst), 1);
	}

	#[tokio::test]
	async fn test_distributed_event_metadata() {
		let broker = InMemoryBroker::new();
		let signal = DistributedSignal::new("metadata_test", broker, "test-service");

		let received_event = Arc::new(Mutex::new(None));
		let r = received_event.clone();

		signal
			.subscribe(move |event: DistributedEvent<String>| {
				*r.lock() = Some(event);
				Ok(())
			})
			.await
			.unwrap();

		signal.publish("payload".to_string()).await.unwrap();

		let event = received_event.lock().clone().unwrap();
		assert_eq!(event.signal_name, "metadata_test");
		assert_eq!(event.source_service, "test-service");
		assert_eq!(event.payload, "payload");
		assert!(event.timestamp > 0);
		assert!(!event.event_id.is_empty());
	}
}
