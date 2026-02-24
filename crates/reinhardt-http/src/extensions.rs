//! Type-safe extensions for Request
//!
//! Provides a simple type-safe storage mechanism for arbitrary data
//! that can be attached to requests.

use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Type-safe extension storage
#[derive(Clone, Default)]
pub struct Extensions {
	map: Arc<Mutex<HashMap<TypeId, Box<dyn Any + Send + Sync>>>>,
}

impl Extensions {
	/// Create a new Extensions instance
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_http::Extensions;
	///
	/// let extensions = Extensions::new();
	/// assert!(!extensions.contains::<String>());
	/// ```
	pub fn new() -> Self {
		Self {
			map: Arc::new(Mutex::new(HashMap::new())),
		}
	}
	/// Insert a value into extensions
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_http::Extensions;
	///
	/// let extensions = Extensions::new();
	/// extensions.insert(42u32);
	/// extensions.insert("hello".to_string());
	///
	/// assert!(extensions.contains::<u32>());
	/// assert!(extensions.contains::<String>());
	/// ```
	pub fn insert<T: Send + Sync + 'static>(&self, value: T) {
		let mut map = self.map.lock().unwrap_or_else(|e| e.into_inner());
		map.insert(TypeId::of::<T>(), Box::new(value));
	}
	/// Get a cloned value from extensions
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_http::Extensions;
	///
	/// let extensions = Extensions::new();
	/// extensions.insert(42u32);
	///
	/// assert_eq!(extensions.get::<u32>(), Some(42));
	/// assert_eq!(extensions.get::<String>(), None);
	/// ```
	pub fn get<T>(&self) -> Option<T>
	where
		T: Clone + Send + Sync + 'static,
	{
		let map = self.map.lock().unwrap_or_else(|e| e.into_inner());
		map.get(&TypeId::of::<T>())
			.and_then(|boxed| boxed.downcast_ref::<T>())
			.cloned()
	}
	/// Check if a value of the given type exists
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_http::Extensions;
	///
	/// let extensions = Extensions::new();
	/// extensions.insert("hello".to_string());
	///
	/// assert!(extensions.contains::<String>());
	/// assert!(!extensions.contains::<u32>());
	/// ```
	pub fn contains<T: Send + Sync + 'static>(&self) -> bool {
		let map = self.map.lock().unwrap_or_else(|e| e.into_inner());
		map.contains_key(&TypeId::of::<T>())
	}
	/// Remove a value from extensions and return it
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_http::Extensions;
	///
	/// let extensions = Extensions::new();
	/// extensions.insert(42u32);
	///
	/// assert_eq!(extensions.remove::<u32>(), Some(42));
	/// assert!(!extensions.contains::<u32>());
	/// assert_eq!(extensions.remove::<u32>(), None);
	/// ```
	pub fn remove<T>(&self) -> Option<T>
	where
		T: Clone + Send + Sync + 'static,
	{
		let mut map = self.map.lock().unwrap_or_else(|e| e.into_inner());
		map.remove(&TypeId::of::<T>())
			.and_then(|boxed| boxed.downcast_ref::<T>().cloned())
	}
	/// Clear all extensions
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_http::Extensions;
	///
	/// let extensions = Extensions::new();
	/// extensions.insert(42u32);
	/// extensions.insert("hello".to_string());
	///
	/// assert!(extensions.contains::<u32>());
	/// assert!(extensions.contains::<String>());
	///
	/// extensions.clear();
	///
	/// assert!(!extensions.contains::<u32>());
	/// assert!(!extensions.contains::<String>());
	/// ```
	pub fn clear(&self) {
		let mut map = self.map.lock().unwrap_or_else(|e| e.into_inner());
		map.clear();
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[derive(Clone, Debug, PartialEq)]
	struct TestData {
		value: String,
	}

	#[test]
	fn test_insert_and_get() {
		let extensions = Extensions::new();
		let data = TestData {
			value: "test".to_string(),
		};

		extensions.insert(data.clone());
		let retrieved = extensions.get::<TestData>();

		assert_eq!(retrieved, Some(data));
	}

	#[test]
	fn test_get_nonexistent() {
		let extensions = Extensions::new();
		let retrieved = extensions.get::<TestData>();

		assert_eq!(retrieved, None);
	}

	#[test]
	fn test_contains() {
		let extensions = Extensions::new();
		extensions.insert(TestData {
			value: "test".to_string(),
		});

		assert!(extensions.contains::<TestData>());
		assert!(!extensions.contains::<String>());
	}

	#[test]
	fn test_remove() {
		let extensions = Extensions::new();
		let data = TestData {
			value: "test".to_string(),
		};

		extensions.insert(data.clone());
		let removed = extensions.remove::<TestData>();

		assert_eq!(removed, Some(data));
		assert!(!extensions.contains::<TestData>());
	}

	#[test]
	fn test_clear() {
		let extensions = Extensions::new();
		extensions.insert(TestData {
			value: "test".to_string(),
		});
		extensions.insert("another value".to_string());

		extensions.clear();

		assert!(!extensions.contains::<TestData>());
		assert!(!extensions.contains::<String>());
	}

	#[test]
	fn test_multiple_types() {
		let extensions = Extensions::new();
		extensions.insert(TestData {
			value: "test".to_string(),
		});
		extensions.insert(42u32);
		extensions.insert("string value".to_string());

		assert_eq!(
			extensions.get::<TestData>(),
			Some(TestData {
				value: "test".to_string()
			})
		);
		assert_eq!(extensions.get::<u32>(), Some(42));
		assert_eq!(extensions.get::<String>(), Some("string value".to_string()));
	}
}
