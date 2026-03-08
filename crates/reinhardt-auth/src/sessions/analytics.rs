//! Session analytics and monitoring
//!
//! This module provides analytics and monitoring capabilities for session operations.
//! It enables tracking of session creation, access, deletion, and expiration events.
//!
//! ## Available Analytics Backends
//!
//! - **LoggerAnalytics**: Log events using `tracing` (always available)
//! - **PrometheusAnalytics**: Export metrics to Prometheus (feature: `analytics-prometheus`)
//!
//! ## Example
//!
//! ```rust,no_run
//! use reinhardt_auth::sessions::analytics::{InstrumentedSessionBackend, LoggerAnalytics};
//! use reinhardt_auth::sessions::backends::InMemorySessionBackend;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let backend = InMemorySessionBackend::new();
//! let analytics = LoggerAnalytics::new();
//! let instrumented = InstrumentedSessionBackend::new(backend, analytics);
//!
//! // All session operations are automatically tracked
//! # Ok(())
//! # }
//! ```

use super::backends::{SessionBackend, SessionError};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

// Submodules
mod logger;
pub use logger::LoggerAnalytics;

#[cfg(feature = "analytics-prometheus")]
mod prometheus;
#[cfg(feature = "analytics-prometheus")]
pub use self::prometheus::PrometheusAnalytics;

/// Deletion reason for session analytics
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeletionReason {
	/// Session was explicitly deleted by user or application
	Explicit,
	/// Session expired due to TTL
	Expired,
	/// Session was invalidated (e.g., logout)
	Invalidated,
	/// Session was replaced (e.g., key rotation)
	Replaced,
}

/// Session event for analytics
///
/// This enum represents all session-related events that can be tracked.
///
/// # Example
///
/// ```rust
/// use reinhardt_auth::sessions::analytics::SessionEvent;
/// use chrono::Utc;
///
/// let event = SessionEvent::Created {
///     session_key: "session_123".to_string(),
///     size_bytes: 1024,
///     ttl_secs: Some(3600),
///     timestamp: Utc::now(),
/// };
/// ```
#[derive(Debug, Clone)]
pub enum SessionEvent {
	/// Session was created
	Created {
		/// The session key identifier.
		session_key: String,
		/// Size of the session data in bytes.
		size_bytes: usize,
		/// Time-to-live in seconds, if set.
		ttl_secs: Option<u64>,
		/// When the session was created.
		timestamp: DateTime<Utc>,
	},
	/// Session was accessed
	Accessed {
		/// The session key identifier.
		session_key: String,
		/// Access latency in milliseconds.
		latency_ms: u64,
		/// Whether the session data was found (cache hit).
		hit: bool,
		/// When the access occurred.
		timestamp: DateTime<Utc>,
	},
	/// Session was deleted
	Deleted {
		/// The session key identifier.
		session_key: String,
		/// The reason the session was deleted.
		reason: DeletionReason,
		/// When the deletion occurred.
		timestamp: DateTime<Utc>,
	},
	/// Session expired
	Expired {
		/// The session key identifier.
		session_key: String,
		/// Age of the session in seconds at expiration.
		age_secs: u64,
		/// When the expiration occurred.
		timestamp: DateTime<Utc>,
	},
}

/// Session analytics trait
///
/// Implement this trait to create custom analytics backends.
///
/// # Example
///
/// ```rust
/// use reinhardt_auth::sessions::analytics::{SessionAnalytics, SessionEvent};
/// use async_trait::async_trait;
///
/// struct MyAnalytics;
///
/// #[async_trait]
/// impl SessionAnalytics for MyAnalytics {
///     async fn record_event(&self, event: SessionEvent) {
///         // Custom analytics implementation
///         println!("Event: {:?}", event);
///     }
/// }
/// ```
#[async_trait]
pub trait SessionAnalytics: Send + Sync {
	/// Record a session event
	async fn record_event(&self, event: SessionEvent);
}

/// Composite analytics backend
///
/// Allows recording events to multiple analytics backends simultaneously.
///
/// # Example
///
/// ```rust,no_run
/// use reinhardt_auth::sessions::analytics::{CompositeAnalytics, LoggerAnalytics};
///
/// let mut composite = CompositeAnalytics::new();
/// composite.add(LoggerAnalytics::new());
/// // composite.add(PrometheusAnalytics::new()); // If prometheus feature is enabled
/// ```
#[derive(Clone)]
pub struct CompositeAnalytics {
	backends: Vec<Arc<dyn SessionAnalytics>>,
}

impl CompositeAnalytics {
	/// Create a new composite analytics backend
	pub fn new() -> Self {
		Self {
			backends: Vec::new(),
		}
	}

	/// Add an analytics backend
	pub fn add<A: SessionAnalytics + 'static>(&mut self, analytics: A) {
		self.backends.push(Arc::new(analytics));
	}
}

impl Default for CompositeAnalytics {
	fn default() -> Self {
		Self::new()
	}
}

#[async_trait]
impl SessionAnalytics for CompositeAnalytics {
	async fn record_event(&self, event: SessionEvent) {
		for backend in &self.backends {
			backend.record_event(event.clone()).await;
		}
	}
}

/// Instrumented session backend wrapper
///
/// This wrapper automatically tracks all session operations and records
/// events to the configured analytics backend.
///
/// # Example
///
/// ```rust,no_run
/// use reinhardt_auth::sessions::analytics::{InstrumentedSessionBackend, LoggerAnalytics};
/// use reinhardt_auth::sessions::backends::InMemorySessionBackend;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let backend = InMemorySessionBackend::new();
/// let analytics = LoggerAnalytics::new();
/// let instrumented = InstrumentedSessionBackend::new(backend, analytics);
///
/// // All operations are automatically tracked
/// # Ok(())
/// # }
/// ```
#[derive(Clone)]
pub struct InstrumentedSessionBackend<B, A> {
	backend: B,
	analytics: A,
}

impl<B, A> InstrumentedSessionBackend<B, A>
where
	B: SessionBackend + Clone,
	A: SessionAnalytics + Clone,
{
	/// Create a new instrumented session backend
	///
	/// # Example
	///
	/// ```rust,no_run
	/// use reinhardt_auth::sessions::analytics::{InstrumentedSessionBackend, LoggerAnalytics};
	/// use reinhardt_auth::sessions::backends::InMemorySessionBackend;
	///
	/// let backend = InMemorySessionBackend::new();
	/// let analytics = LoggerAnalytics::new();
	/// let instrumented = InstrumentedSessionBackend::new(backend, analytics);
	/// ```
	pub fn new(backend: B, analytics: A) -> Self {
		Self { backend, analytics }
	}

	/// Get a reference to the underlying backend
	pub fn backend(&self) -> &B {
		&self.backend
	}

	/// Get a reference to the analytics backend
	pub fn analytics(&self) -> &A {
		&self.analytics
	}
}

#[async_trait]
impl<B, A> SessionBackend for InstrumentedSessionBackend<B, A>
where
	B: SessionBackend + Clone,
	A: SessionAnalytics + Clone,
{
	async fn load<T>(&self, session_key: &str) -> Result<Option<T>, SessionError>
	where
		T: for<'de> Deserialize<'de> + Serialize + Send + Sync,
	{
		let start = std::time::Instant::now();
		let result = self.backend.load(session_key).await;
		let latency_ms = start.elapsed().as_millis() as u64;

		let hit = result.as_ref().map(|opt| opt.is_some()).unwrap_or(false);

		self.analytics
			.record_event(SessionEvent::Accessed {
				session_key: session_key.to_string(),
				latency_ms,
				hit,
				timestamp: Utc::now(),
			})
			.await;

		result
	}

	async fn save<T>(
		&self,
		session_key: &str,
		data: &T,
		ttl: Option<u64>,
	) -> Result<(), SessionError>
	where
		T: Serialize + Send + Sync,
	{
		// Serialize to measure size
		let serialized = serde_json::to_vec(data)
			.map_err(|e| SessionError::SerializationError(e.to_string()))?;
		let size_bytes = serialized.len();

		let result = self.backend.save(session_key, data, ttl).await;

		if result.is_ok() {
			self.analytics
				.record_event(SessionEvent::Created {
					session_key: session_key.to_string(),
					size_bytes,
					ttl_secs: ttl,
					timestamp: Utc::now(),
				})
				.await;
		}

		result
	}

	async fn delete(&self, session_key: &str) -> Result<(), SessionError> {
		let result = self.backend.delete(session_key).await;

		if result.is_ok() {
			self.analytics
				.record_event(SessionEvent::Deleted {
					session_key: session_key.to_string(),
					reason: DeletionReason::Explicit,
					timestamp: Utc::now(),
				})
				.await;
		}

		result
	}

	async fn exists(&self, session_key: &str) -> Result<bool, SessionError> {
		self.backend.exists(session_key).await
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::sessions::InMemorySessionBackend;

	#[tokio::test]
	async fn test_instrumented_backend_save() {
		let backend = InMemorySessionBackend::new();
		let analytics = LoggerAnalytics::new();
		let instrumented = InstrumentedSessionBackend::new(backend, analytics);

		let data = serde_json::json!({"key": "value"});

		instrumented
			.save("test_key", &data, Some(3600))
			.await
			.unwrap();

		let loaded: Option<serde_json::Value> = instrumented.load("test_key").await.unwrap();
		assert_eq!(loaded.unwrap(), data);
	}

	#[tokio::test]
	async fn test_instrumented_backend_delete() {
		let backend = InMemorySessionBackend::new();
		let analytics = LoggerAnalytics::new();
		let instrumented = InstrumentedSessionBackend::new(backend, analytics);

		let data = serde_json::json!({"key": "value"});

		instrumented.save("test_key", &data, None).await.unwrap();
		assert!(instrumented.exists("test_key").await.unwrap());

		instrumented.delete("test_key").await.unwrap();
		assert!(!instrumented.exists("test_key").await.unwrap());
	}

	#[tokio::test]
	async fn test_composite_analytics() {
		let mut composite = CompositeAnalytics::new();
		composite.add(LoggerAnalytics::new());

		let backend = InMemorySessionBackend::new();
		let instrumented = InstrumentedSessionBackend::new(backend, composite);

		let data = serde_json::json!({"key": "value"});
		instrumented.save("test_key", &data, None).await.unwrap();
	}
}
