//! Router integration for deeplink endpoints.
//!
//! This module provides router types and extension traits for integrating
//! deeplink handlers with the Reinhardt routing system.

use hyper::Method;
use reinhardt_urls::routers::{ServerRouter, UnifiedRouter};

use crate::config::DeeplinkConfig;
use crate::endpoints::{AasaHandler, AssetLinksHandler};
use crate::error::DeeplinkError;

/// Dedicated router for deeplink endpoints.
///
/// This router handles the well-known endpoints required for mobile deep linking:
///
/// - `GET /.well-known/apple-app-site-association` - iOS Universal Links
/// - `GET /.well-known/apple-app-site-association.json` - iOS Universal Links (alternative)
/// - `GET /.well-known/assetlinks.json` - Android App Links
///
/// # Example
///
/// ```rust
/// use reinhardt_deeplink::{DeeplinkRouter, DeeplinkConfig, IosConfig, AndroidConfig};
///
/// let config = DeeplinkConfig::builder()
///     .ios(
///         IosConfig::builder()
///             .app_id("TEAM.com.example")
///             .paths(&["/"])
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
/// let router = DeeplinkRouter::new(config).unwrap();
/// ```
pub struct DeeplinkRouter {
	/// The deeplink configuration.
	config: DeeplinkConfig,

	/// The underlying server router.
	server: ServerRouter,
}

impl std::fmt::Debug for DeeplinkRouter {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("DeeplinkRouter")
			.field("config", &self.config)
			.field("server", &"ServerRouter { ... }")
			.finish()
	}
}

impl DeeplinkRouter {
	/// Creates a new deeplink router with the given configuration.
	///
	/// # Errors
	///
	/// Returns an error if:
	/// - iOS is configured but JSON serialization fails
	/// - Android is configured but JSON serialization fails
	pub fn new(config: DeeplinkConfig) -> Result<Self, DeeplinkError> {
		let mut server = ServerRouter::new().with_namespace("wellknown");

		// Register iOS Universal Links endpoints
		if let Some(ios_config) = &config.ios {
			let aasa_handler = AasaHandler::new(ios_config.clone())?;

			// Register at both paths (some tools expect .json extension)
			server = server
				.handler_with_method(
					"/apple-app-site-association",
					Method::GET,
					aasa_handler.clone(),
				)
				.handler_with_method(
					"/apple-app-site-association.json",
					Method::GET,
					aasa_handler,
				);
		}

		// Register Android App Links endpoint
		if let Some(android_config) = &config.android {
			let assetlinks_handler = AssetLinksHandler::new(android_config.clone())?;
			server =
				server.handler_with_method("/assetlinks.json", Method::GET, assetlinks_handler);
		}

		Ok(Self { config, server })
	}

	/// Converts this router into a `ServerRouter`.
	///
	/// This is useful when you need to mount the deeplink router
	/// onto another router manually.
	pub fn into_server(self) -> ServerRouter {
		self.server
	}

	/// Returns a reference to the underlying `ServerRouter`.
	pub fn server(&self) -> &ServerRouter {
		&self.server
	}

	/// Returns a reference to the configuration.
	pub fn config(&self) -> &DeeplinkConfig {
		&self.config
	}
}

/// Extension trait for integrating deeplinks with `UnifiedRouter`.
///
/// This trait provides a convenient method to add deeplink support to
/// any `UnifiedRouter`.
///
/// # Example
///
/// ```rust,ignore
/// use reinhardt_urls::routers::UnifiedRouter;
/// use reinhardt_deeplink::{DeeplinkRouterExt, DeeplinkConfig, IosConfig};
///
/// let config = DeeplinkConfig::builder()
///     .ios(
///         IosConfig::builder()
///             .app_id("TEAM.com.example")
///             .paths(&["/"])
///             .build()
///     )
///     .build();
///
/// let router = UnifiedRouter::new()
///     .with_deeplinks(config)
///     .unwrap();
/// ```
pub trait DeeplinkRouterExt {
	/// The output type after adding deeplinks.
	type Output;

	/// Adds deeplink handlers to the router.
	///
	/// This mounts the deeplink handlers under the `/.well-known/` path prefix.
	///
	/// # Errors
	///
	/// Returns an error if the deeplink router cannot be created.
	fn with_deeplinks(self, config: DeeplinkConfig) -> Result<Self::Output, DeeplinkError>;
}

impl DeeplinkRouterExt for UnifiedRouter {
	type Output = Self;

	fn with_deeplinks(self, config: DeeplinkConfig) -> Result<Self, DeeplinkError> {
		let deeplink_router = DeeplinkRouter::new(config)?;
		Ok(self.mount("/.well-known/", deeplink_router.into_server()))
	}
}

impl DeeplinkRouterExt for ServerRouter {
	type Output = Self;

	fn with_deeplinks(self, config: DeeplinkConfig) -> Result<Self, DeeplinkError> {
		let deeplink_router = DeeplinkRouter::new(config)?;
		Ok(self.mount("/.well-known/", deeplink_router.into_server()))
	}
}

#[cfg(test)]
mod tests {
	use rstest::rstest;

	use super::*;
	use crate::config::{AndroidConfig, IosConfig};

	const VALID_FINGERPRINT: &str = "FA:C6:17:45:DC:09:03:78:6F:B9:ED:E6:2A:96:2B:39:9F:73:48:F0:BB:6F:89:9B:83:32:66:75:91:03:3B:9C";

	fn create_ios_config() -> IosConfig {
		IosConfig::builder()
			.app_id("TEAM123456.com.example.app")
			.paths(&["/products/*"])
			.build()
	}

	fn create_android_config() -> AndroidConfig {
		AndroidConfig::builder()
			.package_name("com.example.app")
			.sha256_fingerprint(VALID_FINGERPRINT)
			.build()
			.unwrap()
	}

	#[rstest]
	fn test_router_creation_ios_only() {
		let config = DeeplinkConfig::builder().ios(create_ios_config()).build();

		let router = DeeplinkRouter::new(config).unwrap();
		assert!(router.config().has_ios());
		assert!(!router.config().has_android());
	}

	#[rstest]
	fn test_router_creation_android_only() {
		let config = DeeplinkConfig::builder()
			.android(create_android_config())
			.build();

		let router = DeeplinkRouter::new(config).unwrap();
		assert!(!router.config().has_ios());
		assert!(router.config().has_android());
	}

	#[rstest]
	fn test_router_creation_both() {
		let config = DeeplinkConfig::builder()
			.ios(create_ios_config())
			.android(create_android_config())
			.build();

		let router = DeeplinkRouter::new(config).unwrap();
		assert!(router.config().has_ios());
		assert!(router.config().has_android());
	}

	#[rstest]
	fn test_into_server() {
		let config = DeeplinkConfig::builder().ios(create_ios_config()).build();

		let router = DeeplinkRouter::new(config).unwrap();
		let _server = router.into_server();
	}

	#[rstest]
	fn test_extension_trait_unified() {
		let config = DeeplinkConfig::builder().ios(create_ios_config()).build();

		let router = UnifiedRouter::new().with_deeplinks(config).unwrap();

		// Verify the router was created (we can't easily test the routes without making requests)
		let _ = router;
	}

	#[rstest]
	fn test_extension_trait_server() {
		let config = DeeplinkConfig::builder().ios(create_ios_config()).build();

		let router = ServerRouter::new().with_deeplinks(config).unwrap();

		// Verify the router was created
		let _ = router;
	}

	#[rstest]
	fn test_empty_config() {
		let config = DeeplinkConfig::default();
		let router = DeeplinkRouter::new(config).unwrap();

		// Empty config should still create a valid router
		assert!(!router.config().is_configured());
	}
}
