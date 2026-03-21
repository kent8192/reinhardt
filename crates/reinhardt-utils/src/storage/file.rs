//! File metadata and representation

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Metadata about a stored file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileMetadata {
	/// The storage path of the file.
	pub path: String,
	/// The size of the file in bytes.
	pub size: u64,
	/// The MIME content type of the file, if known.
	pub content_type: Option<String>,
	/// The timestamp when the file was created.
	pub created_at: DateTime<Utc>,
	/// The timestamp when the file was last modified.
	pub modified_at: DateTime<Utc>,
	/// An optional checksum (e.g., MD5, SHA-256) for integrity verification.
	pub checksum: Option<String>,
}

impl FileMetadata {
	/// Create new file metadata
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_utils::storage::FileMetadata;
	///
	/// let metadata = FileMetadata::new("test.txt".to_string(), 1024);
	/// assert_eq!(metadata.path, "test.txt");
	/// assert_eq!(metadata.size, 1024);
	/// assert!(metadata.checksum.is_none());
	/// ```
	pub fn new(path: String, size: u64) -> Self {
		let now = Utc::now();
		Self {
			path,
			size,
			content_type: None,
			created_at: now,
			modified_at: now,
			checksum: None,
		}
	}
	/// Set content type for the file
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_utils::storage::FileMetadata;
	///
	/// let metadata = FileMetadata::new("test.txt".to_string(), 1024)
	///     .with_content_type("text/plain".to_string());
	/// assert_eq!(metadata.content_type, Some("text/plain".to_string()));
	/// ```
	pub fn with_content_type(mut self, content_type: String) -> Self {
		self.content_type = Some(content_type);
		self
	}
	/// Set checksum for the file
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_utils::storage::FileMetadata;
	///
	/// let metadata = FileMetadata::new("test.txt".to_string(), 1024)
	///     .with_checksum("abc123".to_string());
	/// assert_eq!(metadata.checksum, Some("abc123".to_string()));
	/// ```
	pub fn with_checksum(mut self, checksum: String) -> Self {
		self.checksum = Some(checksum);
		self
	}
}

/// Represents a stored file
#[derive(Debug)]
pub struct StoredFile {
	/// Metadata describing the stored file.
	pub metadata: FileMetadata,
	/// The raw file content.
	pub content: Vec<u8>,
}

impl StoredFile {
	/// Create a new stored file with metadata and content
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_utils::storage::{FileMetadata, StoredFile};
	///
	/// let metadata = FileMetadata::new("test.txt".to_string(), 5);
	/// let file = StoredFile::new(metadata, b"hello".to_vec());
	/// assert_eq!(file.content, b"hello");
	/// assert_eq!(file.size(), 5);
	/// ```
	pub fn new(metadata: FileMetadata, content: Vec<u8>) -> Self {
		Self { metadata, content }
	}
	/// Get the size of the file content in bytes
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_utils::storage::{FileMetadata, StoredFile};
	///
	/// let metadata = FileMetadata::new("test.txt".to_string(), 100);
	/// let file = StoredFile::new(metadata, b"hello world".to_vec());
	/// assert_eq!(file.size(), 11);
	/// ```
	pub fn size(&self) -> u64 {
		self.content.len() as u64
	}
}
