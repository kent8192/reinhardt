//! Chunked upload handling for large files
//!
//! This module provides functionality for handling large file uploads
//! by splitting them into manageable chunks, supporting resumable uploads,
//! and assembling chunks back into complete files.

use percent_encoding::percent_decode_str;
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{self, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Default session timeout for chunked uploads (1 hour).
/// Sessions older than this are considered stale and eligible for cleanup.
const UPLOAD_SESSION_TIMEOUT: Duration = Duration::from_secs(3600);

/// Errors that can occur during chunked upload
#[non_exhaustive]
#[derive(Debug, thiserror::Error)]
pub enum ChunkedUploadError {
	#[error("IO error: {0}")]
	Io(#[from] io::Error),
	#[error("Upload session not found: {0}")]
	SessionNotFound(String),
	#[error("Invalid chunk: expected {expected}, got {actual}")]
	InvalidChunk { expected: usize, actual: usize },
	#[error("Chunk out of order: expected {expected}, got {actual}")]
	ChunkOutOfOrder { expected: usize, actual: usize },
	#[error("Upload already completed")]
	AlreadyCompleted,
	#[error("Checksum mismatch")]
	ChecksumMismatch,
}

/// Upload progress information
#[derive(Debug, Clone)]
pub struct UploadProgress {
	/// Current number of bytes uploaded
	pub bytes_uploaded: usize,
	/// Total file size in bytes
	pub total_bytes: usize,
	/// Progress percentage (0.0 - 100.0)
	pub percentage: f64,
	/// Number of chunks uploaded
	pub chunks_uploaded: usize,
	/// Total number of chunks
	pub total_chunks: usize,
	/// Upload start time
	pub started_at: Instant,
	/// Estimated time remaining in seconds
	pub estimated_time_remaining: Option<f64>,
	/// Upload speed in bytes per second
	pub upload_speed: f64,
}

impl UploadProgress {
	/// Create a new upload progress tracker
	fn new(total_bytes: usize, total_chunks: usize) -> Self {
		Self {
			bytes_uploaded: 0,
			total_bytes,
			percentage: 0.0,
			chunks_uploaded: 0,
			total_chunks,
			started_at: Instant::now(),
			estimated_time_remaining: None,
			upload_speed: 0.0,
		}
	}

	/// Update progress with new chunk
	fn update(&mut self, chunk_size: usize) {
		self.chunks_uploaded += 1;
		self.bytes_uploaded += chunk_size;

		// On the final chunk, update total_bytes to reflect actual cumulative
		// byte count, which may differ from the declared total_size if the
		// last chunk is smaller than chunk_size.
		if self.chunks_uploaded >= self.total_chunks {
			self.total_bytes = self.bytes_uploaded;
		}

		self.percentage = if self.total_bytes > 0 {
			(self.bytes_uploaded as f64 / self.total_bytes as f64) * 100.0
		} else {
			0.0
		};

		// Calculate upload speed
		let elapsed = self.started_at.elapsed().as_secs_f64();
		if elapsed > 0.0 {
			self.upload_speed = self.bytes_uploaded as f64 / elapsed;

			// Estimate time remaining
			let bytes_remaining = self.total_bytes.saturating_sub(self.bytes_uploaded);
			if self.upload_speed > 0.0 {
				self.estimated_time_remaining = Some(bytes_remaining as f64 / self.upload_speed);
			}
		}
	}

	/// Check if upload is complete
	pub fn is_complete(&self) -> bool {
		self.chunks_uploaded >= self.total_chunks
	}

	/// Get formatted upload speed (e.g., "1.5 MB/s")
	pub fn formatted_speed(&self) -> String {
		if self.upload_speed < 1024.0 {
			format!("{:.2} B/s", self.upload_speed)
		} else if self.upload_speed < 1024.0 * 1024.0 {
			format!("{:.2} KB/s", self.upload_speed / 1024.0)
		} else {
			format!("{:.2} MB/s", self.upload_speed / (1024.0 * 1024.0))
		}
	}

	/// Get formatted estimated time remaining (e.g., "2m 30s")
	pub fn formatted_eta(&self) -> String {
		match self.estimated_time_remaining {
			Some(seconds) => {
				let mins = (seconds / 60.0) as u64;
				let secs = (seconds % 60.0) as u64;
				if mins > 0 {
					format!("{}m {}s", mins, secs)
				} else {
					format!("{}s", secs)
				}
			}
			None => "Unknown".to_string(),
		}
	}
}

/// Metadata for a chunked upload session
#[derive(Debug, Clone)]
pub struct ChunkedUploadSession {
	/// Unique session ID
	pub session_id: String,
	/// Original filename
	pub filename: String,
	/// Total file size in bytes
	pub total_size: usize,
	/// Chunk size in bytes
	pub chunk_size: usize,
	/// Total number of chunks
	pub total_chunks: usize,
	/// Number of chunks received so far
	pub received_chunks: usize,
	/// Temporary directory for chunks
	pub temp_dir: PathBuf,
	/// Whether the upload is complete
	pub completed: bool,
	/// Upload progress tracker
	progress: UploadProgress,
	/// When the session was created, used for timeout-based cleanup
	created_at: Instant,
}

impl ChunkedUploadSession {
	/// Create a new upload session
	///
	/// Returns `ChunkedUploadError::InvalidChunk` if `chunk_size` is zero.
	pub fn new(
		session_id: String,
		filename: String,
		total_size: usize,
		chunk_size: usize,
		temp_dir: PathBuf,
	) -> Result<Self, ChunkedUploadError> {
		if chunk_size == 0 {
			return Err(ChunkedUploadError::InvalidChunk {
				expected: 1,
				actual: 0,
			});
		}
		let total_chunks = total_size.div_ceil(chunk_size);
		Ok(Self {
			session_id,
			filename,
			total_size,
			chunk_size,
			total_chunks,
			received_chunks: 0,
			temp_dir,
			completed: false,
			progress: UploadProgress::new(total_size, total_chunks),
			created_at: Instant::now(),
		})
	}

	/// Get progress percentage
	pub fn progress(&self) -> f64 {
		self.progress.percentage
	}

	/// Get detailed upload progress
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_http::chunked_upload::ChunkedUploadSession;
	/// use std::path::PathBuf;
	///
	/// let session = ChunkedUploadSession::new(
	///     "session1".to_string(),
	///     "file.bin".to_string(),
	///     1000,
	///     100,
	///     PathBuf::from("/tmp")
	/// ).unwrap();
	/// let progress = session.get_progress();
	/// assert_eq!(progress.percentage, 0.0);
	/// ```
	pub fn get_progress(&self) -> &UploadProgress {
		&self.progress
	}

	/// Update progress with a new chunk
	///
	/// This is primarily used internally but exposed for testing purposes.
	#[doc(hidden)]
	pub fn update_progress(&mut self, chunk_size: usize) {
		self.progress.update(chunk_size);
	}

	/// Check if upload is complete
	pub fn is_complete(&self) -> bool {
		self.completed || self.received_chunks >= self.total_chunks
	}

	/// Get the path for a specific chunk
	///
	/// Uses the pre-validated session_id (validated during session creation)
	/// combined with the numeric chunk_number, both safe for path construction.
	fn chunk_path(&self, chunk_number: usize) -> PathBuf {
		self.temp_dir
			.join(format!("{}_{}.chunk", self.session_id, chunk_number))
	}
}

/// Manager for chunked uploads
pub struct ChunkedUploadManager {
	sessions: Arc<Mutex<HashMap<String, ChunkedUploadSession>>>,
	temp_base_dir: PathBuf,
}

impl ChunkedUploadManager {
	/// Create a new chunked upload manager
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_http::chunked_upload::ChunkedUploadManager;
	/// use std::path::PathBuf;
	///
	/// let manager = ChunkedUploadManager::new(PathBuf::from("/tmp/chunked_uploads"));
	/// ```
	pub fn new(temp_base_dir: PathBuf) -> Self {
		Self {
			sessions: Arc::new(Mutex::new(HashMap::new())),
			temp_base_dir,
		}
	}

	/// Start a new upload session
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_http::chunked_upload::ChunkedUploadManager;
	/// use std::path::PathBuf;
	///
	/// let manager = ChunkedUploadManager::new(PathBuf::from("/tmp/chunked_uploads"));
	/// let session = manager.start_session(
	///     "session123".to_string(),
	///     "large_file.bin".to_string(),
	///     10_000_000, // 10MB
	///     1_000_000,  // 1MB chunks
	/// ).unwrap();
	/// assert_eq!(session.total_chunks, 10);
	/// ```
	pub fn start_session(
		&self,
		session_id: String,
		filename: String,
		total_size: usize,
		chunk_size: usize,
	) -> Result<ChunkedUploadSession, ChunkedUploadError> {
		// Lazily clean up expired sessions to prevent memory exhaustion
		self.cleanup_expired_sessions();

		// Validate session_id to prevent path traversal attacks.
		// Session IDs are used to construct directory and file paths.
		// Check both raw and URL-decoded forms to prevent bypass via
		// percent-encoded traversal sequences like %2e%2e%2f.
		let decoded = percent_decode_str(&session_id).decode_utf8_lossy();
		for candidate in [session_id.as_str(), decoded.as_ref()] {
			if candidate.is_empty()
				|| candidate.contains('/')
				|| candidate.contains('\\')
				|| candidate.contains('\0')
				|| candidate.contains("..")
			{
				return Err(ChunkedUploadError::SessionNotFound(
					"Invalid session ID".to_string(),
				));
			}
		}
		let temp_dir = self.temp_base_dir.join(&session_id);
		fs::create_dir_all(&temp_dir)?;

		let session = ChunkedUploadSession::new(
			session_id.clone(),
			filename,
			total_size,
			chunk_size,
			temp_dir,
		)?;

		let mut sessions = self.sessions.lock().unwrap_or_else(|e| e.into_inner());
		sessions.insert(session_id, session.clone());

		Ok(session)
	}

	/// Upload a chunk
	///
	/// # Examples
	///
	/// ```no_run
	/// use reinhardt_http::chunked_upload::ChunkedUploadManager;
	/// use std::path::PathBuf;
	///
	/// let manager = ChunkedUploadManager::new(PathBuf::from("/tmp/chunked_uploads"));
	/// manager.start_session("session123".to_string(), "file.bin".to_string(), 1000, 100).unwrap();
	///
	/// let chunk_data = vec![0u8; 100];
	/// manager.upload_chunk("session123", 0, &chunk_data).unwrap();
	/// ```
	pub fn upload_chunk(
		&self,
		session_id: &str,
		chunk_number: usize,
		data: &[u8],
	) -> Result<ChunkedUploadSession, ChunkedUploadError> {
		// Lazily clean up expired sessions to prevent memory exhaustion
		self.cleanup_expired_sessions();

		let mut sessions = self.sessions.lock().unwrap_or_else(|e| e.into_inner());
		let session = sessions
			.get_mut(session_id)
			.ok_or_else(|| ChunkedUploadError::SessionNotFound(session_id.to_string()))?;

		if session.completed {
			return Err(ChunkedUploadError::AlreadyCompleted);
		}

		// Validate chunk number
		if chunk_number >= session.total_chunks {
			return Err(ChunkedUploadError::InvalidChunk {
				expected: session.total_chunks - 1,
				actual: chunk_number,
			});
		}

		// Write chunk to disk
		let chunk_path = session.chunk_path(chunk_number);
		let mut file = File::create(chunk_path)?;
		file.write_all(data)?;

		session.received_chunks += 1;
		session.update_progress(data.len());

		if session.is_complete() {
			session.completed = true;
		}

		Ok(session.clone())
	}

	/// Assemble all chunks into final file
	///
	/// # Examples
	///
	/// ```no_run
	/// use reinhardt_http::chunked_upload::ChunkedUploadManager;
	/// use std::path::PathBuf;
	///
	/// let manager = ChunkedUploadManager::new(PathBuf::from("/tmp/chunked_uploads"));
	/// let output_path = manager.assemble_chunks("session123", PathBuf::from("/tmp/final_file.bin")).unwrap();
	/// ```
	pub fn assemble_chunks(
		&self,
		session_id: &str,
		output_path: PathBuf,
	) -> Result<PathBuf, ChunkedUploadError> {
		let sessions = self.sessions.lock().unwrap_or_else(|e| e.into_inner());
		let session = sessions
			.get(session_id)
			.ok_or_else(|| ChunkedUploadError::SessionNotFound(session_id.to_string()))?;

		if !session.is_complete() {
			return Err(ChunkedUploadError::InvalidChunk {
				expected: session.total_chunks,
				actual: session.received_chunks,
			});
		}

		// Create output file
		let mut output_file = File::create(&output_path)?;

		// Assemble chunks in order
		for i in 0..session.total_chunks {
			let chunk_path = session.chunk_path(i);
			let chunk_data = fs::read(&chunk_path)?;
			output_file.write_all(&chunk_data)?;
		}

		Ok(output_path)
	}

	/// Clean up a session (delete temporary files)
	///
	/// # Examples
	///
	/// ```no_run
	/// use reinhardt_http::chunked_upload::ChunkedUploadManager;
	/// use std::path::PathBuf;
	///
	/// let manager = ChunkedUploadManager::new(PathBuf::from("/tmp/chunked_uploads"));
	/// manager.cleanup_session("session123").unwrap();
	/// ```
	pub fn cleanup_session(&self, session_id: &str) -> Result<(), ChunkedUploadError> {
		let mut sessions = self.sessions.lock().unwrap_or_else(|e| e.into_inner());
		if let Some(session) = sessions.remove(session_id)
			&& session.temp_dir.exists()
		{
			fs::remove_dir_all(session.temp_dir)?;
		}
		Ok(())
	}

	/// Get session information
	pub fn get_session(&self, session_id: &str) -> Option<ChunkedUploadSession> {
		let sessions = self.sessions.lock().unwrap_or_else(|e| e.into_inner());
		sessions.get(session_id).cloned()
	}

	/// List all active sessions
	pub fn list_sessions(&self) -> Vec<ChunkedUploadSession> {
		let sessions = self.sessions.lock().unwrap_or_else(|e| e.into_inner());
		sessions.values().cloned().collect()
	}

	/// Remove sessions that have exceeded the timeout.
	///
	/// Expired sessions have their temporary files cleaned up automatically.
	/// This is called lazily on `start_session` and `upload_chunk` to prevent
	/// unbounded memory growth from abandoned uploads.
	pub fn cleanup_expired_sessions(&self) {
		let mut sessions = self.sessions.lock().unwrap_or_else(|e| e.into_inner());
		sessions.retain(|_, session| {
			let expired = session.created_at.elapsed() >= UPLOAD_SESSION_TIMEOUT;
			if expired && session.temp_dir.exists() {
				let _ = fs::remove_dir_all(&session.temp_dir);
			}
			!expired
		});
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_session_creation() {
		let session = ChunkedUploadSession::new(
			"test123".to_string(),
			"file.bin".to_string(),
			1000,
			100,
			PathBuf::from("/tmp"),
		)
		.unwrap();

		assert_eq!(session.session_id, "test123");
		assert_eq!(session.filename, "file.bin");
		assert_eq!(session.total_size, 1000);
		assert_eq!(session.chunk_size, 100);
		assert_eq!(session.total_chunks, 10);
		assert_eq!(session.received_chunks, 0);
		assert!(!session.completed);
	}

	#[test]
	fn test_session_progress() {
		let mut session = ChunkedUploadSession::new(
			"test123".to_string(),
			"file.bin".to_string(),
			1000,
			100,
			PathBuf::from("/tmp"),
		)
		.unwrap();

		assert_eq!(session.progress(), 0.0);

		// Update progress by simulating chunk uploads
		for _ in 0..5 {
			session.update_progress(100);
		}
		assert_eq!(session.progress(), 50.0);

		for _ in 0..5 {
			session.update_progress(100);
		}
		assert_eq!(session.progress(), 100.0);
	}

	#[test]
	fn test_manager_creation() {
		let manager = ChunkedUploadManager::new(PathBuf::from("/tmp/test_chunks"));
		assert_eq!(manager.list_sessions().len(), 0);
	}

	#[test]
	fn test_start_session() {
		let manager = ChunkedUploadManager::new(PathBuf::from("/tmp/test_chunks"));
		let session = manager
			.start_session("session1".to_string(), "file.bin".to_string(), 1000, 100)
			.unwrap();

		assert_eq!(session.session_id, "session1");
		assert_eq!(session.total_chunks, 10);
		assert_eq!(manager.list_sessions().len(), 1);
	}

	#[test]
	fn test_upload_chunk() {
		let temp_dir = PathBuf::from("/tmp/test_chunks_upload");
		let manager = ChunkedUploadManager::new(temp_dir.clone());

		manager
			.start_session("session2".to_string(), "file.bin".to_string(), 300, 100)
			.unwrap();

		let chunk_data = vec![0u8; 100];
		let result = manager.upload_chunk("session2", 0, &chunk_data);
		assert!(result.is_ok());

		let session = manager.get_session("session2").unwrap();
		assert_eq!(session.received_chunks, 1);

		manager.cleanup_session("session2").unwrap();
	}

	#[test]
	fn test_invalid_session() {
		let manager = ChunkedUploadManager::new(PathBuf::from("/tmp/test_chunks"));
		let chunk_data = vec![0u8; 100];
		let result = manager.upload_chunk("nonexistent", 0, &chunk_data);

		assert!(result.is_err());
		if let Err(ChunkedUploadError::SessionNotFound(id)) = result {
			assert_eq!(id, "nonexistent");
		} else {
			panic!("Expected SessionNotFound error");
		}
	}

	#[test]
	fn test_chunk_assembly() {
		let temp_dir = PathBuf::from("/tmp/test_chunks_assembly");
		let manager = ChunkedUploadManager::new(temp_dir.clone());

		manager
			.start_session("session3".to_string(), "file.bin".to_string(), 300, 100)
			.unwrap();

		// Upload 3 chunks
		for i in 0..3 {
			let chunk_data = vec![i as u8; 100];
			manager.upload_chunk("session3", i, &chunk_data).unwrap();
		}

		let output_path = temp_dir.join("assembled.bin");
		let result = manager.assemble_chunks("session3", output_path.clone());
		assert!(result.is_ok());

		assert!(output_path.exists());
		let content = fs::read(&output_path).unwrap();
		assert_eq!(content.len(), 300);

		// Cleanup
		fs::remove_file(output_path).unwrap();
		manager.cleanup_session("session3").unwrap();
	}

	#[test]
	fn test_session_completion() {
		let temp_dir = PathBuf::from("/tmp/test_chunks_completion");
		let manager = ChunkedUploadManager::new(temp_dir.clone());

		manager
			.start_session("session4".to_string(), "file.bin".to_string(), 200, 100)
			.unwrap();

		let chunk_data = vec![0u8; 100];

		manager.upload_chunk("session4", 0, &chunk_data).unwrap();
		let session = manager.get_session("session4").unwrap();
		assert!(!session.is_complete());

		manager.upload_chunk("session4", 1, &chunk_data).unwrap();
		let session = manager.get_session("session4").unwrap();
		assert!(session.is_complete());

		manager.cleanup_session("session4").unwrap();
	}

	// =================================================================
	// Path traversal prevention tests (Issue #355)
	// =================================================================

	#[rstest::rstest]
	#[case("../../../etc")]
	#[case("foo/bar")]
	#[case("foo\\bar")]
	#[case("null\0byte")]
	#[case("..")]
	#[case("..%2f..%2fetc")]
	#[case("%2e%2e%2f%2e%2e%2f")]
	#[case("..%2fmalicious")]
	fn test_start_session_rejects_traversal_in_session_id(#[case] session_id: &str) {
		// Arrange
		let manager = ChunkedUploadManager::new(PathBuf::from("/tmp/test_chunks_security"));

		// Act
		let result =
			manager.start_session(session_id.to_string(), "file.bin".to_string(), 1000, 100);

		// Assert
		assert!(
			result.is_err(),
			"Expected error for session_id: {}",
			session_id
		);
	}

	#[rstest::rstest]
	fn test_start_session_allows_safe_session_id() {
		// Arrange
		let manager = ChunkedUploadManager::new(PathBuf::from("/tmp/test_chunks_safe"));

		// Act
		let result = manager.start_session(
			"safe-session_123".to_string(),
			"file.bin".to_string(),
			1000,
			100,
		);

		// Assert
		assert!(result.is_ok());
		manager.cleanup_session("safe-session_123").unwrap();
	}

	// =================================================================
	// Division by zero prevention tests (Issue #359)
	// =================================================================

	#[rstest::rstest]
	fn test_chunked_upload_session_rejects_zero_chunk_size() {
		// Arrange
		let session_id = "test-zero".to_string();
		let filename = "file.bin".to_string();
		let total_size = 1000;
		let chunk_size = 0;

		// Act
		let result = ChunkedUploadSession::new(
			session_id,
			filename,
			total_size,
			chunk_size,
			PathBuf::from("/tmp"),
		);

		// Assert
		assert!(result.is_err());
		if let Err(ChunkedUploadError::InvalidChunk { expected, actual }) = result {
			assert_eq!(expected, 1);
			assert_eq!(actual, 0);
		} else {
			panic!("Expected InvalidChunk error for zero chunk_size");
		}
	}

	#[rstest::rstest]
	fn test_start_session_rejects_zero_chunk_size() {
		// Arrange
		let manager = ChunkedUploadManager::new(PathBuf::from("/tmp/test_chunks_zero"));

		// Act
		let result =
			manager.start_session("session-zero".to_string(), "file.bin".to_string(), 1000, 0);

		// Assert
		assert!(result.is_err());
	}
}
