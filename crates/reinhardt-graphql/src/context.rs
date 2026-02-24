//! GraphQL context for request-scoped data
//!
//! Provides context management for GraphQL query execution, including
//! request information, user authentication, data loaders, and custom data.

use async_trait::async_trait;
use serde_json::Value;
use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::RwLock;

/// Error types for context operations
#[derive(Debug, thiserror::Error)]
pub enum ContextError {
	/// Required data was not found in context
	#[error("Required context data not found for key: {0}")]
	DataNotFound(String),
	/// Required data loader was not found in context
	#[error("Required data loader not found: {0}")]
	LoaderNotFound(String),
}

/// Error types for data loader operations
#[derive(Debug, thiserror::Error)]
pub enum LoaderError {
	#[error("Loader error: {0}")]
	Load(String),
	#[error("Not found: {0}")]
	NotFound(String),
	#[error("Invalid data: {0}")]
	InvalidData(String),
}

/// Trait for implementing data loaders
///
/// Data loaders provide batching and caching for database queries,
/// helping to solve the N+1 query problem in GraphQL.
///
/// # Examples
///
/// ```
/// use reinhardt_graphql::context::{DataLoader, LoaderError};
/// use async_trait::async_trait;
///
/// struct UserLoader;
///
/// #[async_trait]
/// impl DataLoader for UserLoader {
///     type Key = String;
///     type Value = String;
///
///     async fn load(&self, key: Self::Key) -> Result<Self::Value, LoaderError> {
///         Ok(format!("User: {}", key))
///     }
///
///     async fn load_many(&self, keys: Vec<Self::Key>) -> Result<Vec<Self::Value>, LoaderError> {
///         Ok(keys.into_iter().map(|k| format!("User: {}", k)).collect())
///     }
/// }
/// ```
#[async_trait]
pub trait DataLoader: Send + Sync + 'static {
	type Key: Send;
	type Value: Send;

	/// Load a single value by key
	async fn load(&self, key: Self::Key) -> Result<Self::Value, LoaderError>;

	/// Load multiple values by keys (batch loading)
	async fn load_many(&self, keys: Vec<Self::Key>) -> Result<Vec<Self::Value>, LoaderError>;
}

/// GraphQL context for managing request-scoped data
///
/// Provides access to request information, user authentication state,
/// data loaders for efficient batch loading, and custom data storage.
///
/// # Examples
///
/// ```
/// use reinhardt_graphql::context::GraphQLContext;
/// use serde_json::json;
///
/// let context = GraphQLContext::new();
///
/// // Set custom data
/// context.set_data("api_version".to_string(), json!("v1"));
///
/// // Get custom data
/// let version = context.get_data("api_version");
/// assert_eq!(version, Some(json!("v1")));
/// ```
pub struct GraphQLContext {
	/// Custom data storage
	custom_data: Arc<RwLock<HashMap<String, Value>>>,
	/// Type-erased data loaders
	data_loaders: Arc<RwLock<HashMap<TypeId, Box<dyn Any + Send + Sync>>>>,
}

impl GraphQLContext {
	/// Create a new GraphQL context
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_graphql::context::GraphQLContext;
	///
	/// let context = GraphQLContext::new();
	/// assert!(context.get_data("nonexistent").is_none());
	/// ```
	pub fn new() -> Self {
		Self {
			custom_data: Arc::new(RwLock::new(HashMap::new())),
			data_loaders: Arc::new(RwLock::new(HashMap::new())),
		}
	}

	/// Set custom data in the context
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_graphql::context::GraphQLContext;
	/// use serde_json::json;
	///
	/// let context = GraphQLContext::new();
	/// context.set_data("user_id".to_string(), json!("123"));
	///
	/// assert_eq!(context.get_data("user_id"), Some(json!("123")));
	/// ```
	pub fn set_data(&self, key: String, value: Value) {
		let mut data = self.custom_data.write().unwrap();
		data.insert(key, value);
	}

	/// Get custom data from the context
	///
	/// Returns `None` if the key does not exist. For GraphQL resolvers that
	/// require the data to be present, use [`require_data`](Self::require_data)
	/// instead to get a proper GraphQL error.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_graphql::context::GraphQLContext;
	/// use serde_json::json;
	///
	/// let context = GraphQLContext::new();
	/// context.set_data("count".to_string(), json!(42));
	///
	/// assert_eq!(context.get_data("count"), Some(json!(42)));
	/// assert_eq!(context.get_data("nonexistent"), None);
	/// ```
	pub fn get_data(&self, key: &str) -> Option<Value> {
		let data = self.custom_data.read().unwrap();
		data.get(key).cloned()
	}

	/// Get required custom data from the context, returning a GraphQL error if missing
	///
	/// This method should be used in GraphQL resolvers where the data is expected
	/// to be present. Instead of panicking on a missing key, it returns a
	/// descriptive [`async_graphql::Error`] that will be surfaced as a GraphQL error
	/// in the response.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_graphql::context::GraphQLContext;
	/// use serde_json::json;
	///
	/// let context = GraphQLContext::new();
	/// context.set_data("api_version".to_string(), json!("v1"));
	///
	/// // Successful lookup
	/// let version = context.require_data("api_version");
	/// assert!(version.is_ok());
	/// assert_eq!(version.unwrap(), json!("v1"));
	///
	/// // Missing key returns an error
	/// let missing = context.require_data("nonexistent");
	/// assert!(missing.is_err());
	/// ```
	pub fn require_data(&self, key: &str) -> async_graphql::Result<Value> {
		self.get_data(key)
			.ok_or_else(|| ContextError::DataNotFound(key.to_string()).into())
	}

	/// Remove custom data from the context
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_graphql::context::GraphQLContext;
	/// use serde_json::json;
	///
	/// let context = GraphQLContext::new();
	/// context.set_data("temp".to_string(), json!("value"));
	///
	/// let removed = context.remove_data("temp");
	/// assert_eq!(removed, Some(json!("value")));
	/// assert_eq!(context.get_data("temp"), None);
	/// ```
	pub fn remove_data(&self, key: &str) -> Option<Value> {
		let mut data = self.custom_data.write().unwrap();
		data.remove(key)
	}

	/// Clear all custom data
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_graphql::context::GraphQLContext;
	/// use serde_json::json;
	///
	/// let context = GraphQLContext::new();
	/// context.set_data("key1".to_string(), json!("value1"));
	/// context.set_data("key2".to_string(), json!("value2"));
	///
	/// context.clear_data();
	///
	/// assert_eq!(context.get_data("key1"), None);
	/// assert_eq!(context.get_data("key2"), None);
	/// ```
	pub fn clear_data(&self) {
		let mut data = self.custom_data.write().unwrap();
		data.clear();
	}

	/// Add a data loader to the context
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_graphql::context::{GraphQLContext, DataLoader, LoaderError};
	/// use async_trait::async_trait;
	/// use std::sync::Arc;
	///
	/// struct SimpleLoader;
	///
	/// #[async_trait]
	/// impl DataLoader for SimpleLoader {
	///     type Key = i32;
	///     type Value = String;
	///
	///     async fn load(&self, key: Self::Key) -> Result<Self::Value, LoaderError> {
	///         Ok(format!("Value {}", key))
	///     }
	///
	///     async fn load_many(&self, keys: Vec<Self::Key>) -> Result<Vec<Self::Value>, LoaderError> {
	///         Ok(keys.iter().map(|k| format!("Value {}", k)).collect())
	///     }
	/// }
	///
	/// let context = GraphQLContext::new();
	/// let loader = Arc::new(SimpleLoader);
	/// context.add_data_loader(loader.clone());
	///
	/// let retrieved = context.get_data_loader::<SimpleLoader>();
	/// assert!(retrieved.is_some());
	/// ```
	pub fn add_data_loader<T: DataLoader>(&self, loader: Arc<T>) {
		let mut loaders = self.data_loaders.write().unwrap();
		loaders.insert(TypeId::of::<T>(), Box::new(loader));
	}

	/// Get a data loader from the context
	///
	/// Returns `None` if the loader has not been registered. For GraphQL
	/// resolvers that require the loader, use
	/// [`require_data_loader`](Self::require_data_loader) instead to get a
	/// proper GraphQL error.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_graphql::context::{GraphQLContext, DataLoader, LoaderError};
	/// use async_trait::async_trait;
	/// use std::sync::Arc;
	///
	/// struct TestLoader;
	///
	/// #[async_trait]
	/// impl DataLoader for TestLoader {
	///     type Key = String;
	///     type Value = i32;
	///
	///     async fn load(&self, _key: Self::Key) -> Result<Self::Value, LoaderError> {
	///         Ok(42)
	///     }
	///
	///     async fn load_many(&self, keys: Vec<Self::Key>) -> Result<Vec<Self::Value>, LoaderError> {
	///         Ok(vec![42; keys.len()])
	///     }
	/// }
	///
	/// let context = GraphQLContext::new();
	/// let loader = Arc::new(TestLoader);
	/// context.add_data_loader(loader);
	///
	/// let retrieved = context.get_data_loader::<TestLoader>();
	/// assert!(retrieved.is_some());
	/// ```
	pub fn get_data_loader<T: DataLoader>(&self) -> Option<Arc<T>> {
		let loaders = self.data_loaders.read().unwrap();
		loaders
			.get(&TypeId::of::<T>())
			.and_then(|loader| loader.downcast_ref::<Arc<T>>().cloned())
	}

	/// Get a required data loader from the context, returning a GraphQL error if missing
	///
	/// This method should be used in GraphQL resolvers where the data loader is
	/// expected to be registered. Instead of panicking on a missing loader, it
	/// returns a descriptive [`async_graphql::Error`] that will be surfaced as a
	/// GraphQL error in the response.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_graphql::context::{GraphQLContext, DataLoader, LoaderError};
	/// use async_trait::async_trait;
	/// use std::sync::Arc;
	///
	/// struct MyLoader;
	///
	/// #[async_trait]
	/// impl DataLoader for MyLoader {
	///     type Key = String;
	///     type Value = i32;
	///
	///     async fn load(&self, _key: Self::Key) -> Result<Self::Value, LoaderError> {
	///         Ok(42)
	///     }
	///
	///     async fn load_many(&self, keys: Vec<Self::Key>) -> Result<Vec<Self::Value>, LoaderError> {
	///         Ok(vec![42; keys.len()])
	///     }
	/// }
	///
	/// let context = GraphQLContext::new();
	///
	/// // Missing loader returns an error
	/// let result = context.require_data_loader::<MyLoader>();
	/// assert!(result.is_err());
	///
	/// // After adding the loader, it succeeds
	/// context.add_data_loader(Arc::new(MyLoader));
	/// let result = context.require_data_loader::<MyLoader>();
	/// assert!(result.is_ok());
	/// ```
	pub fn require_data_loader<T: DataLoader>(&self) -> async_graphql::Result<Arc<T>> {
		self.get_data_loader::<T>().ok_or_else(|| {
			ContextError::LoaderNotFound(std::any::type_name::<T>().to_string()).into()
		})
	}

	/// Remove a data loader from the context
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_graphql::context::{GraphQLContext, DataLoader, LoaderError};
	/// use async_trait::async_trait;
	/// use std::sync::Arc;
	///
	/// struct RemovableLoader;
	///
	/// #[async_trait]
	/// impl DataLoader for RemovableLoader {
	///     type Key = u64;
	///     type Value = String;
	///
	///     async fn load(&self, key: Self::Key) -> Result<Self::Value, LoaderError> {
	///         Ok(key.to_string())
	///     }
	///
	///     async fn load_many(&self, keys: Vec<Self::Key>) -> Result<Vec<Self::Value>, LoaderError> {
	///         Ok(keys.iter().map(|k| k.to_string()).collect())
	///     }
	/// }
	///
	/// let context = GraphQLContext::new();
	/// let loader = Arc::new(RemovableLoader);
	/// context.add_data_loader(loader);
	///
	/// context.remove_data_loader::<RemovableLoader>();
	/// assert!(context.get_data_loader::<RemovableLoader>().is_none());
	/// ```
	pub fn remove_data_loader<T: DataLoader>(&self) {
		let mut loaders = self.data_loaders.write().unwrap();
		loaders.remove(&TypeId::of::<T>());
	}

	/// Clear all data loaders
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_graphql::context::{GraphQLContext, DataLoader, LoaderError};
	/// use async_trait::async_trait;
	/// use std::sync::Arc;
	///
	/// struct Loader1;
	/// struct Loader2;
	///
	/// #[async_trait]
	/// impl DataLoader for Loader1 {
	///     type Key = i32;
	///     type Value = String;
	///     async fn load(&self, key: Self::Key) -> Result<Self::Value, LoaderError> {
	///         Ok(key.to_string())
	///     }
	///     async fn load_many(&self, keys: Vec<Self::Key>) -> Result<Vec<Self::Value>, LoaderError> {
	///         Ok(keys.iter().map(|k| k.to_string()).collect())
	///     }
	/// }
	///
	/// #[async_trait]
	/// impl DataLoader for Loader2 {
	///     type Key = String;
	///     type Value = i32;
	///     async fn load(&self, _key: Self::Key) -> Result<Self::Value, LoaderError> {
	///         Ok(0)
	///     }
	///     async fn load_many(&self, keys: Vec<Self::Key>) -> Result<Vec<Self::Value>, LoaderError> {
	///         Ok(vec![0; keys.len()])
	///     }
	/// }
	///
	/// let context = GraphQLContext::new();
	/// context.add_data_loader(Arc::new(Loader1));
	/// context.add_data_loader(Arc::new(Loader2));
	///
	/// context.clear_loaders();
	///
	/// assert!(context.get_data_loader::<Loader1>().is_none());
	/// assert!(context.get_data_loader::<Loader2>().is_none());
	/// ```
	pub fn clear_loaders(&self) {
		let mut loaders = self.data_loaders.write().unwrap();
		loaders.clear();
	}
}

impl Default for GraphQLContext {
	fn default() -> Self {
		Self::new()
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;

	#[derive(Debug)]
	struct TestLoader;

	#[async_trait]
	impl DataLoader for TestLoader {
		type Key = String;
		type Value = i32;

		async fn load(&self, key: Self::Key) -> Result<Self::Value, LoaderError> {
			key.parse::<i32>()
				.map_err(|e| LoaderError::InvalidData(e.to_string()))
		}

		async fn load_many(&self, keys: Vec<Self::Key>) -> Result<Vec<Self::Value>, LoaderError> {
			keys.into_iter()
				.map(|k| {
					k.parse::<i32>()
						.map_err(|e| LoaderError::InvalidData(e.to_string()))
				})
				.collect()
		}
	}

	#[rstest]
	fn test_context_new() {
		// Arrange & Act
		let context = GraphQLContext::new();

		// Assert
		assert!(context.get_data("any_key").is_none());
	}

	#[rstest]
	fn test_set_and_get_data() {
		// Arrange
		let context = GraphQLContext::new();
		let value = serde_json::json!({"name": "test", "value": 42});

		// Act
		context.set_data("test_key".to_string(), value.clone());

		// Assert
		let retrieved = context.get_data("test_key");
		assert_eq!(retrieved, Some(value));
	}

	#[rstest]
	fn test_get_nonexistent_data() {
		// Arrange
		let context = GraphQLContext::new();

		// Act
		let result = context.get_data("nonexistent");

		// Assert
		assert_eq!(result, None);
	}

	#[rstest]
	fn test_remove_data() {
		// Arrange
		let context = GraphQLContext::new();
		let value = serde_json::json!("test_value");
		context.set_data("key".to_string(), value.clone());

		// Act
		let removed = context.remove_data("key");

		// Assert
		assert_eq!(removed, Some(value));
		assert_eq!(context.get_data("key"), None);
	}

	#[rstest]
	fn test_clear_data() {
		// Arrange
		let context = GraphQLContext::new();
		context.set_data("key1".to_string(), serde_json::json!(1));
		context.set_data("key2".to_string(), serde_json::json!(2));
		context.set_data("key3".to_string(), serde_json::json!(3));

		// Act
		context.clear_data();

		// Assert
		assert_eq!(context.get_data("key1"), None);
		assert_eq!(context.get_data("key2"), None);
		assert_eq!(context.get_data("key3"), None);
	}

	#[rstest]
	fn test_add_and_get_data_loader() {
		// Arrange
		let context = GraphQLContext::new();
		let loader = Arc::new(TestLoader);

		// Act
		context.add_data_loader(loader);

		// Assert
		let retrieved = context.get_data_loader::<TestLoader>();
		assert!(retrieved.is_some());
	}

	#[rstest]
	fn test_get_nonexistent_loader() {
		// Arrange
		let context = GraphQLContext::new();

		// Act
		let result = context.get_data_loader::<TestLoader>();

		// Assert
		assert!(result.is_none());
	}

	#[rstest]
	fn test_remove_data_loader() {
		// Arrange
		let context = GraphQLContext::new();
		let loader = Arc::new(TestLoader);
		context.add_data_loader(loader);

		// Act
		context.remove_data_loader::<TestLoader>();

		// Assert
		let result = context.get_data_loader::<TestLoader>();
		assert!(result.is_none());
	}

	#[rstest]
	fn test_clear_loaders() {
		struct Loader1;
		struct Loader2;

		#[async_trait]
		impl DataLoader for Loader1 {
			type Key = i32;
			type Value = String;
			async fn load(&self, key: Self::Key) -> Result<Self::Value, LoaderError> {
				Ok(key.to_string())
			}
			async fn load_many(
				&self,
				keys: Vec<Self::Key>,
			) -> Result<Vec<Self::Value>, LoaderError> {
				Ok(keys.iter().map(|k| k.to_string()).collect())
			}
		}

		#[async_trait]
		impl DataLoader for Loader2 {
			type Key = String;
			type Value = i32;
			async fn load(&self, _key: Self::Key) -> Result<Self::Value, LoaderError> {
				Ok(0)
			}
			async fn load_many(
				&self,
				keys: Vec<Self::Key>,
			) -> Result<Vec<Self::Value>, LoaderError> {
				Ok(vec![0; keys.len()])
			}
		}

		// Arrange
		let context = GraphQLContext::new();
		context.add_data_loader(Arc::new(Loader1));
		context.add_data_loader(Arc::new(Loader2));

		// Act
		context.clear_loaders();

		// Assert
		assert!(context.get_data_loader::<Loader1>().is_none());
		assert!(context.get_data_loader::<Loader2>().is_none());
	}

	#[rstest]
	fn test_multiple_data_values() {
		// Arrange
		let context = GraphQLContext::new();

		// Act
		context.set_data("int".to_string(), serde_json::json!(123));
		context.set_data("string".to_string(), serde_json::json!("hello"));
		context.set_data("array".to_string(), serde_json::json!([1, 2, 3]));
		context.set_data("object".to_string(), serde_json::json!({"key": "value"}));

		// Assert
		assert_eq!(context.get_data("int"), Some(serde_json::json!(123)));
		assert_eq!(context.get_data("string"), Some(serde_json::json!("hello")));
		assert_eq!(
			context.get_data("array"),
			Some(serde_json::json!([1, 2, 3]))
		);
		assert_eq!(
			context.get_data("object"),
			Some(serde_json::json!({"key": "value"}))
		);
	}

	#[rstest]
	#[tokio::test]
	async fn test_data_loader_load() {
		// Arrange
		let loader = TestLoader;

		// Act
		let result = loader.load("42".to_string()).await;

		// Assert
		assert!(result.is_ok());
		assert_eq!(result.unwrap(), 42);
	}

	#[rstest]
	#[tokio::test]
	async fn test_data_loader_load_many() {
		// Arrange
		let loader = TestLoader;
		let keys = vec!["1".to_string(), "2".to_string(), "3".to_string()];

		// Act
		let result = loader.load_many(keys).await;

		// Assert
		assert!(result.is_ok());
		assert_eq!(result.unwrap(), vec![1, 2, 3]);
	}

	#[rstest]
	#[tokio::test]
	async fn test_data_loader_error() {
		// Arrange
		let loader = TestLoader;

		// Act
		let result = loader.load("invalid".to_string()).await;

		// Assert
		assert!(result.is_err());
		match result {
			Err(LoaderError::InvalidData(_)) => {}
			_ => panic!("Expected InvalidData error"),
		}
	}

	#[rstest]
	fn test_context_default() {
		// Arrange & Act
		let context = GraphQLContext::default();

		// Assert
		assert!(context.get_data("any_key").is_none());
	}

	#[rstest]
	fn test_overwrite_data() {
		// Arrange
		let context = GraphQLContext::new();
		context.set_data("key".to_string(), serde_json::json!(1));

		// Act
		context.set_data("key".to_string(), serde_json::json!(2));

		// Assert
		assert_eq!(context.get_data("key"), Some(serde_json::json!(2)));
	}

	#[rstest]
	fn test_require_data_returns_value_when_present() {
		// Arrange
		let context = GraphQLContext::new();
		context.set_data("user_id".to_string(), serde_json::json!("user-42"));

		// Act
		let result = context.require_data("user_id");

		// Assert
		assert!(result.is_ok());
		assert_eq!(result.unwrap(), serde_json::json!("user-42"));
	}

	#[rstest]
	fn test_require_data_returns_error_when_missing() {
		// Arrange
		let context = GraphQLContext::new();

		// Act
		let result = context.require_data("nonexistent_key");

		// Assert
		assert!(result.is_err());
		let err = result.unwrap_err();
		assert!(
			err.message.contains("nonexistent_key"),
			"Error should mention the missing key, got: {}",
			err.message
		);
		assert!(
			err.message.contains("Required context data not found"),
			"Error should describe the issue, got: {}",
			err.message
		);
	}

	#[rstest]
	fn test_require_data_does_not_panic_on_missing_key() {
		// Arrange
		let context = GraphQLContext::new();

		// Act -- this must NOT panic, unlike unwrap() on get_data()
		let result = context.require_data("missing");

		// Assert
		assert!(result.is_err());
	}

	#[rstest]
	fn test_require_data_loader_returns_loader_when_present() {
		// Arrange
		let context = GraphQLContext::new();
		context.add_data_loader(Arc::new(TestLoader));

		// Act
		let result = context.require_data_loader::<TestLoader>();

		// Assert
		assert!(result.is_ok());
	}

	#[rstest]
	fn test_require_data_loader_returns_error_when_missing() {
		// Arrange
		let context = GraphQLContext::new();

		// Act
		let result = context.require_data_loader::<TestLoader>();

		// Assert
		assert!(result.is_err());
		let err = result.unwrap_err();
		assert!(
			err.message.contains("Required data loader not found"),
			"Error should describe the issue, got: {}",
			err.message
		);
	}

	#[rstest]
	fn test_require_data_loader_does_not_panic_on_missing_loader() {
		// Arrange
		let context = GraphQLContext::new();

		// Act -- this must NOT panic, unlike unwrap() on get_data_loader()
		let result = context.require_data_loader::<TestLoader>();

		// Assert
		assert!(result.is_err());
	}

	#[rstest]
	fn test_require_data_loader_after_removal_returns_error() {
		// Arrange
		let context = GraphQLContext::new();
		context.add_data_loader(Arc::new(TestLoader));
		context.remove_data_loader::<TestLoader>();

		// Act
		let result = context.require_data_loader::<TestLoader>();

		// Assert
		assert!(result.is_err());
	}

	#[rstest]
	fn test_context_error_data_not_found_display() {
		// Arrange
		let err = ContextError::DataNotFound("my_key".to_string());

		// Act
		let message = err.to_string();

		// Assert
		assert_eq!(message, "Required context data not found for key: my_key");
	}

	#[rstest]
	fn test_context_error_loader_not_found_display() {
		// Arrange
		let err = ContextError::LoaderNotFound("MyLoader".to_string());

		// Act
		let message = err.to_string();

		// Assert
		assert_eq!(message, "Required data loader not found: MyLoader");
	}

	#[rstest]
	fn test_context_error_converts_to_graphql_error() {
		// Arrange
		let err = ContextError::DataNotFound("test_key".to_string());

		// Act
		let gql_err: async_graphql::Error = err.into();

		// Assert
		assert!(gql_err.message.contains("test_key"));
		assert!(gql_err.message.contains("Required context data not found"));
	}
}
