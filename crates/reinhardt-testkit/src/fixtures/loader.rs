use rstest::*;
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Errors that can occur during fixture loading.
#[derive(Debug, thiserror::Error)]
pub enum FixtureError {
	/// The requested fixture file was not found.
	#[error("Fixture not found: {0}")]
	NotFound(String),
	/// An error occurred while reading the fixture file.
	#[error("Load error: {0}")]
	Load(String),
	/// An error occurred while parsing the fixture content.
	#[error("Parse error: {0}")]
	Parse(String),
}

/// Result type for fixture loading operations.
pub type FixtureResult<T> = Result<T, FixtureError>;

/// Fixture data loader
pub struct FixtureLoader {
	fixtures: Arc<RwLock<HashMap<String, serde_json::Value>>>,
}

impl FixtureLoader {
	/// Create a new fixture loader
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_testkit::fixtures::FixtureLoader;
	///
	/// let loader = FixtureLoader::new();
	/// // Loader is ready to load fixtures
	/// ```
	pub fn new() -> Self {
		Self {
			fixtures: Arc::new(RwLock::new(HashMap::new())),
		}
	}
	/// Load fixture from JSON string
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_testkit::fixtures::FixtureLoader;
	///
	/// # tokio_test::block_on(async {
	/// let loader = FixtureLoader::new();
	/// let json = r#"{"id": 1, "name": "Test"}"#;
	/// loader.load_from_json("test".to_string(), json).await.unwrap();
	/// assert!(loader.exists("test").await);
	/// # });
	/// ```
	pub async fn load_from_json(&self, name: String, json: &str) -> FixtureResult<()> {
		let value: serde_json::Value =
			serde_json::from_str(json).map_err(|e| FixtureError::Parse(e.to_string()))?;

		self.fixtures.write().await.insert(name, value);
		Ok(())
	}
	/// Load fixture data
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_testkit::fixtures::FixtureLoader;
	/// use serde::Deserialize;
	///
	/// #[derive(Deserialize)]
	/// struct User {
	///     id: i32,
	///     name: String,
	/// }
	///
	/// # tokio_test::block_on(async {
	/// let loader = FixtureLoader::new();
	/// let json = r#"{"id": 1, "name": "Alice"}"#;
	/// loader.load_from_json("user".to_string(), json).await.unwrap();
	/// let user: User = loader.load("user").await.unwrap();
	/// assert_eq!(user.id, 1);
	/// assert_eq!(user.name, "Alice");
	/// # });
	/// ```
	pub async fn load<T: for<'de> Deserialize<'de>>(&self, name: &str) -> FixtureResult<T> {
		let fixtures = self.fixtures.read().await;
		let value = fixtures
			.get(name)
			.ok_or_else(|| FixtureError::NotFound(name.to_string()))?;

		serde_json::from_value(value.clone()).map_err(|e| FixtureError::Parse(e.to_string()))
	}
	/// Get raw fixture value
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_testkit::fixtures::FixtureLoader;
	///
	/// # tokio_test::block_on(async {
	/// let loader = FixtureLoader::new();
	/// let json = r#"{"status": "active"}"#;
	/// loader.load_from_json("config".to_string(), json).await.unwrap();
	/// let value = loader.get("config").await.unwrap();
	/// assert!(value.is_object());
	/// # });
	/// ```
	pub async fn get(&self, name: &str) -> FixtureResult<serde_json::Value> {
		let fixtures = self.fixtures.read().await;
		fixtures
			.get(name)
			.cloned()
			.ok_or_else(|| FixtureError::NotFound(name.to_string()))
	}
	/// Check if fixture exists
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_testkit::fixtures::FixtureLoader;
	///
	/// # tokio_test::block_on(async {
	/// let loader = FixtureLoader::new();
	/// assert!(!loader.exists("missing").await);
	/// loader.load_from_json("test".to_string(), "{}").await.unwrap();
	/// assert!(loader.exists("test").await);
	/// # });
	/// ```
	pub async fn exists(&self, name: &str) -> bool {
		self.fixtures.read().await.contains_key(name)
	}
	/// Clear all fixtures
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_testkit::fixtures::FixtureLoader;
	///
	/// # tokio_test::block_on(async {
	/// let loader = FixtureLoader::new();
	/// loader.load_from_json("test".to_string(), "{}").await.unwrap();
	/// assert_eq!(loader.list().await.len(), 1);
	/// loader.clear().await;
	/// assert_eq!(loader.list().await.len(), 0);
	/// # });
	/// ```
	pub async fn clear(&self) {
		self.fixtures.write().await.clear();
	}
	/// List all fixture names
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_testkit::fixtures::FixtureLoader;
	///
	/// # tokio_test::block_on(async {
	/// let loader = FixtureLoader::new();
	/// loader.load_from_json("test1".to_string(), "{}").await.unwrap();
	/// loader.load_from_json("test2".to_string(), "{}").await.unwrap();
	/// let names = loader.list().await;
	/// assert_eq!(names.len(), 2);
	/// assert!(names.contains(&"test1".to_string()));
	/// # });
	/// ```
	pub async fn list(&self) -> Vec<String> {
		self.fixtures.read().await.keys().cloned().collect()
	}
}

impl Default for FixtureLoader {
	fn default() -> Self {
		Self::new()
	}
}

/// Factory trait for creating test data
pub trait Factory<T>: Send + Sync {
	/// Build a single instance of the test data type.
	fn build(&self) -> T;
	/// Build multiple instances of the test data type.
	fn build_batch(&self, count: usize) -> Vec<T> {
		(0..count).map(|_| self.build()).collect()
	}
}

/// Simple factory builder
pub struct FactoryBuilder<T, F>
where
	F: Fn() -> T + Send + Sync,
{
	builder: F,
	_phantom: std::marker::PhantomData<T>,
}

impl<T, F> FactoryBuilder<T, F>
where
	F: Fn() -> T + Send + Sync,
{
	/// Create a new factory builder
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_testkit::fixtures::{FactoryBuilder, Factory};
	///
	/// #[derive(Debug, PartialEq)]
	/// struct TestData { id: i32 }
	///
	/// let factory = FactoryBuilder::new(|| TestData { id: 42 });
	/// let item = factory.build();
	/// assert_eq!(item.id, 42);
	/// ```
	pub fn new(builder: F) -> Self {
		Self {
			builder,
			_phantom: std::marker::PhantomData,
		}
	}
}

impl<T, F> Factory<T> for FactoryBuilder<T, F>
where
	F: Fn() -> T + Send + Sync,
	T: Send + Sync,
{
	fn build(&self) -> T {
		(self.builder)()
	}
}

/// Generate a random test key using UUID
///
/// # Examples
///
/// ```
/// use reinhardt_testkit::fixtures::random_test_key;
///
/// let key = random_test_key();
/// assert!(key.starts_with("test_key_"));
/// ```
pub fn random_test_key() -> String {
	use uuid::Uuid;
	format!("test_key_{}", Uuid::now_v7().simple())
}

/// Generate test configuration data with timestamp
///
/// # Examples
///
/// ```
/// use reinhardt_testkit::fixtures::test_config_value;
///
/// let value = test_config_value("my_value");
/// assert_eq!(value["value"], "my_value");
/// ```
pub fn test_config_value(value: &str) -> serde_json::Value {
	serde_json::json!({
		"value": value,
		"timestamp": chrono::Utc::now().to_rfc3339(),
	})
}

// ============================================================================
// rstest integration: Fixtures for common test resources
// ============================================================================

/// Fixture providing a FixtureLoader instance
///
/// Use this fixture in tests that need to load JSON fixture data.
///
/// # Examples
///
/// ```rust,no_run
/// use reinhardt_testkit::fixtures::fixture_loader;
/// use rstest::*;
///
/// #[rstest]
/// #[tokio::test]
/// async fn test_with_fixtures(fixture_loader: reinhardt_testkit::fixtures::FixtureLoader) {
///     fixture_loader.load_from_json("test".to_string(), r#"{"id": 1}"#).await.unwrap();
///     // ...
/// }
/// ```
#[fixture]
pub fn fixture_loader() -> FixtureLoader {
	FixtureLoader::new()
}

/// Fixture providing an APIClient instance
///
/// Use this fixture in tests that need to make test HTTP requests.
///
/// # Examples
///
/// ```rust,no_run
/// use reinhardt_testkit::fixtures::api_client;
/// use rstest::*;
///
/// #[rstest]
/// #[tokio::test]
/// async fn test_api_request(api_client: reinhardt_testkit::client::APIClient) {
///     // Make requests with client
/// }
/// ```
#[fixture]
pub fn api_client() -> crate::client::APIClient {
	crate::client::APIClient::new()
}

/// Fixture providing a temporary directory that is automatically cleaned up
///
/// # Examples
///
/// ```rust
/// use reinhardt_testkit::fixtures::temp_dir;
/// use rstest::*;
///
/// #[rstest]
/// fn test_with_temp_dir(temp_dir: tempfile::TempDir) {
///     let path = temp_dir.path();
///     std::fs::write(path.join("test.txt"), "data").unwrap();
///     // temp_dir is automatically cleaned up when test ends
/// }
/// ```
#[fixture]
pub fn temp_dir() -> tempfile::TempDir {
	tempfile::tempdir().expect("Failed to create temporary directory")
}

#[cfg(test)]
mod tests {
	use super::*;
	use serde::Serialize;

	#[derive(Debug, Serialize, Deserialize, PartialEq)]
	struct TestData {
		id: i32,
		name: String,
	}

	#[tokio::test]
	async fn test_fixture_loader() {
		let loader = FixtureLoader::new();
		let json = r#"{"id": 1, "name": "Test"}"#;

		loader
			.load_from_json("test".to_string(), json)
			.await
			.unwrap();

		let data: TestData = loader.load("test").await.unwrap();
		assert_eq!(data.id, 1);
		assert_eq!(data.name, "Test");
	}

	#[tokio::test]
	async fn test_fixture_not_found() {
		let loader = FixtureLoader::new();
		let result: FixtureResult<TestData> = loader.load("missing").await;
		assert!(result.is_err());
	}

	#[test]
	fn test_factory_builder() {
		let factory = FactoryBuilder::new(|| TestData {
			id: 1,
			name: "Test".to_string(),
		});

		let data = factory.build();
		assert_eq!(data.id, 1);

		let batch = factory.build_batch(3);
		assert_eq!(batch.len(), 3);
	}
}
