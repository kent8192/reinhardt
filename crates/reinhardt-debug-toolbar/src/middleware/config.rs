//! Toolbar configuration

use std::net::IpAddr;

/// Debug toolbar configuration
#[derive(Debug, Clone)]
pub struct ToolbarConfig {
	/// Whether the toolbar is enabled
	///
	/// Default: `cfg!(debug_assertions)` (enabled in debug builds only)
	pub enabled: bool,

	/// List of allowed IP addresses
	///
	/// Only requests from these IPs will see the toolbar.
	/// Default: `["127.0.0.1", "::1"]` (localhost only)
	pub internal_ips: Vec<IpAddr>,

	/// List of enabled panel IDs
	///
	/// Empty list means all panels are enabled.
	/// Default: `[]` (all panels)
	pub enabled_panels: Vec<String>,

	/// Slow SQL query threshold in milliseconds
	///
	/// Queries taking longer than this will be highlighted as slow.
	/// Default: `100ms`
	pub sql_warning_threshold_ms: u64,

	/// Maximum number of SQL queries to record
	///
	/// To prevent memory exhaustion from excessive queries.
	/// Default: `1000`
	pub max_sql_queries: usize,

	/// Maximum size of template context data in bytes
	///
	/// Context data larger than this will be truncated.
	/// Default: `10KB`
	pub max_template_context_size: usize,
}

impl Default for ToolbarConfig {
	fn default() -> Self {
		Self {
			enabled: cfg!(debug_assertions),
			internal_ips: vec![
				"127.0.0.1".parse().unwrap(), // IPv4 localhost
				"::1".parse().unwrap(),       // IPv6 localhost
			],
			enabled_panels: Vec::new(),
			sql_warning_threshold_ms: 100,
			max_sql_queries: 1000,
			max_template_context_size: 10 * 1024, // 10KB
		}
	}
}

impl ToolbarConfig {
	/// Check if toolbar should be shown for the given client IP
	pub fn should_show(&self, client_ip: &IpAddr) -> bool {
		self.enabled && self.internal_ips.contains(client_ip)
	}

	/// Check if a specific panel is enabled
	pub fn is_panel_enabled(&self, panel_id: &str) -> bool {
		self.enabled_panels.is_empty() || self.enabled_panels.contains(&panel_id.to_string())
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::*;

	#[rstest]
	fn test_default_config() {
		let config = ToolbarConfig::default();
		assert_eq!(config.enabled, cfg!(debug_assertions));
		assert_eq!(config.internal_ips.len(), 2);
		assert_eq!(config.sql_warning_threshold_ms, 100);
		assert_eq!(config.max_sql_queries, 1000);
	}

	#[rstest]
	#[case("127.0.0.1", true)]
	#[case("::1", true)]
	#[case("192.168.1.100", false)]
	fn test_should_show(#[case] ip_str: &str, #[case] expected: bool) {
		let config = ToolbarConfig::default();
		let client_ip: IpAddr = ip_str.parse().unwrap();
		assert_eq!(config.should_show(&client_ip), expected);
	}

	#[rstest]
	fn test_should_show_when_disabled() {
		let mut config = ToolbarConfig::default();
		config.enabled = false;

		let client_ip: IpAddr = "127.0.0.1".parse().unwrap();
		assert_eq!(config.should_show(&client_ip), false);
	}

	#[rstest]
	fn test_panel_enabled_all() {
		let config = ToolbarConfig::default();
		assert!(config.is_panel_enabled("sql"));
		assert!(config.is_panel_enabled("request"));
		assert!(config.is_panel_enabled("any-panel"));
	}

	#[rstest]
	fn test_panel_enabled_specific() {
		let mut config = ToolbarConfig::default();
		config.enabled_panels = vec!["sql".to_string(), "request".to_string()];

		assert!(config.is_panel_enabled("sql"));
		assert!(config.is_panel_enabled("request"));
		assert!(!config.is_panel_enabled("cache"));
	}
}
