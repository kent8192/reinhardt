//! File-based session backend
//!
//! This module provides session storage using files on the filesystem.
//! Each session is stored as a separate JSON file with file locking for concurrent access.
//!
//! ## Features
//!
//! - Session files stored in configurable directory (default: `/tmp/reinhardt_sessions`)
//! - File locking using `fs2` for safe concurrent access
//! - JSON serialization for session data
//! - Automatic directory creation
//! - File naming: `session_{session_key}.json`
//!
//! ## Example
//!
//! ```rust
//! use reinhardt_auth::sessions::backends::{FileSessionBackend, SessionBackend};
//! use serde_json::json;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // Create a file session backend with default directory
//! let backend = FileSessionBackend::new(None)?;
//!
//! // Store session data
//! let session_data = json!({
//!     "user_id": 42,
//!     "username": "alice",
//!     "is_authenticated": true,
//! });
//!
//! backend.save("session_abc123", &session_data, Some(3600)).await?;
//!
//! // Load session data
//! let loaded: Option<serde_json::Value> = backend.load("session_abc123").await?;
//! assert_eq!(loaded, Some(session_data));
//!
//! // Delete session
//! backend.delete("session_abc123").await?;
//! # Ok(())
//! # }
//! ```
//!
//! ## Custom Directory
//!
//! ```rust
//! use reinhardt_auth::sessions::backends::FileSessionBackend;
//! use std::path::PathBuf;
//!
//! # fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // Use a custom directory for session storage
//! let backend = FileSessionBackend::new(Some(PathBuf::from("/tmp/my_sessions")))?;
//! # Ok(())
//! # }
//! ```

use super::cache::{SessionBackend, SessionError};
use async_trait::async_trait;
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// Default session directory
const DEFAULT_SESSION_DIR: &str = "/tmp/reinhardt_sessions";

/// File-based session backend
///
/// Stores sessions as JSON files on the filesystem with file locking for
/// concurrent access safety.
///
/// ## Example
///
/// ```rust
/// use reinhardt_auth::sessions::backends::{FileSessionBackend, SessionBackend};
/// use serde_json::json;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let backend = FileSessionBackend::new(None)?;
///
/// // Save user session
/// let user_data = json!({
///     "user_id": 123,
///     "email": "user@example.com",
///     "role": "admin",
/// });
///
/// backend.save("user_session_xyz", &user_data, Some(7200)).await?;
///
/// // Check if session exists
/// assert!(backend.exists("user_session_xyz").await?);
/// # Ok(())
/// # }
/// ```
#[derive(Clone, Debug)]
pub struct FileSessionBackend {
	session_dir: PathBuf,
}

impl FileSessionBackend {
	/// Create a new file session backend
	///
	/// # Arguments
	///
	/// * `session_dir` - Optional directory path for session storage.
	///   If `None`, uses `/tmp/reinhardt_sessions`
	///
	/// # Errors
	///
	/// Returns an error if the directory cannot be created.
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_auth::sessions::backends::FileSessionBackend;
	/// use std::path::PathBuf;
	///
	/// # fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// // Use default directory
	/// let backend1 = FileSessionBackend::new(None)?;
	///
	/// // Use custom directory
	/// let backend2 = FileSessionBackend::new(Some(PathBuf::from("/tmp/custom_sessions")))?;
	/// # Ok(())
	/// # }
	/// ```
	pub fn new(session_dir: Option<PathBuf>) -> Result<Self, SessionError> {
		let session_dir = session_dir.unwrap_or_else(|| PathBuf::from(DEFAULT_SESSION_DIR));

		// Create directory if it doesn't exist
		fs::create_dir_all(&session_dir).map_err(|e| {
			SessionError::CacheError(format!("Failed to create session directory: {}", e))
		})?;

		Ok(Self { session_dir })
	}

	/// Get the file path for a session key
	///
	/// Validates the session key to prevent path traversal attacks,
	/// since session IDs originate from user-controlled cookies.
	fn session_file_path(&self, session_key: &str) -> Result<PathBuf, SessionError> {
		// Validate session key contains only safe characters (alphanumeric, hyphen, underscore)
		if !session_key
			.chars()
			.all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
		{
			return Err(SessionError::CacheError(
				"Invalid session key: contains unsafe characters".to_string(),
			));
		}
		if session_key.is_empty() {
			return Err(SessionError::CacheError(
				"Invalid session key: empty".to_string(),
			));
		}
		Ok(self
			.session_dir
			.join(format!("session_{}.json", session_key)))
	}

	/// Check if a session file has expired based on TTL
	fn is_expired(&self, file_path: &Path) -> bool {
		if let Ok(metadata) = fs::metadata(file_path)
			&& let Ok(modified) = metadata.modified()
			&& let Ok(duration) = SystemTime::now().duration_since(modified)
		{
			// Read the stored TTL from the file
			if let Ok(mut file) = File::open(file_path) {
				let _ = file.lock_shared();
				let mut contents = String::new();
				if file.read_to_string(&mut contents).is_ok()
					&& let Ok(stored_data) = serde_json::from_str::<StoredSession>(&contents)
					&& let Some(ttl) = stored_data.ttl
				{
					return duration.as_secs() > ttl;
				}
				let _ = file.unlock();
			}
		}
		false
	}
}

/// Internal structure for storing session data with TTL
#[derive(Debug, Serialize, Deserialize)]
struct StoredSession {
	data: serde_json::Value,
	ttl: Option<u64>,
}

#[async_trait]
impl SessionBackend for FileSessionBackend {
	async fn load<T>(&self, session_key: &str) -> Result<Option<T>, SessionError>
	where
		T: for<'de> Deserialize<'de> + Send,
	{
		let file_path = self.session_file_path(session_key)?;

		if !file_path.exists() {
			return Ok(None);
		}

		// Check if session has expired
		if self.is_expired(&file_path) {
			// Delete expired session
			let _ = fs::remove_file(&file_path);
			return Ok(None);
		}

		// Open file with shared lock for reading
		let mut file = File::open(&file_path)
			.map_err(|e| SessionError::CacheError(format!("Failed to open session file: {}", e)))?;

		file.lock_shared()
			.map_err(|e| SessionError::CacheError(format!("Failed to lock session file: {}", e)))?;

		let mut contents = String::new();
		let read_result = file.read_to_string(&mut contents);

		// Always unlock before handling errors
		let _ = file.unlock();

		read_result
			.map_err(|e| SessionError::CacheError(format!("Failed to read session file: {}", e)))?;

		let stored_session: StoredSession = serde_json::from_str(&contents).map_err(|e| {
			SessionError::SerializationError(format!("Failed to deserialize session: {}", e))
		})?;

		let data: T = serde_json::from_value(stored_session.data).map_err(|e| {
			SessionError::SerializationError(format!("Failed to deserialize session data: {}", e))
		})?;

		Ok(Some(data))
	}

	async fn save<T>(
		&self,
		session_key: &str,
		data: &T,
		ttl: Option<u64>,
	) -> Result<(), SessionError>
	where
		T: Serialize + Send + Sync,
	{
		let file_path = self.session_file_path(session_key)?;

		// Serialize data to JSON value
		let json_value = serde_json::to_value(data).map_err(|e| {
			SessionError::SerializationError(format!("Failed to serialize session data: {}", e))
		})?;

		let stored_session = StoredSession {
			data: json_value,
			ttl,
		};

		let json_string = serde_json::to_string_pretty(&stored_session).map_err(|e| {
			SessionError::SerializationError(format!("Failed to serialize stored session: {}", e))
		})?;

		// Open or create file with exclusive lock for writing
		let mut file = OpenOptions::new()
			.write(true)
			.create(true)
			.truncate(true)
			.open(&file_path)
			.map_err(|e| {
				SessionError::CacheError(format!("Failed to create session file: {}", e))
			})?;

		file.lock_exclusive()
			.map_err(|e| SessionError::CacheError(format!("Failed to lock session file: {}", e)))?;

		let write_result = file.write_all(json_string.as_bytes());

		// Always unlock before handling errors
		let _ = file.unlock();

		write_result.map_err(|e| {
			SessionError::CacheError(format!("Failed to write session file: {}", e))
		})?;

		Ok(())
	}

	async fn delete(&self, session_key: &str) -> Result<(), SessionError> {
		let file_path = self.session_file_path(session_key)?;

		if file_path.exists() {
			fs::remove_file(&file_path).map_err(|e| {
				SessionError::CacheError(format!("Failed to delete session file: {}", e))
			})?;
		}

		Ok(())
	}

	async fn exists(&self, session_key: &str) -> Result<bool, SessionError> {
		let file_path = self.session_file_path(session_key)?;

		if !file_path.exists() {
			return Ok(false);
		}

		// Check if session has expired
		if self.is_expired(&file_path) {
			// Delete expired session
			let _ = fs::remove_file(&file_path);
			return Ok(false);
		}

		Ok(true)
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use serde_json::json;
	use std::sync::Arc;
	use std::time::Duration;
	use tokio::time::sleep;

	/// Helper to create a temporary test directory
	fn create_test_dir() -> PathBuf {
		let test_dir = PathBuf::from(format!("/tmp/reinhardt_test_{}", uuid::Uuid::new_v4()));
		fs::create_dir_all(&test_dir).expect("Failed to create test directory");
		test_dir
	}

	/// Helper to cleanup test directory
	fn cleanup_test_dir(dir: &Path) {
		if dir.exists() {
			fs::remove_dir_all(dir).expect("Failed to cleanup test directory");
		}
	}

	/// RAII guard for automatic test directory cleanup
	struct TestDirGuard {
		path: PathBuf,
	}

	impl TestDirGuard {
		fn new() -> Self {
			Self {
				path: create_test_dir(),
			}
		}

		fn path(&self) -> &Path {
			&self.path
		}
	}

	impl Drop for TestDirGuard {
		fn drop(&mut self) {
			cleanup_test_dir(&self.path);
		}
	}

	#[tokio::test]
	async fn test_file_backend_save_and_load() {
		let _guard = TestDirGuard::new();
		let backend = FileSessionBackend::new(Some(_guard.path().to_path_buf()))
			.expect("Failed to create backend");

		let session_data = json!({
			"user_id": 42,
			"username": "test_user",
			"is_authenticated": true,
		});

		backend
			.save("test_session_1", &session_data, None)
			.await
			.expect("Failed to save session");

		let loaded: Option<serde_json::Value> = backend
			.load("test_session_1")
			.await
			.expect("Failed to load session");

		assert_eq!(loaded, Some(session_data));
	}

	#[tokio::test]
	async fn test_file_backend_delete() {
		let _guard = TestDirGuard::new();
		let backend = FileSessionBackend::new(Some(_guard.path().to_path_buf()))
			.expect("Failed to create backend");

		let session_data = json!({ "data": "value" });

		backend
			.save("test_session_2", &session_data, None)
			.await
			.expect("Failed to save session");

		assert!(
			backend
				.exists("test_session_2")
				.await
				.expect("Failed to check existence")
		);

		backend
			.delete("test_session_2")
			.await
			.expect("Failed to delete session");

		assert!(
			!backend
				.exists("test_session_2")
				.await
				.expect("Failed to check existence")
		);
	}

	#[tokio::test]
	async fn test_file_backend_exists() {
		let _guard = TestDirGuard::new();
		let backend = FileSessionBackend::new(Some(_guard.path().to_path_buf()))
			.expect("Failed to create backend");

		assert!(
			!backend
				.exists("nonexistent")
				.await
				.expect("Failed to check existence")
		);

		let session_data = json!({ "key": "value" });
		backend
			.save("test_session_3", &session_data, None)
			.await
			.expect("Failed to save session");

		assert!(
			backend
				.exists("test_session_3")
				.await
				.expect("Failed to check existence")
		);
	}

	#[tokio::test]
	async fn test_file_backend_ttl_expiration() {
		let _guard = TestDirGuard::new();
		let backend = FileSessionBackend::new(Some(_guard.path().to_path_buf()))
			.expect("Failed to create backend");

		let session_data = json!({ "expires": "soon" });

		// Save session with 1 second TTL
		backend
			.save("test_session_4", &session_data, Some(1))
			.await
			.expect("Failed to save session");

		// Session should exist immediately
		assert!(
			backend
				.exists("test_session_4")
				.await
				.expect("Failed to check existence")
		);

		// Wait for expiration
		sleep(Duration::from_secs(2)).await;

		// Session should be expired and removed
		assert!(
			!backend
				.exists("test_session_4")
				.await
				.expect("Failed to check existence")
		);

		// Loading should return None
		let loaded: Option<serde_json::Value> = backend
			.load("test_session_4")
			.await
			.expect("Failed to load session");

		assert_eq!(loaded, None);
	}

	#[tokio::test]
	async fn test_file_backend_concurrent_access() {
		let _guard = TestDirGuard::new();
		let backend = Arc::new(
			FileSessionBackend::new(Some(_guard.path().to_path_buf()))
				.expect("Failed to create backend"),
		);

		let session_key = "concurrent_session";

		// Spawn multiple tasks writing to the same session
		let mut handles = vec![];
		for i in 0..10 {
			let backend_clone = backend.clone();
			let handle = tokio::spawn(async move {
				let data = json!({ "counter": i });
				backend_clone
					.save(session_key, &data, None)
					.await
					.expect("Failed to save session");
			});
			handles.push(handle);
		}

		// Wait for all tasks to complete
		for handle in handles {
			handle.await.expect("Task panicked");
		}

		// Verify session exists and has valid data
		let loaded: Option<serde_json::Value> = backend
			.load(session_key)
			.await
			.expect("Failed to load session");

		assert!(loaded.is_some());
		assert!(loaded.unwrap()["counter"].is_number());
	}

	#[tokio::test]
	async fn test_file_backend_overwrite() {
		let _guard = TestDirGuard::new();
		let backend = FileSessionBackend::new(Some(_guard.path().to_path_buf()))
			.expect("Failed to create backend");

		let session_key = "overwrite_session";

		// Save initial data
		let data1 = json!({ "version": 1 });
		backend
			.save(session_key, &data1, None)
			.await
			.expect("Failed to save session");

		// Overwrite with new data
		let data2 = json!({ "version": 2 });
		backend
			.save(session_key, &data2, None)
			.await
			.expect("Failed to save session");

		// Verify new data is loaded
		let loaded: Option<serde_json::Value> = backend
			.load(session_key)
			.await
			.expect("Failed to load session");

		assert_eq!(loaded, Some(data2));
	}

	#[tokio::test]
	async fn test_file_backend_default_directory() {
		let backend = FileSessionBackend::new(None).expect("Failed to create backend");

		let session_data = json!({ "test": "default_dir" });
		let session_key = format!("test_default_{}", uuid::Uuid::new_v4());

		backend
			.save(&session_key, &session_data, None)
			.await
			.expect("Failed to save session");

		let loaded: Option<serde_json::Value> = backend
			.load(&session_key)
			.await
			.expect("Failed to load session");

		assert_eq!(loaded, Some(session_data));

		// Cleanup
		backend
			.delete(&session_key)
			.await
			.expect("Failed to delete session");
	}

	#[tokio::test]
	async fn test_file_backend_nonexistent_load() {
		let _guard = TestDirGuard::new();
		let backend = FileSessionBackend::new(Some(_guard.path().to_path_buf()))
			.expect("Failed to create backend");

		let loaded: Option<serde_json::Value> = backend
			.load("nonexistent_session")
			.await
			.expect("Failed to load session");

		assert_eq!(loaded, None);
	}

	#[tokio::test]
	async fn test_file_backend_delete_nonexistent() {
		let _guard = TestDirGuard::new();
		let backend = FileSessionBackend::new(Some(_guard.path().to_path_buf()))
			.expect("Failed to create backend");

		// Deleting non-existent session should not error
		backend
			.delete("nonexistent_session")
			.await
			.expect("Failed to delete session");
	}

	// =================================================================
	// Path traversal prevention tests (Issue #325)
	// =================================================================

	#[rstest::rstest]
	#[case("../../../etc/passwd")]
	#[case("..%2F..%2Fetc%2Fpasswd")]
	#[case("foo/../../bar")]
	#[case("/etc/passwd")]
	#[case("session/../../../etc/shadow")]
	#[tokio::test]
	async fn test_file_backend_rejects_path_traversal_in_load(#[case] malicious_key: &str) {
		// Arrange
		let _guard = TestDirGuard::new();
		let backend = FileSessionBackend::new(Some(_guard.path().to_path_buf()))
			.expect("Failed to create backend");

		// Act
		let result: Result<Option<serde_json::Value>, _> = backend.load(malicious_key).await;

		// Assert
		assert!(result.is_err());
	}

	#[rstest::rstest]
	#[case("../../../etc/passwd")]
	#[case("foo/../bar")]
	#[case("/absolute/path")]
	#[tokio::test]
	async fn test_file_backend_rejects_path_traversal_in_save(#[case] malicious_key: &str) {
		// Arrange
		let _guard = TestDirGuard::new();
		let backend = FileSessionBackend::new(Some(_guard.path().to_path_buf()))
			.expect("Failed to create backend");
		let data = serde_json::json!({"malicious": true});

		// Act
		let result = backend.save(malicious_key, &data, None).await;

		// Assert
		assert!(result.is_err());
	}

	#[rstest::rstest]
	#[case("valid_session_key")]
	#[case("session-with-hyphens")]
	#[case("session123")]
	#[tokio::test]
	async fn test_file_backend_allows_valid_session_keys(#[case] valid_key: &str) {
		// Arrange
		let _guard = TestDirGuard::new();
		let backend = FileSessionBackend::new(Some(_guard.path().to_path_buf()))
			.expect("Failed to create backend");
		let data = serde_json::json!({"valid": true});

		// Act
		let save_result = backend.save(valid_key, &data, None).await;

		// Assert
		assert!(save_result.is_ok());
	}
}
