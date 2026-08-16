//! Settings-first configuration fragment for rate limiting.
//!
//! [`RateLimitSettings`] is the settings-first entry point for the rate limiting
//! middleware. It maps to the `[server_rate_limit]` TOML section and can be
//! composed into a project's settings with the `#[settings]` macro. A conversion
//! into the deprecated compatibility [`RateLimitConfig`] type is provided for the
//! migration window; new code should prefer the fragment and the
//! [`create_rate_limit_handler_from_settings`] constructor.

#![allow(deprecated)] // Conversion targets the legacy RateLimitConfig during the compatibility window.

use std::sync::Arc;
use std::time::Duration;

use reinhardt_core::macros::settings;
use reinhardt_http::Handler;
use serde::{Deserialize, Serialize};

use super::rate_limit::{RateLimitConfig, RateLimitHandler, RateLimitStrategy};

// --- defaults -------------------------------------------------------------

fn default_max_requests() -> usize {
	60
}

fn default_window_secs() -> u64 {
	60
}

fn default_strategy() -> RateLimitStrategyKind {
	RateLimitStrategyKind::FixedWindow
}

/// Serializable mirror of [`RateLimitStrategy`].
///
/// [`RateLimitStrategy`] does not derive `Serialize`/`Deserialize`, so this
/// value object provides the (de)serializable representation used inside
/// [`RateLimitSettings`]. It is not an independently loadable section.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum RateLimitStrategyKind {
	/// Fixed window rate limiting.
	#[default]
	FixedWindow,
	/// Sliding window rate limiting.
	SlidingWindow,
}

impl From<RateLimitStrategyKind> for RateLimitStrategy {
	fn from(kind: RateLimitStrategyKind) -> Self {
		match kind {
			RateLimitStrategyKind::FixedWindow => RateLimitStrategy::FixedWindow,
			RateLimitStrategyKind::SlidingWindow => RateLimitStrategy::SlidingWindow,
		}
	}
}

impl From<RateLimitStrategy> for RateLimitStrategyKind {
	fn from(strategy: RateLimitStrategy) -> Self {
		match strategy {
			RateLimitStrategy::FixedWindow => RateLimitStrategyKind::FixedWindow,
			RateLimitStrategy::SlidingWindow => RateLimitStrategyKind::SlidingWindow,
		}
	}
}

/// Rate limiting settings fragment.
///
/// Maps to the `[server_rate_limit]` section.
#[settings(fragment = true, section = "server_rate_limit")]
#[non_exhaustive]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RateLimitSettings {
	/// Maximum number of requests allowed within the window.
	#[serde(default = "default_max_requests")]
	pub max_requests: usize,
	/// Length of the rate limiting window, in seconds.
	#[serde(default = "default_window_secs")]
	pub window_secs: u64,
	/// Rate limiting strategy.
	#[setting(leaf)]
	#[serde(default = "default_strategy")]
	pub strategy: RateLimitStrategyKind,
	/// Trusted proxy IP addresses/CIDRs.
	///
	/// Only requests originating from these addresses will have their
	/// `X-Forwarded-For`/`X-Real-IP` headers trusted for client IP extraction.
	#[serde(default)]
	pub trusted_proxies: Vec<String>,
}

impl Default for RateLimitSettings {
	fn default() -> Self {
		Self {
			max_requests: default_max_requests(),
			window_secs: default_window_secs(),
			strategy: default_strategy(),
			trusted_proxies: Vec::new(),
		}
	}
}

impl From<&RateLimitSettings> for RateLimitConfig {
	fn from(settings: &RateLimitSettings) -> Self {
		RateLimitConfig::new(
			settings.max_requests,
			Duration::from_secs(settings.window_secs),
			settings.strategy.into(),
		)
		.with_trusted_proxies(settings.trusted_proxies.clone())
	}
}

/// Build a [`RateLimitHandler`] wrapping `inner` from a [`RateLimitSettings`]
/// fragment.
pub fn create_rate_limit_handler_from_settings(
	inner: Arc<dyn Handler>,
	settings: &RateLimitSettings,
) -> RateLimitHandler {
	RateLimitHandler::new(inner, RateLimitConfig::from(settings))
}

#[cfg(test)]
mod tests {
	use super::*;
	use reinhardt_conf::settings::fragment::SettingsFragment;

	#[rstest::rstest]
	fn section_name_is_crate_prefixed() {
		// Arrange / Act / Assert
		assert_eq!(RateLimitSettings::section(), "server_rate_limit");
	}

	#[rstest::rstest]
	fn default_converts_to_config() {
		// Arrange
		let settings = RateLimitSettings::default();

		// Act
		let config = RateLimitConfig::from(&settings);

		// Assert
		assert_eq!(config.max_requests, 60);
		assert_eq!(config.window_duration, Duration::from_secs(60));
		assert_eq!(config.strategy, RateLimitStrategy::FixedWindow);
		assert!(config.trusted_proxies.is_empty());
	}

	#[rstest::rstest]
	fn converts_window_seconds_strategy_and_proxies() {
		// Arrange
		let settings = RateLimitSettings {
			max_requests: 10,
			window_secs: 3600,
			strategy: RateLimitStrategyKind::SlidingWindow,
			trusted_proxies: vec!["10.0.0.0/8".to_string()],
		};

		// Act
		let config = RateLimitConfig::from(&settings);

		// Assert
		assert_eq!(config.max_requests, 10);
		assert_eq!(config.window_duration, Duration::from_secs(3600));
		assert_eq!(config.strategy, RateLimitStrategy::SlidingWindow);
		assert_eq!(config.trusted_proxies, vec!["10.0.0.0/8".to_string()]);
	}

	#[rstest::rstest]
	fn deserializes_with_defaults() {
		// Arrange — only max_requests is provided; everything else uses defaults.
		let json = r#"{ "max_requests": 100 }"#;

		// Act
		let settings: RateLimitSettings = serde_json::from_str(json).unwrap();
		let config = RateLimitConfig::from(&settings);

		// Assert
		assert_eq!(config.max_requests, 100);
		assert_eq!(config.window_duration, Duration::from_secs(60));
		assert_eq!(config.strategy, RateLimitStrategy::FixedWindow);
		assert!(config.trusted_proxies.is_empty());
	}

	#[rstest::rstest]
	fn deserializes_strategy_in_snake_case() {
		// Arrange
		let json = r#"{ "strategy": "sliding_window" }"#;

		// Act
		let settings: RateLimitSettings = serde_json::from_str(json).unwrap();

		// Assert
		assert_eq!(settings.strategy, RateLimitStrategyKind::SlidingWindow);
	}
}
