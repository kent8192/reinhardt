//! Android App Links configuration.
//!
//! This module provides types and builders for generating Digital Asset Links (assetlinks.json) files.

use serde::Serialize;

use crate::error::{DeeplinkError, validate_fingerprint, validate_package_name};

/// Android App Links configuration.
///
/// This struct represents a collection of Digital Asset Links statements.
/// When serialized to JSON, it produces the file that should be served at
/// `/.well-known/assetlinks.json`.
///
/// # Example
///
/// ```rust
/// use reinhardt_deeplink::AndroidConfig;
///
/// let config = AndroidConfig::builder()
///     .package_name("com.example.app")
///     .sha256_fingerprint("FA:C6:17:45:DC:09:03:78:6F:B9:ED:E6:2A:96:2B:39:9F:73:48:F0:BB:6F:89:9B:83:32:66:75:91:03:3B:9C")
///     .build()
///     .unwrap();
/// ```
#[derive(Debug, Clone)]
pub struct AndroidConfig {
	/// Digital Asset Links statements.
	pub statements: Vec<AssetStatement>,
}

impl Serialize for AndroidConfig {
	fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
	where
		S: serde::Serializer,
	{
		// Android assetlinks.json is a JSON array of statements
		self.statements.serialize(serializer)
	}
}

/// A Digital Asset Links statement.
#[derive(Debug, Clone, Serialize)]
pub struct AssetStatement {
	/// Relations this statement establishes.
	pub relation: Vec<String>,

	/// Target application or website.
	pub target: AssetTarget,
}

/// Target of an asset statement.
#[derive(Debug, Clone, Serialize)]
pub struct AssetTarget {
	/// Namespace (e.g., `android_app`).
	pub namespace: String,

	/// Android package name.
	pub package_name: String,

	/// SHA256 certificate fingerprints.
	pub sha256_cert_fingerprints: Vec<String>,
}

impl AndroidConfig {
	/// Creates a new builder for Android configuration.
	pub fn builder() -> AndroidConfigBuilder {
		AndroidConfigBuilder::new()
	}
}

/// Builder for Android App Links configuration.
#[derive(Debug, Default)]
pub struct AndroidConfigBuilder {
	package_name: Option<String>,
	fingerprints: Vec<String>,
	additional_packages: Vec<(String, Vec<String>)>,
}

impl AndroidConfigBuilder {
	/// Creates a new builder.
	pub fn new() -> Self {
		Self::default()
	}

	/// Sets the Android package name.
	///
	/// # Arguments
	///
	/// * `name` - The package name (e.g., `com.example.app`)
	pub fn package_name(mut self, name: impl Into<String>) -> Self {
		self.package_name = Some(name.into());
		self
	}

	/// Adds a SHA256 certificate fingerprint.
	///
	/// The fingerprint should be in the format of 32 colon-separated hex bytes.
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_deeplink::AndroidConfig;
	///
	/// let config = AndroidConfig::builder()
	///     .package_name("com.example.app")
	///     .sha256_fingerprint("FA:C6:17:45:DC:09:03:78:6F:B9:ED:E6:2A:96:2B:39:9F:73:48:F0:BB:6F:89:9B:83:32:66:75:91:03:3B:9C")
	///     .build()
	///     .unwrap();
	/// ```
	pub fn sha256_fingerprint(mut self, fingerprint: impl Into<String>) -> Self {
		self.fingerprints.push(fingerprint.into());
		self
	}

	/// Adds multiple SHA256 certificate fingerprints.
	pub fn sha256_fingerprints(mut self, fingerprints: &[&str]) -> Self {
		self.fingerprints
			.extend(fingerprints.iter().map(|s| (*s).to_string()));
		self
	}

	/// Adds an additional package with its own fingerprints.
	///
	/// Use this when multiple apps should be associated with the same domain.
	pub fn additional_package(mut self, package: impl Into<String>, fingerprints: &[&str]) -> Self {
		self.additional_packages.push((
			package.into(),
			fingerprints.iter().map(|s| (*s).to_string()).collect(),
		));
		self
	}

	/// Validates the configuration.
	///
	/// # Errors
	///
	/// Returns an error if:
	/// - No package name is set
	/// - Package name has invalid format
	/// - No fingerprints are provided
	/// - Any fingerprint has an invalid format
	/// - Any additional package has empty fingerprints
	pub fn validate(&self) -> Result<(), DeeplinkError> {
		if self.package_name.is_none() {
			return Err(DeeplinkError::MissingPackageName);
		}

		// Validate package name format (Java package naming conventions)
		if let Some(ref name) = self.package_name {
			validate_package_name(name)?;
		}

		if self.fingerprints.is_empty() {
			return Err(DeeplinkError::MissingFingerprint);
		}

		for fingerprint in &self.fingerprints {
			validate_fingerprint(fingerprint)?;
		}

		// Validate additional package names and their fingerprints
		for (pkg_name, fps) in &self.additional_packages {
			validate_package_name(pkg_name)?;
			// Each additional package must have at least one fingerprint;
			// an empty fingerprints list produces a semantically invalid
			// Asset Links entry that Android will never match.
			if fps.is_empty() {
				return Err(DeeplinkError::MissingFingerprint);
			}
			for fingerprint in fps {
				validate_fingerprint(fingerprint)?;
			}
		}

		Ok(())
	}

	/// Builds the Android configuration after validation.
	///
	/// # Errors
	///
	/// Returns a `DeeplinkError` if validation fails.
	pub fn build(self) -> Result<AndroidConfig, DeeplinkError> {
		self.validate()?;
		Ok(self.build_unchecked())
	}

	/// Builds the Android configuration without validation.
	///
	/// Use [`build`](Self::build) for validated builds. This method is intended
	/// for advanced use cases where validation has already been performed.
	pub fn build_unchecked(self) -> AndroidConfig {
		let mut statements = Vec::new();

		// Build primary statement
		if let Some(package_name) = self.package_name {
			statements.push(AssetStatement {
				relation: vec!["delegate_permission/common.handle_all_urls".to_string()],
				target: AssetTarget {
					namespace: "android_app".to_string(),
					package_name,
					sha256_cert_fingerprints: self.fingerprints,
				},
			});
		}

		// Add additional packages
		for (package_name, fingerprints) in self.additional_packages {
			statements.push(AssetStatement {
				relation: vec!["delegate_permission/common.handle_all_urls".to_string()],
				target: AssetTarget {
					namespace: "android_app".to_string(),
					package_name,
					sha256_cert_fingerprints: fingerprints,
				},
			});
		}

		AndroidConfig { statements }
	}
}

#[cfg(test)]
mod tests {
	use rstest::rstest;

	use super::*;

	const VALID_FINGERPRINT: &str = "FA:C6:17:45:DC:09:03:78:6F:B9:ED:E6:2A:96:2B:39:9F:73:48:F0:BB:6F:89:9B:83:32:66:75:91:03:3B:9C";

	#[rstest]
	fn test_basic_android_config() {
		let config = AndroidConfig::builder()
			.package_name("com.example.app")
			.sha256_fingerprint(VALID_FINGERPRINT)
			.build()
			.unwrap();

		let json = serde_json::to_string_pretty(&config).unwrap();
		assert!(json.contains("delegate_permission/common.handle_all_urls"));
		assert!(json.contains("android_app"));
		assert!(json.contains("com.example.app"));
		assert!(json.contains(VALID_FINGERPRINT));
	}

	#[rstest]
	fn test_android_config_json_array() {
		let config = AndroidConfig::builder()
			.package_name("com.example.app")
			.sha256_fingerprint(VALID_FINGERPRINT)
			.build()
			.unwrap();

		let json = serde_json::to_string(&config).unwrap();
		// Should be a JSON array
		assert!(json.starts_with('['));
		assert!(json.ends_with(']'));
	}

	#[rstest]
	fn test_multiple_fingerprints() {
		let fp1 = "00:00:00:00:00:00:00:00:00:00:00:00:00:00:00:00:00:00:00:00:00:00:00:00:00:00:00:00:00:00:00:00";
		let fp2 = "11:11:11:11:11:11:11:11:11:11:11:11:11:11:11:11:11:11:11:11:11:11:11:11:11:11:11:11:11:11:11:11";

		let config = AndroidConfig::builder()
			.package_name("com.example.app")
			.sha256_fingerprints(&[fp1, fp2])
			.build()
			.unwrap();

		let json = serde_json::to_string_pretty(&config).unwrap();
		assert!(json.contains(fp1));
		assert!(json.contains(fp2));
	}

	#[rstest]
	fn test_additional_packages() {
		let config = AndroidConfig::builder()
			.package_name("com.example.app")
			.sha256_fingerprint(VALID_FINGERPRINT)
			.additional_package("com.example.app2", &[VALID_FINGERPRINT])
			.build()
			.unwrap();

		assert_eq!(config.statements.len(), 2);
		let json = serde_json::to_string_pretty(&config).unwrap();
		assert!(json.contains("com.example.app"));
		assert!(json.contains("com.example.app2"));
	}

	#[rstest]
	fn test_validation_missing_package() {
		let builder = AndroidConfigBuilder::new().sha256_fingerprint(VALID_FINGERPRINT);
		assert!(matches!(
			builder.validate(),
			Err(DeeplinkError::MissingPackageName)
		));
	}

	#[rstest]
	fn test_validation_missing_fingerprint() {
		let builder = AndroidConfigBuilder::new().package_name("com.example.app");
		assert!(matches!(
			builder.validate(),
			Err(DeeplinkError::MissingFingerprint)
		));
	}

	#[rstest]
	fn test_validation_invalid_fingerprint() {
		let builder = AndroidConfigBuilder::new()
			.package_name("com.example.app")
			.sha256_fingerprint("invalid");
		assert!(matches!(
			builder.validate(),
			Err(DeeplinkError::InvalidFingerprint(_))
		));
	}

	#[rstest]
	fn test_validation_success() {
		let builder = AndroidConfigBuilder::new()
			.package_name("com.example.app")
			.sha256_fingerprint(VALID_FINGERPRINT);
		assert!(builder.validate().is_ok());
	}

	#[rstest]
	fn test_validation_additional_package_empty_fingerprints() {
		// Arrange
		let builder = AndroidConfigBuilder::new()
			.package_name("com.example.app")
			.sha256_fingerprint(VALID_FINGERPRINT)
			.additional_package("com.example.app2", &[]);

		// Act
		let result = builder.validate();

		// Assert
		assert!(
			matches!(result, Err(DeeplinkError::MissingFingerprint)),
			"additional_package with empty fingerprints should be rejected"
		);
	}
}
