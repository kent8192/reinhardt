//! Deeplink configuration types and builders.
//!
//! This module provides unified configuration for mobile app deep linking:
//!
//! - [`IosConfig`] - iOS Universal Links configuration
//! - [`AndroidConfig`] - Android App Links configuration
//! - [`CustomSchemeConfig`] - Custom URL scheme configuration
//! - [`DeeplinkConfig`] - Unified configuration combining all platforms

pub mod android;
pub mod custom;
pub mod ios;

pub use android::{AndroidConfig, AndroidConfigBuilder, AssetStatement, AssetTarget};
pub use custom::{CustomScheme, CustomSchemeBuilder, CustomSchemeConfig};
pub use ios::{
	AppClipsConfig, AppLinkComponent, AppLinkDetail, AppLinksConfig, IosConfig, IosConfigBuilder,
	WebCredentialsConfig,
};

/// Unified deeplink configuration.
///
/// This struct combines iOS, Android, and custom scheme configurations into a single
/// configuration object that can be used with the deeplink router.
///
/// # Example
///
/// ```rust
/// use reinhardt_deeplink::{DeeplinkConfig, IosConfig, AndroidConfig};
///
/// let config = DeeplinkConfig::builder()
///     .ios(
///         IosConfig::builder()
///             .app_id("TEAM123456.com.example.app")
///             .paths(&["/products/*"])
///             .build()
///     )
///     .android(
///         AndroidConfig::builder()
///             .package_name("com.example.app")
///             .sha256_fingerprint("FA:C6:17:45:DC:09:03:78:6F:B9:ED:E6:2A:96:2B:39:9F:73:48:F0:BB:6F:89:9B:83:32:66:75:91:03:3B:9C")
///             .build()
///             .unwrap()
///     )
///     .build();
///
/// assert!(config.is_configured());
/// ```
#[derive(Debug, Clone, Default)]
pub struct DeeplinkConfig {
	/// iOS Universal Links configuration.
	pub ios: Option<IosConfig>,

	/// Android App Links configuration.
	pub android: Option<AndroidConfig>,

	/// Custom URL scheme configurations.
	pub custom_schemes: Vec<CustomScheme>,
}

impl DeeplinkConfig {
	/// Creates a new builder for deeplink configuration.
	pub fn builder() -> DeeplinkConfigBuilder {
		DeeplinkConfigBuilder::new()
	}

	/// Returns `true` if any platform is configured.
	pub fn is_configured(&self) -> bool {
		self.ios.is_some() || self.android.is_some() || !self.custom_schemes.is_empty()
	}

	/// Returns `true` if iOS is configured.
	pub fn has_ios(&self) -> bool {
		self.ios.is_some()
	}

	/// Returns `true` if Android is configured.
	pub fn has_android(&self) -> bool {
		self.android.is_some()
	}

	/// Returns `true` if custom schemes are configured.
	pub fn has_custom_schemes(&self) -> bool {
		!self.custom_schemes.is_empty()
	}
}

/// Builder for unified deeplink configuration.
#[derive(Debug, Default)]
pub struct DeeplinkConfigBuilder {
	ios: Option<IosConfig>,
	android: Option<AndroidConfig>,
	custom_schemes: Vec<CustomScheme>,
}

impl DeeplinkConfigBuilder {
	/// Creates a new builder.
	pub fn new() -> Self {
		Self::default()
	}

	/// Sets the iOS configuration.
	pub fn ios(mut self, config: IosConfig) -> Self {
		self.ios = Some(config);
		self
	}

	/// Sets the Android configuration.
	pub fn android(mut self, config: AndroidConfig) -> Self {
		self.android = Some(config);
		self
	}

	/// Adds a custom URL scheme.
	///
	/// # Arguments
	///
	/// * `scheme` - The scheme name (e.g., `myapp`)
	pub fn custom_scheme(mut self, scheme: impl Into<String>) -> Self {
		self.custom_schemes.push(CustomScheme {
			name: scheme.into(),
			hosts: Vec::new(),
			paths: Vec::new(),
		});
		self
	}

	/// Adds a custom scheme configuration.
	pub fn custom_scheme_config(mut self, config: CustomSchemeConfig) -> Self {
		self.custom_schemes.extend(config.schemes);
		self
	}

	/// Validates all custom scheme names in the configuration.
	///
	/// # Errors
	///
	/// Returns an error if any custom scheme name is invalid per RFC 3986
	/// or is a dangerous scheme.
	pub fn validate_schemes(&self) -> Result<(), crate::error::DeeplinkError> {
		for scheme in &self.custom_schemes {
			crate::error::validate_scheme_name(&scheme.name)?;
		}
		Ok(())
	}

	/// Builds the deeplink configuration.
	pub fn build(self) -> DeeplinkConfig {
		DeeplinkConfig {
			ios: self.ios,
			android: self.android,
			custom_schemes: self.custom_schemes,
		}
	}
}

#[cfg(test)]
mod tests {
	use rstest::rstest;

	use super::*;

	const VALID_FINGERPRINT: &str = "FA:C6:17:45:DC:09:03:78:6F:B9:ED:E6:2A:96:2B:39:9F:73:48:F0:BB:6F:89:9B:83:32:66:75:91:03:3B:9C";

	#[rstest]
	fn test_empty_config() {
		let config = DeeplinkConfig::default();
		assert!(!config.is_configured());
		assert!(!config.has_ios());
		assert!(!config.has_android());
		assert!(!config.has_custom_schemes());
	}

	#[rstest]
	fn test_ios_only() {
		let config = DeeplinkConfig::builder()
			.ios(
				IosConfig::builder()
					.app_id("TEAM.com.example")
					.paths(&["/"])
					.build(),
			)
			.build();

		assert!(config.is_configured());
		assert!(config.has_ios());
		assert!(!config.has_android());
	}

	#[rstest]
	fn test_android_only() {
		let config = DeeplinkConfig::builder()
			.android(
				AndroidConfig::builder()
					.package_name("com.example.app")
					.sha256_fingerprint(VALID_FINGERPRINT)
					.build()
					.unwrap(),
			)
			.build();

		assert!(config.is_configured());
		assert!(!config.has_ios());
		assert!(config.has_android());
	}

	#[rstest]
	fn test_full_config() {
		let config = DeeplinkConfig::builder()
			.ios(
				IosConfig::builder()
					.app_id("TEAM.com.example")
					.paths(&["/"])
					.build(),
			)
			.android(
				AndroidConfig::builder()
					.package_name("com.example.app")
					.sha256_fingerprint(VALID_FINGERPRINT)
					.build()
					.unwrap(),
			)
			.custom_scheme("myapp")
			.build();

		assert!(config.is_configured());
		assert!(config.has_ios());
		assert!(config.has_android());
		assert!(config.has_custom_schemes());
	}

	#[rstest]
	fn test_custom_scheme_config() {
		let custom = CustomSchemeConfig::builder()
			.scheme("myapp")
			.host("open")
			.paths(&["/products/*"])
			.build();

		let config = DeeplinkConfig::builder()
			.custom_scheme_config(custom)
			.build();

		assert!(config.is_configured());
		assert!(config.has_custom_schemes());
		assert_eq!(config.custom_schemes.len(), 1);
		assert_eq!(config.custom_schemes[0].name, "myapp");
	}
}
