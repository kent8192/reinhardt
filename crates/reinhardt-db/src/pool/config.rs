//! Pool configuration

#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct PoolConfig {
	pub max_size: u32,
	pub min_idle: Option<u32>,
	pub max_lifetime: Option<std::time::Duration>,
	pub idle_timeout: Option<std::time::Duration>,
	pub connect_timeout: std::time::Duration,
	pub max_connections: u32,
	pub min_connections: u32,
	pub acquire_timeout: std::time::Duration,
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

	pub fn with_max_connections(mut self, max: u32) -> Self {
		self.max_connections = max;
		self
	}

	pub fn with_min_connections(mut self, min: u32) -> Self {
		self.min_connections = min;
		self
	}

	pub fn with_connection_timeout(mut self, timeout: std::time::Duration) -> Self {
		self.connect_timeout = timeout;
		self
	}

	pub fn with_connect_timeout(mut self, timeout: std::time::Duration) -> Self {
		self.connect_timeout = timeout;
		self
	}

	pub fn with_acquire_timeout(mut self, timeout: std::time::Duration) -> Self {
		self.acquire_timeout = timeout;
		self
	}

	pub fn with_max_lifetime(mut self, lifetime: Option<std::time::Duration>) -> Self {
		self.max_lifetime = lifetime;
		self
	}

	pub fn with_idle_timeout(mut self, timeout: Option<std::time::Duration>) -> Self {
		self.idle_timeout = timeout;
		self
	}

	pub fn with_test_before_acquire(mut self, test: bool) -> Self {
		self.test_before_acquire = test;
		self
	}

	pub fn validate(&self) -> Result<(), String> {
		if self.max_connections < self.min_connections {
			return Err("max_connections must be >= min_connections".to_string());
		}
		Ok(())
	}
}

#[non_exhaustive]
#[derive(Debug, Clone, Default)]
pub struct PoolOptions {
	pub config: PoolConfig,
}

impl PoolOptions {
	pub fn new() -> Self {
		Self::default()
	}

	pub fn max_size(mut self, max_size: u32) -> Self {
		self.config.max_size = max_size;
		self
	}

	pub fn min_idle(mut self, min_idle: u32) -> Self {
		self.config.min_idle = Some(min_idle);
		self
	}
}
