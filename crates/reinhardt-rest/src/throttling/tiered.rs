//! Tiered rate throttling based on user level

use super::backend::ThrottleBackend;
use super::{Throttle, ThrottleError, ThrottleResult};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Tier definition
#[derive(Debug, Clone)]
pub struct Tier {
	pub name: String,
	pub rate: usize,
	pub duration: std::time::Duration,
}

impl Tier {
	/// Creates a new tier with the given name, rate limit, and duration.
	///
	/// # Arguments
	///
	/// * `name` - Name of the tier (e.g., "free", "premium")
	/// * `rate` - Maximum number of requests allowed
	/// * `duration` - Time window for the rate limit
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::throttling::Tier;
	/// use std::time::Duration;
	///
	/// let free_tier = Tier::new("free", 100, Duration::from_secs(3600));
	/// assert_eq!(free_tier.name, "free");
	/// assert_eq!(free_tier.rate, 100);
	/// assert_eq!(free_tier.duration, Duration::from_secs(3600));
	/// ```
	pub fn new(name: impl Into<String>, rate: usize, duration: std::time::Duration) -> Self {
		Self {
			name: name.into(),
			rate,
			duration,
		}
	}
}

/// Tiered rate throttle
pub struct TieredRateThrottle<B: ThrottleBackend> {
	backend: Arc<Mutex<B>>,
	tiers: HashMap<String, Tier>,
	default_tier: Tier,
}

impl<B: ThrottleBackend> TieredRateThrottle<B> {
	/// Creates a new `TieredRateThrottle` with the specified backend and default tier.
	///
	/// # Arguments
	///
	/// * `backend` - Throttle backend wrapped in `Arc<Mutex>`
	/// * `default_tier` - Default tier used when no specific tier matches
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::throttling::{TieredRateThrottle, MemoryBackend, Tier};
	/// use std::sync::Arc;
	/// use tokio::sync::Mutex;
	/// use std::time::Duration;
	///
	/// let backend = Arc::new(Mutex::new(MemoryBackend::new()));
	/// let default_tier = Tier::new("free", 100, Duration::from_secs(3600));
	/// let throttle = TieredRateThrottle::new(backend, default_tier);
	/// ```
	pub fn new(backend: Arc<Mutex<B>>, default_tier: Tier) -> Self {
		Self {
			backend,
			tiers: HashMap::new(),
			default_tier,
		}
	}
	/// Adds a tier to the throttle configuration.
	///
	/// # Arguments
	///
	/// * `tier` - Tier to add
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::throttling::{TieredRateThrottle, MemoryBackend, Tier};
	/// use std::sync::Arc;
	/// use tokio::sync::Mutex;
	/// use std::time::Duration;
	///
	/// let backend = Arc::new(Mutex::new(MemoryBackend::new()));
	/// let default_tier = Tier::new("free", 100, Duration::from_secs(3600));
	/// let mut throttle = TieredRateThrottle::new(backend, default_tier);
	///
	/// let premium_tier = Tier::new("premium", 1000, Duration::from_secs(3600));
	/// throttle.add_tier(premium_tier);
	/// ```
	pub fn add_tier(&mut self, tier: Tier) {
		self.tiers.insert(tier.name.clone(), tier);
	}
	/// Gets a tier by name, or returns the default tier if not found.
	///
	/// # Arguments
	///
	/// * `tier_name` - Name of the tier to retrieve
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::throttling::{TieredRateThrottle, MemoryBackend, Tier};
	/// use std::sync::Arc;
	/// use tokio::sync::Mutex;
	/// use std::time::Duration;
	///
	/// let backend = Arc::new(Mutex::new(MemoryBackend::new()));
	/// let default_tier = Tier::new("free", 100, Duration::from_secs(3600));
	/// let mut throttle = TieredRateThrottle::new(backend, default_tier);
	///
	/// let premium_tier = Tier::new("premium", 1000, Duration::from_secs(3600));
	/// throttle.add_tier(premium_tier.clone());
	///
	/// let tier = throttle.get_tier("premium");
	/// assert_eq!(tier.name, "premium");
	/// assert_eq!(tier.rate, 1000);
	///
	/// // Unknown tier returns default
	/// let unknown = throttle.get_tier("unknown");
	/// assert_eq!(unknown.name, "free");
	/// ```
	pub fn get_tier(&self, tier_name: &str) -> &Tier {
		self.tiers.get(tier_name).unwrap_or(&self.default_tier)
	}
}

#[async_trait::async_trait]
impl<B: ThrottleBackend> Throttle for TieredRateThrottle<B> {
	async fn allow_request(&self, key: &str) -> ThrottleResult<bool> {
		// Extract tier from key (format: "tier:user_id")
		let parts: Vec<&str> = key.split(':').collect();
		let tier = if parts.len() > 1 {
			self.get_tier(parts[0])
		} else {
			&self.default_tier
		};

		let backend = self.backend.lock().await;
		let count = backend
			.get_count(key)
			.await
			.map_err(ThrottleError::ThrottleError)?;

		if count >= tier.rate {
			return Ok(false);
		}

		backend.increment_duration(key, tier.duration).await?;
		Ok(true)
	}

	async fn wait_time(&self, key: &str) -> ThrottleResult<Option<u64>> {
		let backend = self.backend.lock().await;
		backend
			.get_wait_time(key)
			.await
			.map(|opt| opt.map(|d| d.as_secs()))
	}

	fn get_rate(&self) -> (usize, u64) {
		(self.default_tier.rate, self.default_tier.duration.as_secs())
	}
}
