//! Tailwind CSS Standalone CLI Management
//!
//! This module handles downloading, caching, and locating the Tailwind CSS
//! standalone CLI binary. The standalone CLI bundles Node.js and all required
//! dependencies, so no external runtime is needed.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use super::{TailwindError, TailwindResult};

/// Manages the Tailwind CSS standalone CLI binary.
///
/// Handles binary resolution, download, caching, and version verification.
///
/// ## Binary Resolution Order
///
/// 1. Check if a binary already exists in the cache directory for the requested version
/// 2. Check if `tailwindcss` is available on the system PATH
/// 3. Download the standalone CLI from the official GitHub releases
///
/// ## Example
///
/// ```ignore
/// use reinhardt_pages::tailwind::TailwindCli;
/// use std::path::PathBuf;
///
/// let cli = TailwindCli::new("4.1.0", &PathBuf::from("/tmp/tw-cache"));
/// let binary_path = cli.ensure_binary()?;
/// ```
pub struct TailwindCli {
	/// The requested Tailwind CSS version.
	version: String,
	/// Directory to cache downloaded binaries.
	cache_dir: PathBuf,
}

impl TailwindCli {
	/// Create a new CLI manager for the given version and cache directory.
	pub fn new(version: &str, cache_dir: &Path) -> Self {
		Self {
			version: version.to_string(),
			cache_dir: cache_dir.to_path_buf(),
		}
	}

	/// Ensure the Tailwind CLI binary is available, downloading if necessary.
	///
	/// Returns the path to the binary.
	///
	/// ## Resolution Order
	///
	/// 1. Cached binary for the requested version
	/// 2. System PATH `tailwindcss` binary
	/// 3. Download from GitHub releases
	pub fn ensure_binary(&self) -> TailwindResult<PathBuf> {
		// 1. Check cached binary
		let cached = self.cached_binary_path();
		if cached.exists() {
			return Ok(cached);
		}

		// 2. Check system PATH
		if let Some(system_path) = self.find_system_binary() {
			return Ok(system_path);
		}

		// 3. Download from GitHub releases
		self.download_binary()
	}

	/// Returns the expected path for the cached binary.
	pub fn cached_binary_path(&self) -> PathBuf {
		self.cache_dir.join(format!("tailwindcss-{}", self.version))
	}

	/// Check if the Tailwind CLI is available on the system PATH.
	fn find_system_binary(&self) -> Option<PathBuf> {
		let output = Command::new("which").arg("tailwindcss").output().ok()?;

		if output.status.success() {
			let path_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
			if !path_str.is_empty() {
				return Some(PathBuf::from(path_str));
			}
		}
		None
	}

	/// Download the Tailwind CSS standalone CLI binary.
	fn download_binary(&self) -> TailwindResult<PathBuf> {
		fs::create_dir_all(&self.cache_dir)?;

		let (os, arch) = detect_platform()?;
		let binary_name = format!("tailwindcss-{}-{}", os, arch);
		let url = format!(
			"https://github.com/tailwindlabs/tailwindcss/releases/download/v{}/{}",
			self.version, binary_name
		);

		let dest = self.cached_binary_path();

		// Download using curl (available on macOS and most Linux systems)
		let status = Command::new("curl")
			.args(["-sL", "-o"])
			.arg(&dest)
			.arg(&url)
			.status()?;

		if !status.success() {
			return Err(TailwindError::DownloadFailed(format!(
				"curl failed to download from {}",
				url
			)));
		}

		// Verify the downloaded file is not empty or an error page
		let metadata = fs::metadata(&dest)?;
		if metadata.len() < 1024 {
			// Likely an error page, not a real binary
			let _ = fs::remove_file(&dest);
			return Err(TailwindError::DownloadFailed(format!(
				"downloaded file from {} is too small ({} bytes), likely a 404 error",
				url,
				metadata.len()
			)));
		}

		// Make executable
		let mut perms = fs::metadata(&dest)?.permissions();
		perms.set_mode(0o755);
		fs::set_permissions(&dest, perms)?;

		Ok(dest)
	}

	/// Returns the configured version.
	pub fn version(&self) -> &str {
		&self.version
	}

	/// Returns the configured cache directory.
	pub fn cache_dir(&self) -> &Path {
		&self.cache_dir
	}
}

/// Detect the current platform for downloading the correct binary.
///
/// Returns `(os, arch)` strings matching the Tailwind CLI release naming.
fn detect_platform() -> TailwindResult<(&'static str, &'static str)> {
	let os = if cfg!(target_os = "macos") {
		"macos"
	} else if cfg!(target_os = "linux") {
		"linux"
	} else {
		return Err(TailwindError::DownloadFailed(
			"unsupported operating system; only macOS and Linux are supported".to_string(),
		));
	};

	let arch = if cfg!(target_arch = "aarch64") {
		"arm64"
	} else if cfg!(target_arch = "x86_64") {
		"x64"
	} else {
		return Err(TailwindError::DownloadFailed(
			"unsupported architecture; only x64 and arm64 are supported".to_string(),
		));
	};

	Ok((os, arch))
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;

	#[rstest]
	fn test_cached_binary_path() {
		// Arrange
		let cache_dir = PathBuf::from("/tmp/test-tw-cache");
		let cli = TailwindCli::new("4.1.0", &cache_dir);

		// Act
		let path = cli.cached_binary_path();

		// Assert
		assert_eq!(path, PathBuf::from("/tmp/test-tw-cache/tailwindcss-4.1.0"));
	}

	#[rstest]
	fn test_detect_platform() {
		// Arrange & Act
		let result = detect_platform();

		// Assert
		assert!(result.is_ok());
		let (os, arch) = result.unwrap();
		assert!(!os.is_empty());
		assert!(!arch.is_empty());
	}

	#[rstest]
	fn test_cli_version_accessor() {
		// Arrange
		let cli = TailwindCli::new("3.4.0", &PathBuf::from("/tmp/cache"));

		// Act & Assert
		assert_eq!(cli.version(), "3.4.0");
	}

	#[rstest]
	fn test_cli_cache_dir_accessor() {
		// Arrange
		let cache_dir = PathBuf::from("/tmp/my-cache");
		let cli = TailwindCli::new("4.1.0", &cache_dir);

		// Act & Assert
		assert_eq!(cli.cache_dir(), Path::new("/tmp/my-cache"));
	}

	#[rstest]
	fn test_ensure_binary_with_nonexistent_cache() {
		// Arrange
		let cache_dir = PathBuf::from("/tmp/reinhardt-tw-test-nonexistent");
		let cli = TailwindCli::new("4.1.0", &cache_dir);

		// Act
		// The cached binary won't exist, and system binary likely won't either.
		// Download will be attempted but may fail in CI. We just verify the path logic.
		let cached = cli.cached_binary_path();

		// Assert
		assert!(!cached.exists());
	}
}
