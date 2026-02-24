//! WebSocket automatic reconnection support
//!
//! This module provides automatic reconnection functionality when WebSocket connections are disconnected.
//! It uses an exponential backoff algorithm to adjust retry intervals.
//!
//! ## Usage Example
//!
//! ```
//! use reinhardt_websockets::reconnection::{ReconnectionConfig, ReconnectionStrategy};
//! use std::time::Duration;
//!
//! let config = ReconnectionConfig::default()
//!     .with_max_attempts(5)
//!     .with_initial_delay(Duration::from_secs(1))
//!     .with_max_delay(Duration::from_secs(60));
//!
//! let mut strategy = ReconnectionStrategy::new(config);
//!
//! // On connection failure
//! if let Some(delay) = strategy.next_delay() {
//!     println!("Retrying in {:?}", delay);
//! }
//! ```

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::RwLock;

/// Reconnection state representing the current phase of the reconnection process
#[derive(Debug, Clone, PartialEq)]
pub enum ReconnectionState {
	/// No reconnection is needed; connection is active
	Connected,
	/// Connection was lost and reconnection is being attempted
	Reconnecting {
		/// Current attempt number (1-based)
		attempt: u32,
		/// Delay before the next reconnection attempt
		next_delay: Duration,
	},
	/// Reconnection was successful after a disconnection
	Reconnected {
		/// Total number of attempts it took to reconnect
		total_attempts: u32,
	},
	/// All reconnection attempts have been exhausted
	Failed {
		/// Total number of attempts that were made
		total_attempts: u32,
	},
	/// Reconnection was explicitly disabled or cancelled
	Disabled,
}

/// Reconnection configuration
#[derive(Debug, Clone)]
pub struct ReconnectionConfig {
	/// Maximum number of reconnection attempts (None for unlimited)
	pub max_attempts: Option<u32>,
	/// Initial reconnection delay
	pub initial_delay: Duration,
	/// Maximum delay time
	pub max_delay: Duration,
	/// Backoff multiplier (default: 2.0)
	pub backoff_multiplier: f64,
	/// Jitter factor (default: 0.1 = 10%)
	pub jitter_factor: f64,
}

impl Default for ReconnectionConfig {
	/// Creates a default reconnection configuration.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_websockets::reconnection::ReconnectionConfig;
	/// use std::time::Duration;
	///
	/// let config = ReconnectionConfig::default();
	/// assert_eq!(config.max_attempts, Some(10));
	/// assert_eq!(config.initial_delay, Duration::from_secs(1));
	/// assert_eq!(config.max_delay, Duration::from_secs(300));
	/// assert_eq!(config.backoff_multiplier, 2.0);
	/// assert_eq!(config.jitter_factor, 0.1);
	/// ```
	fn default() -> Self {
		Self {
			max_attempts: Some(10),
			initial_delay: Duration::from_secs(1),
			max_delay: Duration::from_secs(300), // 5 minutes
			backoff_multiplier: 2.0,
			jitter_factor: 0.1,
		}
	}
}

impl ReconnectionConfig {
	/// Creates a new reconnection configuration.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_websockets::reconnection::ReconnectionConfig;
	/// use std::time::Duration;
	///
	/// let config = ReconnectionConfig::new(
	///     Some(5),
	///     Duration::from_secs(2),
	///     Duration::from_secs(60),
	/// );
	/// assert_eq!(config.max_attempts, Some(5));
	/// assert_eq!(config.initial_delay, Duration::from_secs(2));
	/// assert_eq!(config.max_delay, Duration::from_secs(60));
	/// ```
	pub fn new(max_attempts: Option<u32>, initial_delay: Duration, max_delay: Duration) -> Self {
		Self {
			max_attempts,
			initial_delay,
			max_delay,
			backoff_multiplier: 2.0,
			jitter_factor: 0.1,
		}
	}

	/// Sets the maximum number of reconnection attempts.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_websockets::reconnection::ReconnectionConfig;
	///
	/// let config = ReconnectionConfig::default()
	///     .with_max_attempts(5);
	/// assert_eq!(config.max_attempts, Some(5));
	/// ```
	pub fn with_max_attempts(mut self, max_attempts: u32) -> Self {
		self.max_attempts = Some(max_attempts);
		self
	}

	/// Sets unlimited reconnection attempts.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_websockets::reconnection::ReconnectionConfig;
	///
	/// let config = ReconnectionConfig::default()
	///     .with_unlimited_attempts();
	/// assert_eq!(config.max_attempts, None);
	/// ```
	pub fn with_unlimited_attempts(mut self) -> Self {
		self.max_attempts = None;
		self
	}

	/// Sets the initial reconnection delay.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_websockets::reconnection::ReconnectionConfig;
	/// use std::time::Duration;
	///
	/// let config = ReconnectionConfig::default()
	///     .with_initial_delay(Duration::from_secs(2));
	/// assert_eq!(config.initial_delay, Duration::from_secs(2));
	/// ```
	pub fn with_initial_delay(mut self, delay: Duration) -> Self {
		self.initial_delay = delay;
		self
	}

	/// Sets the maximum delay time.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_websockets::reconnection::ReconnectionConfig;
	/// use std::time::Duration;
	///
	/// let config = ReconnectionConfig::default()
	///     .with_max_delay(Duration::from_secs(120));
	/// assert_eq!(config.max_delay, Duration::from_secs(120));
	/// ```
	pub fn with_max_delay(mut self, delay: Duration) -> Self {
		self.max_delay = delay;
		self
	}

	/// Sets the backoff multiplier.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_websockets::reconnection::ReconnectionConfig;
	///
	/// let config = ReconnectionConfig::default()
	///     .with_backoff_multiplier(1.5);
	/// assert_eq!(config.backoff_multiplier, 1.5);
	/// ```
	pub fn with_backoff_multiplier(mut self, multiplier: f64) -> Self {
		self.backoff_multiplier = multiplier;
		self
	}

	/// Sets the jitter factor.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_websockets::reconnection::ReconnectionConfig;
	///
	/// let config = ReconnectionConfig::default()
	///     .with_jitter_factor(0.2);
	/// assert_eq!(config.jitter_factor, 0.2);
	/// ```
	pub fn with_jitter_factor(mut self, factor: f64) -> Self {
		self.jitter_factor = factor;
		self
	}
}

/// Reconnection strategy
pub struct ReconnectionStrategy {
	config: ReconnectionConfig,
	current_attempt: u32,
	current_delay: Duration,
}

impl ReconnectionStrategy {
	/// Creates a new reconnection strategy.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_websockets::reconnection::{ReconnectionConfig, ReconnectionStrategy};
	///
	/// let config = ReconnectionConfig::default();
	/// let strategy = ReconnectionStrategy::new(config);
	/// assert_eq!(strategy.attempt_count(), 0);
	/// ```
	pub fn new(config: ReconnectionConfig) -> Self {
		let current_delay = config.initial_delay;
		Self {
			config,
			current_attempt: 0,
			current_delay,
		}
	}

	/// Returns the current attempt count.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_websockets::reconnection::{ReconnectionConfig, ReconnectionStrategy};
	///
	/// let mut strategy = ReconnectionStrategy::new(ReconnectionConfig::default());
	/// assert_eq!(strategy.attempt_count(), 0);
	///
	/// strategy.next_delay();
	/// assert_eq!(strategy.attempt_count(), 1);
	/// ```
	pub fn attempt_count(&self) -> u32 {
		self.current_attempt
	}

	/// Returns the next reconnection delay.
	///
	/// Returns None if the maximum number of attempts has been reached.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_websockets::reconnection::{ReconnectionConfig, ReconnectionStrategy};
	/// use std::time::Duration;
	///
	/// let config = ReconnectionConfig::default()
	///     .with_max_attempts(2)
	///     .with_initial_delay(Duration::from_secs(1));
	///
	/// let mut strategy = ReconnectionStrategy::new(config);
	///
	/// // First attempt
	/// let delay1 = strategy.next_delay();
	/// assert!(delay1.is_some());
	///
	/// // Second attempt
	/// let delay2 = strategy.next_delay();
	/// assert!(delay2.is_some());
	///
	/// // Third attempt (exceeds max attempts)
	/// let delay3 = strategy.next_delay();
	/// assert!(delay3.is_none());
	/// ```
	pub fn next_delay(&mut self) -> Option<Duration> {
		// Check maximum attempt count
		if let Some(max) = self.config.max_attempts
			&& self.current_attempt >= max
		{
			return None;
		}

		let delay = if self.current_attempt == 0 {
			self.config.initial_delay
		} else {
			self.current_delay
		};

		// Apply jitter (±jitter_factor)
		let jitter = self.apply_jitter(delay);

		self.current_attempt += 1;

		// Calculate next delay (exponential backoff with overflow protection).
		// The multiplication is done in f64 space and clamped to max_delay to prevent
		// overflow when retry counts are high.
		let max_delay_secs = self.config.max_delay.as_secs_f64();
		let next_delay_secs = delay.as_secs_f64() * self.config.backoff_multiplier;
		// Clamp to max_delay; also handles NaN/Infinity by falling back to max_delay
		let clamped_secs = if next_delay_secs.is_finite() {
			next_delay_secs.min(max_delay_secs)
		} else {
			max_delay_secs
		};
		self.current_delay = Duration::from_secs_f64(clamped_secs);

		Some(jitter)
	}

	/// Returns the delay with applied jitter.
	fn apply_jitter(&self, delay: Duration) -> Duration {
		use std::collections::hash_map::RandomState;
		use std::hash::BuildHasher;

		// Simple pseudo-random value generation (for testability)
		let hash = RandomState::new().hash_one(self.current_attempt);
		let random = (hash % 1000) as f64 / 1000.0; // 0.0 ~ 1.0

		let jitter_range = delay.as_secs_f64() * self.config.jitter_factor;
		let jitter = (random - 0.5) * 2.0 * jitter_range; // -jitter_range ~ +jitter_range

		let final_delay = (delay.as_secs_f64() + jitter).max(0.0);
		Duration::from_secs_f64(final_delay)
	}

	/// Resets the strategy.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_websockets::reconnection::{ReconnectionConfig, ReconnectionStrategy};
	///
	/// let mut strategy = ReconnectionStrategy::new(ReconnectionConfig::default());
	///
	/// strategy.next_delay();
	/// strategy.next_delay();
	/// assert_eq!(strategy.attempt_count(), 2);
	///
	/// strategy.reset();
	/// assert_eq!(strategy.attempt_count(), 0);
	/// ```
	pub fn reset(&mut self) {
		self.current_attempt = 0;
		self.current_delay = self.config.initial_delay;
	}

	/// Returns whether reconnection is possible.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_websockets::reconnection::{ReconnectionConfig, ReconnectionStrategy};
	///
	/// let config = ReconnectionConfig::default().with_max_attempts(1);
	/// let mut strategy = ReconnectionStrategy::new(config);
	///
	/// assert!(strategy.can_reconnect());
	/// strategy.next_delay();
	/// assert!(!strategy.can_reconnect());
	/// ```
	pub fn can_reconnect(&self) -> bool {
		if let Some(max) = self.config.max_attempts {
			self.current_attempt < max
		} else {
			true
		}
	}

	/// Returns a reference to the reconnection configuration.
	pub fn config(&self) -> &ReconnectionConfig {
		&self.config
	}
}

/// Callback type for reconnection state changes
pub type OnReconnectStateChange = Box<dyn Fn(&ReconnectionState) + Send + Sync>;

/// Automatic reconnection handler that manages the reconnection lifecycle.
///
/// This handler wraps a `ReconnectionStrategy` and provides an async interface
/// for managing automatic reconnection with state tracking and callbacks.
///
/// # Examples
///
/// ```
/// use reinhardt_websockets::reconnection::{
///     AutoReconnectHandler, ReconnectionConfig, ReconnectionState,
/// };
///
/// # tokio_test::block_on(async {
/// let config = ReconnectionConfig::default().with_max_attempts(3);
/// let handler = AutoReconnectHandler::new(config);
///
/// assert!(handler.is_enabled());
/// assert_eq!(*handler.state().await, ReconnectionState::Connected);
/// # });
/// ```
pub struct AutoReconnectHandler {
	strategy: RwLock<ReconnectionStrategy>,
	state: Arc<RwLock<ReconnectionState>>,
	enabled: bool,
	on_state_change: Option<OnReconnectStateChange>,
}

impl AutoReconnectHandler {
	/// Creates a new auto-reconnect handler with the given configuration.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_websockets::reconnection::{AutoReconnectHandler, ReconnectionConfig};
	///
	/// let handler = AutoReconnectHandler::new(ReconnectionConfig::default());
	/// assert!(handler.is_enabled());
	/// ```
	pub fn new(config: ReconnectionConfig) -> Self {
		Self {
			strategy: RwLock::new(ReconnectionStrategy::new(config)),
			state: Arc::new(RwLock::new(ReconnectionState::Connected)),
			enabled: true,
			on_state_change: None,
		}
	}

	/// Creates a disabled auto-reconnect handler.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_websockets::reconnection::AutoReconnectHandler;
	///
	/// # tokio_test::block_on(async {
	/// let handler = AutoReconnectHandler::disabled();
	/// assert!(!handler.is_enabled());
	///
	/// // Attempting reconnection on a disabled handler returns None
	/// assert!(handler.on_disconnect().await.is_none());
	/// # });
	/// ```
	pub fn disabled() -> Self {
		Self {
			strategy: RwLock::new(ReconnectionStrategy::new(ReconnectionConfig::default())),
			state: Arc::new(RwLock::new(ReconnectionState::Disabled)),
			enabled: false,
			on_state_change: None,
		}
	}

	/// Sets a callback to be invoked when the reconnection state changes.
	pub fn with_on_state_change(mut self, callback: OnReconnectStateChange) -> Self {
		self.on_state_change = Some(callback);
		self
	}

	/// Returns whether auto-reconnect is enabled.
	pub fn is_enabled(&self) -> bool {
		self.enabled
	}

	/// Returns the current reconnection state.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_websockets::reconnection::{
	///     AutoReconnectHandler, ReconnectionConfig, ReconnectionState,
	/// };
	///
	/// # tokio_test::block_on(async {
	/// let handler = AutoReconnectHandler::new(ReconnectionConfig::default());
	/// let state = handler.state().await;
	/// assert_eq!(*state, ReconnectionState::Connected);
	/// # });
	/// ```
	pub async fn state(&self) -> tokio::sync::RwLockReadGuard<'_, ReconnectionState> {
		self.state.read().await
	}

	/// Called when a connection is lost. Returns the delay to wait before
	/// the next reconnection attempt, or `None` if reconnection is exhausted
	/// or disabled.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_websockets::reconnection::{
	///     AutoReconnectHandler, ReconnectionConfig, ReconnectionState,
	/// };
	/// use std::time::Duration;
	///
	/// # tokio_test::block_on(async {
	/// let config = ReconnectionConfig::default()
	///     .with_max_attempts(2)
	///     .with_initial_delay(Duration::from_secs(1))
	///     .with_jitter_factor(0.0);
	///
	/// let handler = AutoReconnectHandler::new(config);
	///
	/// // First disconnect
	/// let delay = handler.on_disconnect().await;
	/// assert!(delay.is_some());
	/// assert_eq!(delay.unwrap(), Duration::from_secs(1));
	///
	/// // Second disconnect
	/// let delay = handler.on_disconnect().await;
	/// assert!(delay.is_some());
	///
	/// // Third disconnect (exhausted)
	/// let delay = handler.on_disconnect().await;
	/// assert!(delay.is_none());
	/// # });
	/// ```
	pub async fn on_disconnect(&self) -> Option<Duration> {
		if !self.enabled {
			return None;
		}

		let mut strategy = self.strategy.write().await;
		let delay = strategy.next_delay();

		match delay {
			Some(d) => {
				let new_state = ReconnectionState::Reconnecting {
					attempt: strategy.attempt_count(),
					next_delay: d,
				};
				self.set_state(new_state).await;
				Some(d)
			}
			None => {
				let new_state = ReconnectionState::Failed {
					total_attempts: strategy.attempt_count(),
				};
				self.set_state(new_state).await;
				None
			}
		}
	}

	/// Called when a reconnection attempt succeeds.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_websockets::reconnection::{
	///     AutoReconnectHandler, ReconnectionConfig, ReconnectionState,
	/// };
	///
	/// # tokio_test::block_on(async {
	/// let handler = AutoReconnectHandler::new(ReconnectionConfig::default());
	///
	/// // Simulate disconnect then reconnect
	/// handler.on_disconnect().await;
	/// handler.on_reconnect_success().await;
	///
	/// let state = handler.state().await;
	/// match &*state {
	///     ReconnectionState::Reconnected { total_attempts } => {
	///         assert_eq!(*total_attempts, 1);
	///     }
	///     _ => panic!("Expected Reconnected state"),
	/// }
	/// # });
	/// ```
	pub async fn on_reconnect_success(&self) {
		let mut strategy = self.strategy.write().await;
		let total_attempts = strategy.attempt_count();
		strategy.reset();
		drop(strategy);

		self.set_state(ReconnectionState::Reconnected { total_attempts })
			.await;
	}

	/// Called when the connection is fully established (initial or after reconnection).
	/// Resets the strategy and sets state to Connected.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_websockets::reconnection::{
	///     AutoReconnectHandler, ReconnectionConfig, ReconnectionState,
	/// };
	///
	/// # tokio_test::block_on(async {
	/// let handler = AutoReconnectHandler::new(ReconnectionConfig::default());
	/// handler.on_connected().await;
	///
	/// let state = handler.state().await;
	/// assert_eq!(*state, ReconnectionState::Connected);
	/// # });
	/// ```
	pub async fn on_connected(&self) {
		self.strategy.write().await.reset();
		self.set_state(ReconnectionState::Connected).await;
	}

	/// Updates the state and fires the callback if one is set.
	async fn set_state(&self, new_state: ReconnectionState) {
		if let Some(cb) = &self.on_state_change {
			cb(&new_state);
		}
		*self.state.write().await = new_state;
	}
}

#[cfg(test)]
mod tests {
	use rstest::rstest;

	use super::*;

	#[test]
	fn test_default_config() {
		let config = ReconnectionConfig::default();
		assert_eq!(config.max_attempts, Some(10));
		assert_eq!(config.initial_delay, Duration::from_secs(1));
		assert_eq!(config.max_delay, Duration::from_secs(300));
		assert_eq!(config.backoff_multiplier, 2.0);
		assert_eq!(config.jitter_factor, 0.1);
	}

	#[test]
	fn test_config_builder() {
		let config = ReconnectionConfig::default()
			.with_max_attempts(5)
			.with_initial_delay(Duration::from_secs(2))
			.with_max_delay(Duration::from_secs(60))
			.with_backoff_multiplier(1.5)
			.with_jitter_factor(0.2);

		assert_eq!(config.max_attempts, Some(5));
		assert_eq!(config.initial_delay, Duration::from_secs(2));
		assert_eq!(config.max_delay, Duration::from_secs(60));
		assert_eq!(config.backoff_multiplier, 1.5);
		assert_eq!(config.jitter_factor, 0.2);
	}

	#[test]
	fn test_unlimited_attempts() {
		let config = ReconnectionConfig::default().with_unlimited_attempts();
		assert_eq!(config.max_attempts, None);
	}

	#[test]
	fn test_reconnection_strategy() {
		let config = ReconnectionConfig::default()
			.with_max_attempts(3)
			.with_initial_delay(Duration::from_secs(1))
			.with_jitter_factor(0.0); // No jitter

		let mut strategy = ReconnectionStrategy::new(config);

		assert_eq!(strategy.attempt_count(), 0);
		assert!(strategy.can_reconnect());

		// First: returns initial_delay (1 second)
		// Updates current_delay to 1 * 2.0 = 2 seconds for next time
		let delay1 = strategy.next_delay().unwrap();
		assert_eq!(delay1, Duration::from_secs(1));
		assert_eq!(strategy.attempt_count(), 1);

		// Second: returns current_delay (2 seconds)
		// Updates current_delay to 2 * 2.0 = 4 seconds for next time
		let delay2 = strategy.next_delay().unwrap();
		assert_eq!(delay2, Duration::from_secs(2));
		assert_eq!(strategy.attempt_count(), 2);

		// Third: returns current_delay (4 seconds)
		let delay3 = strategy.next_delay().unwrap();
		assert_eq!(delay3, Duration::from_secs(4));
		assert_eq!(strategy.attempt_count(), 3);

		// Fourth (exceeds max attempts)
		let delay4 = strategy.next_delay();
		assert!(delay4.is_none());
		assert!(!strategy.can_reconnect());
	}

	#[test]
	fn test_exponential_backoff() {
		let config = ReconnectionConfig::default()
			.with_unlimited_attempts()
			.with_initial_delay(Duration::from_secs(1))
			.with_backoff_multiplier(2.0)
			.with_max_delay(Duration::from_secs(100))
			.with_jitter_factor(0.0);

		let mut strategy = ReconnectionStrategy::new(config);

		let delay1 = strategy.next_delay().unwrap();
		assert_eq!(delay1, Duration::from_secs(1));

		let delay2 = strategy.next_delay().unwrap();
		// After 1 second, doubled
		assert!(delay2.as_secs() >= 1);

		let delay3 = strategy.next_delay().unwrap();
		// Doubled again
		assert!(delay3.as_secs() >= 2);
	}

	#[test]
	fn test_max_delay_cap() {
		let config = ReconnectionConfig::default()
			.with_unlimited_attempts()
			.with_initial_delay(Duration::from_secs(1))
			.with_backoff_multiplier(10.0)
			.with_max_delay(Duration::from_secs(5))
			.with_jitter_factor(0.0);

		let mut strategy = ReconnectionStrategy::new(config);

		// Verify that the delay is capped at the maximum delay time after several executions
		for _ in 0..10 {
			if let Some(delay) = strategy.next_delay() {
				assert!(delay.as_secs() <= 5);
			}
		}
	}

	#[test]
	fn test_reset() {
		let config = ReconnectionConfig::default().with_max_attempts(5);
		let mut strategy = ReconnectionStrategy::new(config);

		strategy.next_delay();
		strategy.next_delay();
		assert_eq!(strategy.attempt_count(), 2);

		strategy.reset();
		assert_eq!(strategy.attempt_count(), 0);
		assert!(strategy.can_reconnect());
	}

	#[test]
	fn test_jitter_applied() {
		let config = ReconnectionConfig::default()
			.with_initial_delay(Duration::from_secs(1))
			.with_jitter_factor(0.1);

		let mut strategy = ReconnectionStrategy::new(config);

		let delay = strategy.next_delay().unwrap();
		// Jitter is applied, so it's not exactly 1 second
		// However, it's within 1 second ±10%
		let delay_secs = delay.as_secs_f64();
		assert!((0.9..=1.1).contains(&delay_secs));
	}

	#[rstest]
	fn test_backoff_does_not_overflow_at_high_retry_counts() {
		// Arrange - use a large multiplier to trigger potential overflow quickly
		let config = ReconnectionConfig::default()
			.with_unlimited_attempts()
			.with_initial_delay(Duration::from_secs(1))
			.with_backoff_multiplier(10.0)
			.with_max_delay(Duration::from_secs(300))
			.with_jitter_factor(0.0);

		let mut strategy = ReconnectionStrategy::new(config);

		// Act - run many iterations that would overflow with naive integer math
		for _ in 0..100 {
			if let Some(delay) = strategy.next_delay() {
				// Assert - delay must always be within bounds, never panic or wrap
				assert!(delay <= Duration::from_secs(300));
			}
		}
	}

	#[rstest]
	fn test_strategy_config_accessor() {
		// Arrange
		let config = ReconnectionConfig::default().with_max_attempts(7);

		// Act
		let strategy = ReconnectionStrategy::new(config);

		// Assert
		assert_eq!(strategy.config().max_attempts, Some(7));
	}

	#[rstest]
	fn test_reconnection_state_variants() {
		// Arrange & Act & Assert
		let connected = ReconnectionState::Connected;
		assert_eq!(connected, ReconnectionState::Connected);

		let reconnecting = ReconnectionState::Reconnecting {
			attempt: 1,
			next_delay: Duration::from_secs(2),
		};
		assert_eq!(
			reconnecting,
			ReconnectionState::Reconnecting {
				attempt: 1,
				next_delay: Duration::from_secs(2),
			}
		);

		let reconnected = ReconnectionState::Reconnected { total_attempts: 3 };
		assert_eq!(
			reconnected,
			ReconnectionState::Reconnected { total_attempts: 3 }
		);

		let failed = ReconnectionState::Failed { total_attempts: 5 };
		assert_eq!(failed, ReconnectionState::Failed { total_attempts: 5 });

		let disabled = ReconnectionState::Disabled;
		assert_eq!(disabled, ReconnectionState::Disabled);
	}

	#[rstest]
	#[tokio::test]
	async fn test_auto_reconnect_handler_new() {
		// Arrange & Act
		let handler = AutoReconnectHandler::new(ReconnectionConfig::default());

		// Assert
		assert!(handler.is_enabled());
		assert_eq!(*handler.state().await, ReconnectionState::Connected);
	}

	#[rstest]
	#[tokio::test]
	async fn test_auto_reconnect_handler_disabled() {
		// Arrange & Act
		let handler = AutoReconnectHandler::disabled();

		// Assert
		assert!(!handler.is_enabled());
		assert_eq!(*handler.state().await, ReconnectionState::Disabled);
		assert!(handler.on_disconnect().await.is_none());
	}

	#[tokio::test]
	async fn test_auto_reconnect_handler_disconnect_and_reconnect() {
		// Arrange
		let config = ReconnectionConfig::default()
			.with_max_attempts(3)
			.with_initial_delay(Duration::from_secs(1))
			.with_jitter_factor(0.0);
		let handler = AutoReconnectHandler::new(config);

		// Act - first disconnect
		let delay = handler.on_disconnect().await;

		// Assert - should get initial delay
		assert_eq!(delay, Some(Duration::from_secs(1)));
		match &*handler.state().await {
			ReconnectionState::Reconnecting { attempt, .. } => {
				assert_eq!(*attempt, 1);
			}
			other => panic!("Expected Reconnecting, got {:?}", other),
		}

		// Act - reconnect succeeds
		handler.on_reconnect_success().await;

		// Assert
		match &*handler.state().await {
			ReconnectionState::Reconnected { total_attempts } => {
				assert_eq!(*total_attempts, 1);
			}
			other => panic!("Expected Reconnected, got {:?}", other),
		}
	}

	#[tokio::test]
	async fn test_auto_reconnect_handler_exhausted() {
		// Arrange
		let config = ReconnectionConfig::default()
			.with_max_attempts(2)
			.with_initial_delay(Duration::from_secs(1))
			.with_jitter_factor(0.0);
		let handler = AutoReconnectHandler::new(config);

		// Act - exhaust all attempts
		let delay1 = handler.on_disconnect().await;
		assert!(delay1.is_some());

		let delay2 = handler.on_disconnect().await;
		assert!(delay2.is_some());

		let delay3 = handler.on_disconnect().await;

		// Assert - should be exhausted
		assert!(delay3.is_none());
		match &*handler.state().await {
			ReconnectionState::Failed { total_attempts } => {
				assert_eq!(*total_attempts, 2);
			}
			other => panic!("Expected Failed, got {:?}", other),
		}
	}

	#[tokio::test]
	async fn test_auto_reconnect_handler_on_connected_resets() {
		// Arrange
		let config = ReconnectionConfig::default()
			.with_max_attempts(5)
			.with_jitter_factor(0.0);
		let handler = AutoReconnectHandler::new(config);

		// Act - disconnect and reconnect
		handler.on_disconnect().await;
		handler.on_reconnect_success().await;
		handler.on_connected().await;

		// Assert - state should be Connected and strategy reset
		assert_eq!(*handler.state().await, ReconnectionState::Connected);
	}

	#[tokio::test]
	async fn test_auto_reconnect_handler_with_callback() {
		// Arrange
		let callback_fired = Arc::new(std::sync::atomic::AtomicBool::new(false));
		let callback_fired_clone = callback_fired.clone();

		let config = ReconnectionConfig::default()
			.with_max_attempts(3)
			.with_jitter_factor(0.0);
		let handler =
			AutoReconnectHandler::new(config).with_on_state_change(Box::new(move |_state| {
				callback_fired_clone.store(true, std::sync::atomic::Ordering::SeqCst);
			}));

		// Act
		handler.on_disconnect().await;

		// Assert
		assert!(callback_fired.load(std::sync::atomic::Ordering::SeqCst));
	}

	#[tokio::test]
	async fn test_auto_reconnect_handler_exponential_backoff() {
		// Arrange
		let config = ReconnectionConfig::default()
			.with_max_attempts(4)
			.with_initial_delay(Duration::from_secs(1))
			.with_backoff_multiplier(2.0)
			.with_jitter_factor(0.0);
		let handler = AutoReconnectHandler::new(config);

		// Act & Assert - verify exponential backoff
		let delay1 = handler.on_disconnect().await.unwrap();
		assert_eq!(delay1, Duration::from_secs(1));

		let delay2 = handler.on_disconnect().await.unwrap();
		assert_eq!(delay2, Duration::from_secs(2));

		let delay3 = handler.on_disconnect().await.unwrap();
		assert_eq!(delay3, Duration::from_secs(4));

		let delay4 = handler.on_disconnect().await.unwrap();
		assert_eq!(delay4, Duration::from_secs(8));
	}
}
