//! Adaptive rate limiting implementation
//!
//! Dynamically adjusts rate limits based on system load, error rates,
//! and other performance metrics.

use super::backend::ThrottleBackend;
use super::{Throttle, ThrottleError, ThrottleResult};
use super::time_provider::{SystemTimeProvider, TimeProvider};
use async_trait::async_trait;
use parking_lot::RwLock;
use std::sync::Arc;
use tokio::time::{Duration, Instant};

/// System load metrics for adaptive throttling
#[derive(Debug, Clone, Copy, Default)]
pub struct LoadMetrics {
	/// Current error rate (0.0 - 1.0)
	pub error_rate: f64,
	/// Current average response time in milliseconds
	pub avg_response_time: f64,
	/// Current CPU usage (0.0 - 1.0)
	pub cpu_usage: f64,
}

impl LoadMetrics {
	/// Creates new load metrics
	pub fn new(error_rate: f64, avg_response_time: f64, cpu_usage: f64) -> Self {
		Self {
			error_rate,
			avg_response_time,
			cpu_usage,
		}
	}

	/// Calculate overall system stress (0.0 - 1.0)
	pub fn calculate_stress(&self) -> f64 {
		// Weight different factors
		let error_weight = 0.4;
		let response_weight = 0.3;
		let cpu_weight = 0.3;

		// Normalize response time (assume >1000ms is high stress)
		let response_stress = (self.avg_response_time / 1000.0).min(1.0);

		(self.error_rate * error_weight)
			+ (response_stress * response_weight)
			+ (self.cpu_usage * cpu_weight)
	}
}

/// Configuration for adaptive throttling
#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct AdaptiveConfig {
	/// Minimum rate limit (requests, period)
	pub min_rate: (usize, u64),
	/// Maximum rate limit (requests, period)
	pub max_rate: (usize, u64),
	/// Initial rate limit (requests, period)
	pub initial_rate: (usize, u64),
	/// How quickly to adjust rates (0.0 - 1.0)
	pub adjustment_speed: f64,
	/// Stress threshold for rate decrease (0.0 - 1.0)
	pub stress_threshold: f64,
}

impl AdaptiveConfig {
	/// Creates a new adaptive configuration
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::throttling::adaptive::AdaptiveConfig;
	///
	/// let config = AdaptiveConfig::new(
	///     (10, 60),   // Min: 10 req/min
	///     (100, 60),  // Max: 100 req/min
	///     (50, 60),   // Initial: 50 req/min
	///     0.1,        // Adjustment speed: 10%
	///     0.7         // Decrease rate when stress > 70%
	/// );
	/// ```
	pub fn new(
		min_rate: (usize, u64),
		max_rate: (usize, u64),
		initial_rate: (usize, u64),
		adjustment_speed: f64,
		stress_threshold: f64,
	) -> Self {
		Self {
			min_rate,
			max_rate,
			initial_rate,
			adjustment_speed,
			stress_threshold,
		}
	}
}

impl Default for AdaptiveConfig {
	/// Create configuration with default values
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::throttling::adaptive::AdaptiveConfig;
	///
	/// let config = AdaptiveConfig::default();
	/// assert_eq!(config.min_rate, (10, 60));
	/// assert_eq!(config.max_rate, (1000, 60));
	/// ```
	fn default() -> Self {
		Self {
			min_rate: (10, 60),
			max_rate: (1000, 60),
			initial_rate: (100, 60),
			adjustment_speed: 0.1,
			stress_threshold: 0.7,
		}
	}
}

/// Adaptive state for tracking current rate
#[derive(Debug, Clone)]
struct AdaptiveState {
	current_rate: (usize, u64),
	last_adjustment: Instant,
	metrics_history: Vec<LoadMetrics>,
}

/// Adaptive rate limiting throttle
///
/// # Examples
///
/// ```
/// use reinhardt_rest::throttling::adaptive::{AdaptiveThrottle, AdaptiveConfig};
/// use reinhardt_rest::throttling::{MemoryBackend, Throttle};
/// use std::sync::Arc;
///
/// # tokio_test::block_on(async {
/// let backend = Arc::new(MemoryBackend::new());
/// let config = AdaptiveConfig::default();
/// let throttle = AdaptiveThrottle::new(backend, config);
/// # });
/// ```
pub struct AdaptiveThrottle<B: ThrottleBackend, T: TimeProvider = SystemTimeProvider> {
	backend: Arc<B>,
	config: AdaptiveConfig,
	state: Arc<RwLock<AdaptiveState>>,
	time_provider: Arc<T>,
}

impl<B: ThrottleBackend> AdaptiveThrottle<B, SystemTimeProvider> {
	/// Creates a new adaptive throttle with system time provider
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::throttling::adaptive::{AdaptiveThrottle, AdaptiveConfig};
	/// use reinhardt_rest::throttling::MemoryBackend;
	/// use std::sync::Arc;
	///
	/// let backend = Arc::new(MemoryBackend::new());
	/// let config = AdaptiveConfig::default();
	/// let throttle = AdaptiveThrottle::new(backend, config);
	/// ```
	pub fn new(backend: Arc<B>, config: AdaptiveConfig) -> Self {
		let initial_state = AdaptiveState {
			current_rate: config.initial_rate,
			last_adjustment: SystemTimeProvider::new().now(),
			metrics_history: Vec::new(),
		};

		Self {
			backend,
			config,
			state: Arc::new(RwLock::new(initial_state)),
			time_provider: Arc::new(SystemTimeProvider::new()),
		}
	}
}

impl<B: ThrottleBackend, T: TimeProvider> AdaptiveThrottle<B, T> {
	/// Creates a new adaptive throttle with custom time provider
	pub fn with_time_provider(
		backend: Arc<B>,
		config: AdaptiveConfig,
		time_provider: Arc<T>,
	) -> Self {
		let initial_state = AdaptiveState {
			current_rate: config.initial_rate,
			last_adjustment: time_provider.now(),
			metrics_history: Vec::new(),
		};

		Self {
			backend,
			config,
			state: Arc::new(RwLock::new(initial_state)),
			time_provider,
		}
	}

	/// Update system load metrics
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::throttling::adaptive::{AdaptiveThrottle, AdaptiveConfig, LoadMetrics};
	/// use reinhardt_rest::throttling::{MemoryBackend, Throttle};
	/// use std::sync::Arc;
	///
	/// # tokio_test::block_on(async {
	/// let backend = Arc::new(MemoryBackend::new());
	/// let config = AdaptiveConfig::default();
	/// let throttle = AdaptiveThrottle::new(backend, config);
	///
	/// let metrics = LoadMetrics::new(0.05, 100.0, 0.6);
	/// throttle.update_metrics(metrics).await;
	/// # });
	/// ```
	pub async fn update_metrics(&self, metrics: LoadMetrics) {
		let mut state = self.state.write();

		// Add to history (keep last 10 metrics)
		state.metrics_history.push(metrics);
		if state.metrics_history.len() > 10 {
			state.metrics_history.remove(0);
		}

		// Adjust rate if enough time has passed
		let now = self.time_provider.now();
		if now.duration_since(state.last_adjustment) > Duration::from_secs(5) {
			self.adjust_rate(&mut state, metrics);
			state.last_adjustment = now;
		}
	}

	/// Adjust the rate based on system stress
	fn adjust_rate(&self, state: &mut AdaptiveState, metrics: LoadMetrics) {
		let stress = metrics.calculate_stress();
		let (current_requests, period) = state.current_rate;

		let new_requests = if stress > self.config.stress_threshold {
			// Decrease rate when under stress
			let decrease_factor = 1.0 - self.config.adjustment_speed;
			((current_requests as f64) * decrease_factor) as usize
		} else {
			// Increase rate when system is healthy
			let increase_factor = 1.0 + self.config.adjustment_speed;
			((current_requests as f64) * increase_factor) as usize
		};

		// Clamp to min/max bounds
		let (min_requests, _) = self.config.min_rate;
		let (max_requests, _) = self.config.max_rate;
		let clamped_requests = new_requests.clamp(min_requests, max_requests);

		state.current_rate = (clamped_requests, period);
	}

	/// Get current rate limit
	pub fn get_current_rate(&self) -> (usize, u64) {
		self.state.read().current_rate
	}

	/// Get average system stress from recent metrics
	pub fn get_average_stress(&self) -> f64 {
		let state = self.state.read();
		if state.metrics_history.is_empty() {
			return 0.0;
		}

		let total_stress: f64 = state
			.metrics_history
			.iter()
			.map(|m| m.calculate_stress())
			.sum();

		total_stress / (state.metrics_history.len() as f64)
	}
}

#[async_trait]
impl<B: ThrottleBackend, T: TimeProvider> Throttle for AdaptiveThrottle<B, T> {
	async fn allow_request(&self, key: &str) -> ThrottleResult<bool> {
		let (rate, period) = self.get_current_rate();

		let count = self
			.backend
			.increment(key, period)
			.await
			.map_err(ThrottleError::ThrottleError)?;

		Ok(count <= rate)
	}

	async fn wait_time(&self, key: &str) -> ThrottleResult<Option<u64>> {
		let (rate, period) = self.get_current_rate();

		let count = self
			.backend
			.get_count(key)
			.await
			.map_err(ThrottleError::ThrottleError)?;

		if count > rate {
			Ok(Some(period))
		} else {
			Ok(None)
		}
	}

	fn get_rate(&self) -> (usize, u64) {
		self.get_current_rate()
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::throttling::backend::MemoryBackend;
	use crate::throttling::time_provider::MockTimeProvider;

	#[test]
	fn test_load_metrics_calculate_stress() {
		let metrics = LoadMetrics::new(0.1, 500.0, 0.5);
		let stress = metrics.calculate_stress();

		// Stress should be a weighted combination
		assert!(stress > 0.0 && stress < 1.0);
	}

	#[test]
	fn test_load_metrics_high_stress() {
		let metrics = LoadMetrics::new(0.9, 1500.0, 0.9);
		let stress = metrics.calculate_stress();

		// High metrics should result in high stress
		assert!(stress > 0.7);
	}

	#[tokio::test]
	async fn test_adaptive_throttle_basic() {
		let backend = Arc::new(MemoryBackend::new());
		let config = AdaptiveConfig::new((10, 60), (100, 60), (50, 60), 0.1, 0.7);
		let throttle = AdaptiveThrottle::new(backend, config);

		// Should start with initial rate
		assert_eq!(throttle.get_current_rate(), (50, 60));

		// Should allow requests up to current rate
		for _ in 0..50 {
			assert!(throttle.allow_request("test_key").await.unwrap());
		}

		assert!(!throttle.allow_request("test_key").await.unwrap());
	}

	#[tokio::test]
	async fn test_adaptive_throttle_metrics_update() {
		use tokio::time::Instant;
		let time_provider = Arc::new(MockTimeProvider::new(Instant::now()));
		let backend = Arc::new(MemoryBackend::with_time_provider(time_provider.clone()));
		let config = AdaptiveConfig::new((10, 60), (100, 60), (50, 60), 0.2, 0.7);
		let throttle = AdaptiveThrottle::with_time_provider(backend, config, time_provider.clone());

		// Initial rate
		assert_eq!(throttle.get_current_rate(), (50, 60));

		// Update with high stress metrics
		let high_stress = LoadMetrics::new(0.9, 1500.0, 0.9);
		throttle.update_metrics(high_stress).await;

		// Advance time to trigger adjustment
		time_provider.advance(std::time::Duration::from_secs(6));

		// Update again to trigger adjustment
		throttle.update_metrics(high_stress).await;

		// Rate should decrease
		let new_rate = throttle.get_current_rate();
		assert!(new_rate.0 < 50);
	}

	#[tokio::test]
	async fn test_adaptive_throttle_rate_increase() {
		use tokio::time::Instant;
		let time_provider = Arc::new(MockTimeProvider::new(Instant::now()));
		let backend = Arc::new(MemoryBackend::with_time_provider(time_provider.clone()));
		let config = AdaptiveConfig::new((10, 60), (100, 60), (50, 60), 0.2, 0.7);
		let throttle = AdaptiveThrottle::with_time_provider(backend, config, time_provider.clone());

		// Update with low stress metrics
		let low_stress = LoadMetrics::new(0.0, 50.0, 0.3);
		throttle.update_metrics(low_stress).await;

		// Advance time to trigger adjustment
		time_provider.advance(std::time::Duration::from_secs(6));

		// Update again to trigger adjustment
		throttle.update_metrics(low_stress).await;

		// Rate should increase
		let new_rate = throttle.get_current_rate();
		assert!(new_rate.0 > 50);
	}

	#[tokio::test]
	async fn test_adaptive_throttle_rate_bounds() {
		use tokio::time::Instant;
		let time_provider = Arc::new(MockTimeProvider::new(Instant::now()));
		let backend = Arc::new(MemoryBackend::with_time_provider(time_provider.clone()));
		let config = AdaptiveConfig::new((10, 60), (100, 60), (50, 60), 0.5, 0.7);
		let throttle = AdaptiveThrottle::with_time_provider(backend, config, time_provider.clone());

		// Try to decrease rate below minimum
		let high_stress = LoadMetrics::new(1.0, 2000.0, 1.0);
		for _ in 0..20 {
			throttle.update_metrics(high_stress).await;
			time_provider.advance(std::time::Duration::from_secs(6));
		}

		// Should not go below minimum
		let rate = throttle.get_current_rate();
		assert!(rate.0 >= 10);
	}

	#[tokio::test]
	async fn test_adaptive_throttle_average_stress() {
		let backend = Arc::new(MemoryBackend::new());
		let config = AdaptiveConfig::default();
		let throttle = AdaptiveThrottle::new(backend, config);

		// No metrics yet
		assert_eq!(throttle.get_average_stress(), 0.0);

		// Add some metrics
		throttle
			.update_metrics(LoadMetrics::new(0.5, 500.0, 0.5))
			.await;
		throttle
			.update_metrics(LoadMetrics::new(0.3, 300.0, 0.4))
			.await;

		let avg = throttle.get_average_stress();
		assert!(avg > 0.0 && avg < 1.0);
	}

	#[test]
	fn test_adaptive_config_default() {
		let config = AdaptiveConfig::default();
		assert_eq!(config.min_rate, (10, 60));
		assert_eq!(config.max_rate, (1000, 60));
		assert_eq!(config.initial_rate, (100, 60));
	}
}
