//! Event Bus for Inter-Plugin Communication
//!
//! This module provides an event bus that enables asynchronous communication
//! between plugins using a publish-subscribe pattern.
//!
//! # Design
//!
//! - **Thread-safe**: Uses `parking_lot::RwLock` for concurrent access
//! - **Wildcard patterns**: Supports `*` and `prefix.*` patterns
//! - **Polling-based**: WASM plugins poll for events (no push notifications)
//! - **Per-subscription queues**: Each subscription maintains its own event queue

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

/// Event data that can be transmitted between plugins.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
	/// Event name (e.g., "user.created", "order.completed")
	pub name: String,
	/// MessagePack-serialized payload
	pub payload: Vec<u8>,
	/// Source plugin name that emitted the event
	pub source: String,
	/// Timestamp (Unix epoch milliseconds)
	pub timestamp: u64,
}

impl Event {
	/// Create a new event with the current timestamp.
	pub fn new(name: impl Into<String>, payload: Vec<u8>, source: impl Into<String>) -> Self {
		// Use unwrap_or_default to avoid panic when system clock precedes Unix epoch.
		let timestamp = SystemTime::now()
			.duration_since(UNIX_EPOCH)
			.unwrap_or_default()
			.as_millis() as u64;

		Self {
			name: name.into(),
			payload,
			source: source.into(),
			timestamp,
		}
	}

	/// Create an event with a specific timestamp (for testing).
	#[cfg(test)]
	pub fn with_timestamp(
		name: impl Into<String>,
		payload: Vec<u8>,
		source: impl Into<String>,
		timestamp: u64,
	) -> Self {
		Self {
			name: name.into(),
			payload,
			source: source.into(),
			timestamp,
		}
	}
}

/// Subscription information for event filtering.
struct Subscription {
	/// Pattern to match event names (supports wildcards)
	pattern: String,
	/// Queue of events waiting to be polled
	queue: VecDeque<Event>,
	/// Plugin name that owns this subscription
	owner: String,
}

/// Default maximum number of subscriptions per plugin.
const DEFAULT_MAX_SUBSCRIPTIONS_PER_PLUGIN: usize = 100;

/// Default maximum number of total subscriptions across all plugins.
const DEFAULT_MAX_TOTAL_SUBSCRIPTIONS: usize = 10_000;

/// Event bus for inter-plugin communication.
///
/// The event bus provides a publish-subscribe mechanism that allows plugins
/// to communicate without direct references to each other.
///
/// # Resource Limits
///
/// The event bus enforces subscription limits to prevent memory exhaustion:
/// - Per-plugin subscription limit (default: 100)
/// - Global total subscription limit (default: 10,000)
/// - Per-subscription queue size limit (default: 10,000 events)
///
/// # Example
///
/// ```ignore
/// use reinhardt_dentdelion::wasm::events::EventBus;
///
/// let bus = EventBus::new();
///
/// // Subscribe to user events
/// let sub_id = bus.subscribe("user.*", "my-plugin")?;
///
/// // Emit an event
/// bus.emit("user.created", vec![1, 2, 3], "auth-plugin");
///
/// // Poll for events
/// let events = bus.poll(sub_id, 10);
/// ```
pub struct EventBus {
	/// Active subscriptions keyed by subscription ID
	subscriptions: RwLock<HashMap<u64, Subscription>>,
	/// Counter for generating unique subscription IDs
	next_id: AtomicU64,
	/// Maximum queue size per subscription (prevents memory exhaustion)
	max_queue_size: usize,
	/// Maximum number of subscriptions per plugin
	max_subscriptions_per_plugin: usize,
	/// Maximum total number of subscriptions across all plugins
	max_total_subscriptions: usize,
}

/// Errors that can occur during event bus operations.
#[derive(Debug, Clone, thiserror::Error)]
pub enum EventBusError {
	/// Per-plugin subscription limit exceeded
	#[error("plugin '{plugin}' has reached subscription limit ({limit})")]
	PerPluginLimitExceeded {
		/// Plugin that attempted to subscribe
		plugin: String,
		/// Maximum allowed subscriptions per plugin
		limit: usize,
	},

	/// Global subscription limit exceeded
	#[error("total subscription limit reached ({limit})")]
	TotalLimitExceeded {
		/// Maximum allowed total subscriptions
		limit: usize,
	},
}

impl EventBus {
	/// Create a new event bus with default settings.
	pub fn new() -> Self {
		Self::with_limits(
			10_000,
			DEFAULT_MAX_SUBSCRIPTIONS_PER_PLUGIN,
			DEFAULT_MAX_TOTAL_SUBSCRIPTIONS,
		)
	}

	/// Create a new event bus with a custom maximum queue size.
	pub fn with_max_queue_size(max_queue_size: usize) -> Self {
		Self::with_limits(
			max_queue_size,
			DEFAULT_MAX_SUBSCRIPTIONS_PER_PLUGIN,
			DEFAULT_MAX_TOTAL_SUBSCRIPTIONS,
		)
	}

	/// Create a new event bus with custom resource limits.
	///
	/// # Arguments
	///
	/// * `max_queue_size` - Maximum number of events per subscription queue
	/// * `max_subscriptions_per_plugin` - Maximum subscriptions a single plugin can create
	/// * `max_total_subscriptions` - Maximum total subscriptions across all plugins
	pub fn with_limits(
		max_queue_size: usize,
		max_subscriptions_per_plugin: usize,
		max_total_subscriptions: usize,
	) -> Self {
		Self {
			subscriptions: RwLock::new(HashMap::new()),
			next_id: AtomicU64::new(1),
			max_queue_size,
			max_subscriptions_per_plugin,
			max_total_subscriptions,
		}
	}

	/// Emit an event to all matching subscribers.
	///
	/// # Arguments
	///
	/// * `name` - Event name (e.g., "user.created")
	/// * `payload` - MessagePack-serialized event data
	/// * `source` - Name of the plugin emitting the event
	///
	/// # Returns
	///
	/// The number of subscriptions that received the event.
	pub fn emit(&self, name: &str, payload: Vec<u8>, source: &str) -> usize {
		let event = Event::new(name, payload, source);
		let mut delivered = 0;

		let mut subs = self.subscriptions.write();
		for sub in subs.values_mut() {
			if Self::matches_pattern(&sub.pattern, &event.name) {
				// Enforce queue size limit (drop oldest if full)
				while sub.queue.len() >= self.max_queue_size {
					sub.queue.pop_front();
				}
				sub.queue.push_back(event.clone());
				delivered += 1;
			}
		}

		delivered
	}

	/// Subscribe to events matching a pattern.
	///
	/// # Pattern Syntax
	///
	/// - `*` - Matches all events
	/// - `user.*` - Matches events starting with "user." (e.g., "user.created", "user.deleted")
	/// - `user.created` - Matches only "user.created"
	///
	/// # Arguments
	///
	/// * `pattern` - Event name pattern
	/// * `owner` - Name of the plugin creating the subscription
	///
	/// # Returns
	///
	/// A unique subscription ID that can be used for polling and unsubscribing.
	///
	/// # Errors
	///
	/// Returns an error if:
	/// - The plugin has reached its per-plugin subscription limit
	/// - The global total subscription limit has been reached
	pub fn subscribe(&self, pattern: &str, owner: &str) -> Result<u64, EventBusError> {
		let mut subs = self.subscriptions.write();

		// Check global subscription limit
		if subs.len() >= self.max_total_subscriptions {
			return Err(EventBusError::TotalLimitExceeded {
				limit: self.max_total_subscriptions,
			});
		}

		// Check per-plugin subscription limit
		let plugin_count = subs.values().filter(|s| s.owner == owner).count();
		if plugin_count >= self.max_subscriptions_per_plugin {
			return Err(EventBusError::PerPluginLimitExceeded {
				plugin: owner.to_string(),
				limit: self.max_subscriptions_per_plugin,
			});
		}

		// Wrapping addition is the default behavior of AtomicU64::fetch_add.
		// With u64 range (~1.8 * 10^19), overflow is practically unreachable.
		// If it ever wraps, the subscription map insertion will overwrite any
		// stale entry with the same ID, which is acceptable.
		let id = self.next_id.fetch_add(1, Ordering::Relaxed);
		let subscription = Subscription {
			pattern: pattern.to_string(),
			queue: VecDeque::new(),
			owner: owner.to_string(),
		};
		subs.insert(id, subscription);
		Ok(id)
	}

	/// Unsubscribe from a subscription.
	///
	/// # Arguments
	///
	/// * `id` - Subscription ID returned by `subscribe`
	///
	/// # Returns
	///
	/// `true` if the subscription existed and was removed, `false` otherwise.
	pub fn unsubscribe(&self, id: u64) -> bool {
		self.subscriptions.write().remove(&id).is_some()
	}

	/// Poll for pending events on a subscription.
	///
	/// Events are removed from the queue once polled.
	///
	/// # Arguments
	///
	/// * `id` - Subscription ID
	/// * `limit` - Maximum number of events to return
	///
	/// # Returns
	///
	/// A vector of events, up to `limit` events.
	pub fn poll(&self, id: u64, limit: usize) -> Vec<Event> {
		let mut subs = self.subscriptions.write();
		if let Some(sub) = subs.get_mut(&id) {
			let count = limit.min(sub.queue.len());
			sub.queue.drain(..count).collect()
		} else {
			Vec::new()
		}
	}

	/// Get the number of pending events for a subscription.
	pub fn pending_count(&self, id: u64) -> usize {
		self.subscriptions
			.read()
			.get(&id)
			.map(|s| s.queue.len())
			.unwrap_or(0)
	}

	/// Check if a subscription exists.
	pub fn has_subscription(&self, id: u64) -> bool {
		self.subscriptions.read().contains_key(&id)
	}

	/// Get the total number of active subscriptions.
	pub fn subscription_count(&self) -> usize {
		self.subscriptions.read().len()
	}

	/// Remove all subscriptions owned by a specific plugin.
	///
	/// This is useful for cleanup when a plugin is unloaded.
	pub fn remove_plugin_subscriptions(&self, plugin_name: &str) -> usize {
		let mut subs = self.subscriptions.write();
		let initial_count = subs.len();
		subs.retain(|_, sub| sub.owner != plugin_name);
		initial_count - subs.len()
	}

	/// Check if an event name matches a pattern.
	///
	/// # Pattern Matching Rules
	///
	/// - `*` matches any event name
	/// - `prefix.*` matches any event name starting with "prefix."
	/// - Exact match otherwise
	fn matches_pattern(pattern: &str, event_name: &str) -> bool {
		if pattern == "*" {
			return true;
		}

		if let Some(prefix) = pattern.strip_suffix(".*") {
			// Pattern like "user.*" matches "user.created", "user.deleted", etc.
			// The event must start with the prefix followed by a dot
			event_name.starts_with(prefix)
				&& event_name
					.get(prefix.len()..)
					.is_some_and(|s| s.starts_with('.'))
		} else {
			// Exact match
			pattern == event_name
		}
	}
}

impl Default for EventBus {
	fn default() -> Self {
		Self::new()
	}
}

impl std::fmt::Debug for EventBus {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		let subs = self.subscriptions.read();
		f.debug_struct("EventBus")
			.field("subscription_count", &subs.len())
			.field("max_queue_size", &self.max_queue_size)
			.field(
				"max_subscriptions_per_plugin",
				&self.max_subscriptions_per_plugin,
			)
			.field("max_total_subscriptions", &self.max_total_subscriptions)
			.finish()
	}
}

/// Shared event bus instance type.
pub type SharedEventBus = Arc<EventBus>;

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;

	#[rstest]
	fn test_event_creation() {
		// Arrange & Act
		let event = Event::new("user.created", vec![1, 2, 3], "test-plugin");

		// Assert
		assert_eq!(event.name, "user.created");
		assert_eq!(event.payload, vec![1, 2, 3]);
		assert_eq!(event.source, "test-plugin");
		assert!(event.timestamp > 0);
	}

	#[rstest]
	fn test_subscribe_and_unsubscribe() {
		// Arrange
		let bus = EventBus::new();

		// Act
		let id = bus.subscribe("user.*", "test-plugin").unwrap();

		// Assert
		assert!(bus.has_subscription(id));
		assert_eq!(bus.subscription_count(), 1);

		let removed = bus.unsubscribe(id);
		assert!(removed);
		assert!(!bus.has_subscription(id));
		assert_eq!(bus.subscription_count(), 0);
	}

	#[rstest]
	fn test_emit_and_poll() {
		// Arrange
		let bus = EventBus::new();
		let sub_id = bus.subscribe("user.*", "consumer").unwrap();

		// Act - emit events
		let delivered = bus.emit("user.created", vec![1], "producer");
		assert_eq!(delivered, 1);

		let delivered = bus.emit("user.updated", vec![2], "producer");
		assert_eq!(delivered, 1);

		// Non-matching event
		let delivered = bus.emit("order.created", vec![3], "producer");
		assert_eq!(delivered, 0);

		// Assert - poll events
		let events = bus.poll(sub_id, 10);
		assert_eq!(events.len(), 2);
		assert_eq!(events[0].name, "user.created");
		assert_eq!(events[1].name, "user.updated");

		// Queue should be empty now
		let events = bus.poll(sub_id, 10);
		assert!(events.is_empty());
	}

	#[rstest]
	fn test_pattern_matching() {
		// Wildcard matches all
		assert!(EventBus::matches_pattern("*", "anything"));
		assert!(EventBus::matches_pattern("*", "user.created"));

		// Prefix wildcard
		assert!(EventBus::matches_pattern("user.*", "user.created"));
		assert!(EventBus::matches_pattern("user.*", "user.deleted"));
		assert!(EventBus::matches_pattern("user.*", "user.profile.updated"));
		assert!(!EventBus::matches_pattern("user.*", "user"));
		assert!(!EventBus::matches_pattern("user.*", "users.created"));

		// Exact match
		assert!(EventBus::matches_pattern("user.created", "user.created"));
		assert!(!EventBus::matches_pattern("user.created", "user.deleted"));
	}

	#[rstest]
	fn test_multiple_subscribers() {
		// Arrange
		let bus = EventBus::new();
		let sub1 = bus.subscribe("user.*", "plugin-a").unwrap();
		let sub2 = bus.subscribe("*", "plugin-b").unwrap();
		let sub3 = bus.subscribe("order.*", "plugin-c").unwrap();

		// Act - emit user event (matches sub1 and sub2)
		let delivered = bus.emit("user.created", vec![1], "producer");
		assert_eq!(delivered, 2);

		// Emit order event (matches sub2 and sub3)
		let delivered = bus.emit("order.created", vec![2], "producer");
		assert_eq!(delivered, 2);

		// Assert
		assert_eq!(bus.pending_count(sub1), 1);
		assert_eq!(bus.pending_count(sub2), 2);
		assert_eq!(bus.pending_count(sub3), 1);
	}

	#[rstest]
	fn test_queue_limit() {
		// Arrange
		let bus = EventBus::with_max_queue_size(3);
		let sub_id = bus.subscribe("*", "test").unwrap();

		// Act - emit 5 events
		for i in 0..5 {
			bus.emit(&format!("event.{}", i), vec![i as u8], "producer");
		}

		// Assert - only 3 should remain (oldest dropped)
		let events = bus.poll(sub_id, 10);
		assert_eq!(events.len(), 3);
		assert_eq!(events[0].name, "event.2");
		assert_eq!(events[1].name, "event.3");
		assert_eq!(events[2].name, "event.4");
	}

	#[rstest]
	fn test_remove_plugin_subscriptions() {
		// Arrange
		let bus = EventBus::new();
		bus.subscribe("a.*", "plugin-a").unwrap();
		bus.subscribe("b.*", "plugin-a").unwrap();
		bus.subscribe("c.*", "plugin-b").unwrap();
		assert_eq!(bus.subscription_count(), 3);

		// Act
		let removed = bus.remove_plugin_subscriptions("plugin-a");

		// Assert
		assert_eq!(removed, 2);
		assert_eq!(bus.subscription_count(), 1);
	}

	#[rstest]
	fn test_poll_nonexistent_subscription() {
		// Arrange
		let bus = EventBus::new();

		// Act
		let events = bus.poll(999, 10);

		// Assert
		assert!(events.is_empty());
	}

	#[rstest]
	fn test_poll_limit() {
		// Arrange
		let bus = EventBus::new();
		let sub_id = bus.subscribe("*", "test").unwrap();

		// Emit 10 events
		for i in 0..10 {
			bus.emit(&format!("event.{}", i), vec![], "producer");
		}

		// Act
		let events = bus.poll(sub_id, 3);

		// Assert
		assert_eq!(events.len(), 3);
		assert_eq!(bus.pending_count(sub_id), 7);
	}

	// ==========================================================================
	// Subscription Limit Tests (#685)
	// ==========================================================================

	#[rstest]
	fn test_per_plugin_subscription_limit() {
		// Arrange: allow only 2 subscriptions per plugin
		let bus = EventBus::with_limits(10_000, 2, 10_000);

		// Act
		bus.subscribe("a.*", "greedy-plugin").unwrap();
		bus.subscribe("b.*", "greedy-plugin").unwrap();
		let result = bus.subscribe("c.*", "greedy-plugin");

		// Assert
		assert!(matches!(
			result,
			Err(EventBusError::PerPluginLimitExceeded { limit: 2, .. })
		));
		assert_eq!(bus.subscription_count(), 2);
	}

	#[rstest]
	fn test_per_plugin_limit_is_per_plugin() {
		// Arrange: allow 2 subscriptions per plugin
		let bus = EventBus::with_limits(10_000, 2, 10_000);

		// Act - plugin-a can subscribe up to its limit
		bus.subscribe("a.*", "plugin-a").unwrap();
		bus.subscribe("b.*", "plugin-a").unwrap();

		// plugin-b has its own separate limit
		let result = bus.subscribe("c.*", "plugin-b");

		// Assert
		assert!(result.is_ok());
		assert_eq!(bus.subscription_count(), 3);
	}

	#[rstest]
	fn test_global_subscription_limit() {
		// Arrange: allow only 3 total subscriptions
		let bus = EventBus::with_limits(10_000, 100, 3);

		// Act
		bus.subscribe("a.*", "plugin-a").unwrap();
		bus.subscribe("b.*", "plugin-b").unwrap();
		bus.subscribe("c.*", "plugin-c").unwrap();
		let result = bus.subscribe("d.*", "plugin-d");

		// Assert
		assert!(matches!(
			result,
			Err(EventBusError::TotalLimitExceeded { limit: 3 })
		));
		assert_eq!(bus.subscription_count(), 3);
	}

	#[rstest]
	fn test_unsubscribe_frees_slot_for_new_subscription() {
		// Arrange: allow only 2 subscriptions per plugin
		let bus = EventBus::with_limits(10_000, 2, 10_000);
		let id1 = bus.subscribe("a.*", "plugin-a").unwrap();
		bus.subscribe("b.*", "plugin-a").unwrap();

		// Act: unsubscribe to free a slot
		bus.unsubscribe(id1);
		let result = bus.subscribe("c.*", "plugin-a");

		// Assert
		assert!(result.is_ok());
		assert_eq!(bus.subscription_count(), 2);
	}
}
