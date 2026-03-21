//! Prometheus metrics analytics
//!
//! This module provides a Prometheus-based analytics backend that exports
//! session metrics for monitoring and observability.
//!
//! ## Metrics Provided
//!
//! - `session_created_total`: Counter of sessions created
//! - `session_accessed_total`: Counter of session accesses
//! - `session_access_latency_seconds`: Histogram of access latencies
//! - `session_size_bytes`: Histogram of session sizes
//! - `session_deleted_total`: Counter of sessions deleted (by reason)
//! - `session_expired_total`: Counter of sessions expired
//!
//! ## Example
//!
//! ```rust,no_run
//! use reinhardt_auth::sessions::analytics::{InstrumentedSessionBackend, PrometheusAnalytics};
//! use reinhardt_auth::sessions::backends::InMemorySessionBackend;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let backend = InMemorySessionBackend::new();
//! let analytics = PrometheusAnalytics::new()?;
//! let instrumented = InstrumentedSessionBackend::new(backend, analytics);
//!
//! // Metrics will be exported to Prometheus
//! # Ok(())
//! # }
//! ```

use super::{DeletionReason, SessionAnalytics, SessionEvent};
use async_trait::async_trait;
use prometheus::{
	Histogram, IntCounter, IntCounterVec, histogram_opts, opts, register_histogram,
	register_int_counter, register_int_counter_vec,
};
use std::sync::Arc;

/// Prometheus analytics backend
///
/// Exports session metrics to Prometheus for monitoring and observability.
///
/// # Example
///
/// ```rust,no_run
/// use reinhardt_auth::sessions::analytics::PrometheusAnalytics;
///
/// # fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let analytics = PrometheusAnalytics::new()?;
/// # Ok(())
/// # }
/// ```
#[derive(Clone)]
pub struct PrometheusAnalytics {
	session_created: Arc<IntCounter>,
	session_accessed: Arc<IntCounter>,
	session_access_hit: Arc<IntCounter>,
	session_access_miss: Arc<IntCounter>,
	session_access_latency: Arc<Histogram>,
	session_size_bytes: Arc<Histogram>,
	session_deleted: Arc<IntCounterVec>,
	session_expired: Arc<IntCounter>,
}

impl PrometheusAnalytics {
	/// Create a new Prometheus analytics backend
	///
	/// # Errors
	///
	/// Returns an error if metrics registration fails.
	///
	/// # Example
	///
	/// ```rust,no_run
	/// use reinhardt_auth::sessions::analytics::PrometheusAnalytics;
	///
	/// # fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// let analytics = PrometheusAnalytics::new()?;
	/// # Ok(())
	/// # }
	/// ```
	pub fn new() -> Result<Self, prometheus::Error> {
		let session_created = Arc::new(register_int_counter!(opts!(
			"session_created_total",
			"Total number of sessions created"
		))?);

		let session_accessed = Arc::new(register_int_counter!(opts!(
			"session_accessed_total",
			"Total number of session accesses"
		))?);

		let session_access_hit = Arc::new(register_int_counter!(opts!(
			"session_access_hit_total",
			"Total number of session access hits"
		))?);

		let session_access_miss = Arc::new(register_int_counter!(opts!(
			"session_access_miss_total",
			"Total number of session access misses"
		))?);

		let session_access_latency = Arc::new(register_histogram!(histogram_opts!(
			"session_access_latency_seconds",
			"Session access latency in seconds",
			vec![0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0]
		))?);

		let session_size_bytes = Arc::new(register_histogram!(histogram_opts!(
			"session_size_bytes",
			"Session size in bytes",
			vec![100.0, 500.0, 1024.0, 5120.0, 10240.0, 51200.0, 102400.0]
		))?);

		let session_deleted = Arc::new(register_int_counter_vec!(
			opts!("session_deleted_total", "Total number of sessions deleted"),
			&["reason"]
		)?);

		let session_expired = Arc::new(register_int_counter!(opts!(
			"session_expired_total",
			"Total number of sessions expired"
		))?);

		Ok(Self {
			session_created,
			session_accessed,
			session_access_hit,
			session_access_miss,
			session_access_latency,
			session_size_bytes,
			session_deleted,
			session_expired,
		})
	}

	/// Get the session created counter
	pub fn session_created(&self) -> &IntCounter {
		&self.session_created
	}

	/// Get the session accessed counter
	pub fn session_accessed(&self) -> &IntCounter {
		&self.session_accessed
	}

	/// Get the session access hit counter
	pub fn session_access_hit(&self) -> &IntCounter {
		&self.session_access_hit
	}

	/// Get the session access miss counter
	pub fn session_access_miss(&self) -> &IntCounter {
		&self.session_access_miss
	}

	/// Get the session access latency histogram
	pub fn session_access_latency(&self) -> &Histogram {
		&self.session_access_latency
	}

	/// Get the session size histogram
	pub fn session_size_bytes(&self) -> &Histogram {
		&self.session_size_bytes
	}

	/// Get the session deleted counter
	pub fn session_deleted(&self) -> &IntCounterVec {
		&self.session_deleted
	}

	/// Get the session expired counter
	pub fn session_expired(&self) -> &IntCounter {
		&self.session_expired
	}
}

#[async_trait]
impl SessionAnalytics for PrometheusAnalytics {
	async fn record_event(&self, event: SessionEvent) {
		match event {
			SessionEvent::Created {
				size_bytes,
				ttl_secs: _,
				..
			} => {
				self.session_created.inc();
				self.session_size_bytes.observe(size_bytes as f64);
			}
			SessionEvent::Accessed {
				latency_ms, hit, ..
			} => {
				self.session_accessed.inc();
				if hit {
					self.session_access_hit.inc();
				} else {
					self.session_access_miss.inc();
				}
				self.session_access_latency
					.observe(latency_ms as f64 / 1000.0);
			}
			SessionEvent::Deleted { reason, .. } => {
				let reason_str = match reason {
					DeletionReason::Explicit => "explicit",
					DeletionReason::Expired => "expired",
					DeletionReason::Invalidated => "invalidated",
					DeletionReason::Replaced => "replaced",
				};
				self.session_deleted.with_label_values(&[reason_str]).inc();
			}
			SessionEvent::Expired { .. } => {
				self.session_expired.inc();
			}
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use chrono::Utc;
	use rstest::rstest;
	use std::sync::OnceLock;

	static ANALYTICS: OnceLock<PrometheusAnalytics> = OnceLock::new();

	fn get_analytics() -> &'static PrometheusAnalytics {
		ANALYTICS.get_or_init(|| PrometheusAnalytics::new().unwrap())
	}

	#[rstest]
	#[tokio::test]
	async fn test_prometheus_analytics_created() {
		let analytics = get_analytics();

		let event = SessionEvent::Created {
			session_key: "test_key".to_string(),
			size_bytes: 1024,
			ttl_secs: Some(3600),
			timestamp: Utc::now(),
		};

		analytics.record_event(event).await;

		assert!(analytics.session_created().get() > 0);
	}

	#[rstest]
	#[tokio::test]
	async fn test_prometheus_analytics_accessed() {
		let analytics = get_analytics();

		let event_hit = SessionEvent::Accessed {
			session_key: "test_key".to_string(),
			latency_ms: 10,
			hit: true,
			timestamp: Utc::now(),
		};

		analytics.record_event(event_hit).await;

		assert!(analytics.session_accessed().get() > 0);
		assert!(analytics.session_access_hit().get() > 0);

		let event_miss = SessionEvent::Accessed {
			session_key: "test_key2".to_string(),
			latency_ms: 5,
			hit: false,
			timestamp: Utc::now(),
		};

		analytics.record_event(event_miss).await;

		assert!(analytics.session_access_miss().get() > 0);
	}

	#[rstest]
	#[tokio::test]
	async fn test_prometheus_analytics_deleted() {
		let analytics = get_analytics();

		let event = SessionEvent::Deleted {
			session_key: "test_key".to_string(),
			reason: DeletionReason::Explicit,
			timestamp: Utc::now(),
		};

		analytics.record_event(event).await;

		assert!(
			analytics
				.session_deleted()
				.with_label_values(&["explicit"])
				.get() > 0
		);
	}

	#[rstest]
	#[tokio::test]
	async fn test_prometheus_analytics_expired() {
		let analytics = get_analytics();

		let event = SessionEvent::Expired {
			session_key: "test_key".to_string(),
			age_secs: 7200,
			timestamp: Utc::now(),
		};

		analytics.record_event(event).await;

		assert!(analytics.session_expired().get() > 0);
	}
}
