//! Configuration for WhiteNoise behavior

use std::path::PathBuf;

/// Configuration for WhiteNoise static file serving
#[derive(Debug, Clone)]
pub struct WhiteNoiseConfig {
	/// Root directory for static files
	pub root: PathBuf,

	/// URL prefix for static files (e.g., "/static/")
	pub static_url: String,

	/// Maximum cache age for immutable files (seconds)
	/// Default: 31536000 (1 year)
	pub max_age_immutable: u32,

	/// Maximum cache age for mutable files (seconds)
	/// Default: 60
	pub max_age_mutable: u32,

	/// Enable gzip compression
	pub enable_gzip: bool,

	/// Gzip compression level (0-9)
	/// Default: 6
	pub gzip_level: u32,

	/// Enable brotli compression
	pub enable_brotli: bool,

	/// Brotli quality level (0-11)
	/// Default: 11
	pub brotli_quality: u32,

	/// Minimum file size to compress (bytes)
	/// Default: 1024
	pub min_compress_size: usize,

	/// File extensions to compress
	pub compress_extensions: Vec<String>,

	/// File extensions to exclude from compression
	pub exclude_extensions: Vec<String>,

	/// Enable CORS headers
	pub allow_all_origins: bool,

	/// Path to manifest file
	pub manifest_path: Option<PathBuf>,

	/// Follow symlinks during scanning
	pub follow_symlinks: bool,
}

impl Default for WhiteNoiseConfig {
	fn default() -> Self {
		Self {
			root: PathBuf::from("static"),
			static_url: "/static/".to_string(),
			max_age_immutable: 31536000, // 1 year
			max_age_mutable: 60,
			enable_gzip: true,
			gzip_level: 6,
			enable_brotli: true,
			brotli_quality: 11,
			min_compress_size: 1024,
			compress_extensions: vec![
				"css".to_string(),
				"js".to_string(),
				"html".to_string(),
				"json".to_string(),
				"xml".to_string(),
				"svg".to_string(),
				"txt".to_string(),
			],
			exclude_extensions: vec![
				"jpg".to_string(),
				"jpeg".to_string(),
				"png".to_string(),
				"gif".to_string(),
				"zip".to_string(),
				"woff".to_string(),
				"woff2".to_string(),
				"gz".to_string(),
				"br".to_string(),
			],
			allow_all_origins: false,
			manifest_path: None,
			follow_symlinks: false,
		}
	}
}

impl WhiteNoiseConfig {
	/// Creates a new WhiteNoise configuration
	///
	/// # Arguments
	///
	/// * `root` - Root directory for static files
	/// * `static_url` - URL prefix for static files
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_whitenoise::WhiteNoiseConfig;
	/// use std::path::PathBuf;
	///
	/// let config = WhiteNoiseConfig::new(
	///     PathBuf::from("static"),
	///     "/static/".to_string(),
	/// );
	/// ```
	pub fn new(root: PathBuf, static_url: String) -> Self {
		Self {
			root,
			static_url,
			..Default::default()
		}
	}

	/// Enables or disables compression
	///
	/// # Arguments
	///
	/// * `gzip` - Enable gzip compression
	/// * `brotli` - Enable brotli compression
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_whitenoise::WhiteNoiseConfig;
	/// use std::path::PathBuf;
	///
	/// let config = WhiteNoiseConfig::new(PathBuf::from("static"), "/static/".to_string())
	///     .with_compression(true, true);
	/// ```
	pub fn with_compression(mut self, gzip: bool, brotli: bool) -> Self {
		self.enable_gzip = gzip;
		self.enable_brotli = brotli;
		self
	}

	/// Sets maximum cache age for immutable files
	///
	/// # Arguments
	///
	/// * `max_age` - Maximum age in seconds
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_whitenoise::WhiteNoiseConfig;
	/// use std::path::PathBuf;
	///
	/// let config = WhiteNoiseConfig::new(PathBuf::from("static"), "/static/".to_string())
	///     .with_max_age_immutable(31536000); // 1 year
	/// ```
	pub fn with_max_age_immutable(mut self, max_age: u32) -> Self {
		self.max_age_immutable = max_age;
		self
	}

	/// Sets maximum cache age for mutable files
	///
	/// # Arguments
	///
	/// * `max_age` - Maximum age in seconds
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_whitenoise::WhiteNoiseConfig;
	/// use std::path::PathBuf;
	///
	/// let config = WhiteNoiseConfig::new(PathBuf::from("static"), "/static/".to_string())
	///     .with_max_age_mutable(60);
	/// ```
	pub fn with_max_age_mutable(mut self, max_age: u32) -> Self {
		self.max_age_mutable = max_age;
		self
	}

	/// Enables CORS headers
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_whitenoise::WhiteNoiseConfig;
	/// use std::path::PathBuf;
	///
	/// let config = WhiteNoiseConfig::new(PathBuf::from("static"), "/static/".to_string())
	///     .with_cors(true);
	/// ```
	pub fn with_cors(mut self, enable: bool) -> Self {
		self.allow_all_origins = enable;
		self
	}

	/// Sets path to manifest file
	///
	/// # Arguments
	///
	/// * `path` - Path to manifest.json file
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_whitenoise::WhiteNoiseConfig;
	/// use std::path::PathBuf;
	///
	/// let config = WhiteNoiseConfig::new(PathBuf::from("static"), "/static/".to_string())
	///     .with_manifest(PathBuf::from("staticfiles/manifest.json"));
	/// ```
	pub fn with_manifest(mut self, path: PathBuf) -> Self {
		self.manifest_path = Some(path);
		self
	}

	/// Validates the configuration
	///
	/// # Errors
	///
	/// Returns error if configuration is invalid
	pub fn validate(&self) -> crate::Result<()> {
		// Check if root directory exists
		if !self.root.exists() {
			return Err(crate::WhiteNoiseError::InvalidConfig(format!(
				"Static root directory does not exist: {}",
				self.root.display()
			)));
		}

		// Check if root is a directory
		if !self.root.is_dir() {
			return Err(crate::WhiteNoiseError::InvalidConfig(format!(
				"Static root is not a directory: {}",
				self.root.display()
			)));
		}

		// Validate compression levels
		if self.gzip_level > 9 {
			return Err(crate::WhiteNoiseError::InvalidConfig(format!(
				"Invalid gzip level: {} (must be 0-9)",
				self.gzip_level
			)));
		}

		if self.brotli_quality > 11 {
			return Err(crate::WhiteNoiseError::InvalidConfig(format!(
				"Invalid brotli quality: {} (must be 0-11)",
				self.brotli_quality
			)));
		}

		Ok(())
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;

	#[rstest]
	fn test_config_default() {
		let config = WhiteNoiseConfig::default();
		assert_eq!(config.max_age_immutable, 31536000);
		assert_eq!(config.max_age_mutable, 60);
		assert!(config.enable_gzip);
		assert!(config.enable_brotli);
	}

	#[rstest]
	fn test_config_builder() {
		let config = WhiteNoiseConfig::new(PathBuf::from("static"), "/static/".to_string())
			.with_compression(true, false)
			.with_max_age_immutable(3600)
			.with_cors(true);

		assert_eq!(config.root, PathBuf::from("static"));
		assert_eq!(config.static_url, "/static/");
		assert!(config.enable_gzip);
		assert!(!config.enable_brotli);
		assert_eq!(config.max_age_immutable, 3600);
		assert!(config.allow_all_origins);
	}
}
