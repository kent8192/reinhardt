//! IP-based access control permissions
//!
//! Provides whitelist and blacklist permissions based on client IP addresses.

use crate::{Permission, PermissionContext};
use async_trait::async_trait;
use std::net::IpAddr;
use std::str::FromStr;

/// IP whitelist permission
///
/// Allows access only from specified IP addresses or CIDR ranges.
///
/// # Examples
///
/// ```
/// use reinhardt_auth::IpWhitelistPermission;
///
/// let permission = IpWhitelistPermission::new()
///     .add_ip("192.168.1.1")
///     .add_cidr("10.0.0.0/24");
/// ```
#[derive(Debug, Clone)]
pub struct IpWhitelistPermission {
	/// Allowed IP addresses
	pub allowed_ips: Vec<IpAddr>,
	/// Allowed CIDR ranges
	pub allowed_cidrs: Vec<CidrRange>,
	/// Whether to deny on parse error
	pub deny_on_error: bool,
	/// Trusted proxy IP addresses that are allowed to set forwarding headers
	pub trusted_proxies: Vec<IpAddr>,
}

impl IpWhitelistPermission {
	/// Create a new IP whitelist permission
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_auth::IpWhitelistPermission;
	///
	/// let permission = IpWhitelistPermission::new();
	/// ```
	pub fn new() -> Self {
		Self {
			allowed_ips: Vec::new(),
			allowed_cidrs: Vec::new(),
			deny_on_error: true,
			trusted_proxies: Vec::new(),
		}
	}

	/// Add an allowed IP address
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_auth::IpWhitelistPermission;
	///
	/// let permission = IpWhitelistPermission::new()
	///     .add_ip("192.168.1.1");
	/// ```
	pub fn add_ip(mut self, ip: impl AsRef<str>) -> Self {
		if let Ok(addr) = IpAddr::from_str(ip.as_ref()) {
			self.allowed_ips.push(addr);
		}
		self
	}

	/// Add an allowed CIDR range
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_auth::IpWhitelistPermission;
	///
	/// let permission = IpWhitelistPermission::new()
	///     .add_cidr("10.0.0.0/24");
	/// ```
	pub fn add_cidr(mut self, cidr: impl AsRef<str>) -> Self {
		if let Ok(range) = CidrRange::from_str(cidr.as_ref()) {
			self.allowed_cidrs.push(range);
		}
		self
	}

	/// Set whether to deny on parse error
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_auth::IpWhitelistPermission;
	///
	/// let permission = IpWhitelistPermission::new()
	///     .deny_on_error(false);
	/// ```
	pub fn deny_on_error(mut self, deny: bool) -> Self {
		self.deny_on_error = deny;
		self
	}

	/// Add a trusted proxy IP address
	///
	/// Only requests from trusted proxies will have their
	/// `X-Forwarded-For` and `X-Real-IP` headers honored.
	/// If no trusted proxies are configured, forwarding headers
	/// are ignored and the direct connection IP is used.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_auth::IpWhitelistPermission;
	///
	/// let permission = IpWhitelistPermission::new()
	///     .add_trusted_proxy("10.0.0.1")
	///     .add_ip("192.168.1.1");
	/// ```
	pub fn add_trusted_proxy(mut self, ip: impl AsRef<str>) -> Self {
		if let Ok(addr) = IpAddr::from_str(ip.as_ref()) {
			self.trusted_proxies.push(addr);
		}
		self
	}

	/// Check if an IP address is allowed
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_auth::IpWhitelistPermission;
	/// use std::net::IpAddr;
	/// use std::str::FromStr;
	///
	/// let permission = IpWhitelistPermission::new()
	///     .add_ip("192.168.1.1");
	///
	/// let ip = IpAddr::from_str("192.168.1.1").unwrap();
	/// assert!(permission.is_allowed(&ip));
	/// ```
	pub fn is_allowed(&self, ip: &IpAddr) -> bool {
		self.allowed_ips.contains(ip) || self.allowed_cidrs.iter().any(|cidr| cidr.contains(ip))
	}

	/// Check if the direct connection IP is a trusted proxy
	fn is_trusted_proxy(&self, context: &PermissionContext) -> bool {
		if self.trusted_proxies.is_empty() {
			return false;
		}
		if let Some(remote_addr) = context.request.remote_addr {
			self.trusted_proxies.contains(&remote_addr.ip())
		} else {
			false
		}
	}

	fn extract_client_ip(&self, context: &PermissionContext) -> Option<IpAddr> {
		// Only trust forwarding headers if the direct connection is from a trusted proxy
		if self.is_trusted_proxy(context) {
			// Try X-Forwarded-For header first
			if let Some(forwarded) = context.request.headers.get("x-forwarded-for")
				&& let Ok(forwarded_str) = forwarded.to_str()
				&& let Some(first_ip) = forwarded_str.split(',').next()
				&& let Ok(ip) = IpAddr::from_str(first_ip.trim())
			{
				return Some(ip);
			}

			// Try X-Real-IP header
			if let Some(real_ip) = context.request.headers.get("x-real-ip")
				&& let Ok(real_ip_str) = real_ip.to_str()
				&& let Ok(ip) = IpAddr::from_str(real_ip_str.trim())
			{
				return Some(ip);
			}
		}

		// Fall back to direct connection IP
		if let Some(remote_addr) = context.request.remote_addr {
			return Some(remote_addr.ip());
		}

		None
	}
}

impl Default for IpWhitelistPermission {
	fn default() -> Self {
		Self::new()
	}
}

#[async_trait]
impl Permission for IpWhitelistPermission {
	async fn has_permission(&self, context: &PermissionContext<'_>) -> bool {
		match self.extract_client_ip(context) {
			Some(ip) => self.is_allowed(&ip),
			None => !self.deny_on_error,
		}
	}
}

/// IP blacklist permission
///
/// Denies access from specified IP addresses or CIDR ranges.
///
/// # Examples
///
/// ```
/// use reinhardt_auth::IpBlacklistPermission;
///
/// let permission = IpBlacklistPermission::new()
///     .add_ip("192.168.1.100")
///     .add_cidr("10.0.0.0/8");
/// ```
#[derive(Debug, Clone)]
pub struct IpBlacklistPermission {
	/// Blocked IP addresses
	pub blocked_ips: Vec<IpAddr>,
	/// Blocked CIDR ranges
	pub blocked_cidrs: Vec<CidrRange>,
	/// Whether to allow on parse error
	pub allow_on_error: bool,
	/// Trusted proxy IP addresses that are allowed to set forwarding headers
	pub trusted_proxies: Vec<IpAddr>,
}

impl IpBlacklistPermission {
	/// Create a new IP blacklist permission
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_auth::IpBlacklistPermission;
	///
	/// let permission = IpBlacklistPermission::new();
	/// ```
	pub fn new() -> Self {
		Self {
			blocked_ips: Vec::new(),
			blocked_cidrs: Vec::new(),
			allow_on_error: false,
			trusted_proxies: Vec::new(),
		}
	}

	/// Add a blocked IP address
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_auth::IpBlacklistPermission;
	///
	/// let permission = IpBlacklistPermission::new()
	///     .add_ip("192.168.1.100");
	/// ```
	pub fn add_ip(mut self, ip: impl AsRef<str>) -> Self {
		if let Ok(addr) = IpAddr::from_str(ip.as_ref()) {
			self.blocked_ips.push(addr);
		}
		self
	}

	/// Add a blocked CIDR range
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_auth::IpBlacklistPermission;
	///
	/// let permission = IpBlacklistPermission::new()
	///     .add_cidr("10.0.0.0/8");
	/// ```
	pub fn add_cidr(mut self, cidr: impl AsRef<str>) -> Self {
		if let Ok(range) = CidrRange::from_str(cidr.as_ref()) {
			self.blocked_cidrs.push(range);
		}
		self
	}

	/// Set whether to allow on parse error
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_auth::IpBlacklistPermission;
	///
	/// let permission = IpBlacklistPermission::new()
	///     .allow_on_error(true);
	/// ```
	pub fn allow_on_error(mut self, allow: bool) -> Self {
		self.allow_on_error = allow;
		self
	}

	/// Add a trusted proxy IP address
	///
	/// Only requests from trusted proxies will have their
	/// `X-Forwarded-For` and `X-Real-IP` headers honored.
	/// If no trusted proxies are configured, forwarding headers
	/// are ignored and the direct connection IP is used.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_auth::IpBlacklistPermission;
	///
	/// let permission = IpBlacklistPermission::new()
	///     .add_trusted_proxy("10.0.0.1")
	///     .add_ip("192.168.1.100");
	/// ```
	pub fn add_trusted_proxy(mut self, ip: impl AsRef<str>) -> Self {
		if let Ok(addr) = IpAddr::from_str(ip.as_ref()) {
			self.trusted_proxies.push(addr);
		}
		self
	}

	/// Check if an IP address is blocked
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_auth::IpBlacklistPermission;
	/// use std::net::IpAddr;
	/// use std::str::FromStr;
	///
	/// let permission = IpBlacklistPermission::new()
	///     .add_ip("192.168.1.100");
	///
	/// let ip = IpAddr::from_str("192.168.1.100").unwrap();
	/// assert!(permission.is_blocked(&ip));
	/// ```
	pub fn is_blocked(&self, ip: &IpAddr) -> bool {
		self.blocked_ips.contains(ip) || self.blocked_cidrs.iter().any(|cidr| cidr.contains(ip))
	}

	/// Check if the direct connection IP is a trusted proxy
	fn is_trusted_proxy(&self, context: &PermissionContext) -> bool {
		if self.trusted_proxies.is_empty() {
			return false;
		}
		if let Some(remote_addr) = context.request.remote_addr {
			self.trusted_proxies.contains(&remote_addr.ip())
		} else {
			false
		}
	}

	fn extract_client_ip(&self, context: &PermissionContext) -> Option<IpAddr> {
		// Only trust forwarding headers if the direct connection is from a trusted proxy
		if self.is_trusted_proxy(context) {
			// Try X-Forwarded-For header first
			if let Some(forwarded) = context.request.headers.get("x-forwarded-for")
				&& let Ok(forwarded_str) = forwarded.to_str()
				&& let Some(first_ip) = forwarded_str.split(',').next()
				&& let Ok(ip) = IpAddr::from_str(first_ip.trim())
			{
				return Some(ip);
			}

			// Try X-Real-IP header
			if let Some(real_ip) = context.request.headers.get("x-real-ip")
				&& let Ok(real_ip_str) = real_ip.to_str()
				&& let Ok(ip) = IpAddr::from_str(real_ip_str.trim())
			{
				return Some(ip);
			}
		}

		// Fall back to direct connection IP
		if let Some(remote_addr) = context.request.remote_addr {
			return Some(remote_addr.ip());
		}

		None
	}
}

impl Default for IpBlacklistPermission {
	fn default() -> Self {
		Self::new()
	}
}

#[async_trait]
impl Permission for IpBlacklistPermission {
	async fn has_permission(&self, context: &PermissionContext<'_>) -> bool {
		match self.extract_client_ip(context) {
			Some(ip) => !self.is_blocked(&ip),
			None => self.allow_on_error,
		}
	}
}

/// CIDR range representation
///
/// # Examples
///
/// ```
/// use reinhardt_auth::CidrRange;
/// use std::str::FromStr;
/// use std::net::IpAddr;
///
/// let cidr = CidrRange::from_str("192.168.1.0/24").unwrap();
/// let ip = IpAddr::from_str("192.168.1.100").unwrap();
/// assert!(cidr.contains(&ip));
/// ```
#[derive(Debug, Clone)]
pub struct CidrRange {
	/// Network address
	pub network: IpAddr,
	/// Prefix length (number of bits in the network portion)
	pub prefix_len: u8,
}

impl CidrRange {
	/// Create a new CIDR range
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_auth::CidrRange;
	/// use std::net::IpAddr;
	/// use std::str::FromStr;
	///
	/// let network = IpAddr::from_str("192.168.1.0").unwrap();
	/// let cidr = CidrRange::new(network, 24);
	/// ```
	pub fn new(network: IpAddr, prefix_len: u8) -> Self {
		Self {
			network,
			prefix_len,
		}
	}

	/// Check if an IP address is within this CIDR range
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_auth::CidrRange;
	/// use std::net::IpAddr;
	/// use std::str::FromStr;
	///
	/// let network = IpAddr::from_str("192.168.1.0").unwrap();
	/// let cidr = CidrRange::new(network, 24);
	/// let ip = IpAddr::from_str("192.168.1.100").unwrap();
	/// assert!(cidr.contains(&ip));
	/// ```
	pub fn contains(&self, ip: &IpAddr) -> bool {
		match (self.network, ip) {
			(IpAddr::V4(net), IpAddr::V4(addr)) => {
				let net_u32 = u32::from_be_bytes(net.octets());
				let addr_u32 = u32::from_be_bytes(addr.octets());
				let mask = if self.prefix_len == 0 {
					0
				} else {
					!0u32 << (32 - self.prefix_len)
				};
				(net_u32 & mask) == (addr_u32 & mask)
			}
			(IpAddr::V6(net), IpAddr::V6(addr)) => {
				let net_u128 = u128::from_be_bytes(net.octets());
				let addr_u128 = u128::from_be_bytes(addr.octets());
				let mask = if self.prefix_len == 0 {
					0
				} else {
					!0u128 << (128 - self.prefix_len)
				};
				(net_u128 & mask) == (addr_u128 & mask)
			}
			_ => false,
		}
	}
}

impl FromStr for CidrRange {
	type Err = String;

	fn from_str(s: &str) -> Result<Self, Self::Err> {
		let parts: Vec<&str> = s.split('/').collect();
		if parts.len() != 2 {
			return Err("Invalid CIDR format".to_string());
		}

		let network = IpAddr::from_str(parts[0]).map_err(|e| e.to_string())?;
		let prefix_len = parts[1].parse::<u8>().map_err(|e| e.to_string())?;

		match network {
			IpAddr::V4(_) if prefix_len > 32 => Err("IPv4 prefix length must be <= 32".to_string()),
			IpAddr::V6(_) if prefix_len > 128 => {
				Err("IPv6 prefix length must be <= 128".to_string())
			}
			_ => Ok(CidrRange::new(network, prefix_len)),
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use bytes::Bytes;
	use hyper::{HeaderMap, Method};
	use reinhardt_http::Request;
	use rstest::rstest;
	use std::net::SocketAddr;

	#[rstest]
	fn test_cidr_range_from_str() {
		// Arrange & Act
		let cidr = CidrRange::from_str("192.168.1.0/24").unwrap();
		let cidr6 = CidrRange::from_str("2001:db8::/32").unwrap();

		// Assert
		assert_eq!(cidr.prefix_len, 24);
		assert_eq!(cidr6.prefix_len, 32);
	}

	#[rstest]
	fn test_cidr_range_invalid() {
		// Arrange & Act & Assert
		assert!(CidrRange::from_str("192.168.1.0").is_err());
		assert!(CidrRange::from_str("192.168.1.0/33").is_err());
		assert!(CidrRange::from_str("invalid/24").is_err());
	}

	#[rstest]
	fn test_cidr_contains_ipv4() {
		// Arrange
		let cidr = CidrRange::from_str("192.168.1.0/24").unwrap();
		let ip1 = IpAddr::from_str("192.168.1.1").unwrap();
		let ip2 = IpAddr::from_str("192.168.1.255").unwrap();
		let ip3 = IpAddr::from_str("192.168.2.1").unwrap();

		// Act & Assert
		assert!(cidr.contains(&ip1));
		assert!(cidr.contains(&ip2));
		assert!(!cidr.contains(&ip3));
	}

	#[rstest]
	fn test_cidr_contains_ipv6() {
		// Arrange
		let cidr = CidrRange::from_str("2001:db8::/32").unwrap();
		let ip1 = IpAddr::from_str("2001:db8::1").unwrap();
		let ip2 = IpAddr::from_str("2001:db8:ffff::1").unwrap();
		let ip3 = IpAddr::from_str("2001:db9::1").unwrap();

		// Act & Assert
		assert!(cidr.contains(&ip1));
		assert!(cidr.contains(&ip2));
		assert!(!cidr.contains(&ip3));
	}

	#[rstest]
	fn test_whitelist_permission_creation() {
		// Arrange & Act
		let permission = IpWhitelistPermission::new();

		// Assert
		assert_eq!(permission.allowed_ips.len(), 0);
		assert_eq!(permission.allowed_cidrs.len(), 0);
		assert!(permission.deny_on_error);
		assert_eq!(permission.trusted_proxies.len(), 0);
	}

	#[rstest]
	fn test_whitelist_add_ip() {
		// Arrange & Act
		let permission = IpWhitelistPermission::new()
			.add_ip("192.168.1.1")
			.add_ip("10.0.0.1");

		// Assert
		assert_eq!(permission.allowed_ips.len(), 2);
	}

	#[rstest]
	fn test_whitelist_add_cidr() {
		// Arrange & Act
		let permission = IpWhitelistPermission::new()
			.add_cidr("192.168.1.0/24")
			.add_cidr("10.0.0.0/8");

		// Assert
		assert_eq!(permission.allowed_cidrs.len(), 2);
	}

	#[rstest]
	fn test_whitelist_is_allowed() {
		// Arrange
		let permission = IpWhitelistPermission::new()
			.add_ip("192.168.1.1")
			.add_cidr("10.0.0.0/24");

		let ip1 = IpAddr::from_str("192.168.1.1").unwrap();
		let ip2 = IpAddr::from_str("10.0.0.100").unwrap();
		let ip3 = IpAddr::from_str("172.16.0.1").unwrap();

		// Act & Assert
		assert!(permission.is_allowed(&ip1));
		assert!(permission.is_allowed(&ip2));
		assert!(!permission.is_allowed(&ip3));
	}

	#[rstest]
	#[tokio::test]
	async fn test_whitelist_ignores_forwarded_header_without_trusted_proxy() {
		// Arrange - no trusted proxies configured; x-forwarded-for should be ignored
		let permission = IpWhitelistPermission::new().add_ip("192.168.1.1");

		let mut headers = HeaderMap::new();
		headers.insert("x-forwarded-for", "192.168.1.1".parse().unwrap());

		// Remote addr is an untrusted source claiming to be an allowed IP via header
		let remote_addr: SocketAddr = "10.0.0.99:12345".parse().unwrap();
		let request = Request::builder()
			.method(Method::GET)
			.uri("/test")
			.headers(headers)
			.remote_addr(remote_addr)
			.body(Bytes::new())
			.build()
			.unwrap();

		let context = PermissionContext {
			request: &request,
			is_authenticated: false,
			is_admin: false,
			is_active: false,
			user: None,
		};

		// Act & Assert - should use remote_addr (10.0.0.99), not the spoofed header
		assert!(!permission.has_permission(&context).await);
	}

	#[rstest]
	#[tokio::test]
	async fn test_whitelist_trusts_forwarded_header_from_trusted_proxy() {
		// Arrange - proxy 10.0.0.1 is trusted, client 192.168.1.1 is allowed
		let permission = IpWhitelistPermission::new()
			.add_trusted_proxy("10.0.0.1")
			.add_ip("192.168.1.1");

		let mut headers = HeaderMap::new();
		headers.insert("x-forwarded-for", "192.168.1.1".parse().unwrap());

		let remote_addr: SocketAddr = "10.0.0.1:12345".parse().unwrap();
		let request = Request::builder()
			.method(Method::GET)
			.uri("/test")
			.headers(headers)
			.remote_addr(remote_addr)
			.body(Bytes::new())
			.build()
			.unwrap();

		let context = PermissionContext {
			request: &request,
			is_authenticated: false,
			is_admin: false,
			is_active: false,
			user: None,
		};

		// Act & Assert - trusted proxy, so x-forwarded-for is honored
		assert!(permission.has_permission(&context).await);
	}

	#[rstest]
	#[tokio::test]
	async fn test_whitelist_uses_remote_addr_directly() {
		// Arrange - client connects directly with allowed IP
		let permission = IpWhitelistPermission::new().add_ip("192.168.1.1");

		let remote_addr: SocketAddr = "192.168.1.1:12345".parse().unwrap();
		let request = Request::builder()
			.method(Method::GET)
			.uri("/test")
			.remote_addr(remote_addr)
			.body(Bytes::new())
			.build()
			.unwrap();

		let context = PermissionContext {
			request: &request,
			is_authenticated: false,
			is_admin: false,
			is_active: false,
			user: None,
		};

		// Act & Assert
		assert!(permission.has_permission(&context).await);
	}

	#[rstest]
	fn test_blacklist_permission_creation() {
		// Arrange & Act
		let permission = IpBlacklistPermission::new();

		// Assert
		assert_eq!(permission.blocked_ips.len(), 0);
		assert_eq!(permission.blocked_cidrs.len(), 0);
		assert!(!permission.allow_on_error);
		assert_eq!(permission.trusted_proxies.len(), 0);
	}

	#[rstest]
	fn test_blacklist_add_ip() {
		// Arrange & Act
		let permission = IpBlacklistPermission::new()
			.add_ip("192.168.1.100")
			.add_ip("10.0.0.100");

		// Assert
		assert_eq!(permission.blocked_ips.len(), 2);
	}

	#[rstest]
	fn test_blacklist_is_blocked() {
		// Arrange
		let permission = IpBlacklistPermission::new()
			.add_ip("192.168.1.100")
			.add_cidr("10.0.0.0/24");

		let ip1 = IpAddr::from_str("192.168.1.100").unwrap();
		let ip2 = IpAddr::from_str("10.0.0.50").unwrap();
		let ip3 = IpAddr::from_str("172.16.0.1").unwrap();

		// Act & Assert
		assert!(permission.is_blocked(&ip1));
		assert!(permission.is_blocked(&ip2));
		assert!(!permission.is_blocked(&ip3));
	}

	#[rstest]
	#[tokio::test]
	async fn test_blacklist_ignores_forwarded_header_without_trusted_proxy() {
		// Arrange - attacker tries to bypass blacklist by spoofing x-forwarded-for
		let permission = IpBlacklistPermission::new().add_ip("192.168.1.100");

		let mut headers = HeaderMap::new();
		headers.insert("x-forwarded-for", "10.0.0.1".parse().unwrap());

		// Actual blocked IP connecting directly
		let remote_addr: SocketAddr = "192.168.1.100:12345".parse().unwrap();
		let request = Request::builder()
			.method(Method::GET)
			.uri("/test")
			.headers(headers)
			.remote_addr(remote_addr)
			.body(Bytes::new())
			.build()
			.unwrap();

		let context = PermissionContext {
			request: &request,
			is_authenticated: false,
			is_admin: false,
			is_active: false,
			user: None,
		};

		// Act & Assert - should be denied because remote_addr is blocked
		assert!(!permission.has_permission(&context).await);
	}

	#[rstest]
	#[tokio::test]
	async fn test_blacklist_trusts_forwarded_header_from_trusted_proxy() {
		// Arrange - proxy 10.0.0.1 is trusted, blocked client 192.168.1.100 behind proxy
		let permission = IpBlacklistPermission::new()
			.add_trusted_proxy("10.0.0.1")
			.add_ip("192.168.1.100");

		let mut headers = HeaderMap::new();
		headers.insert("x-forwarded-for", "192.168.1.100".parse().unwrap());

		let remote_addr: SocketAddr = "10.0.0.1:12345".parse().unwrap();
		let request = Request::builder()
			.method(Method::GET)
			.uri("/test")
			.headers(headers)
			.remote_addr(remote_addr)
			.body(Bytes::new())
			.build()
			.unwrap();

		let context = PermissionContext {
			request: &request,
			is_authenticated: false,
			is_admin: false,
			is_active: false,
			user: None,
		};

		// Act & Assert - trusted proxy, so x-forwarded-for is honored; client is blocked
		assert!(!permission.has_permission(&context).await);
	}

	#[rstest]
	#[tokio::test]
	async fn test_blacklist_allows_non_blocked_ip_via_trusted_proxy() {
		// Arrange - proxy trusted, client not blocked
		let permission = IpBlacklistPermission::new()
			.add_trusted_proxy("10.0.0.1")
			.add_ip("192.168.1.100");

		let mut headers = HeaderMap::new();
		headers.insert("x-forwarded-for", "192.168.1.1".parse().unwrap());

		let remote_addr: SocketAddr = "10.0.0.1:12345".parse().unwrap();
		let request = Request::builder()
			.method(Method::GET)
			.uri("/test")
			.headers(headers)
			.remote_addr(remote_addr)
			.body(Bytes::new())
			.build()
			.unwrap();

		let context = PermissionContext {
			request: &request,
			is_authenticated: false,
			is_admin: false,
			is_active: false,
			user: None,
		};

		// Act & Assert - non-blocked client should be allowed
		assert!(permission.has_permission(&context).await);
	}
}
