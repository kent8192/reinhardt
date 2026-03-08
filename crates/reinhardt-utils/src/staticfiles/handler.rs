use mime_guess::from_path;
use std::fs;
use std::io;
use std::path::PathBuf;
use tracing;

/// Errors that can occur when serving static files.
#[derive(Debug, thiserror::Error)]
pub enum StaticError {
	/// The requested file was not found.
	#[error("File not found: {0}")]
	NotFound(String),
	/// A directory traversal attack was detected and blocked.
	#[error("Directory traversal blocked: {0}")]
	DirectoryTraversal(String),
	/// An underlying I/O error occurred.
	#[error("IO error: {0}")]
	Io(#[from] io::Error),
}

/// A static file that has been read from disk, including its content and metadata.
#[derive(Debug)]
pub struct StaticFile {
	/// The raw file content.
	pub content: Vec<u8>,
	/// The resolved filesystem path of the file.
	pub path: PathBuf,
	/// The MIME type of the file (e.g., `"text/css"`).
	pub mime_type: String,
}

impl StaticFile {
	/// Generate ETag for the file based on content hash
	pub fn etag(&self) -> String {
		use std::collections::hash_map::DefaultHasher;
		use std::hash::{Hash, Hasher};

		let mut hasher = DefaultHasher::new();
		self.content.hash(&mut hasher);
		format!("\"{}\"", hasher.finish())
	}
}

/// Serves static files from a root directory with directory traversal protection.
pub struct StaticFileHandler {
	root: PathBuf,
	index_files: Vec<String>,
}

impl StaticFileHandler {
	/// Creates a new handler serving files from the given root directory.
	pub fn new(root: PathBuf) -> Self {
		Self {
			root,
			index_files: vec!["index.html".to_string()],
		}
	}

	/// Configure custom index files to serve for directories
	///
	/// When a directory is requested, the handler will try to serve
	/// the first matching index file from this list.
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_utils::staticfiles::handler::StaticFileHandler;
	/// use std::path::PathBuf;
	///
	/// let handler = StaticFileHandler::new(PathBuf::from("static"))
	///     .with_index_files(vec!["index.html".to_string(), "default.html".to_string()]);
	/// ```
	pub fn with_index_files(mut self, index_files: Vec<String>) -> Self {
		self.index_files = index_files;
		self
	}

	/// Reads and returns the static file at the given path, resolving index files for directories.
	pub async fn serve(&self, path: &str) -> Result<StaticFile, StaticError> {
		let resolved = self.resolve_path(path).await?;
		let content = fs::read(&resolved)?;
		let mime_type = from_path(&resolved).first_or_octet_stream().to_string();

		Ok(StaticFile {
			content,
			path: resolved,
			mime_type,
		})
	}

	/// Resolves and validates a request path to an absolute filesystem path within the root.
	pub async fn resolve_path(&self, path: &str) -> Result<PathBuf, StaticError> {
		let path = path.trim_start_matches('/');

		// Prevent directory traversal attacks
		if path.contains("..") {
			return Err(StaticError::DirectoryTraversal(path.to_string()));
		}

		let file_path = self.root.join(path);

		// Canonicalize paths to prevent traversal
		let canonical_file = file_path.canonicalize().map_err(|e| {
			tracing::warn!(
				"Failed to canonicalize path: {} (root: {}, error: {})",
				file_path.display(),
				self.root.display(),
				e
			);
			StaticError::NotFound(path.to_string())
		})?;

		let canonical_root = self.root.canonicalize().map_err(|e| {
			tracing::error!(
				"Static files root directory does not exist: {} (error: {})",
				self.root.display(),
				e
			);
			StaticError::Io(e)
		})?;

		// Ensure file is within root directory
		if !canonical_file.starts_with(&canonical_root) {
			return Err(StaticError::DirectoryTraversal(path.to_string()));
		}

		// If it's a directory, try to serve an index file
		if canonical_file.is_dir() {
			for index_file in &self.index_files {
				let index_path = canonical_file.join(index_file);
				if index_path.exists() && index_path.is_file() {
					return Ok(index_path);
				}
			}
			// No index file found in directory
			return Err(StaticError::NotFound(format!(
				"{} (directory without index file)",
				path
			)));
		}

		Ok(canonical_file)
	}
}

/// A convenience type alias for `Result<T, StaticError>`.
pub type StaticResult<T> = Result<T, StaticError>;
