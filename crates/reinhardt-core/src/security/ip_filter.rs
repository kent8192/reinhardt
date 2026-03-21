//! IP Filtering
//!
//! Provides IP address whitelist/blacklist functionality.

use ipnet::IpNet;
use std::net::IpAddr;

/// IP Filtering operation mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum IpFilterMode {
	/// Whitelist mode: Only allow IPs in the list
	Whitelist,
	/// Blacklist mode: Deny IPs in the list
	#[default]
	Blacklist,
}

/// IP Filtering configuration
///
/// # Examples
///
/// ```
/// use reinhardt_core::security::ip_filter::{IpFilterConfig, IpFilterMode};
/// use std::net::IpAddr;
///
/// let mut config = IpFilterConfig::new(IpFilterMode::Whitelist);
/// config.add_allowed_ip("192.168.1.0/24").unwrap();
/// config.add_allowed_ip("10.0.0.1").unwrap();
///
/// let ip: IpAddr = "192.168.1.100".parse().unwrap();
/// assert!(config.is_allowed(&ip));
///
/// let ip2: IpAddr = "192.168.2.1".parse().unwrap();
/// assert!(!config.is_allowed(&ip2));
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IpFilterConfig {
	/// Filtering mode
	pub mode: IpFilterMode,
	/// List of allowed IP address ranges
	pub allowed_ranges: Vec<IpNet>,
	/// List of blocked IP address ranges
	pub blocked_ranges: Vec<IpNet>,
}

impl IpFilterConfig {
	/// Create a new IP Filter configuration
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::security::ip_filter::{IpFilterConfig, IpFilterMode};
	///
	/// let config = IpFilterConfig::new(IpFilterMode::Whitelist);
	/// assert_eq!(config.mode, IpFilterMode::Whitelist);
	/// ```
	pub fn new(mode: IpFilterMode) -> Self {
		Self {
			mode,
			allowed_ranges: Vec::new(),
			blocked_ranges: Vec::new(),
		}
	}

	/// Add an allowed IP address or range
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::security::ip_filter::{IpFilterConfig, IpFilterMode};
	///
	/// let mut config = IpFilterConfig::new(IpFilterMode::Whitelist);
	/// config.add_allowed_ip("192.168.1.0/24").unwrap();
	/// config.add_allowed_ip("10.0.0.1/32").unwrap();
	/// ```
	pub fn add_allowed_ip(&mut self, ip_or_range: &str) -> Result<(), String> {
		// Automatically convert single IP address to CIDR notation
		let normalized = if !ip_or_range.contains('/') {
			if ip_or_range.contains(':') {
				// IPv6
				format!("{}/128", ip_or_range)
			} else {
				// IPv4
				format!("{}/32", ip_or_range)
			}
		} else {
			ip_or_range.to_string()
		};

		let net = normalized
			.parse::<IpNet>()
			.map_err(|e| format!("Invalid IP or range: {}", e))?;
		self.allowed_ranges.push(net);
		Ok(())
	}

	/// Add a blocked IP address or range
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::security::ip_filter::{IpFilterConfig, IpFilterMode};
	///
	/// let mut config = IpFilterConfig::new(IpFilterMode::Blacklist);
	/// config.add_blocked_ip("192.168.1.100").unwrap();
	/// config.add_blocked_ip("10.0.0.0/8").unwrap();
	/// ```
	pub fn add_blocked_ip(&mut self, ip_or_range: &str) -> Result<(), String> {
		// Automatically convert single IP address to CIDR notation
		let normalized = if !ip_or_range.contains('/') {
			if ip_or_range.contains(':') {
				// IPv6
				format!("{}/128", ip_or_range)
			} else {
				// IPv4
				format!("{}/32", ip_or_range)
			}
		} else {
			ip_or_range.to_string()
		};

		let net = normalized
			.parse::<IpNet>()
			.map_err(|e| format!("Invalid IP or range: {}", e))?;
		self.blocked_ranges.push(net);
		Ok(())
	}

	/// Check if the IP address is allowed
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::security::ip_filter::{IpFilterConfig, IpFilterMode};
	/// use std::net::IpAddr;
	///
	/// let mut config = IpFilterConfig::new(IpFilterMode::Whitelist);
	/// config.add_allowed_ip("192.168.1.0/24").unwrap();
	///
	/// let ip: IpAddr = "192.168.1.100".parse().unwrap();
	/// assert!(config.is_allowed(&ip));
	///
	/// let ip2: IpAddr = "10.0.0.1".parse().unwrap();
	/// assert!(!config.is_allowed(&ip2));
	/// ```
	pub fn is_allowed(&self, ip: &IpAddr) -> bool {
		// First check the blacklist
		if self.blocked_ranges.iter().any(|range| range.contains(ip)) {
			return false;
		}

		match self.mode {
			IpFilterMode::Whitelist => {
				// Whitelist mode: Only allow if included in the list
				self.allowed_ranges.iter().any(|range| range.contains(ip))
			}
			IpFilterMode::Blacklist => {
				// Blacklist mode: Allow if not in the blacklist
				// (OK if not already rejected by the blacklist check)
				true
			}
		}
	}

	/// Create with whitelist mode
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::security::ip_filter::{IpFilterConfig, IpFilterMode};
	///
	/// let config = IpFilterConfig::whitelist();
	/// assert_eq!(config.mode, IpFilterMode::Whitelist);
	/// ```
	pub fn whitelist() -> Self {
		Self::new(IpFilterMode::Whitelist)
	}

	/// Create with blacklist mode
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::security::ip_filter::{IpFilterConfig, IpFilterMode};
	///
	/// let config = IpFilterConfig::blacklist();
	/// assert_eq!(config.mode, IpFilterMode::Blacklist);
	/// ```
	pub fn blacklist() -> Self {
		Self::new(IpFilterMode::Blacklist)
	}
}

impl Default for IpFilterConfig {
	fn default() -> Self {
		Self::new(IpFilterMode::Blacklist)
	}
}

/// IP Filtering Middleware
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IpFilterMiddleware {
	config: IpFilterConfig,
}

impl IpFilterMiddleware {
	/// Create a new IP Filter Middleware
	pub fn new(config: IpFilterConfig) -> Self {
		Self { config }
	}

	/// Get the configuration
	pub fn config(&self) -> &IpFilterConfig {
		&self.config
	}

	/// Check if the IP address is allowed
	pub fn is_allowed(&self, ip: &IpAddr) -> bool {
		self.config.is_allowed(ip)
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_whitelist_mode() {
		let mut config = IpFilterConfig::whitelist();
		config.add_allowed_ip("192.168.1.0/24").unwrap();
		config.add_allowed_ip("10.0.0.1").unwrap();

		let allowed_ip: IpAddr = "192.168.1.100".parse().unwrap();
		assert!(config.is_allowed(&allowed_ip));

		let allowed_ip2: IpAddr = "10.0.0.1".parse().unwrap();
		assert!(config.is_allowed(&allowed_ip2));

		let blocked_ip: IpAddr = "192.168.2.1".parse().unwrap();
		assert!(!config.is_allowed(&blocked_ip));
	}

	#[test]
	fn test_blacklist_mode() {
		let mut config = IpFilterConfig::blacklist();
		config.add_blocked_ip("192.168.1.100").unwrap();
		config.add_blocked_ip("10.0.0.0/8").unwrap();

		let blocked_ip1: IpAddr = "192.168.1.100".parse().unwrap();
		assert!(!config.is_allowed(&blocked_ip1));

		let blocked_ip2: IpAddr = "10.0.0.50".parse().unwrap();
		assert!(!config.is_allowed(&blocked_ip2));

		let allowed_ip: IpAddr = "192.168.1.1".parse().unwrap();
		assert!(config.is_allowed(&allowed_ip));
	}

	#[test]
	fn test_blacklist_overrides_whitelist() {
		let mut config = IpFilterConfig::whitelist();
		config.add_allowed_ip("192.168.1.0/24").unwrap();
		config.add_blocked_ip("192.168.1.100").unwrap();

		let allowed_ip: IpAddr = "192.168.1.50".parse().unwrap();
		assert!(config.is_allowed(&allowed_ip));

		let blocked_ip: IpAddr = "192.168.1.100".parse().unwrap();
		assert!(!config.is_allowed(&blocked_ip));
	}

	#[test]
	fn test_ipv6_filtering() {
		let mut config = IpFilterConfig::whitelist();
		config.add_allowed_ip("2001:db8::/32").unwrap();

		let allowed_ip: IpAddr = "2001:db8::1".parse().unwrap();
		assert!(config.is_allowed(&allowed_ip));

		let blocked_ip: IpAddr = "2001:db9::1".parse().unwrap();
		assert!(!config.is_allowed(&blocked_ip));
	}

	#[test]
	fn test_middleware_creation() {
		let config = IpFilterConfig::whitelist();
		let middleware = IpFilterMiddleware::new(config);
		assert_eq!(middleware.config().mode, IpFilterMode::Whitelist);
	}

	#[test]
	fn test_middleware_is_allowed() {
		let mut config = IpFilterConfig::blacklist();
		config.add_blocked_ip("192.168.1.100").unwrap();
		let middleware = IpFilterMiddleware::new(config);

		let allowed_ip: IpAddr = "192.168.1.1".parse().unwrap();
		assert!(middleware.is_allowed(&allowed_ip));

		let blocked_ip: IpAddr = "192.168.1.100".parse().unwrap();
		assert!(!middleware.is_allowed(&blocked_ip));
	}

	#[test]
	fn test_invalid_ip_format() {
		let mut config = IpFilterConfig::whitelist();
		let result = config.add_allowed_ip("invalid-ip");
		assert!(result.is_err());
	}
}
