//! File upload handling functionality
//!
//! This module provides file upload processing including handlers,
//! temporary file management, and memory-based uploads.

use percent_encoding::percent_decode_str;
use sha2::{Digest, Sha256};
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

/// Errors that can occur during file upload operations
#[non_exhaustive]
#[derive(Debug, thiserror::Error)]
pub enum FileUploadError {
	#[error("File too large: {0} bytes (max: {1} bytes)")]
	FileTooLarge(usize, usize),
	#[error("Invalid file type: {0}")]
	InvalidFileType(String),
	#[error("IO error: {0}")]
	Io(#[from] io::Error),
	#[error("Upload error: {0}")]
	Upload(String),
	#[error("Checksum verification failed")]
	ChecksumMismatch,
	#[error("MIME type detection failed")]
	MimeDetectionFailed,
	#[error("Path traversal detected in filename")]
	PathTraversal,
}

/// Validate that a filename does not contain path traversal sequences
/// or other unsafe characters that could escape the upload directory.
///
/// Checks both raw and URL-decoded forms of the filename to prevent
/// bypasses via percent-encoding (e.g. `%2e%2e%2f`).
pub fn validate_safe_filename(filename: &str) -> Result<(), FileUploadError> {
	if filename.is_empty() {
		return Err(FileUploadError::Upload("Empty filename".to_string()));
	}

	// Check both the raw filename and its URL-decoded form to prevent
	// bypass via percent-encoded traversal sequences like %2e%2e%2f
	let decoded = percent_decode_str(filename).decode_utf8_lossy();
	for candidate in [filename, decoded.as_ref()] {
		if candidate.contains('\0') {
			return Err(FileUploadError::PathTraversal);
		}
		if candidate.contains("..") {
			return Err(FileUploadError::PathTraversal);
		}
		if candidate.contains('/') || candidate.contains('\\') {
			return Err(FileUploadError::PathTraversal);
		}
		// Reject absolute paths (Unix and Windows)
		if candidate.starts_with('/') || candidate.starts_with('\\') {
			return Err(FileUploadError::PathTraversal);
		}
		if candidate.len() >= 2
			&& candidate.as_bytes()[0].is_ascii_alphabetic()
			&& candidate.as_bytes()[1] == b':'
		{
			return Err(FileUploadError::PathTraversal);
		}
	}
	Ok(())
}

/// FileUploadHandler processes file uploads
///
/// Handles file upload operations including validation, storage,
/// and cleanup of temporary files.
pub struct FileUploadHandler {
	upload_dir: PathBuf,
	max_size: usize,
	allowed_extensions: Option<Vec<String>>,
	verify_checksum: bool,
	allowed_mime_types: Option<Vec<String>>,
}

impl FileUploadHandler {
	/// Create a new FileUploadHandler
	///
	/// # Arguments
	///
	/// * `upload_dir` - Directory where uploaded files will be stored
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_http::upload::FileUploadHandler;
	/// use std::path::PathBuf;
	///
	/// let handler = FileUploadHandler::new(PathBuf::from("/tmp/uploads"));
	/// assert_eq!(handler.max_size(), 10 * 1024 * 1024); // 10MB default
	/// ```
	pub fn new(upload_dir: PathBuf) -> Self {
		Self {
			upload_dir,
			max_size: 10 * 1024 * 1024, // 10MB default
			allowed_extensions: None,
			verify_checksum: false,
			allowed_mime_types: None,
		}
	}

	/// Set maximum file size
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_http::upload::FileUploadHandler;
	/// use std::path::PathBuf;
	///
	/// let handler = FileUploadHandler::new(PathBuf::from("/tmp/uploads"))
	///     .with_max_size(5 * 1024 * 1024); // 5MB
	/// assert_eq!(handler.max_size(), 5 * 1024 * 1024);
	/// ```
	pub fn with_max_size(mut self, max_size: usize) -> Self {
		self.max_size = max_size;
		self
	}

	/// Set allowed file extensions
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_http::upload::FileUploadHandler;
	/// use std::path::PathBuf;
	///
	/// let handler = FileUploadHandler::new(PathBuf::from("/tmp/uploads"))
	///     .with_allowed_extensions(vec!["jpg".to_string(), "png".to_string()]);
	/// ```
	pub fn with_allowed_extensions(mut self, extensions: Vec<String>) -> Self {
		self.allowed_extensions = Some(extensions);
		self
	}

	/// Enable checksum verification
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_http::upload::FileUploadHandler;
	/// use std::path::PathBuf;
	///
	/// let handler = FileUploadHandler::new(PathBuf::from("/tmp/uploads"))
	///     .with_checksum_verification(true);
	/// ```
	pub fn with_checksum_verification(mut self, enabled: bool) -> Self {
		self.verify_checksum = enabled;
		self
	}

	/// Set allowed MIME types
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_http::upload::FileUploadHandler;
	/// use std::path::PathBuf;
	///
	/// let handler = FileUploadHandler::new(PathBuf::from("/tmp/uploads"))
	///     .with_allowed_mime_types(vec![
	///         "image/jpeg".to_string(),
	///         "image/png".to_string()
	///     ]);
	/// ```
	pub fn with_allowed_mime_types(mut self, mime_types: Vec<String>) -> Self {
		self.allowed_mime_types = Some(mime_types);
		self
	}

	/// Get the maximum file size
	pub fn max_size(&self) -> usize {
		self.max_size
	}

	/// Get the upload directory
	pub fn upload_dir(&self) -> &Path {
		&self.upload_dir
	}

	/// Calculate SHA-256 checksum of file content
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_http::upload::FileUploadHandler;
	/// use std::path::PathBuf;
	///
	/// let handler = FileUploadHandler::new(PathBuf::from("/tmp/uploads"));
	/// let checksum = handler.calculate_checksum(b"test data");
	/// assert_eq!(checksum.len(), 64); // SHA-256 produces 64 hex characters
	/// ```
	pub fn calculate_checksum(&self, content: &[u8]) -> String {
		let mut hasher = Sha256::new();
		hasher.update(content);
		let result = hasher.finalize();
		// Convert bytes to hex string
		result.iter().map(|b| format!("{:02x}", b)).collect()
	}

	/// Verify file checksum
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_http::upload::FileUploadHandler;
	/// use std::path::PathBuf;
	///
	/// let handler = FileUploadHandler::new(PathBuf::from("/tmp/uploads"));
	/// let content = b"test data";
	/// let checksum = handler.calculate_checksum(content);
	/// assert!(handler.verify_file_checksum(content, &checksum).is_ok());
	/// ```
	pub fn verify_file_checksum(
		&self,
		content: &[u8],
		expected_checksum: &str,
	) -> Result<(), FileUploadError> {
		let actual_checksum = self.calculate_checksum(content);
		if actual_checksum == expected_checksum {
			Ok(())
		} else {
			Err(FileUploadError::ChecksumMismatch)
		}
	}

	/// Detect MIME type from file content
	///
	/// Basic MIME type detection based on file signatures (magic numbers).
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_http::upload::FileUploadHandler;
	/// use std::path::PathBuf;
	///
	/// let handler = FileUploadHandler::new(PathBuf::from("/tmp/uploads"));
	///
	/// // PNG signature
	/// let png_data = vec![0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
	/// assert_eq!(handler.detect_mime_type(&png_data), Some("image/png".to_string()));
	///
	/// // JPEG signature
	/// let jpeg_data = vec![0xFF, 0xD8, 0xFF];
	/// assert_eq!(handler.detect_mime_type(&jpeg_data), Some("image/jpeg".to_string()));
	/// ```
	pub fn detect_mime_type(&self, content: &[u8]) -> Option<String> {
		if content.is_empty() {
			return None;
		}

		// Check common file signatures
		if content.len() >= 8 && content[0..8] == [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A] {
			return Some("image/png".to_string());
		}

		if content.len() >= 3 && content[0..3] == [0xFF, 0xD8, 0xFF] {
			return Some("image/jpeg".to_string());
		}

		if content.len() >= 4 && content[0..4] == [0x47, 0x49, 0x46, 0x38] {
			return Some("image/gif".to_string());
		}

		if content.len() >= 4 && content[0..4] == [0x25, 0x50, 0x44, 0x46] {
			return Some("application/pdf".to_string());
		}

		if content.len() >= 4
			&& (content[0..4] == [0x50, 0x4B, 0x03, 0x04]
				|| content[0..4] == [0x50, 0x4B, 0x05, 0x06])
		{
			return Some("application/zip".to_string());
		}

		None
	}

	/// Validate MIME type
	fn validate_mime_type(&self, content: &[u8]) -> Result<(), FileUploadError> {
		if let Some(ref allowed) = self.allowed_mime_types {
			let detected_mime = self
				.detect_mime_type(content)
				.ok_or(FileUploadError::MimeDetectionFailed)?;

			if !allowed.contains(&detected_mime) {
				return Err(FileUploadError::InvalidFileType(detected_mime));
			}
		}
		Ok(())
	}

	/// Handle a file upload
	///
	/// # Arguments
	///
	/// * `field_name` - Name of the form field
	/// * `filename` - Original filename
	/// * `content` - File content as bytes
	///
	/// # Returns
	///
	/// Returns the path to the saved file
	///
	/// # Examples
	///
	/// ```no_run
	/// use reinhardt_http::upload::FileUploadHandler;
	/// use std::path::PathBuf;
	///
	/// let handler = FileUploadHandler::new(PathBuf::from("/tmp/uploads"));
	/// let result = handler.handle_upload("avatar", "photo.jpg", b"image data");
	/// assert!(result.is_ok());
	/// ```
	pub fn handle_upload(
		&self,
		field_name: &str,
		filename: &str,
		content: &[u8],
	) -> Result<String, FileUploadError> {
		// Validate field_name to prevent path traversal via form field names
		validate_safe_filename(field_name)?;

		// Check file size
		if content.len() > self.max_size {
			return Err(FileUploadError::FileTooLarge(content.len(), self.max_size));
		}

		// Validate file extension
		if let Some(ref allowed) = self.allowed_extensions {
			let extension = Path::new(filename)
				.extension()
				.and_then(|e| e.to_str())
				.unwrap_or("");

			if !allowed.iter().any(|ext| ext == extension) {
				return Err(FileUploadError::InvalidFileType(extension.to_string()));
			}
		}

		// Validate MIME type
		self.validate_mime_type(content)?;

		// Create upload directory if it doesn't exist
		fs::create_dir_all(&self.upload_dir)?;

		// Generate unique filename
		let unique_filename = self.generate_unique_filename(field_name, filename);
		let file_path = self.upload_dir.join(&unique_filename);

		// Write file
		let mut file = fs::File::create(&file_path)?;
		file.write_all(content)?;

		Ok(unique_filename)
	}

	/// Handle upload with checksum verification
	///
	/// # Examples
	///
	/// ```no_run
	/// use reinhardt_http::upload::FileUploadHandler;
	/// use std::path::PathBuf;
	///
	/// let handler = FileUploadHandler::new(PathBuf::from("/tmp/uploads"))
	///     .with_checksum_verification(true);
	///
	/// let content = b"test data";
	/// let checksum = handler.calculate_checksum(content);
	/// let result = handler.handle_upload_with_checksum(
	///     "file",
	///     "test.txt",
	///     content,
	///     &checksum
	/// );
	/// assert!(result.is_ok());
	/// ```
	pub fn handle_upload_with_checksum(
		&self,
		field_name: &str,
		filename: &str,
		content: &[u8],
		expected_checksum: &str,
	) -> Result<String, FileUploadError> {
		// Verify checksum if enabled
		if self.verify_checksum {
			self.verify_file_checksum(content, expected_checksum)?;
		}

		// Handle the upload normally
		self.handle_upload(field_name, filename, content)
	}

	/// Generate a unique filename using a cryptographically random UUID v4
	///
	/// Extracts only the file extension from the original filename,
	/// discarding the original name to prevent path traversal.
	/// Uses UUID v4 (CSPRNG-based) instead of timestamps to prevent
	/// predictable filename enumeration.
	fn generate_unique_filename(&self, field_name: &str, original_filename: &str) -> String {
		let unique_id = uuid::Uuid::new_v4();

		// Extract only the extension from the basename (strip any directory components)
		let basename = Path::new(original_filename)
			.file_name()
			.and_then(|n| n.to_str())
			.unwrap_or(original_filename);

		let extension = Path::new(basename)
			.extension()
			.and_then(|e| e.to_str())
			.unwrap_or("");

		if extension.is_empty() {
			format!("{}_{}", field_name, unique_id)
		} else {
			format!("{}_{}.{}", field_name, unique_id, extension)
		}
	}

	/// Delete an uploaded file
	///
	/// # Examples
	///
	/// ```no_run
	/// use reinhardt_http::upload::FileUploadHandler;
	/// use std::path::PathBuf;
	///
	/// let handler = FileUploadHandler::new(PathBuf::from("/tmp/uploads"));
	/// let result = handler.delete_upload("avatar_123456.jpg");
	/// assert!(result.is_ok());
	/// ```
	pub fn delete_upload(&self, filename: &str) -> Result<(), FileUploadError> {
		// Validate filename to prevent path traversal attacks
		validate_safe_filename(filename)?;
		let file_path = self.upload_dir.join(filename);
		fs::remove_file(file_path)?;
		Ok(())
	}
}

/// TemporaryFileUpload manages temporary uploaded files
///
/// Automatically cleans up temporary files when dropped.
pub struct TemporaryFileUpload {
	path: PathBuf,
	auto_delete: bool,
}

impl TemporaryFileUpload {
	/// Create a new temporary file upload
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_http::upload::TemporaryFileUpload;
	/// use std::path::PathBuf;
	///
	/// let temp = TemporaryFileUpload::new(PathBuf::from("/tmp/temp_file.dat"));
	/// assert_eq!(temp.path().to_str().unwrap(), "/tmp/temp_file.dat");
	/// ```
	pub fn new(path: PathBuf) -> Self {
		Self {
			path,
			auto_delete: true,
		}
	}

	/// Create a temporary file with content
	///
	/// # Examples
	///
	/// ```no_run
	/// use reinhardt_http::upload::TemporaryFileUpload;
	/// use std::path::PathBuf;
	///
	/// let temp = TemporaryFileUpload::with_content(
	///     PathBuf::from("/tmp/temp.txt"),
	///     b"Hello, World!"
	/// ).unwrap();
	/// ```
	pub fn with_content(path: PathBuf, content: &[u8]) -> Result<Self, FileUploadError> {
		let mut file = fs::File::create(&path)?;
		file.write_all(content)?;
		Ok(Self {
			path,
			auto_delete: true,
		})
	}

	/// Disable automatic deletion
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_http::upload::TemporaryFileUpload;
	/// use std::path::PathBuf;
	///
	/// let mut temp = TemporaryFileUpload::new(PathBuf::from("/tmp/keep_me.txt"));
	/// temp.keep();
	/// assert!(!temp.auto_delete());
	/// ```
	pub fn keep(&mut self) {
		self.auto_delete = false;
	}

	/// Get the file path
	pub fn path(&self) -> &Path {
		&self.path
	}

	/// Check if auto-delete is enabled
	pub fn auto_delete(&self) -> bool {
		self.auto_delete
	}

	/// Read file content
	///
	/// # Examples
	///
	/// ```no_run
	/// use reinhardt_http::upload::TemporaryFileUpload;
	/// use std::path::PathBuf;
	///
	/// let temp = TemporaryFileUpload::with_content(
	///     PathBuf::from("/tmp/test.txt"),
	///     b"content"
	/// ).unwrap();
	/// let content = temp.read_content().unwrap();
	/// assert_eq!(content, b"content");
	/// ```
	pub fn read_content(&self) -> Result<Vec<u8>, FileUploadError> {
		Ok(fs::read(&self.path)?)
	}
}

impl Drop for TemporaryFileUpload {
	fn drop(&mut self) {
		if self.auto_delete && self.path.exists() {
			let _ = fs::remove_file(&self.path);
		}
	}
}

/// MemoryFileUpload stores uploaded files in memory
///
/// Useful for small files or testing scenarios where
/// disk I/O should be avoided.
pub struct MemoryFileUpload {
	filename: String,
	content: Vec<u8>,
	content_type: Option<String>,
}

impl MemoryFileUpload {
	/// Create a new memory-based file upload
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_http::upload::MemoryFileUpload;
	///
	/// let upload = MemoryFileUpload::new(
	///     "document.pdf".to_string(),
	///     vec![0x25, 0x50, 0x44, 0x46]
	/// );
	/// assert_eq!(upload.filename(), "document.pdf");
	/// assert_eq!(upload.size(), 4);
	/// ```
	pub fn new(filename: String, content: Vec<u8>) -> Self {
		Self {
			filename,
			content,
			content_type: None,
		}
	}

	/// Create a memory upload with content type
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_http::upload::MemoryFileUpload;
	///
	/// let upload = MemoryFileUpload::with_content_type(
	///     "image.png".to_string(),
	///     vec![0x89, 0x50, 0x4E, 0x47],
	///     "image/png".to_string()
	/// );
	/// assert_eq!(upload.content_type(), Some("image/png"));
	/// ```
	pub fn with_content_type(filename: String, content: Vec<u8>, content_type: String) -> Self {
		Self {
			filename,
			content,
			content_type: Some(content_type),
		}
	}

	/// Get the filename
	pub fn filename(&self) -> &str {
		&self.filename
	}

	/// Get the file content
	pub fn content(&self) -> &[u8] {
		&self.content
	}

	/// Get the content type
	pub fn content_type(&self) -> Option<&str> {
		self.content_type.as_deref()
	}

	/// Get the file size in bytes
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_http::upload::MemoryFileUpload;
	///
	/// let upload = MemoryFileUpload::new("test.txt".to_string(), vec![1, 2, 3, 4, 5]);
	/// assert_eq!(upload.size(), 5);
	/// ```
	pub fn size(&self) -> usize {
		self.content.len()
	}

	/// Check if the upload is empty
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_http::upload::MemoryFileUpload;
	///
	/// let empty = MemoryFileUpload::new("empty.txt".to_string(), vec![]);
	/// assert!(empty.is_empty());
	///
	/// let non_empty = MemoryFileUpload::new("data.txt".to_string(), vec![1, 2, 3]);
	/// assert!(!non_empty.is_empty());
	/// ```
	pub fn is_empty(&self) -> bool {
		self.content.is_empty()
	}

	/// Save to disk
	///
	/// # Examples
	///
	/// ```no_run
	/// use reinhardt_http::upload::MemoryFileUpload;
	/// use std::path::PathBuf;
	///
	/// let upload = MemoryFileUpload::new("test.txt".to_string(), vec![1, 2, 3]);
	/// let result = upload.save_to_disk(PathBuf::from("/tmp/test.txt"));
	/// assert!(result.is_ok());
	/// ```
	pub fn save_to_disk(&self, path: PathBuf) -> Result<(), FileUploadError> {
		let mut file = fs::File::create(path)?;
		file.write_all(&self.content)?;
		Ok(())
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_file_upload_handler_creation() {
		let handler = FileUploadHandler::new(PathBuf::from("/tmp/uploads"));
		assert_eq!(handler.max_size(), 10 * 1024 * 1024);
		assert_eq!(handler.upload_dir(), Path::new("/tmp/uploads"));
	}

	#[test]
	fn test_file_upload_handler_with_max_size() {
		let handler =
			FileUploadHandler::new(PathBuf::from("/tmp/uploads")).with_max_size(5 * 1024 * 1024);
		assert_eq!(handler.max_size(), 5 * 1024 * 1024);
	}

	#[test]
	fn test_file_upload_handler_size_validation() {
		let handler = FileUploadHandler::new(PathBuf::from("/tmp/uploads")).with_max_size(100);

		let large_content = vec![0u8; 200];
		let result = handler.handle_upload("test", "large.txt", &large_content);

		assert!(result.is_err());
		if let Err(FileUploadError::FileTooLarge(size, max)) = result {
			assert_eq!(size, 200);
			assert_eq!(max, 100);
		} else {
			panic!("Expected FileTooLarge error");
		}
	}

	#[test]
	fn test_file_upload_handler_extension_validation() {
		let handler = FileUploadHandler::new(PathBuf::from("/tmp/uploads"))
			.with_allowed_extensions(vec!["jpg".to_string(), "png".to_string()]);

		let content = b"test content";
		let result = handler.handle_upload("test", "document.pdf", content);

		assert!(result.is_err());
		if let Err(FileUploadError::InvalidFileType(ext)) = result {
			assert_eq!(ext, "pdf");
		} else {
			panic!("Expected InvalidFileType error");
		}
	}

	#[test]
	fn test_temporary_file_upload_creation() {
		let temp = TemporaryFileUpload::new(PathBuf::from("/tmp/test_temp.txt"));
		assert_eq!(temp.path(), Path::new("/tmp/test_temp.txt"));
		assert!(temp.auto_delete());
	}

	#[test]
	fn test_temporary_file_upload_keep() {
		let mut temp = TemporaryFileUpload::new(PathBuf::from("/tmp/test_keep.txt"));
		temp.keep();
		assert!(!temp.auto_delete());
	}

	#[test]
	fn test_temporary_file_upload_with_content() {
		let temp_path = PathBuf::from("/tmp/test_content_temp.txt");
		let content = b"Test content";

		let temp = TemporaryFileUpload::with_content(temp_path.clone(), content).unwrap();
		assert!(temp_path.exists());

		let read_content = temp.read_content().unwrap();
		assert_eq!(read_content, content);

		drop(temp);
		assert!(!temp_path.exists());
	}

	#[test]
	fn test_temporary_file_upload_auto_delete() {
		let temp_path = PathBuf::from("/tmp/test_auto_delete.txt");
		fs::write(&temp_path, b"test").unwrap();

		{
			let _temp = TemporaryFileUpload::new(temp_path.clone());
			assert!(temp_path.exists());
		}

		assert!(!temp_path.exists());
	}

	#[test]
	fn test_memory_file_upload_creation() {
		let upload = MemoryFileUpload::new("test.txt".to_string(), vec![1, 2, 3, 4, 5]);

		assert_eq!(upload.filename(), "test.txt");
		assert_eq!(upload.content(), &[1, 2, 3, 4, 5]);
		assert_eq!(upload.size(), 5);
		assert!(!upload.is_empty());
	}

	#[test]
	fn test_memory_file_upload_with_content_type() {
		let upload = MemoryFileUpload::with_content_type(
			"image.png".to_string(),
			vec![0x89, 0x50, 0x4E, 0x47],
			"image/png".to_string(),
		);

		assert_eq!(upload.filename(), "image.png");
		assert_eq!(upload.content_type(), Some("image/png"));
	}

	#[test]
	fn test_memory_file_upload_is_empty() {
		let empty = MemoryFileUpload::new("empty.txt".to_string(), vec![]);
		assert!(empty.is_empty());
		assert_eq!(empty.size(), 0);

		let non_empty = MemoryFileUpload::new("data.txt".to_string(), vec![1, 2, 3]);
		assert!(!non_empty.is_empty());
		assert_eq!(non_empty.size(), 3);
	}

	#[test]
	fn test_memory_file_upload_save_to_disk() {
		let temp_path = PathBuf::from("/tmp/test_memory_save.txt");
		let upload = MemoryFileUpload::new("test.txt".to_string(), vec![1, 2, 3, 4, 5]);

		let result = upload.save_to_disk(temp_path.clone());
		assert!(result.is_ok());
		assert!(temp_path.exists());

		let content = fs::read(&temp_path).unwrap();
		assert_eq!(content, vec![1, 2, 3, 4, 5]);

		fs::remove_file(temp_path).unwrap();
	}

	// =================================================================
	// Path traversal prevention tests (Issue #355)
	// =================================================================

	#[rstest::rstest]
	#[case("../../../etc/passwd")]
	#[case("foo/../../bar")]
	#[case("/etc/passwd")]
	#[case("test\0file.txt")]
	#[case("..%2f..%2fetc%2fpasswd")]
	#[case("%2e%2e/%2e%2e/etc/passwd")]
	fn test_delete_upload_rejects_path_traversal(#[case] filename: &str) {
		// Arrange
		let handler = FileUploadHandler::new(PathBuf::from("/tmp/uploads"));

		// Act
		let result = handler.delete_upload(filename);

		// Assert
		assert!(
			matches!(result, Err(FileUploadError::PathTraversal)),
			"Expected PathTraversal error for filename: {}",
			filename
		);
	}

	#[rstest::rstest]
	fn test_delete_upload_allows_safe_filenames() {
		// Arrange
		let handler = FileUploadHandler::new(PathBuf::from("/tmp/uploads"));

		// Act - these should not return PathTraversal error
		// (they may return IO NotFound since files don't exist, which is expected)
		let result = handler.delete_upload("safe_file.txt");

		// Assert
		assert!(
			!matches!(result, Err(FileUploadError::PathTraversal)),
			"Safe filename should not trigger path traversal error"
		);
	}

	#[rstest::rstest]
	#[case("normal.txt", true)]
	#[case("my-file_123.jpg", true)]
	#[case("report.pdf", true)]
	#[case("image_2024.png", true)]
	#[case("../../../etc/passwd", false)]
	#[case("foo/../bar.txt", false)]
	#[case("/absolute/path.txt", false)]
	#[case("null\0byte.txt", false)]
	#[case("", false)]
	#[case("back\\slash.txt", false)]
	#[case("C:\\Windows\\system32", false)]
	#[case("..%2f..%2fetc%2fpasswd", false)]
	#[case("%2e%2e%2f%2e%2e%2f", false)]
	fn test_validate_safe_filename(#[case] filename: &str, #[case] should_pass: bool) {
		// Act
		let result = validate_safe_filename(filename);

		// Assert
		assert_eq!(
			result.is_ok(),
			should_pass,
			"validate_safe_filename({:?}) expected {} but got {}",
			filename,
			if should_pass { "Ok" } else { "Err" },
			if result.is_ok() { "Ok" } else { "Err" },
		);
	}

	#[rstest::rstest]
	#[case("../malicious")]
	#[case("foo/../../bar")]
	#[case("..%2fmalicious")]
	fn test_handle_upload_rejects_traversal_in_field_name(#[case] field_name: &str) {
		// Arrange
		let handler = FileUploadHandler::new(PathBuf::from("/tmp/uploads"));

		// Act
		let result = handler.handle_upload(field_name, "safe.txt", b"content");

		// Assert
		assert!(
			matches!(result, Err(FileUploadError::PathTraversal)),
			"Expected PathTraversal error for field_name: {}",
			field_name
		);
	}

	#[rstest::rstest]
	fn test_handle_upload_accepts_safe_field_name() {
		// Arrange
		let handler = FileUploadHandler::new(PathBuf::from("/tmp/reinhardt_upload_test"));

		// Act
		let result = handler.handle_upload("avatar", "photo.jpg", b"image data");

		// Assert
		assert!(result.is_ok());

		// Cleanup
		let _ = fs::remove_dir_all("/tmp/reinhardt_upload_test");
	}
}
