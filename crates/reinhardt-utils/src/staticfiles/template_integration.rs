//! Template integration for static files
//!
//! Provides configuration types for static file URL generation in templates.

use super::{ManifestStaticFilesStorage, StaticFilesConfig};
use std::collections::HashMap;
use std::io;

/// Configuration for static files in templates
///
/// This configuration can be used with template systems to generate URLs for static files.
/// It can be constructed from `StaticFilesConfig`.
#[derive(Debug, Clone)]
pub struct TemplateStaticConfig {
	/// Base URL for static files (e.g., "/static/")
	pub static_url: String,
	/// Whether to use hashed filenames from manifest
	pub use_manifest: bool,
	/// Manifest mapping original paths to hashed paths
	pub manifest: HashMap<String, String>,
}

impl From<&StaticFilesConfig> for TemplateStaticConfig {
	fn from(config: &StaticFilesConfig) -> Self {
		Self {
			static_url: config.static_url.clone(),
			use_manifest: false,
			manifest: HashMap::new(),
		}
	}
}

impl TemplateStaticConfig {
	/// Create a new template static configuration
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_utils::staticfiles::template_integration::TemplateStaticConfig;
	///
	/// let config = TemplateStaticConfig::new("/static/".to_string());
	/// assert_eq!(config.static_url, "/static/");
	/// assert!(!config.use_manifest);
	/// ```
	pub fn new(static_url: String) -> Self {
		Self {
			static_url,
			use_manifest: false,
			manifest: HashMap::new(),
		}
	}

	/// Enable manifest-based hashed filenames
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_utils::staticfiles::template_integration::TemplateStaticConfig;
	/// use std::collections::HashMap;
	///
	/// let mut manifest = HashMap::new();
	/// manifest.insert("css/style.css".to_string(), "css/style.abc123.css".to_string());
	///
	/// let config = TemplateStaticConfig::new("/static/".to_string())
	///     .with_manifest(manifest);
	///
	/// assert!(config.use_manifest);
	/// assert_eq!(config.manifest.len(), 1);
	/// ```
	pub fn with_manifest(mut self, manifest: HashMap<String, String>) -> Self {
		self.use_manifest = true;
		self.manifest = manifest;
		self
	}

	/// Load manifest from ManifestStaticFilesStorage
	///
	/// # Examples
	///
	/// ```rust,no_run
	/// use reinhardt_utils::staticfiles::template_integration::TemplateStaticConfig;
	/// use reinhardt_utils::staticfiles::ManifestStaticFilesStorage;
	/// use std::path::PathBuf;
	///
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// let storage = ManifestStaticFilesStorage::new(
	///     PathBuf::from("/var/www/static"),
	///     "/static/"
	/// );
	///
	/// let config = TemplateStaticConfig::from_storage(&storage).await?;
	/// assert!(config.use_manifest);
	/// # Ok(())
	/// # }
	/// ```
	pub async fn from_storage(storage: &ManifestStaticFilesStorage) -> io::Result<Self> {
		let manifest_path = storage.location.join(&storage.manifest_name);

		if !manifest_path.exists() {
			return Ok(Self {
				static_url: storage.base_url.clone(),
				use_manifest: false,
				manifest: HashMap::new(),
			});
		}

		let manifest_content = tokio::fs::read_to_string(&manifest_path).await?;

		// Try parsing as structured format first: {"version": "...", "paths": {...}} or {"paths": {...}}
		let manifest =
			if let Ok(structured) = serde_json::from_str::<serde_json::Value>(&manifest_content) {
				if let Some(paths) = structured.get("paths").and_then(|v| v.as_object()) {
					paths
						.iter()
						.filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
						.collect()
				} else if let Some(files) = structured.get("files").and_then(|v| v.as_object()) {
					// Legacy format with "files" key
					files
						.iter()
						.filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
						.collect()
				} else {
					// Try as simple HashMap (legacy flat format)
					serde_json::from_str::<HashMap<String, String>>(&manifest_content)
						.map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?
				}
			} else {
				return Err(io::Error::new(
					io::ErrorKind::InvalidData,
					"Invalid manifest JSON",
				));
			};

		Ok(Self {
			static_url: storage.base_url.clone(),
			use_manifest: true,
			manifest,
		})
	}

	/// Resolve a static file path to a URL
	///
	/// This method generates a URL for a static file, optionally using
	/// manifest-based hashed filenames for cache busting.
	///
	/// # Arguments
	///
	/// * `name` - The file path relative to static root, optionally with query string and/or fragment
	///
	/// # Examples
	///
	/// Basic usage:
	///
	/// ```rust
	/// use reinhardt_utils::staticfiles::template_integration::TemplateStaticConfig;
	///
	/// let config = TemplateStaticConfig::new("/static/".to_string());
	/// assert_eq!(config.resolve_url("css/style.css"), "/static/css/style.css");
	/// ```
	///
	/// With manifest:
	///
	/// ```rust
	/// use reinhardt_utils::staticfiles::template_integration::TemplateStaticConfig;
	/// use std::collections::HashMap;
	///
	/// let mut manifest = HashMap::new();
	/// manifest.insert("css/style.css".to_string(), "css/style.abc123.css".to_string());
	///
	/// let config = TemplateStaticConfig::new("/static/".to_string())
	///     .with_manifest(manifest);
	///
	/// assert_eq!(config.resolve_url("css/style.css"), "/static/css/style.abc123.css");
	/// ```
	///
	/// With query string and fragment:
	///
	/// ```rust
	/// use reinhardt_utils::staticfiles::template_integration::TemplateStaticConfig;
	///
	/// let config = TemplateStaticConfig::new("/static/".to_string());
	/// assert_eq!(
	///     config.resolve_url("test.css?v=1#section"),
	///     "/static/test.css?v=1#section"
	/// );
	/// ```
	pub fn resolve_url(&self, name: &str) -> String {
		// 1. Split path, query string, and fragment
		let (path, query_fragment) = match name.split_once('?') {
			Some((p, qf)) => (p, Some(qf)),
			None => (name, None),
		};

		// 2. Check manifest for hashed filename
		let resolved_path = if self.use_manifest {
			self.manifest.get(path).map(|s| s.as_str()).unwrap_or(path)
		} else {
			path
		};

		// 3. Normalize and join URL
		let base = self.static_url.trim_end_matches('/');
		let path = resolved_path.trim_start_matches('/');
		let mut url = format!("{}/{}", base, path);

		// 4. Append query string and fragment
		if let Some(qf) = query_fragment {
			url.push('?');
			url.push_str(qf);
		}

		url
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_template_static_config_new() {
		let config = TemplateStaticConfig::new("/static/".to_string());
		assert_eq!(config.static_url, "/static/");
		assert!(!config.use_manifest);
		assert!(config.manifest.is_empty());
	}

	#[test]
	fn test_template_static_config_with_manifest() {
		let mut manifest = HashMap::new();
		manifest.insert(
			"css/style.css".to_string(),
			"css/style.abc123.css".to_string(),
		);

		let config =
			TemplateStaticConfig::new("/static/".to_string()).with_manifest(manifest.clone());

		assert_eq!(config.static_url, "/static/");
		assert!(config.use_manifest);
		assert_eq!(config.manifest.len(), 1);
		assert_eq!(
			config.manifest.get("css/style.css"),
			Some(&"css/style.abc123.css".to_string())
		);
	}

	#[test]
	fn test_template_static_config_from_static_files_config() {
		let static_config = StaticFilesConfig {
			static_root: std::path::PathBuf::from("/var/www/static"),
			static_url: "/assets/".to_string(),
			staticfiles_dirs: vec![],
			media_url: None,
		};

		let template_config = TemplateStaticConfig::from(&static_config);
		assert_eq!(template_config.static_url, "/assets/");
		assert!(!template_config.use_manifest);
		assert!(template_config.manifest.is_empty());
	}

	#[tokio::test]
	async fn test_from_storage() {
		use tempfile::tempdir;

		let temp_dir = tempdir().unwrap();
		let static_root = temp_dir.path().to_path_buf();

		// Create manifest file (canonical format with version and paths)
		let manifest_content = r#"{
  "version": "1.0",
  "paths": {
    "css/style.css": "css/style.abc123.css",
    "js/app.js": "js/app.def456.js"
  }
}"#;

		std::fs::write(static_root.join("staticfiles.json"), manifest_content).unwrap();

		let storage = ManifestStaticFilesStorage::new(static_root, "/static/");
		let config = TemplateStaticConfig::from_storage(&storage).await.unwrap();

		assert_eq!(config.static_url, "/static/");
		assert!(config.use_manifest);
		assert_eq!(config.manifest.len(), 2);
		assert_eq!(
			config.manifest.get("css/style.css"),
			Some(&"css/style.abc123.css".to_string())
		);
		assert_eq!(
			config.manifest.get("js/app.js"),
			Some(&"js/app.def456.js".to_string())
		);
	}

	#[tokio::test]
	async fn test_from_storage_with_version_and_paths() {
		use tempfile::tempdir;

		let temp_dir = tempdir().unwrap();
		let static_root = temp_dir.path().to_path_buf();

		// Canonical format: {"version": "1.0", "paths": {...}}
		let manifest_content = r#"{
  "version": "1.0",
  "paths": {
    "css/style.css": "css/style.abc123.css"
  }
}"#;

		std::fs::write(static_root.join("staticfiles.json"), manifest_content).unwrap();

		let storage = ManifestStaticFilesStorage::new(static_root, "/static/");
		let config = TemplateStaticConfig::from_storage(&storage).await.unwrap();

		assert_eq!(config.static_url, "/static/");
		assert!(config.use_manifest);
		assert_eq!(config.manifest.len(), 1);
		assert_eq!(
			config.manifest.get("css/style.css"),
			Some(&"css/style.abc123.css".to_string())
		);
	}

	#[tokio::test]
	async fn test_from_storage_with_legacy_files_key() {
		use tempfile::tempdir;

		let temp_dir = tempdir().unwrap();
		let static_root = temp_dir.path().to_path_buf();

		// Legacy format: {"version": "1.0", "files": {...}}
		let manifest_content = r#"{
  "version": "1.0",
  "files": {
    "js/app.js": "js/app.def456.js"
  }
}"#;

		std::fs::write(static_root.join("staticfiles.json"), manifest_content).unwrap();

		let storage = ManifestStaticFilesStorage::new(static_root, "/static/");
		let config = TemplateStaticConfig::from_storage(&storage).await.unwrap();

		assert_eq!(config.static_url, "/static/");
		assert!(config.use_manifest);
		assert_eq!(config.manifest.len(), 1);
		assert_eq!(
			config.manifest.get("js/app.js"),
			Some(&"js/app.def456.js".to_string())
		);
	}

	#[test]
	fn test_resolve_url_basic() {
		let config = TemplateStaticConfig::new("/static/".to_string());
		assert_eq!(config.resolve_url("css/style.css"), "/static/css/style.css");
	}

	#[test]
	fn test_resolve_url_with_leading_slash() {
		let config = TemplateStaticConfig::new("/static/".to_string());
		assert_eq!(
			config.resolve_url("/css/style.css"),
			"/static/css/style.css"
		);
	}

	#[test]
	fn test_resolve_url_with_manifest() {
		let mut manifest = HashMap::new();
		manifest.insert(
			"css/style.css".to_string(),
			"css/style.abc123.css".to_string(),
		);

		let config = TemplateStaticConfig::new("/static/".to_string()).with_manifest(manifest);

		assert_eq!(
			config.resolve_url("css/style.css"),
			"/static/css/style.abc123.css"
		);
	}

	#[test]
	fn test_resolve_url_manifest_fallback() {
		let mut manifest = HashMap::new();
		manifest.insert(
			"css/style.css".to_string(),
			"css/style.abc123.css".to_string(),
		);

		let config = TemplateStaticConfig::new("/static/".to_string()).with_manifest(manifest);

		// File not in manifest should fallback to original path
		assert_eq!(config.resolve_url("js/app.js"), "/static/js/app.js");
	}

	#[test]
	fn test_resolve_url_with_query_string() {
		let config = TemplateStaticConfig::new("/static/".to_string());
		assert_eq!(config.resolve_url("test.css?v=1"), "/static/test.css?v=1");
	}

	#[test]
	fn test_resolve_url_with_fragment() {
		let config = TemplateStaticConfig::new("/static/".to_string());
		assert_eq!(
			config.resolve_url("test.css#section"),
			"/static/test.css#section"
		);
	}

	#[test]
	fn test_resolve_url_with_query_and_fragment() {
		let config = TemplateStaticConfig::new("/static/".to_string());
		assert_eq!(
			config.resolve_url("test.css?v=1#section"),
			"/static/test.css?v=1#section"
		);
	}

	#[test]
	fn test_resolve_url_manifest_with_query_string() {
		let mut manifest = HashMap::new();
		manifest.insert(
			"css/style.css".to_string(),
			"css/style.abc123.css".to_string(),
		);

		let config = TemplateStaticConfig::new("/static/".to_string()).with_manifest(manifest);

		// Manifest lookup should work with query string
		assert_eq!(
			config.resolve_url("css/style.css?v=1"),
			"/static/css/style.abc123.css?v=1"
		);
	}

	#[test]
	fn test_resolve_url_different_base_urls() {
		let config1 = TemplateStaticConfig::new("/static/".to_string());
		assert_eq!(config1.resolve_url("test.txt"), "/static/test.txt");

		let config2 = TemplateStaticConfig::new("/static".to_string());
		assert_eq!(config2.resolve_url("test.txt"), "/static/test.txt");

		let config3 = TemplateStaticConfig::new("static/".to_string());
		assert_eq!(config3.resolve_url("test.txt"), "static/test.txt");
	}

	#[test]
	fn test_resolve_url_empty_path() {
		let config = TemplateStaticConfig::new("/static/".to_string());
		assert_eq!(config.resolve_url(""), "/static/");
	}

	#[test]
	fn test_resolve_url_cdn_url() {
		let config = TemplateStaticConfig::new("https://cdn.example.com/static/".to_string());
		assert_eq!(
			config.resolve_url("css/style.css"),
			"https://cdn.example.com/static/css/style.css"
		);
	}
}
