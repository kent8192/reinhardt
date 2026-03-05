use async_trait::async_trait;
use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::io;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use tracing;

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
	async fn save(&self, name: &str, content: &[u8]) -> io::Result<String>;
	fn exists(&self, name: &str) -> bool;
	async fn open(&self, name: &str) -> io::Result<Vec<u8>>;
	async fn delete(&self, name: &str) -> io::Result<()>;
	fn url(&self, name: &str) -> String;
}

pub struct FileSystemStorage {
	pub location: PathBuf,
	pub base_url: String,
}

impl FileSystemStorage {
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

pub struct MemoryStorage {
	base_url: String,
	files: Arc<RwLock<HashMap<String, Vec<u8>>>>,
}

impl MemoryStorage {
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
		let mut files = self.files.write().unwrap();
		files.insert(name.to_string(), content.to_vec());
		Ok(self.url(name))
	}

	fn exists(&self, name: &str) -> bool {
		self.files.read().unwrap().contains_key(name)
	}

	async fn open(&self, name: &str) -> io::Result<Vec<u8>> {
		self.files
			.read()
			.unwrap()
			.get(name)
			.cloned()
			.ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "File not found"))
	}

	async fn delete(&self, name: &str) -> io::Result<()> {
		self.files.write().unwrap().remove(name);
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

#[derive(Debug, Clone)]
pub struct StaticFilesConfig {
	pub static_root: PathBuf,
	pub static_url: String,
	pub staticfiles_dirs: Vec<PathBuf>,
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

pub struct StaticFilesFinder {
	pub directories: Vec<PathBuf>,
}

impl StaticFilesFinder {
	pub fn new(directories: Vec<PathBuf>) -> Self {
		Self { directories }
	}

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

pub struct HashedFileStorage {
	pub location: PathBuf,
	pub base_url: String,
	hashed_files: Arc<RwLock<HashMap<String, String>>>,
}

impl HashedFileStorage {
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

	pub async fn save(&self, name: &str, content: &[u8]) -> io::Result<String> {
		let hashed_name = self.get_hashed_name(name, content);
		let file_path = self.location.join(&hashed_name);

		if let Some(parent) = file_path.parent() {
			tokio::fs::create_dir_all(parent).await?;
		}

		tokio::fs::write(&file_path, content).await?;

		let mut hashed_files = self.hashed_files.write().unwrap();
		hashed_files.insert(name.to_string(), hashed_name.clone());

		Ok(hashed_name)
	}

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
		let mut hashed_files = self.hashed_files.write().unwrap();
		for (orig, hashed) in processed_files {
			hashed_files.insert(orig, hashed);
		}

		Ok(hashed_map.len())
	}

	pub async fn open(&self, name: &str) -> io::Result<Vec<u8>> {
		let hashed_name = {
			let hashed_files = self.hashed_files.read().unwrap();
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

	pub fn url(&self, name: &str) -> String {
		let hashed_files = self.hashed_files.read().unwrap();
		if let Some(hashed_name) = hashed_files.get(name) {
			format!("{}{}", self.base_url, hashed_name)
		} else {
			format!("{}{}", self.base_url, name)
		}
	}

	pub fn exists(&self, name: &str) -> bool {
		let hashed_files = self.hashed_files.read().unwrap();
		if let Some(hashed_name) = hashed_files.get(name) {
			self.location.join(hashed_name).exists()
		} else {
			false
		}
	}

	pub fn get_hashed_path(&self, name: &str) -> Option<String> {
		let hashed_files = self.hashed_files.read().unwrap();
		hashed_files.get(name).cloned()
	}
}

/// Manifest file format version
pub enum ManifestVersion {
	V1,
}

/// Manifest file structure for static files
pub struct Manifest {
	pub version: ManifestVersion,
	pub paths: std::collections::HashMap<String, String>,
}

pub struct ManifestStaticFilesStorage {
	pub location: PathBuf,
	pub base_url: String,
	pub manifest_name: String,
	pub manifest_strict: bool,
	hashed_files: Arc<RwLock<HashMap<String, String>>>,
}

impl ManifestStaticFilesStorage {
	pub fn new<P: Into<PathBuf>>(location: P, base_url: &str) -> Self {
		Self {
			location: location.into(),
			base_url: base_url.to_string(),
			manifest_name: "staticfiles.json".to_string(),
			manifest_strict: true,
			hashed_files: Arc::new(RwLock::new(HashMap::new())),
		}
	}

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
			let mut hashed_files = self.hashed_files.write().unwrap();
			hashed_files.extend(processed_files);
		}

		// Save manifest
		self.save_manifest().await?;

		Ok(hashed_map.len())
	}

	async fn save_manifest(&self) -> io::Result<()> {
		let (manifest_path, manifest_json) = {
			let hashed_files = self.hashed_files.read().unwrap();
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
			let mut hashed_files = self.hashed_files.write().unwrap();
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
		let hashed_files = self.hashed_files.read().unwrap();
		hashed_files.get(name).cloned()
	}

	pub fn exists(&self, name: &str) -> bool {
		// First check if we have a hashed version of this file
		let hashed_files = self.hashed_files.read().unwrap();
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
			let hashed_files = self.hashed_files.read().unwrap();
			hashed_files.get(name).unwrap_or(&name.to_string()).clone()
		};

		let file_path = self.normalize_path(&actual_name);
		tokio::fs::read(file_path).await
	}

	/// Get URL for a file
	pub fn url(&self, name: &str) -> String {
		let hashed_files = self.hashed_files.read().unwrap();
		let actual_name = hashed_files.get(name).unwrap_or(&name.to_string()).clone();
		drop(hashed_files);

		self.normalize_url(&self.base_url, &actual_name)
	}
}
