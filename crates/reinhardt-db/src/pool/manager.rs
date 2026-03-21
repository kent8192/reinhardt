//! Pool manager

use super::config::PoolConfig;
use super::errors::{PoolError, PoolResult};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Pool manager for managing multiple connection pools
pub struct PoolManager {
	pools: Arc<RwLock<HashMap<String, Arc<dyn std::any::Any + Send + Sync>>>>,
	config: PoolConfig,
}

impl PoolManager {
	/// Creates a new instance.
	pub fn new(config: PoolConfig) -> Self {
		Self {
			pools: Arc::new(RwLock::new(HashMap::new())),
			config,
		}
	}

	/// Adds pool.
	pub async fn add_pool<T: 'static + Send + Sync>(
		&self,
		name: impl Into<String>,
		pool: T,
	) -> PoolResult<()> {
		let mut pools = self.pools.write().await;
		pools.insert(name.into(), Arc::new(pool));
		Ok(())
	}

	/// Returns the pool.
	pub async fn get_pool<T: 'static + Send + Sync>(&self, name: &str) -> PoolResult<Arc<T>> {
		let pools = self.pools.read().await;
		pools
			.get(name)
			.and_then(|pool| pool.clone().downcast::<T>().ok())
			.ok_or_else(|| PoolError::PoolNotFound(name.to_string()))
	}

	/// Removes pool.
	pub async fn remove_pool(&self, name: &str) -> PoolResult<()> {
		let mut pools = self.pools.write().await;
		pools
			.remove(name)
			.ok_or_else(|| PoolError::PoolNotFound(name.to_string()))?;
		Ok(())
	}

	/// Performs the config operation.
	pub fn config(&self) -> &PoolConfig {
		&self.config
	}
}

impl Default for PoolManager {
	fn default() -> Self {
		Self::new(PoolConfig::default())
	}
}
