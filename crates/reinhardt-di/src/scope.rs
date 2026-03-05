//! Dependency scopes

use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::sync::{Arc, PoisonError, RwLock};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
	Request,
	Singleton,
}

#[derive(Clone)]
pub struct RequestScope {
	cache: Arc<RwLock<HashMap<TypeId, Arc<dyn Any + Send + Sync>>>>,
}

impl RequestScope {
	/// Creates a new RequestScope with an empty cache.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_di::RequestScope;
	///
	/// let scope = RequestScope::new();
	/// ```
	pub fn new() -> Self {
		Self {
			cache: Arc::new(RwLock::new(HashMap::new())),
		}
	}
	/// Retrieves a value from the request scope cache by type.
	///
	/// Returns `None` if no value of type `T` exists in the cache.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_di::RequestScope;
	///
	/// let scope = RequestScope::new();
	/// scope.set(42i32);
	///
	/// let value = scope.get::<i32>().unwrap();
	/// assert_eq!(*value, 42);
	/// ```
	pub fn get<T: Any + Send + Sync>(&self) -> Option<Arc<T>> {
		let cache = self.cache.read().unwrap_or_else(PoisonError::into_inner);
		let type_id = TypeId::of::<T>();
		cache
			.get(&type_id)
			.and_then(|arc| arc.clone().downcast::<T>().ok())
	}
	/// Stores a value in the request scope cache.
	///
	/// The value is stored by its type and can be retrieved later using `get`.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_di::RequestScope;
	///
	/// let scope = RequestScope::new();
	/// scope.set(42i32);
	/// scope.set("hello".to_string());
	///
	/// assert_eq!(*scope.get::<i32>().unwrap(), 42);
	/// assert_eq!(*scope.get::<String>().unwrap(), "hello");
	/// ```
	pub fn set<T: Any + Send + Sync>(&self, value: T) {
		let mut cache = self.cache.write().unwrap_or_else(PoisonError::into_inner);
		let type_id = TypeId::of::<T>();
		cache.insert(type_id, Arc::new(value));
	}

	/// Stores a pre-wrapped `Arc<T>` in the request scope cache.
	///
	/// Unlike `set`, this method accepts an already-wrapped Arc value,
	/// avoiding the need to unwrap and re-wrap. This is useful when
	/// the value is produced by a factory that returns `Arc<T>`.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_di::RequestScope;
	/// use std::sync::Arc;
	///
	/// let scope = RequestScope::new();
	/// let value = Arc::new(42i32);
	/// scope.set_arc(value);
	///
	/// assert_eq!(*scope.get::<i32>().unwrap(), 42);
	/// ```
	pub fn set_arc<T: Any + Send + Sync>(&self, value: Arc<T>) {
		let mut cache = self.cache.write().unwrap_or_else(PoisonError::into_inner);
		let type_id = TypeId::of::<T>();
		cache.insert(type_id, value);
	}
}

impl RequestScope {
	/// Creates a deep clone of this scope with an independent cache.
	///
	/// The cloned scope contains the same cached entries as the original,
	/// but modifications to either scope will not affect the other.
	pub fn deep_clone(&self) -> Self {
		let cache = self.cache.read().unwrap_or_else(PoisonError::into_inner);
		Self {
			cache: Arc::new(RwLock::new(cache.clone())),
		}
	}
}

impl Default for RequestScope {
	fn default() -> Self {
		Self::new()
	}
}

pub struct SingletonScope {
	cache: Arc<RwLock<HashMap<TypeId, Arc<dyn Any + Send + Sync>>>>,
}

impl SingletonScope {
	/// Creates a new SingletonScope with an empty cache.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_di::SingletonScope;
	///
	/// let scope = SingletonScope::new();
	/// ```
	pub fn new() -> Self {
		Self {
			cache: Arc::new(RwLock::new(HashMap::new())),
		}
	}
	/// Retrieves a singleton value from the cache by type.
	///
	/// Returns `None` if no value of type `T` exists in the singleton cache.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_di::SingletonScope;
	///
	/// let scope = SingletonScope::new();
	/// scope.set(100u64);
	///
	/// let value = scope.get::<u64>().unwrap();
	/// assert_eq!(*value, 100);
	/// ```
	pub fn get<T: Any + Send + Sync>(&self) -> Option<Arc<T>> {
		let cache = self.cache.read().unwrap_or_else(PoisonError::into_inner);
		let type_id = TypeId::of::<T>();
		cache
			.get(&type_id)
			.and_then(|arc| arc.clone().downcast::<T>().ok())
	}
	/// Stores a singleton value in the cache.
	///
	/// The value persists across multiple requests and is shared application-wide.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_di::SingletonScope;
	///
	/// let scope = SingletonScope::new();
	/// scope.set(42i32);
	///
	/// // Same value retrieved across multiple calls
	/// let val1 = scope.get::<i32>().unwrap();
	/// let val2 = scope.get::<i32>().unwrap();
	/// assert_eq!(*val1, *val2);
	/// ```
	pub fn set<T: Any + Send + Sync>(&self, value: T) {
		let mut cache = self.cache.write().unwrap_or_else(PoisonError::into_inner);
		let type_id = TypeId::of::<T>();
		cache.insert(type_id, Arc::new(value));
	}

	/// Stores a pre-wrapped `Arc<T>` in the singleton scope cache.
	///
	/// Unlike `set`, this method accepts an already-wrapped Arc value,
	/// avoiding the need to unwrap and re-wrap. This is useful when
	/// the value is produced by a factory that returns `Arc<T>`.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_di::SingletonScope;
	/// use std::sync::Arc;
	///
	/// let scope = SingletonScope::new();
	/// let value = Arc::new(42i32);
	/// scope.set_arc(value);
	///
	/// assert_eq!(*scope.get::<i32>().unwrap(), 42);
	/// ```
	pub fn set_arc<T: Any + Send + Sync>(&self, value: Arc<T>) {
		let mut cache = self.cache.write().unwrap_or_else(PoisonError::into_inner);
		let type_id = TypeId::of::<T>();
		cache.insert(type_id, value);
	}
}

impl Default for SingletonScope {
	fn default() -> Self {
		Self::new()
	}
}
