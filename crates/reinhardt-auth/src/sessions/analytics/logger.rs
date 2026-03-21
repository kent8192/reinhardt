//! Tracing-based logger analytics
//!
//! This module provides a simple analytics backend that logs session events
//! using the `tracing` crate.
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
//! // Events will be logged using tracing
//! # Ok(())
//! # }
//! ```

use super::{SessionAnalytics, SessionEvent};
use async_trait::async_trait;

/// Logger analytics backend
///
/// Logs session events using the `tracing` crate.
///
/// # Example
///
/// ```rust,no_run
/// use reinhardt_auth::sessions::analytics::LoggerAnalytics;
///
/// let analytics = LoggerAnalytics::new();
/// ```
#[derive(Debug, Clone)]
pub struct LoggerAnalytics;

impl LoggerAnalytics {
	/// Create a new logger analytics backend
	///
	/// # Example
	///
	/// ```rust,no_run
	/// use reinhardt_auth::sessions::analytics::LoggerAnalytics;
	///
	/// let analytics = LoggerAnalytics::new();
	/// ```
	pub fn new() -> Self {
		Self
	}
}

impl Default for LoggerAnalytics {
	fn default() -> Self {
		Self::new()
	}
}

#[async_trait]
impl SessionAnalytics for LoggerAnalytics {
	async fn record_event(&self, event: SessionEvent) {
		match event {
			SessionEvent::Created {
				session_key,
				size_bytes,
				ttl_secs,
				timestamp,
			} => {
				tracing::info!(
					session_key = %session_key,
					size_bytes = size_bytes,
					ttl_secs = ?ttl_secs,
					timestamp = %timestamp,
					"Session created"
				);
			}
			SessionEvent::Accessed {
				session_key,
				latency_ms,
				hit,
				timestamp,
			} => {
				tracing::debug!(
					session_key = %session_key,
					latency_ms = latency_ms,
					hit = hit,
					timestamp = %timestamp,
					"Session accessed"
				);
			}
			SessionEvent::Deleted {
				session_key,
				reason,
				timestamp,
			} => {
				tracing::info!(
					session_key = %session_key,
					reason = ?reason,
					timestamp = %timestamp,
					"Session deleted"
				);
			}
			SessionEvent::Expired {
				session_key,
				age_secs,
				timestamp,
			} => {
				tracing::info!(
					session_key = %session_key,
					age_secs = age_secs,
					timestamp = %timestamp,
					"Session expired"
				);
			}
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use chrono::Utc;
	use rstest::rstest;

	#[rstest]
	#[tokio::test]
	async fn test_logger_analytics_created() {
		let analytics = LoggerAnalytics::new();

		let event = SessionEvent::Created {
			session_key: "test_key".to_string(),
			size_bytes: 1024,
			ttl_secs: Some(3600),
			timestamp: Utc::now(),
		};

		analytics.record_event(event).await;
	}

	#[rstest]
	#[tokio::test]
	async fn test_logger_analytics_accessed() {
		let analytics = LoggerAnalytics::new();

		let event = SessionEvent::Accessed {
			session_key: "test_key".to_string(),
			latency_ms: 10,
			hit: true,
			timestamp: Utc::now(),
		};

		analytics.record_event(event).await;
	}

	#[rstest]
	#[tokio::test]
	async fn test_logger_analytics_deleted() {
		let analytics = LoggerAnalytics::new();

		let event = SessionEvent::Deleted {
			session_key: "test_key".to_string(),
			reason: crate::sessions::DeletionReason::Explicit,
			timestamp: Utc::now(),
		};

		analytics.record_event(event).await;
	}

	#[rstest]
	#[tokio::test]
	async fn test_logger_analytics_expired() {
		let analytics = LoggerAnalytics::new();

		let event = SessionEvent::Expired {
			session_key: "test_key".to_string(),
			age_secs: 7200,
			timestamp: Utc::now(),
		};

		analytics.record_event(event).await;
	}
}
