//! WASM Plugin Loader
//!
//! This module provides the `WasmPluginLoader` for discovering and loading
//! WASM plugins from the filesystem.

use crate::error::{PluginError, PluginResult};
use crate::manifest::WasmPluginConfig;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use super::host::HostState;
use super::instance::WasmPluginInstance;
use super::runtime::WasmRuntime;
use super::types::ConfigValue;

/// Default plugin directory relative to project root.
// Allow dead_code: reserved for future plugin auto-discovery from default location
#[allow(dead_code)]
pub(super) const DEFAULT_PLUGIN_DIR: &str = ".dentdelion/plugins";

/// WASM file extension.
pub(super) const WASM_EXTENSION: &str = "wasm";

/// Plugin manifest filename.
pub(super) const PLUGIN_MANIFEST_FILENAME: &str = "plugin.toml";

/// Information about a discovered plugin.
#[derive(Debug, Clone)]
pub struct DiscoveredPlugin {
	/// Path to the WASM file.
	pub wasm_path: PathBuf,
	/// Plugin name (derived from filename or manifest).
	pub name: String,
	/// Optional plugin manifest path.
	pub manifest_path: Option<PathBuf>,
	/// Plugin configuration from manifest.
	pub config: Option<WasmPluginConfig>,
}

/// WASM plugin loader for discovering and loading plugins.
pub struct WasmPluginLoader {
	/// Directory containing WASM plugins.
	plugin_dir: PathBuf,
	/// WASM runtime for loading components.
	runtime: Arc<WasmRuntime>,
}

impl WasmPluginLoader {
	/// Create a new WASM plugin loader.
	///
	/// # Arguments
	///
	/// * `plugin_dir` - Directory to search for WASM plugins
	/// * `runtime` - WASM runtime for loading components
	pub fn new<P: AsRef<Path>>(plugin_dir: P, runtime: Arc<WasmRuntime>) -> Self {
		Self {
			plugin_dir: plugin_dir.as_ref().to_path_buf(),
			runtime,
		}
	}

	/// Get the plugin directory.
	pub fn plugin_dir(&self) -> &Path {
		&self.plugin_dir
	}

	/// Get a reference to the runtime.
	pub fn runtime(&self) -> &WasmRuntime {
		&self.runtime
	}

	async fn resolve_plugin_path(&self, path: &Path) -> PluginResult<PathBuf> {
		let root = tokio::fs::canonicalize(&self.plugin_dir)
			.await
			.map_err(|error| {
				PluginError::Io(std::io::Error::new(
					error.kind(),
					format!(
						"Failed to resolve plugin directory {}: {}",
						self.plugin_dir.display(),
						error
					),
				))
			})?;
		let resolved = tokio::fs::canonicalize(path).await.map_err(|error| {
			PluginError::Io(std::io::Error::new(
				error.kind(),
				format!(
					"Failed to resolve plugin path {}: {}",
					path.display(),
					error
				),
			))
		})?;
		if !resolved.starts_with(&root) {
			return Err(PluginError::Custom(format!(
				"Plugin path {} is outside plugin directory",
				path.display()
			)));
		}
		Ok(resolved)
	}

	/// Discover all WASM plugins in the plugin directory.
	///
	/// This method scans the plugin directory for `.wasm` files and
	/// attempts to parse any accompanying manifest files.
	///
	/// # Errors
	///
	/// Returns an error if the directory cannot be read.
	pub async fn discover(&self) -> PluginResult<Vec<DiscoveredPlugin>> {
		let mut plugins = Vec::new();

		// Check if directory exists
		if !self.plugin_dir.exists() {
			tracing::debug!(
				"Plugin directory does not exist: {}",
				self.plugin_dir.display()
			);
			return Ok(plugins);
		}

		// Read directory entries
		let mut entries = tokio::fs::read_dir(&self.plugin_dir).await.map_err(|e| {
			PluginError::Io(std::io::Error::new(
				e.kind(),
				format!("Failed to read plugin directory: {}", e),
			))
		})?;

		// Scan for .wasm files
		while let Some(entry) = entries.next_entry().await.map_err(|e| {
			PluginError::Io(std::io::Error::new(
				e.kind(),
				format!("Failed to read directory entry: {}", e),
			))
		})? {
			let path = entry.path();

			if path
				.extension()
				.map(|e| e == WASM_EXTENSION)
				.unwrap_or(false)
			{
				// Found a .wasm file
				if let Some(discovered) = self.discover_plugin(&path).await? {
					plugins.push(discovered);
				}
			} else if path.is_dir() {
				// Check subdirectory for plugin
				if let Some(discovered) = self.discover_plugin_in_dir(&path).await? {
					plugins.push(discovered);
				}
			}
		}

		tracing::info!("Discovered {} WASM plugins", plugins.len());
		Ok(plugins)
	}

	/// Discover a plugin from a .wasm file path.
	async fn discover_plugin(&self, wasm_path: &Path) -> PluginResult<Option<DiscoveredPlugin>> {
		let resolved_wasm_path = self.resolve_plugin_path(wasm_path).await?;
		// Validate that the file is a valid WASM binary
		let bytes = tokio::fs::read(&resolved_wasm_path).await.map_err(|e| {
			PluginError::Io(std::io::Error::new(
				e.kind(),
				format!("Failed to read {}: {}", wasm_path.display(), e),
			))
		})?;

		if !super::is_valid_wasm(&bytes) {
			tracing::warn!("Invalid WASM file (bad magic): {}", wasm_path.display());
			return Ok(None);
		}

		// Derive plugin name from filename
		let name = wasm_path
			.file_stem()
			.and_then(|s| s.to_str())
			.map(|s| s.to_string())
			.unwrap_or_else(|| "unknown".to_string());

		// Look for accompanying manifest
		let manifest_path = wasm_path.with_extension("toml");
		let (manifest_path, config) = if manifest_path.exists() {
			let resolved_manifest_path = self.resolve_plugin_path(&manifest_path).await?;
			match self.parse_plugin_manifest(&resolved_manifest_path).await {
				Ok(config) => (Some(manifest_path), Some(config)),
				Err(e) => {
					tracing::warn!("Failed to parse manifest for {}: {}", name, e);
					(None, None)
				}
			}
		} else {
			(None, None)
		};

		Ok(Some(DiscoveredPlugin {
			wasm_path: wasm_path.to_path_buf(),
			name,
			manifest_path,
			config,
		}))
	}

	/// Discover a plugin in a subdirectory.
	///
	/// Looks for `plugin.wasm` and `plugin.toml` in the directory.
	async fn discover_plugin_in_dir(&self, dir: &Path) -> PluginResult<Option<DiscoveredPlugin>> {
		self.resolve_plugin_path(dir).await?;
		let dir_name = dir
			.file_name()
			.and_then(|s| s.to_str())
			.unwrap_or("unknown");

		// Look for plugin.wasm or <dirname>.wasm
		let wasm_paths = [
			dir.join("plugin.wasm"),
			dir.join(format!("{}.wasm", dir_name)),
		];

		let wasm_path = wasm_paths.iter().find(|p| p.exists());

		let wasm_path = match wasm_path {
			Some(p) => p.clone(),
			None => return Ok(None),
		};
		let resolved_wasm_path = self.resolve_plugin_path(&wasm_path).await?;

		// Validate WASM file
		let bytes = tokio::fs::read(&resolved_wasm_path).await.map_err(|e| {
			PluginError::Io(std::io::Error::new(
				e.kind(),
				format!("Failed to read {}: {}", wasm_path.display(), e),
			))
		})?;

		if !super::is_valid_wasm(&bytes) {
			tracing::warn!("Invalid WASM file in {}: bad magic", dir.display());
			return Ok(None);
		}

		// Look for manifest
		let manifest_paths = [
			dir.join(PLUGIN_MANIFEST_FILENAME),
			dir.join(format!("{}.toml", dir_name)),
		];

		let manifest_path = manifest_paths.iter().find(|p| p.exists());
		let (manifest_path, config) = if let Some(mp) = manifest_path {
			let resolved_manifest_path = self.resolve_plugin_path(mp).await?;
			match self.parse_plugin_manifest(&resolved_manifest_path).await {
				Ok(config) => (Some(mp.clone()), Some(config)),
				Err(e) => {
					tracing::warn!("Failed to parse manifest in {}: {}", dir.display(), e);
					(None, None)
				}
			}
		} else {
			(None, None)
		};

		Ok(Some(DiscoveredPlugin {
			wasm_path,
			name: dir_name.to_string(),
			manifest_path,
			config,
		}))
	}

	/// Parse a plugin manifest file.
	async fn parse_plugin_manifest(&self, path: &Path) -> PluginResult<WasmPluginConfig> {
		let content = tokio::fs::read_to_string(path).await.map_err(|e| {
			PluginError::Io(std::io::Error::new(
				e.kind(),
				format!("Failed to read {}: {}", path.display(), e),
			))
		})?;

		// Parse as TOML - look for [wasm] section
		let value: toml::Value = toml::from_str(&content).map_err(|e| {
			PluginError::ManifestParseError(format!("Failed to parse {}: {}", path.display(), e))
		})?;

		// Extract wasm configuration
		let wasm_section = value
			.get("wasm")
			.or_else(|| value.get("plugin"))
			.or(Some(&value));

		let memory_limit_mb = wasm_section
			.and_then(|v| v.get("memory_limit_mb"))
			.and_then(|v| v.as_integer())
			.map(|v| {
				u32::try_from(v).map_err(|_| {
					PluginError::ConfigError(format!(
						"memory_limit_mb value {} is out of u32 range",
						v
					))
				})
			})
			.transpose()?
			.unwrap_or(128);

		let timeout_secs = wasm_section
			.and_then(|v| v.get("timeout_secs"))
			.and_then(|v| v.as_integer())
			.map(|v| {
				u32::try_from(v).map_err(|_| {
					PluginError::ConfigError(format!(
						"timeout_secs value {} is out of u32 range",
						v
					))
				})
			})
			.transpose()?
			.unwrap_or(30);

		let capabilities = wasm_section
			.and_then(|v| v.get("capabilities"))
			.and_then(|v| v.as_array())
			.map(|arr| {
				arr.iter()
					.filter_map(|v| v.as_str().map(|s| s.to_string()))
					.collect()
			})
			.unwrap_or_default();

		Ok(WasmPluginConfig {
			memory_limit_mb,
			capabilities,
			timeout_secs,
		})
	}

	/// Load a plugin from a discovered plugin info.
	///
	/// # Arguments
	///
	/// * `discovered` - Information about the discovered plugin
	/// * `config` - Optional configuration values for the plugin
	///
	/// # Errors
	///
	/// Returns an error if the plugin cannot be loaded.
	pub async fn load(
		&self,
		discovered: &DiscoveredPlugin,
		config: Option<std::collections::HashMap<String, ConfigValue>>,
	) -> PluginResult<WasmPluginInstance> {
		tracing::info!("Loading WASM plugin: {}", discovered.name);
		let wasm_path = self.resolve_plugin_path(&discovered.wasm_path).await?;

		// Load the component
		let component = self.runtime.load_component(&wasm_path).await?;

		// Create host state
		let host_state = HostState::new(&discovered.name);
		if let Some(cfg) = config {
			host_state.set_config_all(cfg);
		}

		// Create the instance
		WasmPluginInstance::new(
			discovered.name.clone(),
			wasm_path,
			component,
			host_state,
			self.runtime.clone(),
			discovered.config.clone(),
		)
	}

	/// Load a plugin by path.
	///
	/// # Arguments
	///
	/// * `path` - Path to the .wasm file
	///
	/// # Errors
	///
	/// Returns an error if the plugin cannot be loaded.
	pub async fn load_from_path<P: AsRef<Path>>(
		&self,
		path: P,
	) -> PluginResult<WasmPluginInstance> {
		let discovered = self
			.discover_plugin(path.as_ref())
			.await?
			.ok_or_else(|| PluginError::InvalidWasmBinary)?;

		self.load(&discovered, None).await
	}

	/// Load a plugin by name.
	///
	/// Searches the plugin directory for a plugin with the given name.
	///
	/// # Arguments
	///
	/// * `name` - Name of the plugin to load
	///
	/// # Errors
	///
	/// Returns an error if the plugin cannot be found or loaded.
	pub async fn load_by_name(&self, name: &str) -> PluginResult<WasmPluginInstance> {
		// Try different possible paths
		let possible_paths = [
			self.plugin_dir.join(format!("{}.wasm", name)),
			self.plugin_dir.join(name).join("plugin.wasm"),
			self.plugin_dir.join(name).join(format!("{}.wasm", name)),
		];

		for path in &possible_paths {
			if path.exists() {
				return self.load_from_path(path).await;
			}
		}

		Err(PluginError::NotFound(name.to_string()))
	}
}

impl std::fmt::Debug for WasmPluginLoader {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("WasmPluginLoader")
			.field("plugin_dir", &self.plugin_dir)
			.finish_non_exhaustive()
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::wasm::runtime::WasmRuntimeConfig;
	use std::fs;
	use tempfile::TempDir;

	fn new_test_loader(plugin_dir: &Path) -> WasmPluginLoader {
		let runtime = Arc::new(
			WasmRuntime::new(WasmRuntimeConfig::default()).expect("test runtime should initialize"),
		);
		WasmPluginLoader::new(plugin_dir, runtime)
	}

	fn write_valid_wasm(path: &Path) {
		fs::write(path, b"\0asm\x01\0\0\0").expect("test WASM should be written");
	}

	#[tokio::test]
	async fn test_discover_empty_dir() {
		let temp = TempDir::new().expect("temporary directory should be created");
		let loader = new_test_loader(&temp.path().join("missing"));

		let plugins = loader
			.discover()
			.await
			.expect("missing directory discovery should succeed");
		assert!(plugins.is_empty());
	}

	#[tokio::test]
	async fn test_load_nonexistent_plugin() {
		let temp = TempDir::new().expect("temporary plugin directory should be created");
		let loader = new_test_loader(temp.path());

		let result = loader.load_by_name("nonexistent-plugin").await;
		assert!(matches!(result, Err(PluginError::NotFound(_))));
	}

	#[tokio::test]
	async fn load_from_path_rejects_file_outside_plugin_directory() {
		let plugins = TempDir::new().expect("plugin directory should be created");
		let outside = TempDir::new().expect("outside directory should be created");
		let outside_wasm = outside.path().join("outside.wasm");
		write_valid_wasm(&outside_wasm);
		let loader = new_test_loader(plugins.path());

		let result = loader.load_from_path(&outside_wasm).await;

		assert!(
			matches!(result, Err(PluginError::Custom(message)) if message.contains("outside plugin directory"))
		);
	}

	#[cfg(unix)]
	#[tokio::test]
	async fn discover_rejects_directory_symlink_outside_plugin_directory() {
		use std::os::unix::fs::symlink;

		let plugins = TempDir::new().expect("plugin directory should be created");
		let outside = TempDir::new().expect("outside directory should be created");
		write_valid_wasm(&outside.path().join("plugin.wasm"));
		symlink(outside.path(), plugins.path().join("alias"))
			.expect("test symlink should be created");
		let loader = new_test_loader(plugins.path());

		let result = loader.discover().await;

		assert!(
			matches!(result, Err(PluginError::Custom(message)) if message.contains("outside plugin directory"))
		);
	}

	#[tokio::test]
	async fn discover_flat_wasm_uses_filename_and_defaults() {
		let temp = TempDir::new().expect("temporary plugin directory should be created");
		let wasm_path = temp.path().join("alpha.wasm");
		write_valid_wasm(&wasm_path);

		let plugins = new_test_loader(temp.path())
			.discover()
			.await
			.expect("plugin discovery should succeed");

		assert_eq!(plugins.len(), 1);
		assert_eq!(plugins[0].name, "alpha");
		assert_eq!(plugins[0].wasm_path, wasm_path);
		assert_eq!(plugins[0].manifest_path, None);
		assert!(plugins[0].config.is_none());
	}

	#[tokio::test]
	async fn discover_flat_wasm_parses_explicit_manifest_values() {
		let temp = TempDir::new().expect("temporary plugin directory should be created");
		let wasm_path = temp.path().join("configured.wasm");
		let manifest_path = temp.path().join("configured.toml");
		write_valid_wasm(&wasm_path);
		fs::write(
			&manifest_path,
			"[wasm]\nmemory_limit_mb = 64\ntimeout_secs = 9\ncapabilities = [\"commands\", \"routing\"]\n",
		)
		.expect("test manifest should be written");

		let plugins = new_test_loader(temp.path())
			.discover()
			.await
			.expect("plugin discovery should succeed");
		let config = plugins[0]
			.config
			.as_ref()
			.expect("valid manifest should produce configuration");

		assert_eq!(plugins.len(), 1);
		assert_eq!(plugins[0].manifest_path.as_ref(), Some(&manifest_path));
		assert_eq!(config.memory_limit_mb, 64);
		assert_eq!(config.timeout_secs, 9);
		assert_eq!(config.capabilities, vec!["commands", "routing"]);
	}

	#[tokio::test]
	async fn discover_nested_plugin_honors_manifest_precedence() {
		let temp = TempDir::new().expect("temporary plugin directory should be created");
		let plugin_dir = temp.path().join("nested");
		fs::create_dir(&plugin_dir).expect("nested plugin directory should be created");
		let preferred_wasm = plugin_dir.join("plugin.wasm");
		let preferred_manifest = plugin_dir.join("plugin.toml");
		write_valid_wasm(&preferred_wasm);
		write_valid_wasm(&plugin_dir.join("nested.wasm"));
		fs::write(
			&preferred_manifest,
			"[wasm]\nmemory_limit_mb = 32\ntimeout_secs = 7\ncapabilities = [\"handlers\"]\n",
		)
		.expect("preferred manifest should be written");
		fs::write(
			plugin_dir.join("nested.toml"),
			"[wasm]\nmemory_limit_mb = 99\ntimeout_secs = 99\n",
		)
		.expect("fallback manifest should be written");

		let plugins = new_test_loader(temp.path())
			.discover()
			.await
			.expect("plugin discovery should succeed");
		let config = plugins[0]
			.config
			.as_ref()
			.expect("preferred manifest should produce configuration");

		assert_eq!(plugins.len(), 1);
		assert_eq!(plugins[0].name, "nested");
		assert_eq!(plugins[0].wasm_path, preferred_wasm);
		assert_eq!(plugins[0].manifest_path.as_ref(), Some(&preferred_manifest));
		assert_eq!(config.memory_limit_mb, 32);
		assert_eq!(config.timeout_secs, 7);
		assert_eq!(config.capabilities, vec!["handlers"]);
	}

	#[tokio::test]
	async fn discover_nested_plugin_falls_back_to_directory_names() {
		let temp = TempDir::new().expect("temporary plugin directory should be created");
		let plugin_dir = temp.path().join("fallback");
		fs::create_dir(&plugin_dir).expect("nested plugin directory should be created");
		let wasm_path = plugin_dir.join("fallback.wasm");
		let manifest_path = plugin_dir.join("fallback.toml");
		write_valid_wasm(&wasm_path);
		fs::write(
			&manifest_path,
			"[wasm]\nmemory_limit_mb = 128\ntimeout_secs = 11\n",
		)
		.expect("fallback manifest should be written");

		let plugins = new_test_loader(temp.path())
			.discover()
			.await
			.expect("plugin discovery should succeed");
		let config = plugins[0]
			.config
			.as_ref()
			.expect("fallback manifest should produce configuration");

		assert_eq!(plugins.len(), 1);
		assert_eq!(plugins[0].name, "fallback");
		assert_eq!(plugins[0].wasm_path, wasm_path);
		assert_eq!(plugins[0].manifest_path.as_ref(), Some(&manifest_path));
		assert_eq!(config.memory_limit_mb, 128);
		assert_eq!(config.timeout_secs, 11);
		assert_eq!(config.capabilities, Vec::<String>::new());
	}

	#[tokio::test]
	async fn discover_ignores_invalid_wasm_magic() {
		let temp = TempDir::new().expect("temporary plugin directory should be created");
		fs::write(temp.path().join("invalid.wasm"), b"not wasm")
			.expect("invalid WASM fixture should be written");

		let plugins = new_test_loader(temp.path())
			.discover()
			.await
			.expect("plugin discovery should succeed");

		assert_eq!(plugins.len(), 0);
	}

	#[tokio::test]
	async fn discover_retains_plugin_when_manifest_is_invalid() {
		let temp = TempDir::new().expect("temporary plugin directory should be created");
		let wasm_path = temp.path().join("invalid-manifest.wasm");
		write_valid_wasm(&wasm_path);
		fs::write(temp.path().join("invalid-manifest.toml"), "[wasm\n")
			.expect("invalid manifest fixture should be written");

		let plugins = new_test_loader(temp.path())
			.discover()
			.await
			.expect("plugin discovery should succeed");

		assert_eq!(plugins.len(), 1);
		assert_eq!(plugins[0].wasm_path, wasm_path);
		assert_eq!(plugins[0].manifest_path, None);
		assert!(plugins[0].config.is_none());
	}

	#[tokio::test]
	async fn parse_plugin_manifest_rejects_memory_limit_outside_u32_range() {
		let temp = TempDir::new().expect("temporary plugin directory should be created");
		let manifest_path = temp.path().join("memory-limit.toml");
		fs::write(&manifest_path, "[wasm]\nmemory_limit_mb = 4294967296\n")
			.expect("test manifest should be written");

		let error = new_test_loader(temp.path())
			.parse_plugin_manifest(&manifest_path)
			.await
			.expect_err("out-of-range memory limit should be rejected");

		match error {
			PluginError::ConfigError(message) => assert_eq!(
				message,
				"memory_limit_mb value 4294967296 is out of u32 range"
			),
			other => panic!("expected ConfigError, got {other:?}"),
		}
	}

	#[tokio::test]
	async fn parse_plugin_manifest_rejects_timeout_outside_u32_range() {
		let temp = TempDir::new().expect("temporary plugin directory should be created");
		let manifest_path = temp.path().join("timeout.toml");
		fs::write(&manifest_path, "[wasm]\ntimeout_secs = 4294967296\n")
			.expect("test manifest should be written");

		let error = new_test_loader(temp.path())
			.parse_plugin_manifest(&manifest_path)
			.await
			.expect_err("out-of-range timeout should be rejected");

		match error {
			PluginError::ConfigError(message) => {
				assert_eq!(message, "timeout_secs value 4294967296 is out of u32 range")
			}
			other => panic!("expected ConfigError, got {other:?}"),
		}
	}
}
