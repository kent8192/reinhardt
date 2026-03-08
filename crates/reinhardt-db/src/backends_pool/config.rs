//! Pool configuration

#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
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
	/// The connection timeout.
	pub connection_timeout: std::time::Duration,
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
			connection_timeout: std::time::Duration::from_secs(30),
			max_connections: 10,
			min_connections: 0,
			acquire_timeout: std::time::Duration::from_secs(30),
			test_before_acquire: false,
		}
	}
}

impl PoolConfig {
	/// Performs the validate operation.
	pub fn validate(&self) -> Result<(), String> {
		if self.max_connections < self.min_connections {
			return Err("max_connections must be >= min_connections".to_string());
		}
		Ok(())
	}
}

#[non_exhaustive]
#[derive(Debug, Clone, Default, PartialEq, Eq)]
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
