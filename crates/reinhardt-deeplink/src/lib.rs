//! Deeplink support for Reinhardt framework.
//!
//! This crate provides support for mobile app deep linking:
//!
//! - **iOS Universal Links**: Apple App Site Association (AASA) file generation
//! - **Android App Links**: Digital Asset Links (assetlinks.json) generation
//! - **Custom URL Schemes**: Configuration helpers for custom schemes (e.g., `myapp://`)
//!
//! # Quick Start
//!
//! ```rust
//! use reinhardt_deeplink::{DeeplinkConfig, IosConfig, AndroidConfig};
//!
//! let config = DeeplinkConfig::builder()
//!     .ios(
//!         IosConfig::builder()
//!             .app_id("TEAM_ID.com.example.app")
//!             .paths(&["/products/*", "/users/*"])
//!             .build()
//!     )
//!     .android(
//!         AndroidConfig::builder()
//!             .package_name("com.example.app")
//!             .sha256_fingerprint("FA:C6:17:45:DC:09:03:78:6F:B9:ED:E6:2A:96:2B:39:9F:73:48:F0:BB:6F:89:9B:83:32:66:75:91:03:3B:9C")
//!             .build()
//!             .unwrap()
//!     )
//!     .build();
//! ```
//!
//! # Router Integration
//!
//! ```rust,ignore
//! use reinhardt_urls::routers::UnifiedRouter;
//! use reinhardt_deeplink::DeeplinkRouterExt;
//!
//! let router = UnifiedRouter::new()
//!     .with_deeplinks(config);
//! ```

pub mod config;
pub mod endpoints;
pub mod error;
pub mod router;

// Re-export main types for convenience
pub use config::{
	AndroidConfig, AndroidConfigBuilder, AppClipsConfig, AppLinkComponent, AppLinkDetail,
	AppLinksConfig, AssetStatement, AssetTarget, CustomScheme, CustomSchemeBuilder,
	CustomSchemeConfig, DeeplinkConfig, DeeplinkConfigBuilder, IosConfig, IosConfigBuilder,
	WebCredentialsConfig,
};
pub use endpoints::{AasaHandler, AssetLinksHandler};
pub use error::{
	DeeplinkError, validate_app_id, validate_bundle_id, validate_fingerprint,
	validate_package_name, validate_scheme_name,
};
pub use router::{DeeplinkRouter, DeeplinkRouterExt};

/// Result type for deeplink operations.
pub type DeeplinkResult<T> = Result<T, DeeplinkError>;
