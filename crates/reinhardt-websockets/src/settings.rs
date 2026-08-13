//! Settings-first configuration fragments for WebSocket support.
//!
//! These fragments are the settings-first configuration entry points for the
//! WebSocket layer. Each public, independently loadable fragment maps to a
//! `[ws_*]` TOML section and can be composed into a project's settings with the
//! `#[settings]` macro. Conversions into the deprecated compatibility
//! `XxxConfig` types are provided for the migration window; new code should
//! prefer the fragments and the `create_*_from_settings` constructors.

#![allow(deprecated)] // Conversions target legacy config types during the compatibility window.

use std::time::Duration;

use reinhardt_core::macros::settings;
use serde::{Deserialize, Serialize};

use crate::connection::ConnectionConfig;
use crate::origin::{OriginPolicy, OriginValidationConfig};
use crate::reconnection::ReconnectionConfig;
use crate::throttling::WebSocketRateLimitConfig;

#[cfg(feature = "redis-channel")]
use crate::redis_channel::RedisConfig;

// --- defaults -------------------------------------------------------------

fn default_idle_timeout_secs() -> u64 {
	300
}
fn default_handshake_timeout_secs() -> u64 {
	10
}
fn default_cleanup_interval_secs() -> u64 {
	30
}
fn default_reconnect_max_attempts() -> Option<u32> {
	Some(10)
}
fn default_reconnect_initial_delay_secs() -> u64 {
	1
}
fn default_reconnect_max_delay_secs() -> u64 {
	300
}
fn default_reconnect_backoff_multiplier() -> f64 {
	2.0
}
fn default_reconnect_jitter_factor() -> f64 {
	0.1
}
fn default_reject_missing_origin() -> bool {
	true
}
fn default_rate_max_connections_per_window() -> usize {
	20
}
fn default_rate_connection_window_secs() -> u64 {
	60
}
fn default_rate_max_concurrent_connections_per_ip() -> usize {
	10
}
fn default_rate_max_messages_per_window() -> usize {
	100
}
fn default_rate_message_window_secs() -> u64 {
	60
}

#[cfg(feature = "redis-channel")]
fn default_redis_url() -> String {
	"redis://127.0.0.1:6379".to_string()
}
#[cfg(feature = "redis-channel")]
fn default_redis_channel_prefix() -> String {
	"ws:channel:".to_string()
}
#[cfg(feature = "redis-channel")]
fn default_redis_group_prefix() -> String {
	"ws:group:".to_string()
}
#[cfg(feature = "redis-channel")]
fn default_redis_message_expiry() -> u64 {
	60
}
#[cfg(feature = "redis-channel")]
fn default_redis_require_auth() -> bool {
	true
}

// --- connection -----------------------------------------------------------

/// WebSocket connection settings fragment.
///
/// Maps to the `[ws_connection]` section.
#[settings(fragment = true, section = "ws_connection")]
#[non_exhaustive]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConnectionSettings {
	/// Maximum duration a connection can be idle before being closed, in seconds.
	#[serde(default = "default_idle_timeout_secs")]
	pub idle_timeout_secs: u64,
	/// Maximum duration for the WebSocket handshake to complete, in seconds.
	#[serde(default = "default_handshake_timeout_secs")]
	pub handshake_timeout_secs: u64,
	/// Interval for checking idle connections, in seconds.
	#[serde(default = "default_cleanup_interval_secs")]
	pub cleanup_interval_secs: u64,
	/// Maximum number of concurrent connections allowed (None for unlimited).
	#[serde(default)]
	pub max_connections: Option<usize>,
}

impl Default for ConnectionSettings {
	fn default() -> Self {
		Self {
			idle_timeout_secs: default_idle_timeout_secs(),
			handshake_timeout_secs: default_handshake_timeout_secs(),
			cleanup_interval_secs: default_cleanup_interval_secs(),
			max_connections: None,
		}
	}
}

impl From<&ConnectionSettings> for ConnectionConfig {
	fn from(settings: &ConnectionSettings) -> Self {
		// `ConnectionConfig` has private fields, so rebuild through its builder.
		ConnectionConfig::new()
			.with_idle_timeout(Duration::from_secs(settings.idle_timeout_secs))
			.with_handshake_timeout(Duration::from_secs(settings.handshake_timeout_secs))
			.with_cleanup_interval(Duration::from_secs(settings.cleanup_interval_secs))
			.with_max_connections(settings.max_connections)
	}
}

/// Build a [`ConnectionConfig`] from a [`ConnectionSettings`] fragment.
pub fn create_connection_config_from_settings(settings: &ConnectionSettings) -> ConnectionConfig {
	ConnectionConfig::from(settings)
}

// --- reconnection ---------------------------------------------------------

/// WebSocket reconnection settings fragment.
///
/// Maps to the `[ws_reconnection]` section.
#[settings(fragment = true, section = "ws_reconnection")]
#[non_exhaustive]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReconnectionSettings {
	/// Maximum number of reconnection attempts (None for unlimited).
	#[serde(default = "default_reconnect_max_attempts")]
	pub max_attempts: Option<u32>,
	/// Initial reconnection delay, in seconds.
	#[serde(default = "default_reconnect_initial_delay_secs")]
	pub initial_delay_secs: u64,
	/// Maximum reconnection delay, in seconds.
	#[serde(default = "default_reconnect_max_delay_secs")]
	pub max_delay_secs: u64,
	/// Backoff multiplier for exponential backoff.
	#[serde(default = "default_reconnect_backoff_multiplier")]
	pub backoff_multiplier: f64,
	/// Jitter factor applied to each delay (0.1 = 10%).
	#[serde(default = "default_reconnect_jitter_factor")]
	pub jitter_factor: f64,
}

impl Default for ReconnectionSettings {
	fn default() -> Self {
		Self {
			max_attempts: default_reconnect_max_attempts(),
			initial_delay_secs: default_reconnect_initial_delay_secs(),
			max_delay_secs: default_reconnect_max_delay_secs(),
			backoff_multiplier: default_reconnect_backoff_multiplier(),
			jitter_factor: default_reconnect_jitter_factor(),
		}
	}
}

impl From<&ReconnectionSettings> for ReconnectionConfig {
	fn from(settings: &ReconnectionSettings) -> Self {
		// `ReconnectionConfig` fields are public, so build it directly.
		ReconnectionConfig {
			max_attempts: settings.max_attempts,
			initial_delay: Duration::from_secs(settings.initial_delay_secs),
			max_delay: Duration::from_secs(settings.max_delay_secs),
			backoff_multiplier: settings.backoff_multiplier,
			jitter_factor: settings.jitter_factor,
		}
	}
}

/// Build a [`ReconnectionConfig`] from a [`ReconnectionSettings`] fragment.
pub fn create_reconnection_config_from_settings(
	settings: &ReconnectionSettings,
) -> ReconnectionConfig {
	ReconnectionConfig::from(settings)
}

// --- origin validation ----------------------------------------------------

/// Origin validation policy as a serializable value object.
///
/// This mirrors [`OriginPolicy`] but is `Serialize`/`Deserialize` so it can be
/// loaded from settings. It is nested under [`OriginValidationSettings`].
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "mode", content = "origins")]
pub enum OriginPolicySettings {
	/// Allow only specific origins.
	AllowList(Vec<String>),
	/// Allow all origins (disables validation; not recommended for production).
	AllowAll,
}

impl Default for OriginPolicySettings {
	fn default() -> Self {
		// Mirror `OriginValidationConfig::default()`: an empty allow-list that
		// rejects every origin until explicitly configured.
		OriginPolicySettings::AllowList(Vec::new())
	}
}

impl From<&OriginPolicySettings> for OriginPolicy {
	fn from(settings: &OriginPolicySettings) -> Self {
		match settings {
			OriginPolicySettings::AllowList(origins) => OriginPolicy::AllowList(origins.clone()),
			OriginPolicySettings::AllowAll => OriginPolicy::AllowAll,
		}
	}
}

/// Origin validation settings fragment.
///
/// Maps to the `[ws_origin]` section and controls Cross-Site WebSocket
/// Hijacking (CSWSH) protection during the handshake.
#[settings(fragment = true, section = "ws_origin")]
#[non_exhaustive]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OriginValidationSettings {
	/// The origin policy to apply.
	#[serde(default)]
	pub policy: OriginPolicySettings,
	/// Whether to reject connections with a missing Origin header.
	#[serde(default = "default_reject_missing_origin")]
	pub reject_missing_origin: bool,
}

impl Default for OriginValidationSettings {
	fn default() -> Self {
		Self {
			policy: OriginPolicySettings::default(),
			reject_missing_origin: default_reject_missing_origin(),
		}
	}
}

impl From<&OriginValidationSettings> for OriginValidationConfig {
	fn from(settings: &OriginValidationSettings) -> Self {
		OriginValidationConfig {
			policy: OriginPolicy::from(&settings.policy),
			reject_missing_origin: settings.reject_missing_origin,
		}
	}
}

/// Build an [`OriginValidationConfig`] from an [`OriginValidationSettings`] fragment.
pub fn create_origin_validation_config_from_settings(
	settings: &OriginValidationSettings,
) -> OriginValidationConfig {
	OriginValidationConfig::from(settings)
}

// --- rate limiting --------------------------------------------------------

/// WebSocket rate limit settings fragment.
///
/// Maps to the `[ws_rate_limit]` section.
#[settings(fragment = true, section = "ws_rate_limit")]
#[non_exhaustive]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RateLimitSettings {
	/// Maximum new connections per IP within the connection window.
	#[serde(default = "default_rate_max_connections_per_window")]
	pub max_connections_per_window: usize,
	/// Time window for connection rate limiting, in seconds.
	#[serde(default = "default_rate_connection_window_secs")]
	pub connection_window_secs: u64,
	/// Maximum concurrent connections per IP.
	#[serde(default = "default_rate_max_concurrent_connections_per_ip")]
	pub max_concurrent_connections_per_ip: usize,
	/// Maximum messages per connection within the message window.
	#[serde(default = "default_rate_max_messages_per_window")]
	pub max_messages_per_window: usize,
	/// Time window for message rate limiting, in seconds.
	#[serde(default = "default_rate_message_window_secs")]
	pub message_window_secs: u64,
}

impl Default for RateLimitSettings {
	fn default() -> Self {
		Self {
			max_connections_per_window: default_rate_max_connections_per_window(),
			connection_window_secs: default_rate_connection_window_secs(),
			max_concurrent_connections_per_ip: default_rate_max_concurrent_connections_per_ip(),
			max_messages_per_window: default_rate_max_messages_per_window(),
			message_window_secs: default_rate_message_window_secs(),
		}
	}
}

impl From<&RateLimitSettings> for WebSocketRateLimitConfig {
	fn from(settings: &RateLimitSettings) -> Self {
		// `WebSocketRateLimitConfig` has private fields, so rebuild through its builder.
		WebSocketRateLimitConfig::default()
			.with_max_connections_per_window(settings.max_connections_per_window)
			.with_connection_window(Duration::from_secs(settings.connection_window_secs))
			.with_max_concurrent_connections_per_ip(settings.max_concurrent_connections_per_ip)
			.with_max_messages_per_window(settings.max_messages_per_window)
			.with_message_window(Duration::from_secs(settings.message_window_secs))
	}
}

/// Build a [`WebSocketRateLimitConfig`] from a [`RateLimitSettings`] fragment.
pub fn create_rate_limit_config_from_settings(
	settings: &RateLimitSettings,
) -> WebSocketRateLimitConfig {
	WebSocketRateLimitConfig::from(settings)
}

// --- redis channel layer --------------------------------------------------

/// Redis channel layer settings fragment.
///
/// Maps to the `[ws_redis]` section. Requires the `redis-channel` feature.
#[cfg(feature = "redis-channel")]
#[settings(fragment = true, section = "ws_redis")]
#[non_exhaustive]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RedisChannelSettings {
	/// Redis connection URL.
	#[serde(default = "default_redis_url")]
	pub url: String,
	/// Channel key prefix.
	#[serde(default = "default_redis_channel_prefix")]
	pub channel_prefix: String,
	/// Group key prefix.
	#[serde(default = "default_redis_group_prefix")]
	pub group_prefix: String,
	/// Message expiry time, in seconds.
	#[serde(default = "default_redis_message_expiry")]
	pub message_expiry: u64,
	/// Redis password for authentication.
	#[setting(secret)]
	#[serde(default)]
	pub password: Option<String>,
	/// Redis username for authentication (Redis 6+ ACL).
	#[serde(default)]
	pub username: Option<String>,
	/// Enable TLS for secure connection.
	#[serde(default)]
	pub tls: bool,
	/// Require authentication (warns if disabled without credentials).
	#[serde(default = "default_redis_require_auth")]
	pub require_auth: bool,
}

#[cfg(feature = "redis-channel")]
impl Default for RedisChannelSettings {
	fn default() -> Self {
		Self {
			url: default_redis_url(),
			channel_prefix: default_redis_channel_prefix(),
			group_prefix: default_redis_group_prefix(),
			message_expiry: default_redis_message_expiry(),
			password: None,
			username: None,
			tls: false,
			require_auth: default_redis_require_auth(),
		}
	}
}

#[cfg(feature = "redis-channel")]
impl From<&RedisChannelSettings> for RedisConfig {
	fn from(settings: &RedisChannelSettings) -> Self {
		// `RedisConfig` fields are public, so build it directly.
		RedisConfig {
			url: settings.url.clone(),
			channel_prefix: settings.channel_prefix.clone(),
			group_prefix: settings.group_prefix.clone(),
			message_expiry: settings.message_expiry,
			password: settings.password.clone(),
			username: settings.username.clone(),
			tls: settings.tls,
			require_auth: settings.require_auth,
		}
	}
}

/// Build a [`RedisConfig`] from a [`RedisChannelSettings`] fragment.
#[cfg(feature = "redis-channel")]
pub fn create_redis_config_from_settings(settings: &RedisChannelSettings) -> RedisConfig {
	RedisConfig::from(settings)
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn connection_settings_default_converts() {
		let settings = ConnectionSettings::default();
		let config = ConnectionConfig::from(&settings);
		assert_eq!(config.idle_timeout(), Duration::from_secs(300));
		assert_eq!(config.handshake_timeout(), Duration::from_secs(10));
		assert_eq!(config.cleanup_interval(), Duration::from_secs(30));
		assert_eq!(config.max_connections(), None);
	}

	#[test]
	fn reconnection_settings_default_converts() {
		let settings = ReconnectionSettings::default();
		let config = ReconnectionConfig::from(&settings);
		assert_eq!(config.max_attempts, Some(10));
		assert_eq!(config.initial_delay, Duration::from_secs(1));
		assert_eq!(config.max_delay, Duration::from_secs(300));
		assert_eq!(config.backoff_multiplier, 2.0);
		assert_eq!(config.jitter_factor, 0.1);
	}

	#[test]
	fn origin_settings_default_rejects_all() {
		let settings = OriginValidationSettings::default();
		let config = OriginValidationConfig::from(&settings);
		assert!(config.reject_missing_origin);
		assert!(matches!(config.policy, OriginPolicy::AllowList(ref v) if v.is_empty()));
	}

	#[test]
	fn origin_settings_allow_list_roundtrips() {
		let settings = OriginValidationSettings {
			policy: OriginPolicySettings::AllowList(vec!["https://example.com".to_string()]),
			reject_missing_origin: false,
		};
		let config = OriginValidationConfig::from(&settings);
		assert!(!config.reject_missing_origin);
		match config.policy {
			OriginPolicy::AllowList(origins) => {
				assert_eq!(origins, vec!["https://example.com".to_string()]);
			}
			OriginPolicy::AllowAll => panic!("expected AllowList"),
		}
	}

	#[test]
	fn rate_limit_settings_default_converts() {
		let settings = RateLimitSettings::default();
		let config = WebSocketRateLimitConfig::from(&settings);
		assert_eq!(config.max_connections_per_window(), 20);
		assert_eq!(config.connection_window(), Duration::from_secs(60));
		assert_eq!(config.max_concurrent_connections_per_ip(), 10);
		assert_eq!(config.max_messages_per_window(), 100);
		assert_eq!(config.message_window(), Duration::from_secs(60));
	}

	#[test]
	fn origin_policy_settings_serde_roundtrip() {
		let settings = OriginValidationSettings::default();
		let json = serde_json::to_string(&settings).expect("serialize");
		let parsed: OriginValidationSettings = serde_json::from_str(&json).expect("deserialize");
		assert_eq!(parsed.reject_missing_origin, settings.reject_missing_origin);
	}

	#[cfg(feature = "redis-channel")]
	#[test]
	fn redis_settings_default_converts() {
		let settings = RedisChannelSettings::default();
		let config = RedisConfig::from(&settings);
		assert_eq!(config.url, "redis://127.0.0.1:6379");
		assert_eq!(config.channel_prefix, "ws:channel:");
		assert_eq!(config.group_prefix, "ws:group:");
		assert_eq!(config.message_expiry, 60);
		assert!(config.password.is_none());
		assert!(config.username.is_none());
		assert!(!config.tls);
		assert!(config.require_auth);
	}
}
