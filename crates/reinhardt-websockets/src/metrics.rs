//! WebSocket metrics and monitoring
//!
//! This module provides functionality for collecting and monitoring metrics of WebSocket connections and messages.
//! It can track the number of active connections, messages, errors, and more.
//!
//! ## Usage Examples
//!
//! ```
//! use reinhardt_websockets::metrics::{WebSocketMetrics, MetricsCollector};
//!
//! let metrics = WebSocketMetrics::new();
//!
//! // Record connections
//! metrics.record_connection();
//! metrics.record_message_sent();
//! metrics.record_message_received();
//!
//! // Get metrics
//! let snapshot = metrics.snapshot();
//! println!("Active connections: {}", snapshot.active_connections);
//! println!("Messages sent: {}", snapshot.messages_sent);
//! println!("Messages received: {}", snapshot.messages_received);
//! ```

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

/// WebSocket metrics snapshot
#[derive(Debug, Clone, Default)]
pub struct MetricsSnapshot {
	/// Number of active connections
	pub active_connections: u64,
	/// Total number of connections
	pub total_connections: u64,
	/// Number of messages sent
	pub messages_sent: u64,
	/// Number of messages received
	pub messages_received: u64,
	/// Number of bytes sent
	pub bytes_sent: u64,
	/// Number of bytes received
	pub bytes_received: u64,
	/// Number of errors
	pub errors: u64,
	/// Number of disconnections
	pub disconnections: u64,
}

impl MetricsSnapshot {
	/// Returns a summary of the metrics as a string.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_websockets::metrics::MetricsSnapshot;
	///
	/// let snapshot = MetricsSnapshot {
	///     active_connections: 10,
	///     total_connections: 100,
	///     messages_sent: 500,
	///     messages_received: 450,
	///     ..Default::default()
	/// };
	///
	/// let summary = snapshot.summary();
	/// assert!(summary.contains("Active: 10"));
	/// assert!(summary.contains("Total: 100"));
	/// ```
	pub fn summary(&self) -> String {
		format!(
			"Connections [Active: {}, Total: {}, Disconnections: {}] | Messages [Sent: {}, Received: {}] | Bytes [Sent: {}, Received: {}] | Errors: {}",
			self.active_connections,
			self.total_connections,
			self.disconnections,
			self.messages_sent,
			self.messages_received,
			self.bytes_sent,
			self.bytes_received,
			self.errors
		)
	}
}

/// Metrics collection trait
pub trait MetricsCollector: Send + Sync {
	/// Record a connection
	fn record_connection(&self);
	/// Record a disconnection
	fn record_disconnection(&self);
	/// Record a sent message
	fn record_message_sent(&self);
	/// Record a received message
	fn record_message_received(&self);
	/// Record bytes sent
	fn record_bytes_sent(&self, bytes: u64);
	/// Record bytes received
	fn record_bytes_received(&self, bytes: u64);
	/// Record an error
	fn record_error(&self);
	/// Get a snapshot of metrics
	fn snapshot(&self) -> MetricsSnapshot;
	/// Reset metrics
	fn reset(&self);
}

/// WebSocket metrics
///
/// # Examples
///
/// ```
/// use reinhardt_websockets::metrics::{WebSocketMetrics, MetricsCollector};
///
/// let metrics = WebSocketMetrics::new();
///
/// metrics.record_connection();
/// metrics.record_message_sent();
///
/// let snapshot = metrics.snapshot();
/// assert_eq!(snapshot.active_connections, 1);
/// assert_eq!(snapshot.messages_sent, 1);
/// ```
pub struct WebSocketMetrics {
	active_connections: Arc<AtomicU64>,
	total_connections: Arc<AtomicU64>,
	messages_sent: Arc<AtomicU64>,
	messages_received: Arc<AtomicU64>,
	bytes_sent: Arc<AtomicU64>,
	bytes_received: Arc<AtomicU64>,
	errors: Arc<AtomicU64>,
	disconnections: Arc<AtomicU64>,
}

impl WebSocketMetrics {
	/// Creates a new metrics instance.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_websockets::metrics::WebSocketMetrics;
	///
	/// let metrics = WebSocketMetrics::new();
	/// ```
	pub fn new() -> Self {
		Self {
			active_connections: Arc::new(AtomicU64::new(0)),
			total_connections: Arc::new(AtomicU64::new(0)),
			messages_sent: Arc::new(AtomicU64::new(0)),
			messages_received: Arc::new(AtomicU64::new(0)),
			bytes_sent: Arc::new(AtomicU64::new(0)),
			bytes_received: Arc::new(AtomicU64::new(0)),
			errors: Arc::new(AtomicU64::new(0)),
			disconnections: Arc::new(AtomicU64::new(0)),
		}
	}
}

impl Default for WebSocketMetrics {
	fn default() -> Self {
		Self::new()
	}
}

impl MetricsCollector for WebSocketMetrics {
	fn record_connection(&self) {
		// Use saturating_add for the gauge to prevent overflow in extreme cases
		let _ = self
			.active_connections
			.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |v| {
				Some(v.saturating_add(1))
			});
		// Wrapping is acceptable for monotonic counters (overflow after ~584,942 years at 1M/s)
		self.total_connections.fetch_add(1, Ordering::Relaxed);
	}

	fn record_disconnection(&self) {
		// Use saturating_sub to prevent underflow when disconnection events
		// exceed connection events (e.g., due to duplicate disconnect callbacks)
		let _ = self
			.active_connections
			.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |v| {
				Some(v.saturating_sub(1))
			});
		self.disconnections.fetch_add(1, Ordering::Relaxed);
	}

	fn record_message_sent(&self) {
		self.messages_sent.fetch_add(1, Ordering::Relaxed);
	}

	fn record_message_received(&self) {
		self.messages_received.fetch_add(1, Ordering::Relaxed);
	}

	fn record_bytes_sent(&self, bytes: u64) {
		// Use saturating_add for byte counters to prevent wrapping on high-throughput servers
		let _ = self
			.bytes_sent
			.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |v| {
				Some(v.saturating_add(bytes))
			});
	}

	fn record_bytes_received(&self, bytes: u64) {
		let _ = self
			.bytes_received
			.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |v| {
				Some(v.saturating_add(bytes))
			});
	}

	fn record_error(&self) {
		self.errors.fetch_add(1, Ordering::Relaxed);
	}

	fn snapshot(&self) -> MetricsSnapshot {
		MetricsSnapshot {
			active_connections: self.active_connections.load(Ordering::Relaxed),
			total_connections: self.total_connections.load(Ordering::Relaxed),
			messages_sent: self.messages_sent.load(Ordering::Relaxed),
			messages_received: self.messages_received.load(Ordering::Relaxed),
			bytes_sent: self.bytes_sent.load(Ordering::Relaxed),
			bytes_received: self.bytes_received.load(Ordering::Relaxed),
			errors: self.errors.load(Ordering::Relaxed),
			disconnections: self.disconnections.load(Ordering::Relaxed),
		}
	}

	fn reset(&self) {
		self.active_connections.store(0, Ordering::Relaxed);
		self.total_connections.store(0, Ordering::Relaxed);
		self.messages_sent.store(0, Ordering::Relaxed);
		self.messages_received.store(0, Ordering::Relaxed);
		self.bytes_sent.store(0, Ordering::Relaxed);
		self.bytes_received.store(0, Ordering::Relaxed);
		self.errors.store(0, Ordering::Relaxed);
		self.disconnections.store(0, Ordering::Relaxed);
	}
}

/// Trait for converting metrics to exportable formats
#[cfg(feature = "metrics")]
pub trait MetricsExporter {
	/// Export in Prometheus format
	fn export_prometheus(&self) -> String;
	/// Export in JSON format
	fn export_json(&self) -> String;
}

#[cfg(feature = "metrics")]
impl MetricsExporter for MetricsSnapshot {
	/// Exports metrics in Prometheus format.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_websockets::metrics::{MetricsSnapshot, MetricsExporter};
	///
	/// let snapshot = MetricsSnapshot {
	///     active_connections: 10,
	///     total_connections: 100,
	///     ..Default::default()
	/// };
	///
	/// let prometheus = snapshot.export_prometheus();
	/// assert!(prometheus.contains("websocket_active_connections 10"));
	/// ```
	fn export_prometheus(&self) -> String {
		format!(
			"# HELP websocket_active_connections Number of active WebSocket connections\n\
             # TYPE websocket_active_connections gauge\n\
             websocket_active_connections {}\n\
             # HELP websocket_total_connections Total number of WebSocket connections\n\
             # TYPE websocket_total_connections counter\n\
             websocket_total_connections {}\n\
             # HELP websocket_messages_sent Total messages sent\n\
             # TYPE websocket_messages_sent counter\n\
             websocket_messages_sent {}\n\
             # HELP websocket_messages_received Total messages received\n\
             # TYPE websocket_messages_received counter\n\
             websocket_messages_received {}\n\
             # HELP websocket_bytes_sent Total bytes sent\n\
             # TYPE websocket_bytes_sent counter\n\
             websocket_bytes_sent {}\n\
             # HELP websocket_bytes_received Total bytes received\n\
             # TYPE websocket_bytes_received counter\n\
             websocket_bytes_received {}\n\
             # HELP websocket_errors Total errors\n\
             # TYPE websocket_errors counter\n\
             websocket_errors {}\n\
             # HELP websocket_disconnections Total disconnections\n\
             # TYPE websocket_disconnections counter\n\
             websocket_disconnections {}\n",
			self.active_connections,
			self.total_connections,
			self.messages_sent,
			self.messages_received,
			self.bytes_sent,
			self.bytes_received,
			self.errors,
			self.disconnections
		)
	}

	/// Exports metrics in JSON format.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_websockets::metrics::{MetricsSnapshot, MetricsExporter};
	///
	/// let snapshot = MetricsSnapshot {
	///     active_connections: 10,
	///     total_connections: 100,
	///     ..Default::default()
	/// };
	///
	/// let json = snapshot.export_json();
	/// assert!(json.contains("\"active_connections\":10"));
	/// ```
	fn export_json(&self) -> String {
		serde_json::json!({
			"active_connections": self.active_connections,
			"total_connections": self.total_connections,
			"messages_sent": self.messages_sent,
			"messages_received": self.messages_received,
			"bytes_sent": self.bytes_sent,
			"bytes_received": self.bytes_received,
			"errors": self.errors,
			"disconnections": self.disconnections
		})
		.to_string()
	}
}

/// Periodic metrics reporter
///
/// # Examples
///
/// ```
/// use reinhardt_websockets::metrics::{WebSocketMetrics, PeriodicReporter};
/// use std::sync::Arc;
/// use std::time::Duration;
///
/// # tokio_test::block_on(async {
/// let metrics = Arc::new(WebSocketMetrics::new());
/// let reporter = PeriodicReporter::new(
///     metrics.clone(),
///     Duration::from_secs(10),
///     |snapshot| {
///         println!("Metrics: {}", snapshot.summary());
///     }
/// );
///
/// // Start reporting in the background
/// // reporter.start().await;
/// # });
/// ```
pub struct PeriodicReporter<F>
where
	F: Fn(MetricsSnapshot) + Send + Sync + 'static,
{
	metrics: Arc<dyn MetricsCollector>,
	interval: Duration,
	callback: F,
}

impl<F> PeriodicReporter<F>
where
	F: Fn(MetricsSnapshot) + Send + Sync + 'static,
{
	/// Creates a new periodic reporter.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_websockets::metrics::{WebSocketMetrics, PeriodicReporter};
	/// use std::sync::Arc;
	/// use std::time::Duration;
	///
	/// let metrics = Arc::new(WebSocketMetrics::new());
	/// let reporter = PeriodicReporter::new(
	///     metrics,
	///     Duration::from_secs(10),
	///     |snapshot| {
	///         println!("{}", snapshot.summary());
	///     }
	/// );
	/// ```
	pub fn new(metrics: Arc<dyn MetricsCollector>, interval: Duration, callback: F) -> Self {
		Self {
			metrics,
			interval,
			callback,
		}
	}

	/// Starts reporting (background task).
	///
	/// # Examples
	///
	/// ```no_run
	/// use reinhardt_websockets::metrics::{WebSocketMetrics, PeriodicReporter};
	/// use std::sync::Arc;
	/// use std::time::Duration;
	///
	/// # tokio_test::block_on(async {
	/// let metrics = Arc::new(WebSocketMetrics::new());
	/// let reporter = PeriodicReporter::new(
	///     metrics,
	///     Duration::from_secs(10),
	///     |snapshot| {
	///         println!("{}", snapshot.summary());
	///     }
	/// );
	///
	/// reporter.start().await;
	/// # });
	/// ```
	pub async fn start(self) {
		let mut interval = tokio::time::interval(self.interval);

		loop {
			interval.tick().await;
			let snapshot = self.metrics.snapshot();
			(self.callback)(snapshot);
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_metrics_creation() {
		let metrics = WebSocketMetrics::new();
		let snapshot = metrics.snapshot();

		assert_eq!(snapshot.active_connections, 0);
		assert_eq!(snapshot.total_connections, 0);
		assert_eq!(snapshot.messages_sent, 0);
		assert_eq!(snapshot.messages_received, 0);
	}

	#[test]
	fn test_record_connection() {
		let metrics = WebSocketMetrics::new();

		metrics.record_connection();
		metrics.record_connection();

		let snapshot = metrics.snapshot();
		assert_eq!(snapshot.active_connections, 2);
		assert_eq!(snapshot.total_connections, 2);
	}

	#[test]
	fn test_record_disconnection() {
		let metrics = WebSocketMetrics::new();

		metrics.record_connection();
		metrics.record_connection();
		metrics.record_disconnection();

		let snapshot = metrics.snapshot();
		assert_eq!(snapshot.active_connections, 1);
		assert_eq!(snapshot.disconnections, 1);
		assert_eq!(snapshot.total_connections, 2);
	}

	#[test]
	fn test_record_messages() {
		let metrics = WebSocketMetrics::new();

		metrics.record_message_sent();
		metrics.record_message_sent();
		metrics.record_message_received();

		let snapshot = metrics.snapshot();
		assert_eq!(snapshot.messages_sent, 2);
		assert_eq!(snapshot.messages_received, 1);
	}

	#[test]
	fn test_record_bytes() {
		let metrics = WebSocketMetrics::new();

		metrics.record_bytes_sent(100);
		metrics.record_bytes_sent(50);
		metrics.record_bytes_received(200);

		let snapshot = metrics.snapshot();
		assert_eq!(snapshot.bytes_sent, 150);
		assert_eq!(snapshot.bytes_received, 200);
	}

	#[test]
	fn test_record_error() {
		let metrics = WebSocketMetrics::new();

		metrics.record_error();
		metrics.record_error();

		let snapshot = metrics.snapshot();
		assert_eq!(snapshot.errors, 2);
	}

	#[test]
	fn test_reset() {
		let metrics = WebSocketMetrics::new();

		metrics.record_connection();
		metrics.record_message_sent();
		metrics.record_error();

		metrics.reset();

		let snapshot = metrics.snapshot();
		assert_eq!(snapshot.active_connections, 0);
		assert_eq!(snapshot.messages_sent, 0);
		assert_eq!(snapshot.errors, 0);
	}

	#[test]
	fn test_disconnection_does_not_underflow() {
		// Arrange - simulate more disconnections than connections
		let metrics = WebSocketMetrics::new();

		// Act - disconnect without prior connection
		metrics.record_disconnection();

		// Assert - should saturate at 0, not wrap to u64::MAX
		let snapshot = metrics.snapshot();
		assert_eq!(snapshot.active_connections, 0);
		assert_eq!(snapshot.disconnections, 1);
	}

	#[test]
	fn test_bytes_sent_saturates_instead_of_wrapping() {
		// Arrange
		let metrics = WebSocketMetrics::new();

		// Act - set bytes_sent near max and add more
		metrics.bytes_sent.store(u64::MAX - 10, Ordering::Relaxed);
		metrics.record_bytes_sent(100);

		// Assert - should saturate at u64::MAX
		let snapshot = metrics.snapshot();
		assert_eq!(snapshot.bytes_sent, u64::MAX);
	}

	#[test]
	fn test_snapshot_summary() {
		let snapshot = MetricsSnapshot {
			active_connections: 10,
			total_connections: 100,
			messages_sent: 500,
			messages_received: 450,
			bytes_sent: 10000,
			bytes_received: 9000,
			errors: 5,
			disconnections: 90,
		};

		let summary = snapshot.summary();
		assert!(summary.contains("Active: 10"));
		assert!(summary.contains("Total: 100"));
		assert!(summary.contains("Sent: 500"));
		assert!(summary.contains("Received: 450"));
	}

	#[cfg(feature = "metrics")]
	#[test]
	fn test_prometheus_export() {
		let snapshot = MetricsSnapshot {
			active_connections: 10,
			total_connections: 100,
			messages_sent: 500,
			messages_received: 450,
			..Default::default()
		};

		let prometheus = snapshot.export_prometheus();
		assert!(prometheus.contains("websocket_active_connections 10"));
		assert!(prometheus.contains("websocket_total_connections 100"));
		assert!(prometheus.contains("websocket_messages_sent 500"));
	}

	#[cfg(feature = "metrics")]
	#[test]
	fn test_json_export() {
		let snapshot = MetricsSnapshot {
			active_connections: 10,
			total_connections: 100,
			..Default::default()
		};

		let json = snapshot.export_json();
		assert!(json.contains("\"active_connections\":10"));
		assert!(json.contains("\"total_connections\":100"));
	}
}
