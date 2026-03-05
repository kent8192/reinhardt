//! Pool configuration

#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PoolConfig {
	pub max_size: u32,
	pub min_idle: Option<u32>,
	pub max_lifetime: Option<std::time::Duration>,
	pub idle_timeout: Option<std::time::Duration>,
	pub connection_timeout: std::time::Duration,
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
			connection_timeout: std::time::Duration::from_secs(30),
			max_connections: 10,
			min_connections: 0,
			acquire_timeout: std::time::Duration::from_secs(30),
			test_before_acquire: false,
		}
	}
}

impl PoolConfig {
	pub fn validate(&self) -> Result<(), String> {
		if self.max_connections < self.min_connections {
			return Err("max_connections must be >= min_connections".to_string());
		}
		Ok(())
	}
}

#[non_exhaustive]
#[derive(Debug, Clone, Default, PartialEq, Eq)]
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
