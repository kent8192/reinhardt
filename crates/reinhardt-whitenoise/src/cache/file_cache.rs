//! File cache and metadata structures

use super::etag::generate_etag;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// Metadata for a static file
#[derive(Debug, Clone)]
pub struct FileMetadata {
	/// File size in bytes
	pub size: u64,

	/// File modification time
	pub modified: SystemTime,

	/// ETag for conditional requests
	pub etag: String,

	/// MIME type
	pub mime_type: String,

	/// Absolute path to the file
	pub path: PathBuf,
}

impl FileMetadata {
	/// Creates metadata from a file path
	///
	/// # Arguments
	///
	/// * `path` - Path to the file
	///
	/// # Errors
	///
	/// Returns error if file cannot be accessed
	///
	/// # Example
	///
	/// ```rust,ignore
	/// use reinhardt_whitenoise::cache::FileMetadata;
	/// use std::path::Path;
	///
	/// let metadata = FileMetadata::from_path(Path::new("test.css"))?;
	/// ```
	pub fn from_path(path: &Path) -> std::io::Result<Self> {
		let metadata = fs::metadata(path)?;

		let size = metadata.len();
		let modified = metadata.modified()?;
		let etag = generate_etag(modified, size);

		let mime_type = mime_guess::from_path(path)
			.first_or_octet_stream()
			.to_string();

		Ok(Self {
			size,
			modified,
			etag,
			mime_type,
			path: path.to_path_buf(),
		})
	}
}

/// Compressed file variants
#[derive(Debug, Clone, Default)]
pub struct CompressedVariants {
	/// Path to gzip-compressed variant
	pub gzip: Option<PathBuf>,

	/// Path to brotli-compressed variant
	pub brotli: Option<PathBuf>,
}

impl CompressedVariants {
	/// Creates a new empty variants struct
	pub fn new() -> Self {
		Self::default()
	}

	/// Sets the gzip variant path
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_whitenoise::cache::CompressedVariants;
	/// use std::path::PathBuf;
	///
	/// let variants = CompressedVariants::new()
	///     .with_gzip(PathBuf::from("app.js.gz"));
	/// ```
	pub fn with_gzip(mut self, path: PathBuf) -> Self {
		self.gzip = Some(path);
		self
	}

	/// Sets the brotli variant path
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_whitenoise::cache::CompressedVariants;
	/// use std::path::PathBuf;
	///
	/// let variants = CompressedVariants::new()
	///     .with_brotli(PathBuf::from("app.js.br"));
	/// ```
	pub fn with_brotli(mut self, path: PathBuf) -> Self {
		self.brotli = Some(path);
		self
	}

	/// Checks if any compressed variants exist
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_whitenoise::cache::CompressedVariants;
	/// use std::path::PathBuf;
	///
	/// let variants = CompressedVariants::new()
	///     .with_gzip(PathBuf::from("app.js.gz"));
	/// assert!(variants.has_variants());
	/// ```
	pub fn has_variants(&self) -> bool {
		self.gzip.is_some() || self.brotli.is_some()
	}
}

/// In-memory cache for file metadata
#[derive(Debug)]
pub struct FileCache {
	/// Map of relative path to file metadata
	pub files: HashMap<String, FileMetadata>,

	/// Map of relative path to compressed variants
	pub compressed: HashMap<String, CompressedVariants>,

	/// Manifest mapping original paths to hashed paths
	pub manifest: HashMap<String, String>,
}

impl FileCache {
	/// Creates a new empty file cache
	pub fn new() -> Self {
		Self {
			files: HashMap::new(),
			compressed: HashMap::new(),
			manifest: HashMap::new(),
		}
	}

	/// Inserts file metadata into the cache
	///
	/// # Arguments
	///
	/// * `relative_path` - Relative path from static root
	/// * `metadata` - File metadata
	pub fn insert_file(&mut self, relative_path: String, metadata: FileMetadata) {
		self.files.insert(relative_path, metadata);
	}

	/// Inserts compressed variants into the cache
	///
	/// # Arguments
	///
	/// * `relative_path` - Relative path from static root
	/// * `variants` - Compressed file variants
	pub fn insert_compressed(&mut self, relative_path: String, variants: CompressedVariants) {
		self.compressed.insert(relative_path, variants);
	}

	/// Gets file metadata from the cache
	///
	/// # Arguments
	///
	/// * `relative_path` - Relative path from static root
	///
	/// # Returns
	///
	/// File metadata if found
	pub fn get(&self, relative_path: &str) -> Option<&FileMetadata> {
		self.files.get(relative_path)
	}

	/// Gets compressed variants from the cache
	///
	/// # Arguments
	///
	/// * `relative_path` - Relative path from static root
	///
	/// # Returns
	///
	/// Compressed variants if found
	pub fn get_compressed(&self, relative_path: &str) -> Option<&CompressedVariants> {
		self.compressed.get(relative_path)
	}

	/// Resolves a path using the manifest
	///
	/// # Arguments
	///
	/// * `path` - Original file path
	///
	/// # Returns
	///
	/// Hashed path if found in manifest, otherwise original path
	pub fn resolve(&self, path: &str) -> String {
		self.manifest
			.get(path)
			.cloned()
			.unwrap_or_else(|| path.to_string())
	}

	/// Loads manifest from a JSON file
	///
	/// # Arguments
	///
	/// * `path` - Path to manifest.json file
	///
	/// # Errors
	///
	/// Returns error if file cannot be read or parsed
	pub fn load_manifest(&mut self, path: &Path) -> crate::Result<()> {
		let content = fs::read_to_string(path)?;
		self.manifest = serde_json::from_str(&content)?;
		Ok(())
	}
}

impl Default for FileCache {
	fn default() -> Self {
		Self::new()
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;
	use std::fs::File;
	use std::io::Write;
	use tempfile::TempDir;

	#[rstest]
	fn test_file_metadata_from_path() {
		let temp_dir = TempDir::new().unwrap();
		let file_path = temp_dir.path().join("test.css");
		let mut file = File::create(&file_path).unwrap();
		writeln!(file, "body {{ color: red; }}").unwrap();

		let metadata = FileMetadata::from_path(&file_path).unwrap();
		assert!(metadata.size > 0);
		assert!(!metadata.etag.is_empty());
		assert!(metadata.mime_type.contains("css"));
	}

	#[rstest]
	fn test_compressed_variants_builder() {
		let variants = CompressedVariants::new()
			.with_gzip(PathBuf::from("app.js.gz"))
			.with_brotli(PathBuf::from("app.js.br"));

		assert!(variants.gzip.is_some());
		assert!(variants.brotli.is_some());
		assert!(variants.has_variants());
	}

	#[rstest]
	fn test_file_cache_operations() {
		let temp_dir = TempDir::new().unwrap();
		let file_path = temp_dir.path().join("test.css");
		File::create(&file_path).unwrap();

		let mut cache = FileCache::new();
		let metadata = FileMetadata::from_path(&file_path).unwrap();

		cache.insert_file("test.css".to_string(), metadata);

		assert!(cache.get("test.css").is_some());
		assert!(cache.get("nonexistent.css").is_none());
	}

	#[rstest]
	fn test_manifest_resolution() {
		let mut cache = FileCache::new();
		cache
			.manifest
			.insert("app.js".to_string(), "app.abc123.js".to_string());

		assert_eq!(cache.resolve("app.js"), "app.abc123.js");
		assert_eq!(cache.resolve("other.js"), "other.js");
	}
}
