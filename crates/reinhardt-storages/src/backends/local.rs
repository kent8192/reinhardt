//! Local file system storage backend implementation.

#![allow(deprecated)] // Backend constructor keeps accepting legacy config during compatibility.

use async_trait::async_trait;
use cap_fs_ext::{DirExt, FollowSymlinks, OpenOptionsFollowExt};
use cap_std::ambient_authority;
use cap_std::fs::{Dir, OpenOptions};
use chrono::{DateTime, Utc};
use std::fmt;
use std::io::Write;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use tokio::fs;

use crate::config::LocalConfig;
use crate::{Result, StorageBackend, StorageCapabilities, StorageError};

/// Validate that the given name does not escape the storage root.
///
/// Rejects empty strings, absolute paths, parent directory references (`..`),
/// and degenerate directory-only references (`.` and `..`).
fn validate_path(name: &str) -> Result<&str> {
	if name.is_empty() {
		return Err(StorageError::InvalidPath(
			"path must not be empty".to_string(),
		));
	}

	let path = Path::new(name);

	if path.is_absolute() || name.starts_with('\\') {
		return Err(StorageError::InvalidPath(format!(
			"absolute paths are not allowed: {name}"
		)));
	}

	// Reject Windows drive-letter absolute paths on all platforms for
	// consistent behavior (on Unix, `Path::is_absolute` misses these).
	if let Some(rest) = name.as_bytes().get(1..)
		&& name.as_bytes()[0].is_ascii_alphabetic()
		&& rest.first() == Some(&b':')
		&& rest.get(1).is_some_and(|&b| b == b'/' || b == b'\\')
	{
		return Err(StorageError::InvalidPath(format!(
			"absolute paths are not allowed: {name}"
		)));
	}

	for component in path.components() {
		match component {
			Component::ParentDir => {
				return Err(StorageError::InvalidPath(format!(
					"parent directory references are not allowed: {name}"
				)));
			}
			Component::RootDir | Component::Prefix(_) => {
				return Err(StorageError::InvalidPath(format!(
					"absolute paths are not allowed: {name}"
				)));
			}
			_ => {}
		}
	}

	if name == "." || name == ".." {
		return Err(StorageError::InvalidPath(format!(
			"path must refer to a file, not a directory reference: {name}"
		)));
	}

	Ok(name)
}

/// Local file system storage backend.
#[derive(Clone)]
pub struct LocalStorage {
	base_path: PathBuf,
	canonical_base: PathBuf,
	base_dir: Arc<Dir>,
}

impl fmt::Debug for LocalStorage {
	fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
		formatter
			.debug_struct("LocalStorage")
			.field("base_path", &self.base_path)
			.field("canonical_base", &self.canonical_base)
			.finish_non_exhaustive()
	}
}

impl LocalStorage {
	/// Create a new local storage backend.
	///
	/// # Arguments
	///
	/// * `config` - Local storage configuration
	///
	/// # Errors
	///
	/// Returns `` `StorageError::ConfigError` `` if the base path is invalid.
	pub fn new(config: LocalConfig) -> Result<Self> {
		let base_path = PathBuf::from(config.base_path);

		if !base_path.exists() {
			return Err(StorageError::ConfigError(format!(
				"Base path does not exist: {}",
				base_path.display()
			)));
		}

		if !base_path.is_dir() {
			return Err(StorageError::ConfigError(format!(
				"Base path is not a directory: {}",
				base_path.display()
			)));
		}

		let canonical_base = base_path.canonicalize().map_err(|e| {
			StorageError::ConfigError(format!(
				"Failed to canonicalize base path {}: {e}",
				base_path.display()
			))
		})?;
		let base_dir =
			Dir::open_ambient_dir(&canonical_base, ambient_authority()).map_err(|e| {
				StorageError::ConfigError(format!(
					"Failed to open base path {}: {e}",
					canonical_base.display()
				))
			})?;

		Ok(Self {
			base_path,
			canonical_base,
			base_dir: Arc::new(base_dir),
		})
	}

	/// Get the full file path after validating it does not escape the storage root.
	fn get_path(&self, name: &str) -> Result<PathBuf> {
		let validated = validate_path(name)?;
		Ok(self.base_path.join(validated))
	}

	/// Verify that a resolved path is contained within the canonical base.
	fn check_containment(&self, canonical_path: &Path) -> Result<()> {
		if !canonical_path.starts_with(&self.canonical_base) {
			return Err(StorageError::InvalidPath(
				"resolved path escapes storage root".to_string(),
			));
		}
		Ok(())
	}
}

fn write_file_if_absent(base_dir: Dir, name: String, content: Vec<u8>) -> Result<String> {
	let path = Path::new(&name);
	let components: Vec<_> = path
		.components()
		.filter_map(|component| match component {
			Component::Normal(component) => Some(component),
			Component::CurDir => None,
			Component::ParentDir | Component::RootDir | Component::Prefix(_) => None,
		})
		.collect();
	let (file_name, parent_components) = components
		.split_last()
		.expect("validated file paths always include a normal component");

	let mut directory = base_dir;
	for component in parent_components {
		directory = match directory.open_dir_nofollow(component) {
			Ok(next) => next,
			Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
				directory.create_dir(component)?;
				directory.open_dir_nofollow(component)?
			}
			Err(error) => return Err(StorageError::IoError(error)),
		};
	}

	if directory
		.symlink_metadata(file_name)
		.is_ok_and(|metadata| metadata.file_type().is_symlink())
	{
		return Err(StorageError::InvalidPath(format!(
			"symbolic links are not allowed: {name}"
		)));
	}

	let mut options = OpenOptions::new();
	options
		.write(true)
		.create_new(true)
		.follow(FollowSymlinks::No);
	let mut file = directory.open_with(file_name, &options).map_err(|error| {
		if error.kind() == std::io::ErrorKind::AlreadyExists {
			StorageError::AlreadyExists(name.clone())
		} else {
			StorageError::IoError(error)
		}
	})?;
	file.write_all(&content)?;
	file.flush()?;

	Ok(name)
}

#[async_trait]
impl StorageBackend for LocalStorage {
	async fn save(&self, name: &str, content: &[u8]) -> Result<String> {
		let path = self.get_path(name)?;

		if let Some(parent) = path.parent() {
			fs::create_dir_all(parent).await?;
			let canonical_parent = parent.canonicalize()?;
			self.check_containment(&canonical_parent)?;
		}

		fs::write(&path, content).await?;

		Ok(name.to_string())
	}

	async fn save_if_absent(&self, name: &str, content: &[u8]) -> Result<String> {
		validate_path(name)?;
		let base_dir = self.base_dir.try_clone()?;
		let name = name.to_owned();
		let content = content.to_vec();

		tokio::task::spawn_blocking(move || write_file_if_absent(base_dir, name, content))
			.await
			.map_err(|error| {
				StorageError::Other(format!("exclusive create task failed: {error}"))
			})?
	}

	fn capabilities(&self) -> StorageCapabilities {
		StorageCapabilities {
			exclusive_create: true,
		}
	}

	async fn open(&self, name: &str) -> Result<Vec<u8>> {
		let path = self.get_path(name)?;

		if !path.exists() {
			return Err(StorageError::NotFound(name.to_string()));
		}

		let canonical = path.canonicalize()?;
		self.check_containment(&canonical)?;

		let content = fs::read(&canonical).await?;
		Ok(content)
	}

	async fn delete(&self, name: &str) -> Result<()> {
		let path = self.get_path(name)?;

		if !path.exists() {
			return Err(StorageError::NotFound(name.to_string()));
		}

		let canonical = path.canonicalize()?;
		self.check_containment(&canonical)?;

		fs::remove_file(&canonical).await?;
		Ok(())
	}

	async fn exists(&self, name: &str) -> Result<bool> {
		let path = self.get_path(name)?;

		if !path.exists() || !path.is_file() {
			return Ok(false);
		}

		let canonical = path.canonicalize()?;
		self.check_containment(&canonical)?;

		Ok(true)
	}

	async fn url(&self, name: &str, _expiry_secs: u64) -> Result<String> {
		let path = self.get_path(name)?;

		if !path.exists() {
			return Err(StorageError::NotFound(name.to_string()));
		}

		let canonical = path.canonicalize()?;
		self.check_containment(&canonical)?;

		Ok(format!("file://{}", canonical.display()))
	}

	async fn size(&self, name: &str) -> Result<u64> {
		let path = self.get_path(name)?;

		if !path.exists() {
			return Err(StorageError::NotFound(name.to_string()));
		}

		let canonical = path.canonicalize()?;
		self.check_containment(&canonical)?;

		let metadata = fs::metadata(&canonical).await?;
		Ok(metadata.len())
	}

	async fn get_modified_time(&self, name: &str) -> Result<DateTime<Utc>> {
		let path = self.get_path(name)?;

		if !path.exists() {
			return Err(StorageError::NotFound(name.to_string()));
		}

		let canonical = path.canonicalize()?;
		self.check_containment(&canonical)?;

		let metadata = fs::metadata(&canonical).await?;
		let modified = metadata.modified()?;

		let datetime: DateTime<Utc> = modified.into();
		Ok(datetime)
	}
}
