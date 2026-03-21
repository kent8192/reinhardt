//! Pool configuration

#[non_exhaustive]
#[derive(Debug, Clone)]
/// Represents a pool config.
pub struct PoolConfig {
	/// The max size.
	pub max_size: u32,
	/// The min idle.
	pub min_idle: Option<u32>,
	/// The max lifetime.
	pub max_lifetime: Option<std::time::Duration>,
	/// The idle timeout.
	pub idle_timeout: Option<std::time::Duration>,
	/// The connect timeout.
	pub connect_timeout: std::time::Duration,
	/// The max connections.
	pub max_connections: u32,
	/// The min connections.
	pub min_connections: u32,
	/// The acquire timeout.
	pub acquire_timeout: std::time::Duration,
	/// The test before acquire.
	pub test_before_acquire: bool,
}

impl Default for PoolConfig {
	fn default() -> Self {
		Self {
			max_size: 10,
			min_idle: None,
			max_lifetime: Some(std::time::Duration::from_secs(1800)),
			idle_timeout: Some(std::time::Duration::from_secs(600)),
			connect_timeout: std::time::Duration::from_secs(30),
			max_connections: 10,
			min_connections: 1,
			acquire_timeout: std::time::Duration::from_secs(30),
			test_before_acquire: false,
		}
	}
}

impl PoolConfig {
	/// Create a new pool configuration with default values
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_db::pool::PoolConfig;
	///
	/// let config = PoolConfig::new();
	/// assert_eq!(config.max_connections, 10);
	/// ```
	pub fn new() -> Self {
		Self::default()
	}

	/// Sets the max connections and returns self for chaining.
	pub fn with_max_connections(mut self, max: u32) -> Self {
		self.max_connections = max;
		self
	}

	/// Sets the min connections and returns self for chaining.
	pub fn with_min_connections(mut self, min: u32) -> Self {
		self.min_connections = min;
		self
	}

	/// Sets the connection timeout and returns self for chaining.
	pub fn with_connection_timeout(mut self, timeout: std::time::Duration) -> Self {
		self.connect_timeout = timeout;
		self
	}

	/// Sets the connect timeout and returns self for chaining.
	pub fn with_connect_timeout(mut self, timeout: std::time::Duration) -> Self {
		self.connect_timeout = timeout;
		self
	}

	/// Sets the acquire timeout and returns self for chaining.
	pub fn with_acquire_timeout(mut self, timeout: std::time::Duration) -> Self {
		self.acquire_timeout = timeout;
		self
	}

	/// Sets the max lifetime and returns self for chaining.
	pub fn with_max_lifetime(mut self, lifetime: Option<std::time::Duration>) -> Self {
		self.max_lifetime = lifetime;
		self
	}

	/// Sets the idle timeout and returns self for chaining.
	pub fn with_idle_timeout(mut self, timeout: Option<std::time::Duration>) -> Self {
		self.idle_timeout = timeout;
		self
	}

	/// Sets the test before acquire and returns self for chaining.
	pub fn with_test_before_acquire(mut self, test: bool) -> Self {
		self.test_before_acquire = test;
		self
	}

	/// Performs the validate operation.
	pub fn validate(&self) -> Result<(), String> {
		if self.max_connections < self.min_connections {
			return Err("max_connections must be >= min_connections".to_string());
		}
		Ok(())
	}
}

#[non_exhaustive]
#[derive(Debug, Clone, Default)]
/// Represents a pool options.
pub struct PoolOptions {
	/// The config.
	pub config: PoolConfig,
}

impl PoolOptions {
	/// Creates a new instance.
	pub fn new() -> Self {
		Self::default()
	}

	/// Performs the max size operation.
	pub fn max_size(mut self, max_size: u32) -> Self {
		self.config.max_size = max_size;
		self
	}

	/// Performs the min idle operation.
	pub fn min_idle(mut self, min_idle: u32) -> Self {
		self.config.min_idle = Some(min_idle);
		self
	}
}
