//! Lazy loading and eager loading strategies for association proxies

use async_trait::async_trait;
use std::sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard};
use tracing::warn;

use crate::proxy::ProxyResult;

/// Loading strategy for relationships
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoadStrategy {
	/// Load immediately when accessed
	Eager,
	/// Load only when needed
	Lazy,
	/// Load using a separate SELECT query
	Select,
	/// Load using a JOIN
	Joined,
}

/// Trait for lazy-loadable relationships
#[async_trait]
pub trait LazyLoadable: Send + Sync {
	/// The type of the loaded data
	type Data: Clone + Send + Sync;

	/// Check if data is loaded
	fn is_loaded(&self) -> bool;

	/// Load the data if not already loaded
	async fn load(&mut self) -> ProxyResult<()>;

	/// Get the loaded data, loading if necessary
	async fn get(&mut self) -> ProxyResult<Self::Data>;

	/// Get the loaded data without loading (returns None if not loaded)
	fn get_if_loaded(&self) -> Option<Self::Data>;
}

/// Lazy-loaded wrapper for relationship data
pub struct LazyLoaded<T, F>
where
	T: Clone + Send + Sync,
	F: Fn() -> futures::future::BoxFuture<'static, ProxyResult<T>> + Send + Sync,
{
	/// The cached data
	data: Arc<RwLock<Option<T>>>,
	/// The loader function
	loader: Arc<F>,
}

impl<T, F> LazyLoaded<T, F>
where
	T: Clone + Send + Sync,
	F: Fn() -> futures::future::BoxFuture<'static, ProxyResult<T>> + Send + Sync,
{
	/// Create a new lazy-loaded value
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_urls::proxy::loading::LazyLoaded;
	///
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// let lazy = LazyLoaded::new(|| {
	///     Box::pin(async { Ok(vec![1, 2, 3]) })
	/// });
	///
	/// // Data is not loaded yet
	/// assert!(!lazy.is_loaded());
	/// # Ok(())
	/// # }
	/// ```
	pub fn new(loader: F) -> Self {
		Self {
			data: Arc::new(RwLock::new(None)),
			loader: Arc::new(loader),
		}
	}

	/// Create a pre-loaded lazy value
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_urls::proxy::loading::LazyLoaded;
	///
	/// let lazy = LazyLoaded::preloaded(vec![1, 2, 3], || {
	///     Box::pin(async { Ok(vec![4, 5, 6]) })
	/// });
	///
	/// // Data is already loaded
	/// assert!(lazy.is_loaded());
	/// ```
	pub fn preloaded(data: T, loader: F) -> Self {
		Self {
			data: Arc::new(RwLock::new(Some(data))),
			loader: Arc::new(loader),
		}
	}

	/// Acquire a read lock, recovering from poison if necessary
	fn read_lock(&self) -> RwLockReadGuard<'_, Option<T>> {
		self.data.read().unwrap_or_else(|e| {
			warn!("RwLock poisoned on read, recovering with inner value");
			e.into_inner()
		})
	}

	/// Acquire a write lock, recovering from poison if necessary
	fn write_lock(&self) -> RwLockWriteGuard<'_, Option<T>> {
		self.data.write().unwrap_or_else(|e| {
			warn!("RwLock poisoned on write, recovering with inner value");
			e.into_inner()
		})
	}

	/// Check if data is loaded
	pub fn is_loaded(&self) -> bool {
		self.read_lock().is_some()
	}

	/// Load the data if not already loaded
	pub async fn load(&self) -> ProxyResult<()> {
		// Check if already loaded
		if self.is_loaded() {
			return Ok(());
		}

		// Load the data
		let data = (self.loader)().await?;

		// Store it
		let mut guard = self.write_lock();
		*guard = Some(data);

		Ok(())
	}

	/// Get the loaded data, loading if necessary
	pub async fn get(&self) -> ProxyResult<T> {
		// Ensure data is loaded
		self.load().await?;

		// Handle the case where data was reset between load() and this read
		// by another task calling reset() concurrently.
		// Extract the cloned data while holding the lock briefly, then drop it
		// before any await point to satisfy clippy::await_holding_lock.
		let cached = {
			let guard = self.read_lock();
			guard.as_ref().cloned()
		};

		match cached {
			Some(data) => Ok(data),
			None => {
				// Data was reset after load() completed; reload
				let data = (self.loader)().await?;
				let mut write_guard = self.write_lock();
				*write_guard = Some(data.clone());
				Ok(data)
			}
		}
	}

	/// Get the loaded data without loading (returns None if not loaded)
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_urls::proxy::loading::LazyLoaded;
	///
	/// let lazy = LazyLoaded::new(|| {
	///     Box::pin(async { Ok(vec![1, 2, 3]) })
	/// });
	///
	/// // Returns None since not loaded yet
	/// assert!(lazy.get_if_loaded().is_none());
	/// ```
	pub fn get_if_loaded(&self) -> Option<T> {
		self.read_lock().as_ref().cloned()
	}

	/// Reset the lazy-loaded value, forcing a reload on next access
	pub fn reset(&self) {
		let mut guard = self.write_lock();
		*guard = None;
	}
}

/// Eager loading configuration
#[derive(Debug, Clone)]
pub struct EagerLoadConfig {
	/// Maximum depth to eager load
	pub max_depth: usize,
	/// Relationships to eager load
	pub relationships: Vec<String>,
}

impl EagerLoadConfig {
	/// Create a new eager load configuration
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_urls::proxy::loading::EagerLoadConfig;
	///
	/// let config = EagerLoadConfig::new()
	///     .with_relationship("posts")
	///     .with_relationship("comments")
	///     .max_depth(3);
	///
	/// assert_eq!(config.max_depth, 3);
	/// assert_eq!(config.relationships.len(), 2);
	/// ```
	pub fn new() -> Self {
		Self {
			max_depth: 2,
			relationships: Vec::new(),
		}
	}

	/// Add a relationship to eager load
	pub fn with_relationship(mut self, relationship: &str) -> Self {
		self.relationships.push(relationship.to_string());
		self
	}

	/// Set maximum depth
	pub fn max_depth(mut self, depth: usize) -> Self {
		self.max_depth = depth;
		self
	}
}

impl Default for EagerLoadConfig {
	fn default() -> Self {
		Self::new()
	}
}

/// Trait for models that support eager loading
#[async_trait]
pub trait EagerLoadable: Send + Sync {
	/// Eager load specified relationships
	async fn eager_load(&mut self, config: &EagerLoadConfig) -> ProxyResult<()>;

	/// Check if a relationship is loaded
	fn is_relationship_loaded(&self, name: &str) -> bool;
}

/// Cache for loaded relationship data
pub struct RelationshipCache {
	cache: Arc<RwLock<std::collections::HashMap<String, Box<dyn std::any::Any + Send + Sync>>>>,
}

impl RelationshipCache {
	/// Create a new relationship cache
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_urls::proxy::loading::RelationshipCache;
	///
	/// let cache = RelationshipCache::new();
	/// assert!(!cache.contains("posts"));
	/// ```
	pub fn new() -> Self {
		Self {
			cache: Arc::new(RwLock::new(std::collections::HashMap::new())),
		}
	}

	/// Acquire a read lock, recovering from poison if necessary
	fn read_lock(
		&self,
	) -> RwLockReadGuard<'_, std::collections::HashMap<String, Box<dyn std::any::Any + Send + Sync>>>
	{
		self.cache.read().unwrap_or_else(|e| {
			warn!("RelationshipCache RwLock poisoned on read, recovering with inner value");
			e.into_inner()
		})
	}

	/// Acquire a write lock, recovering from poison if necessary
	fn write_lock(
		&self,
	) -> RwLockWriteGuard<'_, std::collections::HashMap<String, Box<dyn std::any::Any + Send + Sync>>>
	{
		self.cache.write().unwrap_or_else(|e| {
			warn!("RelationshipCache RwLock poisoned on write, recovering with inner value");
			e.into_inner()
		})
	}

	/// Check if a relationship is cached
	pub fn contains(&self, key: &str) -> bool {
		self.read_lock().contains_key(key)
	}

	/// Get a cached relationship
	pub fn get<T>(&self, key: &str) -> Option<T>
	where
		T: 'static + Clone,
	{
		let cache = self.read_lock();
		cache.get(key).and_then(|v| v.downcast_ref::<T>().cloned())
	}

	/// Set a cached relationship
	pub fn set<T: 'static + Send + Sync>(&self, key: String, value: T) {
		let mut cache = self.write_lock();
		cache.insert(key, Box::new(value));
	}

	/// Remove a cached relationship
	pub fn remove(&self, key: &str) -> bool {
		let mut cache = self.write_lock();
		cache.remove(key).is_some()
	}

	/// Clear all cached relationships
	pub fn clear(&self) {
		let mut cache = self.write_lock();
		cache.clear();
	}
}

impl Default for RelationshipCache {
	fn default() -> Self {
		Self::new()
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[tokio::test]
	async fn test_lazy_loaded() {
		let lazy = LazyLoaded::new(|| Box::pin(async { Ok(vec![1, 2, 3]) }));

		assert!(!lazy.is_loaded());

		lazy.load().await.unwrap();
		assert!(lazy.is_loaded());

		let data = lazy.get_if_loaded().unwrap();
		assert_eq!(data, vec![1, 2, 3]);
	}

	#[tokio::test]
	async fn test_lazy_loaded_preloaded() {
		let lazy = LazyLoaded::preloaded(vec![1, 2, 3], || Box::pin(async { Ok(vec![4, 5, 6]) }));

		assert!(lazy.is_loaded());

		let data = lazy.get_if_loaded().unwrap();
		assert_eq!(data, vec![1, 2, 3]);
	}

	#[test]
	fn test_eager_load_config() {
		let config = EagerLoadConfig::new()
			.with_relationship("posts")
			.with_relationship("comments")
			.max_depth(5);

		assert_eq!(config.max_depth, 5);
		assert_eq!(config.relationships, vec!["posts", "comments"]);
	}

	#[tokio::test]
	async fn test_lazy_loaded_get_returns_owned_clone() {
		// Arrange
		let lazy = LazyLoaded::new(|| Box::pin(async { Ok(vec![10, 20, 30]) }));

		// Act - get() loads and returns owned T via clone
		let data = lazy.get().await.unwrap();

		// Assert
		assert_eq!(data, vec![10, 20, 30]);
		assert!(lazy.is_loaded());
	}

	#[tokio::test]
	async fn test_lazy_loaded_get_if_loaded_returns_none_when_not_loaded() {
		// Arrange
		let lazy = LazyLoaded::new(|| Box::pin(async { Ok(42) }));

		// Act
		let result = lazy.get_if_loaded();

		// Assert
		assert_eq!(result, None);
	}

	#[tokio::test]
	async fn test_lazy_loaded_get_if_loaded_returns_cloned_value() {
		// Arrange
		let lazy = LazyLoaded::preloaded(String::from("hello"), || {
			Box::pin(async { Ok(String::from("world")) })
		});

		// Act
		let result = lazy.get_if_loaded();

		// Assert
		assert_eq!(result, Some(String::from("hello")));
	}

	#[tokio::test]
	async fn test_lazy_loaded_reset_forces_reload() {
		// Arrange
		let lazy = LazyLoaded::preloaded(vec![1], || Box::pin(async { Ok(vec![2]) }));
		assert!(lazy.is_loaded());

		// Act
		lazy.reset();

		// Assert
		assert!(!lazy.is_loaded());
		assert_eq!(lazy.get_if_loaded(), None);

		// Reload
		let data = lazy.get().await.unwrap();
		assert_eq!(data, vec![2]);
	}

	#[test]
	fn test_relationship_cache() {
		let cache = RelationshipCache::new();

		assert!(!cache.contains("key1"));

		cache.set("key1".to_string(), vec![1, 2, 3]);
		assert!(cache.contains("key1"));

		let value: Vec<i32> = cache.get("key1").unwrap();
		assert_eq!(value, vec![1, 2, 3]);

		assert!(cache.remove("key1"));
		assert!(!cache.contains("key1"));
	}
}
