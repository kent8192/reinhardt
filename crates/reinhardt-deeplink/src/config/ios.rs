//! iOS Universal Links configuration.
//!
//! This module provides types and builders for generating Apple App Site Association (AASA) files.

use serde::Serialize;

use crate::error::{DeeplinkError, validate_app_id};

/// iOS Universal Links configuration.
///
/// This struct represents the complete Apple App Site Association (AASA) file format.
/// When serialized to JSON, it produces the file that should be served at
/// `/.well-known/apple-app-site-association`.
///
/// # Example
///
/// ```rust
/// use reinhardt_deeplink::IosConfig;
///
/// let config = IosConfig::builder()
///     .app_id("TEAM123456.com.example.app")
///     .paths(&["/products/*", "/users/*"])
///     .exclude_paths(&["/api/*"])
///     .build();
/// ```
#[derive(Debug, Clone, Serialize)]
pub struct IosConfig {
	/// App links configuration for Universal Links.
	pub applinks: AppLinksConfig,

	/// Web credentials configuration for password autofill.
	#[serde(skip_serializing_if = "Option::is_none")]
	pub webcredentials: Option<WebCredentialsConfig>,

	/// App Clips configuration.
	#[serde(skip_serializing_if = "Option::is_none")]
	pub appclips: Option<AppClipsConfig>,
}

/// App links section of the AASA file.
#[derive(Debug, Clone, Serialize)]
pub struct AppLinksConfig {
	/// Legacy field, should always be an empty array.
	pub apps: Vec<String>,

	/// Details for each app that can handle links.
	pub details: Vec<AppLinkDetail>,
}

/// Individual app link detail entry.
#[derive(Debug, Clone, Serialize)]
pub struct AppLinkDetail {
	/// App IDs that can handle these paths.
	#[serde(rename = "appIDs")]
	pub app_ids: Vec<String>,

	/// URL paths that should open the app.
	#[serde(skip_serializing_if = "Vec::is_empty")]
	pub paths: Vec<String>,

	/// URL paths that should NOT open the app.
	#[serde(skip_serializing_if = "Vec::is_empty")]
	pub exclude: Vec<String>,

	/// iOS 13+ component-based URL matching.
	#[serde(skip_serializing_if = "Option::is_none")]
	pub components: Option<Vec<AppLinkComponent>>,
}

/// iOS 13+ component-based URL matching.
///
/// Allows more granular control over URL matching including query parameters
/// and URL fragments.
#[derive(Debug, Clone, Serialize)]
pub struct AppLinkComponent {
	/// Path pattern to match.
	#[serde(rename = "/")]
	pub path: String,

	/// Query string pattern to match.
	#[serde(rename = "?", skip_serializing_if = "Option::is_none")]
	pub query: Option<String>,

	/// URL fragment pattern to match.
	#[serde(rename = "#", skip_serializing_if = "Option::is_none")]
	pub fragment: Option<String>,

	/// Whether to exclude this pattern.
	#[serde(skip_serializing_if = "Option::is_none")]
	pub exclude: Option<bool>,

	/// Optional comment for documentation.
	#[serde(skip_serializing_if = "Option::is_none")]
	pub comment: Option<String>,
}

/// Web credentials configuration for password autofill.
#[derive(Debug, Clone, Serialize)]
pub struct WebCredentialsConfig {
	/// App IDs that can use web credentials from this domain.
	pub apps: Vec<String>,
}

/// App Clips configuration.
#[derive(Debug, Clone, Serialize)]
pub struct AppClipsConfig {
	/// App Clip bundle IDs.
	pub apps: Vec<String>,
}

impl IosConfig {
	/// Creates a new builder for iOS configuration.
	pub fn builder() -> IosConfigBuilder {
		IosConfigBuilder::new()
	}
}

/// Builder for iOS Universal Links configuration.
#[derive(Debug, Default)]
pub struct IosConfigBuilder {
	app_ids: Vec<String>,
	paths: Vec<String>,
	exclude_paths: Vec<String>,
	components: Vec<AppLinkComponent>,
	additional_details: Vec<AppLinkDetail>,
	web_credentials_apps: Vec<String>,
	app_clips: Vec<String>,
}

impl IosConfigBuilder {
	/// Creates a new builder.
	pub fn new() -> Self {
		Self::default()
	}

	/// Sets the primary app ID.
	///
	/// # Arguments
	///
	/// * `app_id` - The app ID in format `TEAM_ID.bundle_identifier`
	pub fn app_id(mut self, app_id: impl Into<String>) -> Self {
		let id = app_id.into();
		if !self.app_ids.contains(&id) {
			self.app_ids.push(id);
		}
		self
	}

	/// Adds additional app IDs.
	pub fn additional_app_id(mut self, app_id: impl Into<String>) -> Self {
		let id = app_id.into();
		if !self.app_ids.contains(&id) {
			self.app_ids.push(id);
		}
		self
	}

	/// Sets the URL paths that should open the app.
	///
	/// Paths support wildcards:
	/// - `*` matches any sequence of characters
	/// - `?` matches any single character
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_deeplink::IosConfig;
	///
	/// let config = IosConfig::builder()
	///     .app_id("TEAM.com.example")
	///     .paths(&["/products/*", "/users/*"])
	///     .build();
	/// ```
	pub fn paths(mut self, paths: &[&str]) -> Self {
		self.paths.extend(paths.iter().map(|s| (*s).to_string()));
		self
	}

	/// Adds a single path.
	pub fn path(mut self, path: impl Into<String>) -> Self {
		self.paths.push(path.into());
		self
	}

	/// Sets paths that should NOT open the app.
	pub fn exclude_paths(mut self, paths: &[&str]) -> Self {
		self.exclude_paths
			.extend(paths.iter().map(|s| (*s).to_string()));
		self
	}

	/// Adds a single exclude path.
	pub fn exclude_path(mut self, path: impl Into<String>) -> Self {
		self.exclude_paths.push(path.into());
		self
	}

	/// Adds an iOS 13+ component for fine-grained URL matching.
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_deeplink::{IosConfig, AppLinkComponent};
	///
	/// let config = IosConfig::builder()
	///     .app_id("TEAM.com.example")
	///     .component(AppLinkComponent {
	///         path: "/products/*".to_string(),
	///         query: Some("ref=*".to_string()),
	///         fragment: None,
	///         exclude: None,
	///         comment: Some("Product pages with referral".to_string()),
	///     })
	///     .build();
	/// ```
	pub fn component(mut self, component: AppLinkComponent) -> Self {
		self.components.push(component);
		self
	}

	/// Adds a detail entry for a different app.
	///
	/// Use this when multiple apps should handle different URL patterns.
	pub fn additional_app(mut self, app_id: impl Into<String>, paths: &[&str]) -> Self {
		self.additional_details.push(AppLinkDetail {
			app_ids: vec![app_id.into()],
			paths: paths.iter().map(|s| (*s).to_string()).collect(),
			exclude: Vec::new(),
			components: None,
		});
		self
	}

	/// Enables web credentials for the configured app IDs.
	///
	/// This allows password autofill to work with your app.
	pub fn with_web_credentials(mut self) -> Self {
		self.web_credentials_apps.clone_from(&self.app_ids);
		self
	}

	/// Adds an App Clip configuration.
	///
	/// App Clip paths are configured via App Store Connect, not in the AASA file.
	/// This method only registers the App Clip bundle ID.
	///
	/// # Arguments
	///
	/// * `app_id` - The App Clip bundle ID (usually ends with `.Clip`)
	pub fn app_clip(mut self, app_id: impl Into<String>) -> Self {
		self.app_clips.push(app_id.into());
		self
	}

	/// Validates the configuration.
	///
	/// # Errors
	///
	/// Returns an error if:
	/// - No app IDs are configured
	/// - Any app ID has an invalid format
	/// - No paths or components are specified
	pub fn validate(&self) -> Result<(), DeeplinkError> {
		if self.app_ids.is_empty() {
			return Err(DeeplinkError::InvalidAppId(
				"no app IDs configured".to_string(),
			));
		}

		for app_id in &self.app_ids {
			validate_app_id(app_id)?;
		}

		if self.has_no_paths_or_components() {
			return Err(DeeplinkError::NoPathsSpecified);
		}

		Ok(())
	}

	/// Checks if the builder has no paths, components, or additional details configured.
	fn has_no_paths_or_components(&self) -> bool {
		self.paths.is_empty() && self.components.is_empty() && self.additional_details.is_empty()
	}

	/// Builds the iOS configuration.
	///
	/// This method does not validate the configuration. Use [`validate`](Self::validate)
	/// before building if validation is needed.
	pub fn build(self) -> IosConfig {
		let mut details = Vec::new();

		// Build primary detail if we have paths or components
		if !self.paths.is_empty() || !self.components.is_empty() {
			details.push(AppLinkDetail {
				app_ids: self.app_ids.clone(),
				paths: self.paths,
				exclude: self.exclude_paths,
				components: if self.components.is_empty() {
					None
				} else {
					Some(self.components)
				},
			});
		}

		// Add additional details
		details.extend(self.additional_details);

		// Build webcredentials if configured
		let webcredentials = if self.web_credentials_apps.is_empty() {
			None
		} else {
			Some(WebCredentialsConfig {
				apps: self.web_credentials_apps,
			})
		};

		// Build appclips if configured
		let appclips = if self.app_clips.is_empty() {
			None
		} else {
			Some(AppClipsConfig {
				apps: self.app_clips,
			})
		};

		IosConfig {
			applinks: AppLinksConfig {
				apps: Vec::new(), // Always empty per Apple spec
				details,
			},
			webcredentials,
			appclips,
		}
	}
}

#[cfg(test)]
mod tests {
	use rstest::rstest;

	use super::*;

	#[rstest]
	fn test_basic_ios_config() {
		let config = IosConfig::builder()
			.app_id("TEAM123456.com.example.app")
			.paths(&["/products/*", "/users/*"])
			.build();

		let json = serde_json::to_string_pretty(&config).unwrap();
		assert!(json.contains("applinks"));
		assert!(json.contains("TEAM123456.com.example.app"));
		assert!(json.contains("/products/*"));
	}

	#[rstest]
	fn test_ios_config_with_exclude() {
		let config = IosConfig::builder()
			.app_id("TEAM.com.example")
			.paths(&["/products/*"])
			.exclude_paths(&["/api/*"])
			.build();

		let json = serde_json::to_string_pretty(&config).unwrap();
		assert!(json.contains("/api/*"));
	}

	#[rstest]
	fn test_ios_config_with_components() {
		let config = IosConfig::builder()
			.app_id("TEAM.com.example")
			.component(AppLinkComponent {
				path: "/products/*".to_string(),
				query: Some("ref=*".to_string()),
				fragment: None,
				exclude: None,
				comment: Some("Product pages".to_string()),
			})
			.build();

		let json = serde_json::to_string_pretty(&config).unwrap();
		assert!(json.contains("components"));
		assert!(json.contains("ref=*"));
	}

	#[rstest]
	fn test_ios_config_with_web_credentials() {
		let config = IosConfig::builder()
			.app_id("TEAM.com.example")
			.paths(&["/"])
			.with_web_credentials()
			.build();

		let json = serde_json::to_string_pretty(&config).unwrap();
		assert!(json.contains("webcredentials"));
	}

	#[rstest]
	fn test_ios_config_with_app_clips() {
		let config = IosConfig::builder()
			.app_id("TEAM.com.example")
			.paths(&["/"])
			.app_clip("TEAM.com.example.Clip")
			.build();

		let json = serde_json::to_string_pretty(&config).unwrap();
		assert!(json.contains("appclips"));
	}

	#[rstest]
	fn test_validation_no_app_ids() {
		let builder = IosConfigBuilder::new().paths(&["/"]);
		assert!(builder.validate().is_err());
	}

	#[rstest]
	fn test_validation_no_paths() {
		let builder = IosConfigBuilder::new().app_id("TEAM.com.example");
		assert!(builder.validate().is_err());
	}

	#[rstest]
	fn test_validation_success() {
		let builder = IosConfigBuilder::new()
			.app_id("TEAM.com.example")
			.paths(&["/"]);
		assert!(builder.validate().is_ok());
	}
}
