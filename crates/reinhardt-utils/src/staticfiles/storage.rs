use async_trait::async_trait;
use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::io;
use std::path::PathBuf;
use std::sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard};
use tracing;

/// Acquire a read lock, recovering from poisoning with a warning log.
fn read_or_recover<'a, T>(lock: &'a RwLock<T>, context: &str) -> RwLockReadGuard<'a, T> {
	lock.read().unwrap_or_else(|e| {
		tracing::warn!("RwLock poisoned during read ({}), recovering", context);
		e.into_inner()
	})
}

/// Acquire a write lock, recovering from poisoning with a warning log.
fn write_or_recover<'a, T>(lock: &'a RwLock<T>, context: &str) -> RwLockWriteGuard<'a, T> {
	lock.write().unwrap_or_else(|e| {
		tracing::warn!("RwLock poisoned during write ({}), recovering", context);
		e.into_inner()
	})
}

// Cloud storage backends
#[cfg(feature = "s3")]
pub mod s3;

#[cfg(feature = "s3")]
pub use s3::{S3Config, S3Storage};

#[cfg(feature = "azure")]
pub mod azure;

#[cfg(feature = "azure")]
pub use azure::{AzureBlobConfig, AzureBlobStorage};

#[cfg(feature = "gcs")]
pub mod gcs;

#[cfg(feature = "gcs")]
pub use gcs::{GcsConfig, GcsStorage};

// Storage registry
pub mod registry;
pub use registry::StorageRegistry;

/// Storage trait for static files
#[async_trait]
pub trait Storage: Send + Sync {
	/// Saves content under the given name and returns the URL.
	async fn save(&self, name: &str, content: &[u8]) -> io::Result<String>;
	/// Returns whether a file with the given name exists in storage.
	fn exists(&self, name: &str) -> bool;
	/// Reads and returns the content of the file with the given name.
	async fn open(&self, name: &str) -> io::Result<Vec<u8>>;
	/// Deletes the file with the given name from storage.
	async fn delete(&self, name: &str) -> io::Result<()>;
	/// Returns the URL for accessing the file with the given name.
	fn url(&self, name: &str) -> String;
}

/// A storage backend that reads and writes files on the local filesystem.
pub struct FileSystemStorage {
	/// The root directory where files are stored.
	pub location: PathBuf,
	/// The base URL prefix used to generate file URLs.
	pub base_url: String,
}

impl FileSystemStorage {
	/// Creates a new filesystem storage rooted at the given location.
	pub fn new<P: Into<PathBuf>>(location: P, base_url: &str) -> Self {
		Self {
			location: location.into(),
			base_url: base_url.to_string(),
		}
	}

	fn normalize_path(&self, name: &str) -> PathBuf {
		let name = name.trim_start_matches('/');
		// Use safe_path_join to prevent directory traversal attacks.
		// Falls back to simple join only if safe_path_join succeeds.
		match crate::safe_path_join(&self.location, name) {
			Ok(safe_path) => safe_path,
			Err(_) => {
				tracing::warn!(
					"Path traversal attempt blocked in FileSystemStorage: {}",
					name
				);
				// Return a path that won't resolve to anything valid outside base
				self.location.join("__invalid_path__")
			}
		}
	}

	fn normalize_url(&self, base: &str, name: &str) -> String {
		let base = base.trim_end_matches('/');
		let name = name.trim_start_matches('/');
		format!("{}/{}", base, name)
	}
}

#[async_trait]
impl Storage for FileSystemStorage {
	async fn save(&self, name: &str, content: &[u8]) -> io::Result<String> {
		let file_path = self.normalize_path(name);

		// Create parent directories if they don't exist
		if let Some(parent) = file_path.parent() {
			tokio::fs::create_dir_all(parent).await?;
		}

		tokio::fs::write(&file_path, content).await?;
		Ok(self.url(name))
	}

	fn exists(&self, name: &str) -> bool {
		self.normalize_path(name).exists()
	}

	async fn open(&self, name: &str) -> io::Result<Vec<u8>> {
		tokio::fs::read(self.normalize_path(name)).await
	}

	async fn delete(&self, name: &str) -> io::Result<()> {
		let file_path = self.normalize_path(name);
		if file_path.exists() {
			tokio::fs::remove_file(file_path).await?;
		}
		Ok(())
	}

	fn url(&self, name: &str) -> String {
		self.normalize_url(&self.base_url, name)
	}
}

/// A storage backend that keeps files in memory, useful for testing.
pub struct MemoryStorage {
	base_url: String,
	files: Arc<RwLock<HashMap<String, Vec<u8>>>>,
}

impl MemoryStorage {
	/// Creates a new in-memory storage with the given base URL.
	pub fn new(base_url: &str) -> Self {
		Self {
			base_url: base_url.to_string(),
			files: Arc::new(RwLock::new(HashMap::new())),
		}
	}

	fn normalize_url(&self, base: &str, name: &str) -> String {
		let base = base.trim_end_matches('/');
		let name = name.trim_start_matches('/');
		format!("{}/{}", base, name)
	}
}

#[async_trait]
impl Storage for MemoryStorage {
	async fn save(&self, name: &str, content: &[u8]) -> io::Result<String> {
		let mut files = write_or_recover(&self.files, "MemoryStorage::save");
		files.insert(name.to_string(), content.to_vec());
		Ok(self.url(name))
	}

	fn exists(&self, name: &str) -> bool {
		read_or_recover(&self.files, "MemoryStorage::exists").contains_key(name)
	}

	async fn open(&self, name: &str) -> io::Result<Vec<u8>> {
		read_or_recover(&self.files, "MemoryStorage::open")
			.get(name)
			.cloned()
			.ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "File not found"))
	}

	async fn delete(&self, name: &str) -> io::Result<()> {
		write_or_recover(&self.files, "MemoryStorage::delete").remove(name);
		Ok(())
	}

	fn url(&self, name: &str) -> String {
		self.normalize_url(&self.base_url, name)
	}
}

impl Default for MemoryStorage {
	fn default() -> Self {
		Self::new("/static/")
	}
}

/// Configuration for the static files system.
#[derive(Debug, Clone)]
pub struct StaticFilesConfig {
	/// The directory where collected static files are stored for deployment.
	pub static_root: PathBuf,
	/// The URL prefix for serving static files (e.g., `"/static/"`).
	pub static_url: String,
	/// Source directories containing static files to be collected.
	pub staticfiles_dirs: Vec<PathBuf>,
	/// Optional URL prefix for user-uploaded media files.
	pub media_url: Option<String>,
}

impl Default for StaticFilesConfig {
	fn default() -> Self {
		Self {
			static_root: PathBuf::from("static"),
			static_url: "/static/".to_string(),
			staticfiles_dirs: Vec::new(),
			media_url: None,
		}
	}
}

/// Locates static files across multiple source directories.
pub struct StaticFilesFinder {
	/// The list of directories to search for static files.
	pub directories: Vec<PathBuf>,
}

impl StaticFilesFinder {
	/// Creates a new finder that searches the given directories.
	pub fn new(directories: Vec<PathBuf>) -> Self {
		Self { directories }
	}

	/// Finds the first file matching the given path across all configured directories.
	pub fn find(&self, path: &str) -> Result<PathBuf, io::Error> {
		let path = path.trim_start_matches('/');
		for dir in &self.directories {
			// Use safe_path_join to prevent directory traversal attacks
			match crate::safe_path_join(dir, path) {
				Ok(file_path) => {
					if file_path.exists() {
						return Ok(file_path);
					}
				}
				Err(_) => {
					tracing::warn!(
						"Path traversal attempt blocked in StaticFilesFinder: {}",
						path
					);
					continue;
				}
			}
		}
		Err(io::Error::new(
			io::ErrorKind::NotFound,
			format!("File not found in any directory: {}", path),
		))
	}

	/// Find all static files across all configured directories
	///
	/// Returns a vector of all static file paths found in the configured directories.
	/// Each path is relative to its source directory.
	///
	/// # Example
	///
	/// ```rust,no_run
	/// use reinhardt_utils::staticfiles::StaticFilesFinder;
	/// use std::path::PathBuf;
	///
	/// let finder = StaticFilesFinder::new(vec![
	///     PathBuf::from("static"),
	///     PathBuf::from("assets"),
	/// ]);
	///
	/// let files = finder.find_all();
	/// // Returns: ["css/style.css", "js/app.js", "images/logo.png", ...]
	/// ```
	pub fn find_all(&self) -> Vec<String> {
		let mut all_files = Vec::new();

		for dir in &self.directories {
			if !dir.exists() || !dir.is_dir() {
				continue;
			}

			if let Ok(entries) = self.walk_directory(dir, dir) {
				all_files.extend(entries);
			}
		}

		all_files
	}

	/// Recursively walk a directory and collect all file paths
	#[allow(clippy::only_used_in_recursion)]
	fn walk_directory(&self, base_dir: &PathBuf, current_dir: &PathBuf) -> io::Result<Vec<String>> {
		let mut files = Vec::new();

		for entry in fs::read_dir(current_dir)? {
			let entry = entry?;
			let path = entry.path();

			if path.is_file() {
				// Get relative path from base directory
				if let Ok(relative) = path.strip_prefix(base_dir)
					&& let Some(path_str) = relative.to_str()
				{
					files.push(path_str.to_string());
				}
			} else if path.is_dir() {
				// Recursively walk subdirectories
				if let Ok(sub_files) = self.walk_directory(base_dir, &path) {
					files.extend(sub_files);
				}
			}
		}

		Ok(files)
	}
}

/// A storage backend that renames files with a content hash for cache busting.
pub struct HashedFileStorage {
	/// The root directory where hashed files are stored.
	pub location: PathBuf,
	/// The base URL prefix used to generate file URLs.
	pub base_url: String,
	hashed_files: Arc<RwLock<HashMap<String, String>>>,
}

impl HashedFileStorage {
	/// Creates a new hashed file storage rooted at the given location.
	pub fn new<P: Into<PathBuf>>(location: P, base_url: &str) -> Self {
		Self {
			location: location.into(),
			base_url: base_url.to_string(),
			hashed_files: Arc::new(RwLock::new(HashMap::new())),
		}
	}

	fn hash_content(content: &[u8]) -> String {
		use std::collections::hash_map::DefaultHasher;
		use std::hash::{Hash, Hasher};
		let mut hasher = DefaultHasher::new();
		content.hash(&mut hasher);
		format!("{:x}", hasher.finish())
	}

	fn get_hashed_name(&self, name: &str, content: &[u8]) -> String {
		let hash = Self::hash_content(content);
		let hash_short = &hash[..12];
		if let Some(dot_pos) = name.rfind('.') {
			format!("{}.{}{}", &name[..dot_pos], hash_short, &name[dot_pos..])
		} else {
			format!("{}.{}", name, hash_short)
		}
	}

	/// Saves a file with a content-hashed filename and returns the hashed name.
	pub async fn save(&self, name: &str, content: &[u8]) -> io::Result<String> {
		let hashed_name = self.get_hashed_name(name, content);
		let file_path = self.location.join(&hashed_name);

		if let Some(parent) = file_path.parent() {
			tokio::fs::create_dir_all(parent).await?;
		}

		tokio::fs::write(&file_path, content).await?;

		let mut hashed_files = write_or_recover(&self.hashed_files, "HashedFileStorage::save");
		hashed_files.insert(name.to_string(), hashed_name.clone());

		Ok(hashed_name)
	}

	/// Saves multiple files with inter-file dependency resolution (e.g., CSS URL rewriting).
	///
	/// Returns the number of files processed.
	pub async fn save_with_dependencies(
		&self,
		files: HashMap<String, Vec<u8>>,
	) -> io::Result<usize> {
		let mut hashed_map = HashMap::new();
		let mut processed_files = HashMap::new();

		// First pass: hash all files to build the mapping
		for (name, content) in &files {
			let hashed_name = self.get_hashed_name(name, content);
			hashed_map.insert(name.clone(), hashed_name);
		}

		// Second pass: process CSS files to update references, then save all files
		for (name, content) in files {
			let mut final_content = content;

			// If it's a CSS file, update URL references
			if name.ends_with(".css") {
				let content_str = String::from_utf8_lossy(&final_content);
				let mut updated = content_str.to_string();

				// Replace all references to other files with their hashed names
				for (orig_name, hashed_name) in &hashed_map {
					if orig_name != &name {
						updated = updated.replace(orig_name, hashed_name);
					}
				}

				final_content = updated.into_bytes();
			}

			let hashed_name = hashed_map.get(&name).unwrap();
			let file_path = self.location.join(hashed_name);

			if let Some(parent) = file_path.parent() {
				tokio::fs::create_dir_all(parent).await?;
			}

			tokio::fs::write(&file_path, &final_content).await?;
			processed_files.insert(name, hashed_name.clone());
		}

		// Update the internal mapping
		let mut hashed_files = write_or_recover(
			&self.hashed_files,
			"HashedFileStorage::save_with_dependencies",
		);
		for (orig, hashed) in processed_files {
			hashed_files.insert(orig, hashed);
		}

		Ok(hashed_map.len())
	}

	/// Opens and reads the content of a previously saved file by its original name.
	pub async fn open(&self, name: &str) -> io::Result<Vec<u8>> {
		let hashed_name = {
			let hashed_files = read_or_recover(&self.hashed_files, "HashedFileStorage::open");
			hashed_files
				.get(name)
				.ok_or_else(|| {
					io::Error::new(io::ErrorKind::NotFound, "File not found in mapping")
				})?
				.clone()
		};

		let file_path = self.location.join(&hashed_name);
		tokio::fs::read(file_path).await
	}

	/// Returns the URL for a file, using the hashed name if available.
	pub fn url(&self, name: &str) -> String {
		let hashed_files = read_or_recover(&self.hashed_files, "HashedFileStorage::url");
		if let Some(hashed_name) = hashed_files.get(name) {
			format!("{}{}", self.base_url, hashed_name)
		} else {
			format!("{}{}", self.base_url, name)
		}
	}

	/// Returns whether a file with the given name exists in the hashed storage.
	pub fn exists(&self, name: &str) -> bool {
		let hashed_files = read_or_recover(&self.hashed_files, "HashedFileStorage::exists");
		if let Some(hashed_name) = hashed_files.get(name) {
			self.location.join(hashed_name).exists()
		} else {
			false
		}
	}

	/// Returns the hashed filename for the given original name, if available.
	pub fn get_hashed_path(&self, name: &str) -> Option<String> {
		let hashed_files =
			read_or_recover(&self.hashed_files, "HashedFileStorage::get_hashed_path");
		hashed_files.get(name).cloned()
	}
}

/// Manifest file format version.
pub enum ManifestVersion {
	/// Version 1 of the manifest format.
	V1,
}

/// Manifest file structure that maps original filenames to hashed filenames.
pub struct Manifest {
	/// The manifest format version.
	pub version: ManifestVersion,
	/// Mapping from original file paths to their hashed counterparts.
	pub paths: std::collections::HashMap<String, String>,
}

/// A storage backend that persists a JSON manifest mapping original names to hashed names.
pub struct ManifestStaticFilesStorage {
	/// The root directory where files and the manifest are stored.
	pub location: PathBuf,
	/// The base URL prefix used to generate file URLs.
	pub base_url: String,
	/// The filename of the manifest file (default: `"staticfiles.json"`).
	pub manifest_name: String,
	/// If true, lookups for unmapped files will fail rather than fall back.
	pub manifest_strict: bool,
	hashed_files: Arc<RwLock<HashMap<String, String>>>,
}

impl ManifestStaticFilesStorage {
	/// Creates a new manifest-based storage at the given location.
	pub fn new<P: Into<PathBuf>>(location: P, base_url: &str) -> Self {
		Self {
			location: location.into(),
			base_url: base_url.to_string(),
			manifest_name: "staticfiles.json".to_string(),
			manifest_strict: true,
			hashed_files: Arc::new(RwLock::new(HashMap::new())),
		}
	}

	/// Configures whether strict mode is enabled for manifest lookups.
	pub fn with_manifest_strict(mut self, strict: bool) -> Self {
		self.manifest_strict = strict;
		self
	}

	fn hash_content(content: &[u8]) -> String {
		let mut hasher = DefaultHasher::new();
		content.hash(&mut hasher);
		format!("{:x}", hasher.finish())
	}

	fn get_hashed_name(&self, name: &str, content: &[u8]) -> String {
		let hash = Self::hash_content(content);
		let hash_short = &hash[..12];

		if let Some(dot_pos) = name.rfind('.') {
			format!("{}.{}{}", &name[..dot_pos], hash_short, &name[dot_pos..])
		} else {
			format!("{}.{}", name, hash_short)
		}
	}

	fn normalize_path(&self, name: &str) -> PathBuf {
		let name = name.trim_start_matches('/');
		self.location.join(name)
	}

	fn normalize_url(&self, base: &str, name: &str) -> String {
		let base = base.trim_end_matches('/');
		let name = name.trim_start_matches('/');
		format!("{}/{}", base, name)
	}

	/// Save multiple files with dependency resolution
	pub async fn save_with_dependencies(
		&self,
		files: HashMap<String, Vec<u8>>,
	) -> io::Result<usize> {
		let mut hashed_map = HashMap::new();
		let mut processed_files = HashMap::new();

		// First pass: hash all files and create mapping
		for (name, content) in &files {
			let hashed_name = self.get_hashed_name(name, content);
			hashed_map.insert(name.clone(), hashed_name);
		}

		// Second pass: update CSS references and save files
		for (name, content) in files {
			let mut final_content = content;

			// If this is a CSS file, update image references
			if name.ends_with(".css") {
				let content_str = String::from_utf8_lossy(&final_content);
				let mut updated = content_str.to_string();

				// Update all url() references
				for (orig_name, hashed_name) in &hashed_map {
					if orig_name != &name {
						updated = updated.replace(orig_name, hashed_name);
					}
				}

				final_content = updated.into_bytes();
			}

			let hashed_name = hashed_map.get(&name).unwrap();
			let file_path = self.normalize_path(hashed_name);

			if let Some(parent) = file_path.parent() {
				tokio::fs::create_dir_all(parent).await?;
			}

			tokio::fs::write(&file_path, &final_content).await?;
			processed_files.insert(name, hashed_name.clone());
		}

		// Update internal mapping
		{
			let mut hashed_files = write_or_recover(
				&self.hashed_files,
				"ManifestStaticFilesStorage::save_with_dependencies",
			);
			hashed_files.extend(processed_files);
		}

		// Save manifest
		self.save_manifest().await?;

		Ok(hashed_map.len())
	}

	async fn save_manifest(&self) -> io::Result<()> {
		let (manifest_path, manifest_json) = {
			let hashed_files = read_or_recover(
				&self.hashed_files,
				"ManifestStaticFilesStorage::save_manifest",
			);
			let manifest_path = self.normalize_path(&self.manifest_name);

			// Create manifest with "paths" key to match Django's manifest structure
			let manifest_data = serde_json::json!({
				"paths": *hashed_files
			});

			let manifest_json =
				serde_json::to_string_pretty(&manifest_data).map_err(io::Error::other)?;

			(manifest_path, manifest_json)
		};

		tokio::fs::write(manifest_path, manifest_json).await
	}

	/// Load manifest from disk
	pub async fn load_manifest(&self) -> io::Result<()> {
		let manifest_path = self.normalize_path(&self.manifest_name);

		if !manifest_path.exists() {
			// No manifest file exists yet, that's okay
			return Ok(());
		}

		let manifest_content = tokio::fs::read_to_string(manifest_path).await?;
		let manifest_data: serde_json::Value =
			serde_json::from_str(&manifest_content).map_err(io::Error::other)?;

		// Extract "paths" object from manifest
		if let Some(paths) = manifest_data.get("paths").and_then(|p| p.as_object()) {
			let mut hashed_files = write_or_recover(
				&self.hashed_files,
				"ManifestStaticFilesStorage::load_manifest",
			);
			for (key, value) in paths {
				if let Some(hashed_name) = value.as_str() {
					hashed_files.insert(key.clone(), hashed_name.to_string());
				}
			}
		}

		Ok(())
	}

	/// Get the hashed path for a given file
	pub fn get_hashed_path(&self, name: &str) -> Option<String> {
		let hashed_files = read_or_recover(
			&self.hashed_files,
			"ManifestStaticFilesStorage::get_hashed_path",
		);
		hashed_files.get(name).cloned()
	}

	/// Returns whether a file with the given name exists (checking both hashed and original paths).
	pub fn exists(&self, name: &str) -> bool {
		// First check if we have a hashed version of this file
		let hashed_files =
			read_or_recover(&self.hashed_files, "ManifestStaticFilesStorage::exists");
		if let Some(hashed_name) = hashed_files.get(name) {
			// Check hashed file path
			let hashed_path = self.normalize_path(hashed_name);
			if hashed_path.exists() {
				return true;
			}
		}
		drop(hashed_files);

		// Fall back to checking original path
		self.normalize_path(name).exists()
	}

	/// Open a file by its original name
	pub async fn open(&self, name: &str) -> io::Result<Vec<u8>> {
		let actual_name = {
			let hashed_files =
				read_or_recover(&self.hashed_files, "ManifestStaticFilesStorage::open");
			hashed_files
				.get(name)
				.cloned()
				.unwrap_or_else(|| name.to_string())
		};

		let file_path = self.normalize_path(&actual_name);
		tokio::fs::read(file_path).await
	}

	/// Get URL for a file
	pub fn url(&self, name: &str) -> String {
		let hashed_files = read_or_recover(&self.hashed_files, "ManifestStaticFilesStorage::url");
		let actual_name = hashed_files
			.get(name)
			.cloned()
			.unwrap_or_else(|| name.to_string());
		drop(hashed_files);

		self.normalize_url(&self.base_url, &actual_name)
	}
}
