//! Geo-based rate limiting implementation
//!
//! Allows different rate limits based on the geographical location of the client,
//! determined by their IP address.

use super::backend::ThrottleBackend;
use super::{Throttle, ThrottleError, ThrottleResult};
use async_trait::async_trait;
use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::Arc;

#[cfg(feature = "geo-limiting")]
use maxminddb::geoip2;

/// Configuration for geo-based rate limiting
#[derive(Debug, Clone)]
pub struct GeoRateConfig {
	/// Rate for specific country codes (ISO 3166-1 alpha-2)
	pub country_rates: HashMap<String, (usize, u64)>,
	/// Default rate for countries not specified
	pub default_rate: (usize, u64),
}

impl GeoRateConfig {
	/// Creates a new geo-based rate configuration
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_throttling::geo::GeoRateConfig;
	/// use std::collections::HashMap;
	///
	/// let mut country_rates = HashMap::new();
	/// country_rates.insert("US".to_string(), (100, 60)); // 100 requests per minute for US
	/// country_rates.insert("JP".to_string(), (200, 60)); // 200 requests per minute for JP
	///
	/// let config = GeoRateConfig::new(country_rates, (50, 60)); // Default: 50 requests per minute
	/// ```
	pub fn new(country_rates: HashMap<String, (usize, u64)>, default_rate: (usize, u64)) -> Self {
		Self {
			country_rates,
			default_rate,
		}
	}

	/// Add a country-specific rate limit
	pub fn add_country_rate(&mut self, country_code: &str, rate: usize, period: u64) {
		self.country_rates
			.insert(country_code.to_string(), (rate, period));
	}

	/// Get rate for a specific country
	pub fn get_rate(&self, country_code: &str) -> (usize, u64) {
		self.country_rates
			.get(country_code)
			.copied()
			.unwrap_or(self.default_rate)
	}
}

/// Geo-based rate limiting throttle
///
/// # Examples
///
/// ```
/// use reinhardt_throttling::geo::{GeoRateThrottle, GeoRateConfig};
/// use reinhardt_throttling::{MemoryBackend, Throttle};
/// use std::collections::HashMap;
/// use std::sync::Arc;
///
/// # tokio_test::block_on(async {
/// let backend = Arc::new(MemoryBackend::new());
/// let mut country_rates = HashMap::new();
/// country_rates.insert("US".to_string(), (100, 60));
/// let config = GeoRateConfig::new(country_rates, (50, 60));
///
/// // Without GeoIP database (will use default rate)
/// let throttle = GeoRateThrottle::new_without_geoip(backend, config);
/// # });
/// ```
pub struct GeoRateThrottle<B: ThrottleBackend> {
	backend: Arc<B>,
	config: GeoRateConfig,
	#[cfg(feature = "geo-limiting")]
	geoip_reader: Option<Arc<maxminddb::Reader<Vec<u8>>>>,
}

impl<B: ThrottleBackend> GeoRateThrottle<B> {
	/// Creates a new geo-based throttle without GeoIP database
	/// This will always use the default rate
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_throttling::geo::{GeoRateThrottle, GeoRateConfig};
	/// use reinhardt_throttling::{MemoryBackend, Throttle};
	/// use std::collections::HashMap;
	/// use std::sync::Arc;
	///
	/// let backend = Arc::new(MemoryBackend::new());
	/// let config = GeoRateConfig::new(HashMap::new(), (50, 60));
	/// let throttle = GeoRateThrottle::new_without_geoip(backend, config);
	/// ```
	pub fn new_without_geoip(backend: Arc<B>, config: GeoRateConfig) -> Self {
		Self {
			backend,
			config,
			#[cfg(feature = "geo-limiting")]
			geoip_reader: None,
		}
	}

	#[cfg(feature = "geo-limiting")]
	/// Creates a new geo-based throttle with GeoIP database
	///
	/// # Examples
	///
	/// ```no_run
	/// use reinhardt_throttling::geo::{GeoRateThrottle, GeoRateConfig};
	/// use reinhardt_throttling::{MemoryBackend, Throttle};
	/// use std::collections::HashMap;
	/// use std::sync::Arc;
	///
	/// # tokio_test::block_on(async {
	/// let backend = Arc::new(MemoryBackend::new());
	/// let config = GeoRateConfig::new(HashMap::new(), (50, 60));
	/// let throttle = GeoRateThrottle::new(
	///     backend,
	///     config,
	///     "/path/to/GeoLite2-Country.mmdb"
	/// ).unwrap();
	/// # });
	/// ```
	pub fn new(
		backend: Arc<B>,
		config: GeoRateConfig,
		geoip_db_path: &str,
	) -> Result<Self, String> {
		let reader = maxminddb::Reader::open_readfile(geoip_db_path)
			.map_err(|e| format!("Failed to open GeoIP database: {}", e))?;

		Ok(Self {
			backend,
			config,
			geoip_reader: Some(Arc::new(reader)),
		})
	}

	#[cfg(feature = "geo-limiting")]
	/// Get country code from IP address
	fn get_country_code(&self, ip: IpAddr) -> Option<String> {
		let reader = self.geoip_reader.as_ref()?;

		let result = reader.lookup(ip).ok()?;
		let country: geoip2::Country = result.decode().ok()??;
		country.country.iso_code.map(|s| s.to_string())
	}

	#[cfg(not(feature = "geo-limiting"))]
	/// Get country code from IP address (stub for non-geo builds)
	fn get_country_code(&self, _ip: IpAddr) -> Option<String> {
		None
	}

	/// Extract IP address from key (expects format "ip:xxx.xxx.xxx.xxx")
	fn extract_ip(&self, key: &str) -> Option<IpAddr> {
		if let Some(ip_str) = key.strip_prefix("ip:") {
			ip_str.parse().ok()
		} else {
			None
		}
	}

	/// Get rate limit for the given key
	async fn get_rate_for_key(&self, key: &str) -> (usize, u64) {
		if let Some(ip) = self.extract_ip(key)
			&& let Some(country_code) = self.get_country_code(ip)
		{
			return self.config.get_rate(&country_code);
		}
		self.config.default_rate
	}
}

#[async_trait]
impl<B: ThrottleBackend> Throttle for GeoRateThrottle<B> {
	async fn allow_request(&self, key: &str) -> ThrottleResult<bool> {
		let (rate, period) = self.get_rate_for_key(key).await;

		let count = self
			.backend
			.increment(key, period)
			.await
			.map_err(ThrottleError::ThrottleError)?;

		Ok(count <= rate)
	}

	async fn wait_time(&self, key: &str) -> ThrottleResult<Option<u64>> {
		let (rate, period) = self.get_rate_for_key(key).await;

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
		self.config.default_rate
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::backend::MemoryBackend;

	#[tokio::test]
	async fn test_geo_rate_throttle_without_geoip() {
		let backend = Arc::new(MemoryBackend::new());
		let config = GeoRateConfig::new(HashMap::new(), (5, 60));
		let throttle = GeoRateThrottle::new_without_geoip(backend, config);

		// Should use default rate
		for _ in 0..5 {
			assert!(throttle.allow_request("test_key").await.unwrap());
		}

		// 6th request should fail
		assert!(!throttle.allow_request("test_key").await.unwrap());
	}

	#[tokio::test]
	async fn test_geo_rate_config_add_country() {
		let mut config = GeoRateConfig::new(HashMap::new(), (50, 60));
		config.add_country_rate("US", 100, 60);
		config.add_country_rate("JP", 200, 60);

		assert_eq!(config.get_rate("US"), (100, 60));
		assert_eq!(config.get_rate("JP"), (200, 60));
		assert_eq!(config.get_rate("UK"), (50, 60)); // Default
	}

	#[tokio::test]
	async fn test_geo_rate_throttle_extract_ip() {
		let backend = Arc::new(MemoryBackend::new());
		let config = GeoRateConfig::new(HashMap::new(), (50, 60));
		let throttle = GeoRateThrottle::new_without_geoip(backend, config);

		let ip = throttle.extract_ip("ip:192.168.1.1");
		assert!(ip.is_some());
		assert_eq!(ip.unwrap().to_string(), "192.168.1.1");

		let no_ip = throttle.extract_ip("user:123");
		assert!(no_ip.is_none());
	}

	#[tokio::test]
	async fn test_geo_rate_config_get_rate() {
		let mut country_rates = HashMap::new();
		country_rates.insert("US".to_string(), (100, 60));
		country_rates.insert("JP".to_string(), (200, 60));

		let config = GeoRateConfig::new(country_rates, (50, 60));

		assert_eq!(config.get_rate("US"), (100, 60));
		assert_eq!(config.get_rate("JP"), (200, 60));
		assert_eq!(config.get_rate("UK"), (50, 60));
	}

	#[tokio::test]
	async fn test_geo_rate_throttle_get_rate() {
		let backend = Arc::new(MemoryBackend::new());
		let config = GeoRateConfig::new(HashMap::new(), (50, 60));
		let throttle = GeoRateThrottle::new_without_geoip(backend, config);

		assert_eq!(throttle.get_rate(), (50, 60));
	}
}
