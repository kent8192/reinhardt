//! crates.io API Client
//!
//! This module provides a client for interacting with the crates.io API
//! to search, fetch, and query information about Reinhardt plugins.
//!
//! # Naming Convention
//!
//! Reinhardt plugins follow the `xxx-delion` naming pattern:
//! - `auth-delion` - Authentication plugin
//! - `cache-delion` - Caching plugin
//! - `rate-limit-delion` - Rate limiting plugin
//!
//! # Example
//!
//! ```ignore
//! use reinhardt_dentdelion::crates_io::CratesIoClient;
//!
//! let client = CratesIoClient::new()?;
//!
//! // Search for plugins
//! let plugins = client.search_plugins("auth", 10)?;
//!
//! // Get specific crate info
//! let info = client.get_crate_info("auth-delion")?;
//! ```

use crate::error::{PluginError, PluginResult};
use crates_io_api::{AsyncClient, CratesQuery};

/// The suffix for Reinhardt plugin names.
pub const PLUGIN_SUFFIX: &str = "-delion";

/// Information about a crate from crates.io.
#[derive(Debug, Clone)]
pub struct CrateInfo {
	/// Crate name
	pub name: String,
	/// Latest version
	pub version: String,
	/// Description
	pub description: Option<String>,
	/// Documentation URL
	pub documentation: Option<String>,
	/// Repository URL
	pub repository: Option<String>,
	/// Download count
	pub downloads: u64,
	/// Whether this is a Reinhardt plugin (ends with -delion)
	pub is_plugin: bool,
	/// All available versions
	pub versions: Vec<VersionInfo>,
}

/// Information about a specific version.
#[derive(Debug, Clone)]
pub struct VersionInfo {
	/// Version string
	pub version: String,
	/// Whether this version is yanked
	pub yanked: bool,
	/// Dependencies
	pub dependencies: Vec<String>,
}

/// Detailed dependency information including version requirements.
#[derive(Debug, Clone)]
pub struct DependencyInfo {
	/// Crate name
	pub name: String,
	/// Version requirement (e.g., "^1.0.0", ">=0.5, <2.0")
	pub version_req: String,
	/// Whether this is an optional dependency
	pub optional: bool,
	/// Dependency kind (normal, dev, build)
	pub kind: String,
}

/// Client for interacting with the crates.io API.
pub struct CratesIoClient {
	client: crates_io_api::AsyncClient,
}

impl CratesIoClient {
	/// Default User-Agent string for crates.io API requests.
	///
	/// Uses the project repository URL as contact information per
	/// crates.io API policy requirements.
	const DEFAULT_USER_AGENT: &str =
		"reinhardt-dentdelion (https://github.com/kent8192/reinhardt-web)";

	/// Create a new crates.io client with the default User-Agent.
	///
	/// # Errors
	///
	/// Returns an error if the client cannot be initialized.
	pub fn new() -> PluginResult<Self> {
		Self::with_user_agent(Self::DEFAULT_USER_AGENT)
	}

	/// Create a new crates.io client with a custom User-Agent string.
	///
	/// The User-Agent must include contact information (email or URL) per
	/// [crates.io API policy](https://crates.io/policies).
	///
	/// # Arguments
	///
	/// * `user_agent` - User-Agent string including contact information
	///
	/// # Errors
	///
	/// Returns an error if the client cannot be initialized.
	pub fn with_user_agent(user_agent: &str) -> PluginResult<Self> {
		let client = AsyncClient::new(user_agent, std::time::Duration::from_millis(1000))
			.map_err(|e| PluginError::Network(format!("Failed to create crates.io client: {e}")))?;

		Ok(Self { client })
	}

	/// Get information about a specific crate.
	///
	/// # Arguments
	///
	/// * `name` - The crate name
	///
	/// # Errors
	///
	/// Returns an error if the crate cannot be found or the API request fails.
	pub async fn get_crate_info(&self, name: &str) -> PluginResult<CrateInfo> {
		let response =
			self.client.get_crate(name).await.map_err(|e| {
				PluginError::Network(format!("Failed to fetch crate '{name}': {e}"))
			})?;

		let crate_data = response.crate_data;
		let versions = response
			.versions
			.into_iter()
			.map(|v| VersionInfo {
				version: v.num,
				yanked: v.yanked,
				dependencies: Vec::new(), // Dependencies are fetched separately
			})
			.collect();

		Ok(CrateInfo {
			name: crate_data.name.clone(),
			version: crate_data.max_version,
			description: crate_data.description,
			documentation: crate_data.documentation,
			repository: crate_data.repository,
			downloads: crate_data.downloads,
			is_plugin: crate_data.name.ends_with(PLUGIN_SUFFIX),
			versions,
		})
	}

	/// Get all versions of a crate.
	///
	/// # Arguments
	///
	/// * `name` - The crate name
	///
	/// # Errors
	///
	/// Returns an error if the crate cannot be found or the API request fails.
	pub async fn get_versions(&self, name: &str) -> PluginResult<Vec<VersionInfo>> {
		let response = self.client.get_crate(name).await.map_err(|e| {
			PluginError::Network(format!("Failed to fetch versions for '{name}': {e}"))
		})?;

		let versions = response
			.versions
			.into_iter()
			.map(|v| VersionInfo {
				version: v.num,
				yanked: v.yanked,
				dependencies: Vec::new(),
			})
			.collect();

		Ok(versions)
	}

	/// Get the latest non-yanked version of a crate.
	///
	/// # Arguments
	///
	/// * `name` - The crate name
	///
	/// # Errors
	///
	/// Returns an error if the crate cannot be found or has no valid versions.
	pub async fn get_latest_version(&self, name: &str) -> PluginResult<String> {
		let versions = self.get_versions(name).await?;

		versions
			.into_iter()
			.find(|v| !v.yanked)
			.map(|v| v.version)
			.ok_or_else(|| {
				PluginError::NotFound(format!("No valid (non-yanked) versions found for '{name}'"))
			})
	}

	/// Search for Reinhardt plugins on crates.io.
	///
	/// This searches for crates matching the query that follow the `xxx-delion`
	/// naming convention.
	///
	/// # Arguments
	///
	/// * `query` - Search query
	/// * `limit` - Maximum number of results (default: 10, max: 100)
	///
	/// # Errors
	///
	/// Returns an error if the API request fails.
	pub async fn search_plugins(&self, query: &str, limit: u64) -> PluginResult<Vec<CrateInfo>> {
		// Search for crates matching the query with -delion suffix
		let search_query = format!("{query} -delion");

		let crates_query = CratesQuery::builder()
			.search(&search_query)
			.page_size(limit.min(100))
			.build();

		let response = self
			.client
			.crates(crates_query)
			.await
			.map_err(|e| PluginError::Network(format!("Failed to search crates.io: {e}")))?;

		let plugins: Vec<CrateInfo> = response
			.crates
			.into_iter()
			.filter(|c| c.name.ends_with(PLUGIN_SUFFIX))
			.map(|c| CrateInfo {
				name: c.name.clone(),
				version: c.max_version,
				description: c.description,
				documentation: c.documentation,
				repository: c.repository,
				downloads: c.downloads,
				is_plugin: true,
				versions: Vec::new(), // Versions are fetched separately
			})
			.collect();

		Ok(plugins)
	}

	/// List all Reinhardt plugins on crates.io.
	///
	/// This fetches all crates with the `-delion` suffix.
	///
	/// # Arguments
	///
	/// * `limit` - Maximum number of results
	///
	/// # Errors
	///
	/// Returns an error if the API request fails.
	pub async fn list_all_plugins(&self, limit: u64) -> PluginResult<Vec<CrateInfo>> {
		let crates_query = CratesQuery::builder()
			.search("-delion")
			.page_size(limit.min(100))
			.build();

		let response = self
			.client
			.crates(crates_query)
			.await
			.map_err(|e| PluginError::Network(format!("Failed to list plugins: {e}")))?;

		let plugins: Vec<CrateInfo> = response
			.crates
			.into_iter()
			.filter(|c| c.name.ends_with(PLUGIN_SUFFIX))
			.map(|c| CrateInfo {
				name: c.name.clone(),
				version: c.max_version,
				description: c.description,
				documentation: c.documentation,
				repository: c.repository,
				downloads: c.downloads,
				is_plugin: true,
				versions: Vec::new(),
			})
			.collect();

		Ok(plugins)
	}

	/// Check if a plugin exists on crates.io.
	///
	/// # Arguments
	///
	/// * `name` - Plugin name
	///
	/// # Returns
	///
	/// `true` if the plugin exists, `false` otherwise.
	pub async fn plugin_exists(&self, name: &str) -> bool {
		self.client.get_crate(name).await.is_ok()
	}

	/// Validate that a crate name follows the plugin naming convention.
	///
	/// # Arguments
	///
	/// * `name` - The crate name to validate
	///
	/// # Returns
	///
	/// `true` if the name ends with `-delion`, `false` otherwise.
	pub fn is_valid_plugin_name(name: &str) -> bool {
		name.ends_with(PLUGIN_SUFFIX)
	}

	/// Suggest the correct plugin name if the user forgot the suffix.
	///
	/// # Arguments
	///
	/// * `name` - The crate name
	///
	/// # Returns
	///
	/// The name with `-delion` suffix added if needed.
	pub fn suggest_plugin_name(name: &str) -> String {
		if name.ends_with(PLUGIN_SUFFIX) {
			name.to_string()
		} else {
			format!("{name}{PLUGIN_SUFFIX}")
		}
	}

	/// Get dependencies for a specific version of a crate.
	///
	/// # Arguments
	///
	/// * `name` - The crate name
	/// * `version` - The version string
	///
	/// # Errors
	///
	/// Returns an error if the dependencies cannot be fetched.
	pub async fn get_dependencies(&self, name: &str, version: &str) -> PluginResult<Vec<String>> {
		let deps = self
			.client
			.crate_dependencies(name, version)
			.await
			.map_err(|e| {
				PluginError::Network(format!(
					"Failed to fetch dependencies for '{name}@{version}': {e}"
				))
			})?;

		Ok(deps.into_iter().map(|d| d.crate_id).collect())
	}

	/// Get detailed dependencies for a specific version of a crate.
	///
	/// This method returns detailed dependency information including version requirements,
	/// which is needed for semver compatibility checking.
	///
	/// # Arguments
	///
	/// * `name` - The crate name
	/// * `version` - The version string
	///
	/// # Errors
	///
	/// Returns an error if the dependencies cannot be fetched.
	pub async fn get_dependencies_detailed(
		&self,
		name: &str,
		version: &str,
	) -> PluginResult<Vec<DependencyInfo>> {
		let deps = self
			.client
			.crate_dependencies(name, version)
			.await
			.map_err(|e| {
				PluginError::Network(format!(
					"Failed to fetch dependencies for '{name}@{version}': {e}"
				))
			})?;

		Ok(deps
			.into_iter()
			.map(|d| DependencyInfo {
				name: d.crate_id,
				version_req: d.req,
				optional: d.optional,
				kind: d.kind,
			})
			.collect())
	}

	/// Check compatibility with current Reinhardt version.
	///
	/// This checks if the plugin has a compatible dependency on reinhardt
	/// by verifying that the plugin's version requirements are satisfied
	/// by the current Reinhardt version using semver.
	///
	/// # Arguments
	///
	/// * `name` - Plugin name
	/// * `version` - Plugin version
	/// * `reinhardt_version` - Current Reinhardt version
	///
	/// # Returns
	///
	/// `Ok(true)` if compatible, `Ok(false)` if not compatible.
	///
	/// # Errors
	///
	/// Returns an error if version information cannot be fetched or if
	/// version parsing fails.
	pub async fn check_compatibility(
		&self,
		name: &str,
		version: &str,
		reinhardt_version: &str,
	) -> PluginResult<bool> {
		let deps = self.get_dependencies_detailed(name, version).await?;

		// Find reinhardt dependencies
		let reinhardt_deps: Vec<_> = deps
			.iter()
			.filter(|d| d.name == "reinhardt" || d.name.starts_with("reinhardt-"))
			.collect();

		if reinhardt_deps.is_empty() {
			// No reinhardt dependency, assume compatible
			return Ok(true);
		}

		// Parse the current Reinhardt version
		let current_version = semver::Version::parse(reinhardt_version)
			.map_err(|e| PluginError::InvalidVersion(format!("{reinhardt_version}: {e}")))?;

		// Check each reinhardt dependency's version requirement
		for dep in reinhardt_deps {
			let version_req = semver::VersionReq::parse(&dep.version_req)
				.map_err(|e| PluginError::InvalidVersionReq(format!("{}: {e}", dep.version_req)))?;

			if !version_req.matches(&current_version) {
				return Ok(false);
			}
		}

		Ok(true)
	}
}

impl std::fmt::Debug for CratesIoClient {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("CratesIoClient").finish_non_exhaustive()
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	// ==========================================================================
	// Plugin Naming Tests (existing)
	// ==========================================================================

	#[test]
	fn test_is_valid_plugin_name() {
		assert!(CratesIoClient::is_valid_plugin_name("auth-delion"));
		assert!(CratesIoClient::is_valid_plugin_name("rate-limit-delion"));
		assert!(!CratesIoClient::is_valid_plugin_name("auth"));
		assert!(!CratesIoClient::is_valid_plugin_name("auth-plugin"));
	}

	#[test]
	fn test_suggest_plugin_name() {
		assert_eq!(CratesIoClient::suggest_plugin_name("auth"), "auth-delion");
		assert_eq!(
			CratesIoClient::suggest_plugin_name("rate-limit"),
			"rate-limit-delion"
		);
		assert_eq!(
			CratesIoClient::suggest_plugin_name("auth-delion"),
			"auth-delion"
		);
	}

	#[test]
	fn test_plugin_suffix_constant() {
		assert_eq!(PLUGIN_SUFFIX, "-delion");
	}

	// ==========================================================================
	// Edge Case Tests
	// ==========================================================================

	#[test]
	fn test_is_valid_plugin_name_empty() {
		assert!(!CratesIoClient::is_valid_plugin_name(""));
	}

	#[test]
	fn test_is_valid_plugin_name_just_suffix() {
		// "-delion" alone technically ends with the suffix
		assert!(CratesIoClient::is_valid_plugin_name("-delion"));
	}

	#[test]
	fn test_is_valid_plugin_name_case_sensitive() {
		// Suffix is case-sensitive
		assert!(!CratesIoClient::is_valid_plugin_name("auth-DELION"));
		assert!(!CratesIoClient::is_valid_plugin_name("auth-Delion"));
	}

	#[test]
	fn test_suggest_plugin_name_empty_string() {
		assert_eq!(CratesIoClient::suggest_plugin_name(""), "-delion");
	}

	#[test]
	fn test_suggest_plugin_name_already_has_suffix() {
		assert_eq!(
			CratesIoClient::suggest_plugin_name("my-delion"),
			"my-delion"
		);
	}

	#[test]
	fn test_suggest_plugin_name_with_hyphen() {
		assert_eq!(
			CratesIoClient::suggest_plugin_name("my-plugin"),
			"my-plugin-delion"
		);
	}

	// ==========================================================================
	// CrateInfo Tests
	// ==========================================================================

	#[test]
	fn test_crate_info_is_plugin() {
		let info = CrateInfo {
			name: "auth-delion".to_string(),
			version: "1.0.0".to_string(),
			description: Some("Test plugin".to_string()),
			documentation: None,
			repository: None,
			downloads: 100,
			is_plugin: true,
			versions: vec![],
		};
		assert!(info.is_plugin);
		assert_eq!(info.name, "auth-delion");
		assert_eq!(info.downloads, 100);
	}

	#[test]
	fn test_crate_info_non_plugin() {
		let info = CrateInfo {
			name: "serde".to_string(),
			version: "1.0.0".to_string(),
			description: Some("Serialization framework".to_string()),
			documentation: Some("https://docs.rs/serde".to_string()),
			repository: Some("https://github.com/serde-rs/serde".to_string()),
			downloads: 1000000,
			is_plugin: false,
			versions: vec![],
		};
		assert!(!info.is_plugin);
		assert!(info.documentation.is_some());
		assert!(info.repository.is_some());
	}

	#[test]
	fn test_crate_info_with_versions() {
		let info = CrateInfo {
			name: "test-delion".to_string(),
			version: "2.0.0".to_string(),
			description: None,
			documentation: None,
			repository: None,
			downloads: 50,
			is_plugin: true,
			versions: vec![
				VersionInfo {
					version: "2.0.0".to_string(),
					yanked: false,
					dependencies: vec![],
				},
				VersionInfo {
					version: "1.0.0".to_string(),
					yanked: true,
					dependencies: vec!["reinhardt".to_string()],
				},
			],
		};
		assert_eq!(info.versions.len(), 2);
		assert!(!info.versions[0].yanked);
		assert!(info.versions[1].yanked);
	}

	#[test]
	fn test_crate_info_clone() {
		let info = CrateInfo {
			name: "test-delion".to_string(),
			version: "1.0.0".to_string(),
			description: None,
			documentation: None,
			repository: None,
			downloads: 0,
			is_plugin: true,
			versions: vec![],
		};
		let cloned = info.clone();
		assert_eq!(info.name, cloned.name);
		assert_eq!(info.version, cloned.version);
	}

	#[test]
	fn test_crate_info_debug() {
		let info = CrateInfo {
			name: "test-delion".to_string(),
			version: "1.0.0".to_string(),
			description: None,
			documentation: None,
			repository: None,
			downloads: 0,
			is_plugin: true,
			versions: vec![],
		};
		let debug_str = format!("{:?}", info);
		assert!(debug_str.contains("CrateInfo"));
		assert!(debug_str.contains("test-delion"));
	}

	// ==========================================================================
	// VersionInfo Tests
	// ==========================================================================

	#[test]
	fn test_version_info_yanked() {
		let version = VersionInfo {
			version: "1.0.0".to_string(),
			yanked: true,
			dependencies: vec![],
		};
		assert!(version.yanked);
		assert_eq!(version.version, "1.0.0");
	}

	#[test]
	fn test_version_info_not_yanked() {
		let version = VersionInfo {
			version: "2.0.0".to_string(),
			yanked: false,
			dependencies: vec!["serde".to_string()],
		};
		assert!(!version.yanked);
		assert_eq!(version.dependencies.len(), 1);
	}

	#[test]
	fn test_version_info_clone() {
		let version = VersionInfo {
			version: "1.0.0".to_string(),
			yanked: false,
			dependencies: vec!["reinhardt".to_string(), "tokio".to_string()],
		};
		let cloned = version.clone();
		assert_eq!(version.version, cloned.version);
		assert_eq!(version.dependencies, cloned.dependencies);
	}

	#[test]
	fn test_version_info_debug() {
		let version = VersionInfo {
			version: "1.0.0".to_string(),
			yanked: false,
			dependencies: vec![],
		};
		let debug_str = format!("{:?}", version);
		assert!(debug_str.contains("VersionInfo"));
		assert!(debug_str.contains("1.0.0"));
	}

	// ==========================================================================
	// Debug Trait Tests
	// ==========================================================================

	#[test]
	fn test_crates_io_client_debug_trait_bound() {
		// Verify CratesIoClient implements Debug
		fn _assert_debug<T: std::fmt::Debug>() {}
		_assert_debug::<CratesIoClient>();
	}

	// ==========================================================================
	// Semver Compatibility Tests
	// ==========================================================================

	#[test]
	fn test_dependency_info_struct() {
		let dep = DependencyInfo {
			name: "reinhardt".to_string(),
			version_req: "^1.0.0".to_string(),
			optional: false,
			kind: "normal".to_string(),
		};
		assert_eq!(dep.name, "reinhardt");
		assert_eq!(dep.version_req, "^1.0.0");
		assert!(!dep.optional);
	}

	#[test]
	fn test_dependency_info_clone() {
		let dep = DependencyInfo {
			name: "reinhardt-orm".to_string(),
			version_req: ">=0.5, <2.0".to_string(),
			optional: true,
			kind: "dev".to_string(),
		};
		let cloned = dep.clone();
		assert_eq!(dep.name, cloned.name);
		assert_eq!(dep.version_req, cloned.version_req);
		assert_eq!(dep.optional, cloned.optional);
		assert_eq!(dep.kind, cloned.kind);
	}

	#[test]
	fn test_dependency_info_debug() {
		let dep = DependencyInfo {
			name: "reinhardt".to_string(),
			version_req: "^1.0.0".to_string(),
			optional: false,
			kind: "normal".to_string(),
		};
		let debug_str = format!("{:?}", dep);
		assert!(debug_str.contains("DependencyInfo"));
		assert!(debug_str.contains("reinhardt"));
	}

	#[test]
	fn test_semver_version_req_matches_compatible() {
		// Test that ^1.0.0 matches 1.2.0
		let req = semver::VersionReq::parse("^1.0.0").unwrap();
		let version = semver::Version::parse("1.2.0").unwrap();
		assert!(req.matches(&version));
	}

	#[test]
	fn test_semver_version_req_matches_incompatible() {
		// Test that ^1.0.0 does not match 2.0.0
		let req = semver::VersionReq::parse("^1.0.0").unwrap();
		let version = semver::Version::parse("2.0.0").unwrap();
		assert!(!req.matches(&version));
	}

	#[test]
	fn test_semver_version_req_matches_exact() {
		// Test exact version requirement
		let req = semver::VersionReq::parse("=1.5.0").unwrap();
		let version_match = semver::Version::parse("1.5.0").unwrap();
		let version_no_match = semver::Version::parse("1.5.1").unwrap();
		assert!(req.matches(&version_match));
		assert!(!req.matches(&version_no_match));
	}

	#[test]
	fn test_semver_version_req_matches_range() {
		// Test range requirement >=0.5, <2.0
		let req = semver::VersionReq::parse(">=0.5, <2.0").unwrap();
		assert!(req.matches(&semver::Version::parse("0.5.0").unwrap()));
		assert!(req.matches(&semver::Version::parse("1.0.0").unwrap()));
		assert!(req.matches(&semver::Version::parse("1.9.9").unwrap()));
		assert!(!req.matches(&semver::Version::parse("0.4.0").unwrap()));
		assert!(!req.matches(&semver::Version::parse("2.0.0").unwrap()));
	}

	#[test]
	fn test_semver_prerelease_version() {
		// Pre-release versions
		let req = semver::VersionReq::parse("^1.0.0-alpha").unwrap();
		let version = semver::Version::parse("1.0.0-alpha.1").unwrap();
		assert!(req.matches(&version));
	}

	#[test]
	fn test_invalid_version_error() {
		let result = semver::Version::parse("not-a-version");
		assert!(result.is_err());
	}

	#[test]
	fn test_invalid_version_req_error() {
		let result = semver::VersionReq::parse("invalid-req");
		assert!(result.is_err());
	}
}
