//! Connection pool optimization
//!
//! Provides advanced connection pooling configuration for optimal performance:
//! - Idle connection timeout management
//! - Dynamic pool sizing
//! - Connection health checks

use sqlx::pool::PoolOptions;
use std::time::Duration;

/// Pool optimization configuration
#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct PoolOptimizationConfig {
	/// Maximum number of connections in the pool
	pub max_connections: u32,

	/// Minimum number of idle connections to maintain
	pub min_connections: u32,

	/// Maximum lifetime of a connection before it's closed
	pub max_lifetime: Option<Duration>,

	/// Maximum idle time before a connection is closed
	pub idle_timeout: Option<Duration>,

	/// Timeout for acquiring a connection from the pool
	pub acquire_timeout: Duration,

	/// Enable connection health checks
	pub test_on_acquire: bool,
}

impl Default for PoolOptimizationConfig {
	fn default() -> Self {
		Self {
			max_connections: 10,
			min_connections: 2,
			max_lifetime: Some(Duration::from_secs(30 * 60)), // 30 minutes
			idle_timeout: Some(Duration::from_secs(10 * 60)), // 10 minutes
			acquire_timeout: Duration::from_secs(30),
			test_on_acquire: true,
		}
	}
}

/// Builder for optimized connection pools
pub struct OptimizedPoolBuilder<DB: sqlx::Database> {
	config: PoolOptimizationConfig,
	_phantom: std::marker::PhantomData<DB>,
}

impl<DB: sqlx::Database> OptimizedPoolBuilder<DB> {
	/// Create a new optimized pool builder
	pub fn new() -> Self {
		Self {
			config: PoolOptimizationConfig::default(),
			_phantom: std::marker::PhantomData,
		}
	}

	/// Set maximum connections
	pub fn max_connections(mut self, max: u32) -> Self {
		self.config.max_connections = max;
		self
	}

	/// Set minimum idle connections
	pub fn min_connections(mut self, min: u32) -> Self {
		self.config.min_connections = min;
		self
	}

	/// Set maximum connection lifetime
	pub fn max_lifetime(mut self, lifetime: Duration) -> Self {
		self.config.max_lifetime = Some(lifetime);
		self
	}

	/// Set idle timeout
	pub fn idle_timeout(mut self, timeout: Duration) -> Self {
		self.config.idle_timeout = Some(timeout);
		self
	}

	/// Set acquire timeout
	pub fn acquire_timeout(mut self, timeout: Duration) -> Self {
		self.config.acquire_timeout = timeout;
		self
	}

	/// Enable/disable connection health checks
	pub fn test_on_acquire(mut self, test: bool) -> Self {
		self.config.test_on_acquire = test;
		self
	}

	/// Build pool options with optimizations
	pub fn build_options(&self) -> PoolOptions<DB> {
		let mut options = PoolOptions::new()
			.max_connections(self.config.max_connections)
			.min_connections(self.config.min_connections)
			.acquire_timeout(self.config.acquire_timeout)
			.test_before_acquire(self.config.test_on_acquire);

		if let Some(lifetime) = self.config.max_lifetime {
			options = options.max_lifetime(lifetime);
		}

		if let Some(timeout) = self.config.idle_timeout {
			options = options.idle_timeout(timeout);
		}

		options
	}
}

impl<DB: sqlx::Database> Default for OptimizedPoolBuilder<DB> {
	fn default() -> Self {
		Self::new()
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_default_config() {
		let config = PoolOptimizationConfig::default();
		assert_eq!(config.max_connections, 10);
		assert_eq!(config.min_connections, 2);
		assert!(config.test_on_acquire);
	}

	#[test]
	fn test_builder() {
		let builder = OptimizedPoolBuilder::<sqlx::Postgres>::new()
			.max_connections(20)
			.min_connections(5)
			.test_on_acquire(false);

		assert_eq!(builder.config.max_connections, 20);
		assert_eq!(builder.config.min_connections, 5);
		assert!(!builder.config.test_on_acquire);
	}
}
