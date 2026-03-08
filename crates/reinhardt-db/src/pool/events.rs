//! Connection pool events

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Events that can occur in the connection pool
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PoolEvent {
	/// Connection acquired from pool
	ConnectionAcquired {
		/// The connection id.
		connection_id: String,
		/// The timestamp.
		timestamp: DateTime<Utc>,
	},

	/// Connection returned to pool
	ConnectionReturned {
		/// The connection id.
		connection_id: String,
		/// The timestamp.
		timestamp: DateTime<Utc>,
	},

	/// New connection created
	ConnectionCreated {
		/// The connection id.
		connection_id: String,
		/// The timestamp.
		timestamp: DateTime<Utc>,
	},

	/// Connection closed
	ConnectionClosed {
		/// The connection id.
		connection_id: String,
		/// The reason.
		reason: String,
		/// The timestamp.
		timestamp: DateTime<Utc>,
	},

	/// Connection test failed
	ConnectionTestFailed {
		/// The connection id.
		connection_id: String,
		/// The error.
		error: String,
		/// The timestamp.
		timestamp: DateTime<Utc>,
	},

	/// Connection invalidated (hard invalidation)
	ConnectionInvalidated {
		/// The connection id.
		connection_id: String,
		/// The reason.
		reason: String,
		/// The timestamp.
		timestamp: DateTime<Utc>,
	},

	/// Connection soft invalidated (can complete current operation)
	ConnectionSoftInvalidated {
		/// The connection id.
		connection_id: String,
		/// The timestamp.
		timestamp: DateTime<Utc>,
	},

	/// Connection reset
	ConnectionReset {
		/// The connection id.
		connection_id: String,
		/// The timestamp.
		timestamp: DateTime<Utc>,
	},
}

impl PoolEvent {
	/// Documentation for `connection_acquired`
	///
	pub fn connection_acquired(connection_id: String) -> Self {
		Self::ConnectionAcquired {
			connection_id,
			timestamp: Utc::now(),
		}
	}
	/// Documentation for `connection_returned`
	///
	pub fn connection_returned(connection_id: String) -> Self {
		Self::ConnectionReturned {
			connection_id,
			timestamp: Utc::now(),
		}
	}
	/// Documentation for `connection_created`
	///
	pub fn connection_created(connection_id: String) -> Self {
		Self::ConnectionCreated {
			connection_id,
			timestamp: Utc::now(),
		}
	}
	/// Documentation for `connection_closed`
	///
	pub fn connection_closed(connection_id: String, reason: String) -> Self {
		Self::ConnectionClosed {
			connection_id,
			reason,
			timestamp: Utc::now(),
		}
	}

	/// Performs the connection test failed operation.
	pub fn connection_test_failed(connection_id: String, error: String) -> Self {
		Self::ConnectionTestFailed {
			connection_id,
			error,
			timestamp: Utc::now(),
		}
	}
	/// Documentation for `connection_invalidated`
	///
	pub fn connection_invalidated(connection_id: String, reason: String) -> Self {
		Self::ConnectionInvalidated {
			connection_id,
			reason,
			timestamp: Utc::now(),
		}
	}
	/// Documentation for `connection_soft_invalidated`
	///
	pub fn connection_soft_invalidated(connection_id: String) -> Self {
		Self::ConnectionSoftInvalidated {
			connection_id,
			timestamp: Utc::now(),
		}
	}
	/// Documentation for `connection_reset`
	///
	pub fn connection_reset(connection_id: String) -> Self {
		Self::ConnectionReset {
			connection_id,
			timestamp: Utc::now(),
		}
	}
}

/// Trait for listening to pool events
#[async_trait]
pub trait PoolEventListener: Send + Sync {
	/// Handle a pool event
	async fn on_event(&self, event: PoolEvent);
}

/// Simple event logger
pub struct EventLogger;

#[async_trait]
impl PoolEventListener for EventLogger {
	async fn on_event(&self, event: PoolEvent) {
		match event {
			PoolEvent::ConnectionAcquired { connection_id, .. } => {
				println!("Connection acquired: {}", connection_id);
			}
			PoolEvent::ConnectionReturned { connection_id, .. } => {
				println!("Connection returned: {}", connection_id);
			}
			PoolEvent::ConnectionCreated { connection_id, .. } => {
				println!("Connection created: {}", connection_id);
			}
			PoolEvent::ConnectionClosed {
				connection_id,
				reason,
				..
			} => {
				println!("Connection closed: {} (reason: {})", connection_id, reason);
			}
			PoolEvent::ConnectionTestFailed {
				connection_id,
				error,
				..
			} => {
				println!(
					"Connection test failed: {} (error: {})",
					connection_id, error
				);
			}
			PoolEvent::ConnectionInvalidated {
				connection_id,
				reason,
				..
			} => {
				println!(
					"Connection invalidated: {} (reason: {})",
					connection_id, reason
				);
			}
			PoolEvent::ConnectionSoftInvalidated { connection_id, .. } => {
				println!("Connection soft invalidated: {}", connection_id);
			}
			PoolEvent::ConnectionReset { connection_id, .. } => {
				println!("Connection reset: {}", connection_id);
			}
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_pool_event_creation() {
		let event = PoolEvent::connection_acquired("conn-1".to_string());
		match event {
			PoolEvent::ConnectionAcquired { connection_id, .. } => {
				assert_eq!(connection_id, "conn-1");
			}
			_ => panic!("Wrong event type"),
		}
	}
}
