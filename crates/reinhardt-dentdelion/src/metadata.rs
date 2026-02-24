//! Plugin metadata and dependency definitions.
//!
//! This module defines the metadata structures that describe plugins,
//! including their identity, version, dependencies, and capabilities.

use crate::capability::Capability;
use crate::error::{PluginError, PluginResult};
use semver::{Version, VersionReq};
use serde::{Deserialize, Serialize};

/// Plugin metadata containing identification and dependency information.
///
/// This structure follows the pattern established by Cargo's package metadata,
/// adapted for the Dentdelion plugin system.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginMetadata {
	/// Unique plugin identifier (e.g., "auth-delion").
	///
	/// Plugin names should follow the naming convention: `xxx-delion`
	/// where xxx describes the plugin's functionality.
	pub name: String,

	/// Plugin version following Semantic Versioning 2.0.0.
	#[serde(with = "version_serde")]
	pub version: Version,

	/// Human-readable description of the plugin.
	#[serde(default)]
	pub description: String,

	/// Plugin author(s).
	#[serde(default)]
	pub authors: Vec<String>,

	/// License identifier (SPDX format).
	#[serde(default)]
	pub license: String,

	/// Repository URL.
	#[serde(default)]
	pub repository: Option<String>,

	/// Homepage URL.
	#[serde(default)]
	pub homepage: Option<String>,

	/// Plugin dependencies on other plugins.
	#[serde(default)]
	pub dependencies: Vec<PluginDependency>,

	/// Capabilities provided by this plugin.
	#[serde(default)]
	pub provides: Vec<Capability>,

	/// Capabilities required from other plugins or the framework.
	#[serde(default)]
	pub requires: Vec<Capability>,

	/// Plugin keywords for search/discovery.
	#[serde(default)]
	pub keywords: Vec<String>,

	/// Plugin categories.
	#[serde(default)]
	pub categories: Vec<String>,
}

impl PluginMetadata {
	/// Creates a new PluginMetadataBuilder.
	///
	/// # Arguments
	///
	/// * `name` - Plugin name (should follow xxx-delion convention)
	/// * `version` - Plugin version string (semver format)
	///
	/// # Example
	///
	/// ```ignore
	/// use reinhardt_dentdelion::metadata::PluginMetadata;
	///
	/// let metadata = PluginMetadata::builder("auth-delion", "1.0.0")
	///     .description("JWT authentication plugin")
	///     .author("Plugin Author")
	///     .build()
	///     .unwrap();
	/// ```
	pub fn builder(name: impl Into<String>, version: impl AsRef<str>) -> PluginMetadataBuilder {
		PluginMetadataBuilder::new(name, version)
	}

	/// Maximum allowed length for plugin names.
	const MAX_NAME_LENGTH: usize = 128;

	/// Validates the metadata.
	pub fn validate(&self) -> PluginResult<()> {
		Self::validate_plugin_name(&self.name)?;

		// Warn if not following naming convention (but don't error)
		if !self.name.ends_with("-delion") {
			tracing::warn!(
				"plugin '{}' does not follow the recommended naming convention (xxx-delion)",
				self.name
			);
		}

		Ok(())
	}

	/// Validates a plugin name to prevent path traversal and log injection.
	///
	/// Valid plugin names contain only ASCII alphanumeric characters, hyphens,
	/// and underscores. They must not be empty, must not exceed
	/// [`Self::MAX_NAME_LENGTH`] characters, and must not contain path
	/// separators or control characters.
	pub(crate) fn validate_plugin_name(name: &str) -> PluginResult<()> {
		if name.is_empty() {
			return Err(PluginError::InvalidManifest(
				"plugin name cannot be empty".to_string(),
			));
		}

		if name.len() > Self::MAX_NAME_LENGTH {
			return Err(PluginError::InvalidManifest(format!(
				"plugin name exceeds maximum length of {} characters",
				Self::MAX_NAME_LENGTH,
			)));
		}

		// Reject names containing path separators to prevent path traversal
		if name.contains('/') || name.contains('\\') || name.contains("..") {
			return Err(PluginError::InvalidManifest(
				"plugin name must not contain path separators or traversal sequences".to_string(),
			));
		}

		// Reject names containing null bytes
		if name.contains('\0') {
			return Err(PluginError::InvalidManifest(
				"plugin name must not contain null bytes".to_string(),
			));
		}

		// Reject control characters (prevents log injection via newlines, tabs, etc.)
		if name.chars().any(|c| c.is_control()) {
			return Err(PluginError::InvalidManifest(
				"plugin name must not contain control characters".to_string(),
			));
		}

		// Only allow ASCII alphanumeric, hyphens, and underscores
		if !name
			.chars()
			.all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
		{
			return Err(PluginError::InvalidManifest(
				"plugin name must contain only ASCII alphanumeric characters, hyphens, and underscores".to_string(),
			));
		}

		Ok(())
	}

	/// Returns the qualified name (name@version).
	pub fn qualified_name(&self) -> String {
		format!("{}@{}", self.name, self.version)
	}
}

/// Plugin dependency specification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginDependency {
	/// Name of the required plugin.
	pub name: String,

	/// Semver version requirement (e.g., "^1.0.0", ">=2.0, <3.0").
	#[serde(with = "version_req_serde")]
	pub version_req: VersionReq,

	/// Whether this dependency is optional.
	#[serde(default)]
	pub optional: bool,

	/// Specific capabilities required from this dependency.
	#[serde(default)]
	pub required_capabilities: Vec<Capability>,
}

impl PluginDependency {
	/// Creates a new required dependency.
	pub fn new(name: impl Into<String>, version_req: impl AsRef<str>) -> PluginResult<Self> {
		let name = name.into();
		PluginMetadata::validate_plugin_name(&name)?;
		Ok(Self {
			name,
			version_req: VersionReq::parse(version_req.as_ref())
				.map_err(|e| PluginError::InvalidVersionReq(e.to_string()))?,
			optional: false,
			required_capabilities: Vec::new(),
		})
	}

	/// Creates a new optional dependency.
	pub fn optional(name: impl Into<String>, version_req: impl AsRef<str>) -> PluginResult<Self> {
		let mut dep = Self::new(name, version_req)?;
		dep.optional = true;
		Ok(dep)
	}

	/// Adds a required capability to this dependency.
	pub fn with_capability(mut self, capability: impl Into<Capability>) -> Self {
		self.required_capabilities.push(capability.into());
		self
	}

	/// Checks if a version satisfies this dependency.
	pub fn matches(&self, version: &Version) -> bool {
		self.version_req.matches(version)
	}
}

/// Builder pattern for PluginMetadata.
///
/// Matches Reinhardt's existing builder patterns used throughout the framework.
pub struct PluginMetadataBuilder {
	name: String,
	version: String,
	description: String,
	authors: Vec<String>,
	license: String,
	repository: Option<String>,
	homepage: Option<String>,
	dependencies: Vec<PluginDependency>,
	provides: Vec<Capability>,
	requires: Vec<Capability>,
	keywords: Vec<String>,
	categories: Vec<String>,
}

impl PluginMetadataBuilder {
	/// Creates a new builder with required fields.
	pub fn new(name: impl Into<String>, version: impl AsRef<str>) -> Self {
		Self {
			name: name.into(),
			version: version.as_ref().to_string(),
			description: String::new(),
			authors: Vec::new(),
			license: String::new(),
			repository: None,
			homepage: None,
			dependencies: Vec::new(),
			provides: Vec::new(),
			requires: Vec::new(),
			keywords: Vec::new(),
			categories: Vec::new(),
		}
	}

	/// Sets the plugin description.
	pub fn description(mut self, desc: impl Into<String>) -> Self {
		self.description = desc.into();
		self
	}

	/// Adds an author.
	pub fn author(mut self, author: impl Into<String>) -> Self {
		self.authors.push(author.into());
		self
	}

	/// Sets the license.
	pub fn license(mut self, license: impl Into<String>) -> Self {
		self.license = license.into();
		self
	}

	/// Sets the repository URL.
	pub fn repository(mut self, repo: impl Into<String>) -> Self {
		self.repository = Some(repo.into());
		self
	}

	/// Sets the homepage URL.
	pub fn homepage(mut self, homepage: impl Into<String>) -> Self {
		self.homepage = Some(homepage.into());
		self
	}

	/// Adds a capability that this plugin provides.
	pub fn provides(mut self, capability: impl Into<Capability>) -> Self {
		self.provides.push(capability.into());
		self
	}

	/// Adds a capability that this plugin requires.
	pub fn requires(mut self, capability: impl Into<Capability>) -> Self {
		self.requires.push(capability.into());
		self
	}

	/// Adds a dependency on another plugin.
	///
	/// # Panics
	///
	/// Panics if the plugin name or version requirement is invalid.
	pub fn depends_on(mut self, name: impl Into<String>, version_req: impl AsRef<str>) -> Self {
		let dep = PluginDependency::new(name, version_req)
			.expect("invalid plugin dependency specification");
		self.dependencies.push(dep);
		self
	}

	/// Adds an optional dependency on another plugin.
	///
	/// # Panics
	///
	/// Panics if the plugin name or version requirement is invalid.
	pub fn optionally_depends_on(
		mut self,
		name: impl Into<String>,
		version_req: impl AsRef<str>,
	) -> Self {
		let dep = PluginDependency::optional(name, version_req)
			.expect("invalid optional plugin dependency specification");
		self.dependencies.push(dep);
		self
	}

	/// Adds a keyword for search/discovery.
	pub fn keyword(mut self, keyword: impl Into<String>) -> Self {
		self.keywords.push(keyword.into());
		self
	}

	/// Adds a category.
	pub fn category(mut self, category: impl Into<String>) -> Self {
		self.categories.push(category.into());
		self
	}

	/// Builds the PluginMetadata.
	///
	/// # Errors
	///
	/// Returns an error if the version string is invalid.
	pub fn build(self) -> PluginResult<PluginMetadata> {
		let version = Version::parse(&self.version)
			.map_err(|e| PluginError::InvalidVersion(e.to_string()))?;

		let metadata = PluginMetadata {
			name: self.name,
			version,
			description: self.description,
			authors: self.authors,
			license: self.license,
			repository: self.repository,
			homepage: self.homepage,
			dependencies: self.dependencies,
			provides: self.provides,
			requires: self.requires,
			keywords: self.keywords,
			categories: self.categories,
		};

		metadata.validate()?;
		Ok(metadata)
	}
}

/// Serde support for semver::Version.
mod version_serde {
	use semver::Version;
	use serde::{Deserialize, Deserializer, Serialize, Serializer};

	pub(super) fn serialize<S>(version: &Version, serializer: S) -> Result<S::Ok, S::Error>
	where
		S: Serializer,
	{
		version.to_string().serialize(serializer)
	}

	pub(super) fn deserialize<'de, D>(deserializer: D) -> Result<Version, D::Error>
	where
		D: Deserializer<'de>,
	{
		let s = String::deserialize(deserializer)?;
		Version::parse(&s).map_err(serde::de::Error::custom)
	}
}

/// Serde support for semver::VersionReq.
mod version_req_serde {
	use semver::VersionReq;
	use serde::{Deserialize, Deserializer, Serialize, Serializer};

	pub(super) fn serialize<S>(version_req: &VersionReq, serializer: S) -> Result<S::Ok, S::Error>
	where
		S: Serializer,
	{
		version_req.to_string().serialize(serializer)
	}

	pub(super) fn deserialize<'de, D>(deserializer: D) -> Result<VersionReq, D::Error>
	where
		D: Deserializer<'de>,
	{
		let s = String::deserialize(deserializer)?;
		VersionReq::parse(&s).map_err(serde::de::Error::custom)
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::capability::PluginCapability;
	use rstest::rstest;

	#[rstest]
	fn test_metadata_builder() {
		// Arrange
		let name = "auth-delion";
		let version = "1.0.0";

		// Act
		let metadata = PluginMetadata::builder(name, version)
			.description("JWT authentication plugin")
			.author("Test Author")
			.license("MIT")
			.provides(PluginCapability::Auth)
			.provides(PluginCapability::Middleware)
			.depends_on("core-delion", "^0.1.0")
			.build()
			.unwrap();

		// Assert
		assert_eq!(metadata.name, "auth-delion");
		assert_eq!(metadata.version.to_string(), "1.0.0");
		assert_eq!(metadata.description, "JWT authentication plugin");
		assert_eq!(metadata.authors, vec!["Test Author"]);
		assert_eq!(metadata.license, "MIT");
		assert_eq!(metadata.provides.len(), 2);
		assert_eq!(metadata.dependencies.len(), 1);
	}

	#[rstest]
	fn test_dependency_matches() {
		// Arrange
		let dep = PluginDependency::new("test-delion", "^1.0.0").unwrap();

		// Act & Assert
		assert!(dep.matches(&Version::parse("1.0.0").unwrap()));
		assert!(dep.matches(&Version::parse("1.5.0").unwrap()));
		assert!(!dep.matches(&Version::parse("2.0.0").unwrap()));
		assert!(!dep.matches(&Version::parse("0.9.0").unwrap()));
	}

	#[rstest]
	fn test_qualified_name() {
		// Arrange
		let metadata = PluginMetadata::builder("test-delion", "2.1.0")
			.build()
			.unwrap();

		// Act
		let qualified = metadata.qualified_name();

		// Assert
		assert_eq!(qualified, "test-delion@2.1.0");
	}

	// ==========================================================================
	// Plugin Name Validation Tests
	// ==========================================================================

	#[rstest]
	#[case("auth-delion")]
	#[case("my_plugin")]
	#[case("plugin123")]
	#[case("a")]
	#[case("A-B_c-123")]
	fn test_validate_plugin_name_accepts_valid_names(#[case] name: &str) {
		// Act
		let result = PluginMetadata::validate_plugin_name(name);

		// Assert
		assert!(result.is_ok(), "expected valid name: {name}");
	}

	#[rstest]
	fn test_validate_plugin_name_rejects_empty() {
		// Act
		let result = PluginMetadata::validate_plugin_name("");

		// Assert
		let err = result.unwrap_err();
		assert_eq!(
			err.to_string(),
			"invalid manifest format: plugin name cannot be empty"
		);
	}

	#[rstest]
	fn test_validate_plugin_name_rejects_exceeding_max_length() {
		// Arrange
		let long_name = "a".repeat(PluginMetadata::MAX_NAME_LENGTH + 1);

		// Act
		let result = PluginMetadata::validate_plugin_name(&long_name);

		// Assert
		let err = result.unwrap_err();
		assert!(err.to_string().contains("exceeds maximum length"));
	}

	#[rstest]
	fn test_validate_plugin_name_accepts_max_length() {
		// Arrange
		let name = "a".repeat(PluginMetadata::MAX_NAME_LENGTH);

		// Act
		let result = PluginMetadata::validate_plugin_name(&name);

		// Assert
		assert!(result.is_ok());
	}

	#[rstest]
	#[case("../etc/passwd", "path separators")]
	#[case("plugin/evil", "path separators")]
	#[case("plugin\\evil", "path separators")]
	#[case("..%2f..%2fetc", "path separators")]
	fn test_validate_plugin_name_rejects_path_traversal(
		#[case] name: &str,
		#[case] expected_msg: &str,
	) {
		// Act
		let result = PluginMetadata::validate_plugin_name(name);

		// Assert
		let err = result.unwrap_err();
		assert!(
			err.to_string().contains(expected_msg),
			"expected error containing '{expected_msg}', got: {err}"
		);
	}

	#[rstest]
	#[case("plugin\nnewline", "control characters")]
	#[case("plugin\rcarriage", "control characters")]
	#[case("plugin\ttab", "control characters")]
	#[case("plugin\0null", "null bytes")]
	fn test_validate_plugin_name_rejects_control_chars(
		#[case] name: &str,
		#[case] expected_msg: &str,
	) {
		// Act
		let result = PluginMetadata::validate_plugin_name(name);

		// Assert
		let err = result.unwrap_err();
		assert!(
			err.to_string().contains(expected_msg),
			"expected error containing '{expected_msg}', got: {err}"
		);
	}

	#[rstest]
	#[case("plugin name")]
	#[case("plugin@1.0")]
	#[case("plugin.exe")]
	#[case("プラグイン")]
	fn test_validate_plugin_name_rejects_invalid_chars(#[case] name: &str) {
		// Act
		let result = PluginMetadata::validate_plugin_name(name);

		// Assert
		assert!(result.is_err(), "expected invalid name: {name}");
	}

	#[rstest]
	fn test_builder_rejects_invalid_plugin_name() {
		// Act
		let result = PluginMetadata::builder("../evil", "1.0.0").build();

		// Assert
		assert!(result.is_err());
	}

	#[rstest]
	fn test_dependency_rejects_invalid_plugin_name() {
		// Act
		let result = PluginDependency::new("../evil\ninjection", "^1.0.0");

		// Assert
		assert!(result.is_err());
	}
}
