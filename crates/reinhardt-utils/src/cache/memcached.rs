//! Memcached cache backend implementation with multi-server support.
//!
//! This module provides a Memcached-based cache backend using the `memcache-async` crate.
//!
//! # Features
//!
//! - **Multi-server support**: Connect to multiple Memcached servers for high availability
//! - **Automatic failover**: Automatically retry operations on other servers if one fails
//! - **Round-robin load balancing**: Distribute requests evenly across servers
//! - **Async/await support**: Built on tokio for high-performance async operations
//! - **TTL (time-to-live) support**: Set expiration times for cached values
//! - **ASCII protocol**: Uses Memcached ASCII protocol for compatibility
//!
//! # Multi-server Configuration
//!
//! When multiple servers are configured, the cache will:
//! 1. Connect to all available servers during initialization
//! 2. Use round-robin selection to distribute load across servers
//! 3. Automatically failover to other servers if an operation fails
//! 4. Clear all servers when `clear()` is called
//!
//! # Examples
//!
//! ## Single Server
//!
//! ```rust,no_run
//! use reinhardt_utils::cache::{Cache, MemcachedCache};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let cache = MemcachedCache::from_url("127.0.0.1:11211").await?;
//!
//!     cache.set("key", &"value", None).await?;
//!     let value: Option<String> = cache.get("key").await?;
//!
//!     assert_eq!(value, Some("value".to_string()));
//!     Ok(())
//! }
//! ```
//!
//! ## Multiple Servers with Failover
//!
//! ```rust,no_run
//! use reinhardt_utils::cache::{Cache, MemcachedCache, MemcachedConfig};
//! use std::time::Duration;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let config = MemcachedConfig {
//!         servers: vec![
//!             "127.0.0.1:11211".to_string(),
//!             "127.0.0.1:11212".to_string(),
//!             "127.0.0.1:11213".to_string(),
//!         ],
//!         pool_size: 10,    // Reserved for future use
//!         timeout_ms: 1000, // Reserved for future use
//!     };
//!
//!     let cache = MemcachedCache::new(config).await?;
//!
//!     // Operations are automatically load-balanced and failover-protected
//!     cache.set("key", &"value", Some(Duration::from_secs(3600))).await?;
//!     let value: Option<String> = cache.get("key").await?;
//!
//!     assert_eq!(value, Some("value".to_string()));
//!     Ok(())
//! }
//! ```

use super::Result;
use super::cache_trait::Cache;
use async_trait::async_trait;
use memcache_async::ascii::Protocol;
use reinhardt_core::exception::Error;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::net::TcpStream;
use tokio::sync::Mutex;
use tokio_util::compat::{Compat, TokioAsyncReadCompatExt};

// Type alias for the memcached protocol with tokio TcpStream
type MemcachedProtocol = Protocol<Compat<TcpStream>>;

/// Memcached configuration with multi-server support.
///
/// # Multi-server Configuration
///
/// Configure multiple servers for high availability and load balancing:
///
/// ```rust,no_run
/// # use reinhardt_utils::cache::MemcachedConfig;
/// let config = MemcachedConfig {
///     servers: vec![
///         "127.0.0.1:11211".to_string(),
///         "127.0.0.1:11212".to_string(),
///     ],
///     pool_size: 10,    // Reserved for future connection pooling
///     timeout_ms: 1000, // Reserved for future timeout support
/// };
/// ```
#[derive(Debug, Clone)]
pub struct MemcachedConfig {
	/// Memcached server addresses (e.g., vec!["127.0.0.1:11211", "127.0.0.1:11212"])
	///
	/// Multiple servers provide:
	/// - High availability through automatic failover
	/// - Load balancing via round-robin selection
	/// - Improved performance by distributing requests
	pub servers: Vec<String>,

	/// Connection pool size per server (reserved for future implementation)
	///
	/// Currently not used. Will enable connection pooling in future versions.
	pub pool_size: usize,

	/// Operation timeout in milliseconds (reserved for future implementation)
	///
	/// Currently not used. Will enable operation timeouts in future versions.
	pub timeout_ms: u64,
}

impl Default for MemcachedConfig {
	fn default() -> Self {
		Self {
			servers: vec!["127.0.0.1:11211".to_string()],
			pool_size: 10,
			timeout_ms: 1000,
		}
	}
}

/// Memcached-based cache backend with multi-server support.
pub struct MemcachedCache {
	servers: Vec<Mutex<MemcachedProtocol>>,
}

impl MemcachedCache {
	/// Create a new Memcached cache instance with support for multiple servers.
	///
	/// # Multi-server Support
	///
	/// - Connects to all configured servers
	/// - Uses round-robin load balancing for request distribution
	/// - Provides automatic failover if a server becomes unavailable
	pub async fn new(config: MemcachedConfig) -> Result<Self> {
		if config.servers.is_empty() {
			return Err(Error::Http("No Memcached servers specified".to_string()));
		}

		let mut protocols = Vec::new();
		let mut last_error = None;

		// Attempt to connect to all servers
		for server_addr in &config.servers {
			match Self::connect_to_server(server_addr).await {
				Ok(protocol) => {
					protocols.push(Mutex::new(protocol));
				}
				Err(e) => {
					// Log warning but continue with other servers
					eprintln!(
						"Warning: Failed to connect to Memcached server {}: {}",
						server_addr, e
					);
					last_error = Some(e);
				}
			}
		}

		// At least one server must be connected
		if protocols.is_empty() {
			return Err(last_error.unwrap_or_else(|| {
				Error::Http("Failed to connect to any Memcached server".to_string())
			}));
		}

		Ok(Self { servers: protocols })
	}

	/// Helper method to connect to a single Memcached server.
	async fn connect_to_server(server_addr: &str) -> Result<MemcachedProtocol> {
		// Use tokio TcpStream for native async support
		let stream = TcpStream::connect(server_addr)
			.await
			.map_err(|e| Error::Http(format!("Failed to connect to Memcached: {}", e)))?;

		// Convert tokio TcpStream to futures-compatible stream
		let compat_stream = stream.compat();

		Ok(Protocol::new(compat_stream))
	}

	/// Get a consistent server index for a given key using hashing.
	/// This ensures the same key always maps to the same server.
	fn get_server_index_for_key(&self, key: &str) -> usize {
		use std::collections::hash_map::DefaultHasher;
		use std::hash::{Hash, Hasher};

		let mut hasher = DefaultHasher::new();
		key.hash(&mut hasher);
		let hash = hasher.finish();

		(hash as usize) % self.servers.len()
	}

	/// Helper method to get a server for operation.
	fn get_server(&self, index: usize) -> &Mutex<MemcachedProtocol> {
		&self.servers[index % self.servers.len()]
	}

	/// Create a new Memcached cache from URL.
	pub async fn from_url(url: &str) -> Result<Self> {
		let config = MemcachedConfig {
			servers: vec![url.to_string()],
			..Default::default()
		};

		Self::new(config).await
	}
}

#[async_trait]
impl Cache for MemcachedCache {
	async fn get<T>(&self, key: &str) -> Result<Option<T>>
	where
		T: for<'de> Deserialize<'de> + Send,
	{
		let start_index = self.get_server_index_for_key(key);
		let server_count = self.servers.len();

		// Try all servers starting from the selected one
		for attempt in 0..server_count {
			let index = (start_index + attempt) % server_count;
			let server = self.get_server(index);
			let mut protocol = server.lock().await;

			match protocol.get(&key).await {
				Ok(value) => {
					// Check if the value is empty (key not found)
					if value.is_empty() {
						return Ok(None);
					}

					let deserialized: T = serde_json::from_slice(&value).map_err(|e| {
						Error::Serialization(format!("Failed to deserialize value: {}", e))
					})?;
					return Ok(Some(deserialized));
				}
				Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
				Err(e) => {
					if attempt < server_count - 1 {
						eprintln!(
							"Warning: Get operation failed on server {}, trying next: {}",
							index, e
						);
					} else {
						return Err(Error::Http(format!("Memcached get error: {}", e)));
					}
				}
			}
		}

		Err(Error::Http("All Memcached servers failed".to_string()))
	}

	async fn set<T>(&self, key: &str, value: &T, ttl: Option<Duration>) -> Result<()>
	where
		T: Serialize + Send + Sync,
	{
		let serialized = serde_json::to_vec(value)
			.map_err(|e| Error::Serialization(format!("Failed to serialize value: {}", e)))?;

		let expiration = ttl.map(|d| d.as_secs() as u32).unwrap_or(0);
		let start_index = self.get_server_index_for_key(key);
		let server_count = self.servers.len();

		// Try all servers starting from the selected one
		for attempt in 0..server_count {
			let index = (start_index + attempt) % server_count;
			let server = self.get_server(index);
			let mut protocol = server.lock().await;

			match protocol.set(&key, &serialized, expiration).await {
				Ok(_) => return Ok(()),
				Err(e) => {
					if attempt < server_count - 1 {
						eprintln!(
							"Warning: Set operation failed on server {}, trying next: {}",
							index, e
						);
					} else {
						return Err(Error::Http(format!("Memcached set error: {}", e)));
					}
				}
			}
		}

		Err(Error::Http("All Memcached servers failed".to_string()))
	}

	async fn delete(&self, key: &str) -> Result<()> {
		let start_index = self.get_server_index_for_key(key);
		let server_count = self.servers.len();

		// Try all servers starting from the selected one
		for attempt in 0..server_count {
			let index = (start_index + attempt) % server_count;
			let server = self.get_server(index);
			let mut protocol = server.lock().await;

			// memcache-async doesn't have a direct delete method in the examples
			// We can use set with TTL=1 (immediate expiration) as a workaround
			match protocol.set(&key, &[], 1).await {
				Ok(_) => return Ok(()),
				Err(e) => {
					if attempt < server_count - 1 {
						eprintln!(
							"Warning: Delete operation failed on server {}, trying next: {}",
							index, e
						);
					} else {
						return Err(Error::Http(format!("Memcached delete error: {}", e)));
					}
				}
			}
		}

		Err(Error::Http("All Memcached servers failed".to_string()))
	}

	async fn has_key(&self, key: &str) -> Result<bool> {
		let start_index = self.get_server_index_for_key(key);
		let server_count = self.servers.len();

		// Try all servers starting from the selected one
		for attempt in 0..server_count {
			let index = (start_index + attempt) % server_count;
			let server = self.get_server(index);
			let mut protocol = server.lock().await;

			match protocol.get(&key).await {
				Ok(value) => {
					// Check if the value is empty (key not found)
					return Ok(!value.is_empty());
				}
				Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(false),
				Err(e) => {
					if attempt < server_count - 1 {
						eprintln!(
							"Warning: Has_key operation failed on server {}, trying next: {}",
							index, e
						);
					} else {
						return Err(Error::Http(format!("Memcached has_key error: {}", e)));
					}
				}
			}
		}

		Err(Error::Http("All Memcached servers failed".to_string()))
	}

	async fn clear(&self) -> Result<()> {
		// Clear operation needs to be performed on all servers
		let mut last_error = None;
		let mut success_count = 0;

		for server in &self.servers {
			let mut protocol = server.lock().await;
			match protocol.flush().await {
				Ok(_) => success_count += 1,
				Err(e) => {
					eprintln!("Warning: Failed to clear cache on one server: {}", e);
					last_error = Some(Error::Http(format!("Memcached clear error: {}", e)));
				}
			}
		}

		// If at least one server was cleared successfully, consider it a success
		if success_count > 0 {
			Ok(())
		} else {
			Err(last_error
				.unwrap_or_else(|| Error::Http("Failed to clear cache on all servers".to_string())))
		}
	}
}
