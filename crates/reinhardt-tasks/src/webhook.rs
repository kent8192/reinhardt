//! Webhook notifications for task completion events
//!
//! This module provides webhook notification support for the Reinhardt tasks system.
//! Webhooks are HTTP callbacks that are triggered when tasks complete, fail, or are cancelled.
//!
//! # Features
//!
//! - HTTP webhook sender with configurable retry logic
//! - Exponential backoff with jitter for failed requests
//! - Configurable timeout and max retries
//! - Automatic serialization of task events to JSON
//!
//! # Example
//!
//! ```rust
//! use reinhardt_tasks::webhook::{
//!     WebhookConfig, RetryConfig, HttpWebhookSender, WebhookSender, WebhookEvent, TaskStatus
//! };
//! use std::time::Duration;
//! use std::collections::HashMap;
//! use chrono::Utc;
//! use reinhardt_tasks::TaskId;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // Configure webhook with retry logic
//! let retry_config = RetryConfig {
//!     max_retries: 3,
//!     initial_backoff: Duration::from_millis(100),
//!     max_backoff: Duration::from_secs(10),
//!     backoff_multiplier: 2.0,
//! };
//!
//! let config = WebhookConfig {
//!     url: "https://example.com/webhook".to_string(),
//!     method: "POST".to_string(),
//!     headers: HashMap::new(),
//!     timeout: Duration::from_secs(5),
//!     retry_config,
//! };
//!
//! let sender = HttpWebhookSender::new(config);
//!
//! // Create and send event
//! let now = Utc::now();
//! let event = WebhookEvent {
//!     task_id: TaskId::new(),
//!     task_name: "example_task".to_string(),
//!     status: TaskStatus::Success,
//!     result: Some("Task completed".to_string()),
//!     error: None,
//!     started_at: now - chrono::Duration::seconds(10),
//!     completed_at: now,
//!     duration_ms: 10000,
//! };
//!
//! sender.send(&event).await?;
//! # Ok(())
//! # }
//! ```

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use ipnet::IpNet;
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::IpAddr;
use std::time::Duration;
use thiserror::Error;
use url::Url;

use crate::TaskId;

/// Webhook-related errors
///
/// # Example
///
/// ```rust
/// use reinhardt_tasks::webhook::WebhookError;
///
/// let error = WebhookError::RequestFailed("Network timeout".to_string());
/// assert_eq!(error.to_string(), "Webhook request failed: Network timeout");
/// ```
#[derive(Debug, Error)]
pub enum WebhookError {
	/// HTTP request failed
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_tasks::webhook::WebhookError;
	///
	/// let error = WebhookError::RequestFailed("Connection refused".to_string());
	/// ```
	#[error("Webhook request failed: {0}")]
	RequestFailed(String),

	/// Max retries exceeded
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_tasks::webhook::WebhookError;
	///
	/// let error = WebhookError::MaxRetriesExceeded;
	/// assert_eq!(error.to_string(), "Max retries exceeded for webhook");
	/// ```
	#[error("Max retries exceeded for webhook")]
	MaxRetriesExceeded,

	/// Serialization error
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_tasks::webhook::WebhookError;
	///
	/// let error = WebhookError::SerializationError("Invalid JSON".to_string());
	/// ```
	#[error("Webhook serialization error: {0}")]
	SerializationError(String),

	/// Invalid URL format
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_tasks::webhook::WebhookError;
	///
	/// let error = WebhookError::InvalidUrl("not-a-url".to_string());
	/// ```
	#[error("Invalid webhook URL: {0}")]
	InvalidUrl(String),

	/// URL scheme not allowed (only HTTPS is permitted)
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_tasks::webhook::WebhookError;
	///
	/// let error = WebhookError::SchemeNotAllowed("http".to_string());
	/// ```
	#[error("URL scheme not allowed: {0}. Only HTTPS is permitted for webhooks")]
	SchemeNotAllowed(String),

	/// SSRF protection: URL resolves to blocked IP address
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_tasks::webhook::WebhookError;
	///
	/// let error = WebhookError::BlockedIpAddress("127.0.0.1".to_string());
	/// ```
	#[error("Webhook URL resolves to blocked IP address: {0}")]
	BlockedIpAddress(String),

	/// DNS resolution failed
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_tasks::webhook::WebhookError;
	///
	/// let error = WebhookError::DnsResolutionFailed("example.invalid".to_string());
	/// ```
	#[error("DNS resolution failed for webhook URL host: {0}")]
	DnsResolutionFailed(String),
}

/// SSRF protection: blocked IP address ranges
///
/// These ranges are blocked to prevent Server-Side Request Forgery attacks:
/// - Loopback addresses (127.0.0.0/8, ::1/128)
/// - Private IPv4 ranges (10.0.0.0/8, 172.16.0.0/12, 192.168.0.0/16)
/// - Link-local addresses (169.254.0.0/16, fe80::/10)
/// - Cloud metadata endpoints (169.254.169.254/32 for AWS/GCP/Azure)
const BLOCKED_IP_RANGES: &[&str] = &[
	// IPv4 loopback
	"127.0.0.0/8",
	// IPv4 private ranges
	"10.0.0.0/8",
	"172.16.0.0/12",
	"192.168.0.0/16",
	// IPv4 link-local (includes cloud metadata at 169.254.169.254)
	"169.254.0.0/16",
	// IPv6 loopback
	"::1/128",
	// IPv6 link-local
	"fe80::/10",
	// IPv6 unique local (private)
	"fc00::/7",
];

/// Check if an IP address is in a blocked range for SSRF protection.
///
/// # Arguments
///
/// * `ip` - The IP address to check
///
/// # Returns
///
/// `true` if the IP is in a blocked range, `false` otherwise
///
/// # Example
///
/// ```rust
/// use reinhardt_tasks::webhook::is_blocked_ip;
/// use std::net::IpAddr;
///
/// let loopback: IpAddr = "127.0.0.1".parse().unwrap();
/// assert!(is_blocked_ip(&loopback));
///
/// let public: IpAddr = "8.8.8.8".parse().unwrap();
/// assert!(!is_blocked_ip(&public));
/// ```
pub fn is_blocked_ip(ip: &IpAddr) -> bool {
	BLOCKED_IP_RANGES.iter().any(|range| {
		range
			.parse::<IpNet>()
			.map(|net| net.contains(ip))
			.unwrap_or(false)
	})
}

/// Validate a webhook URL for SSRF protection.
///
/// This function performs the following checks:
/// 1. URL must be parseable
/// 2. URL scheme must be HTTPS
/// 3. URL hostname must resolve to a non-blocked IP address
///
/// # Arguments
///
/// * `url_str` - The URL string to validate
///
/// # Returns
///
/// `Ok(Url)` if the URL is valid and safe, `Err(WebhookError)` otherwise
///
/// # Example
///
/// ```rust
/// use reinhardt_tasks::webhook::validate_webhook_url;
///
/// // Valid public HTTPS URL
/// let result = validate_webhook_url("https://example.com/webhook");
/// assert!(result.is_ok());
///
/// // Invalid: HTTP scheme
/// let result = validate_webhook_url("http://example.com/webhook");
/// assert!(result.is_err());
///
/// // Invalid: Private IP
/// let result = validate_webhook_url("https://192.168.1.1/webhook");
/// assert!(result.is_err());
/// ```
pub fn validate_webhook_url(url_str: &str) -> Result<Url, WebhookError> {
	// Parse the URL
	let parsed_url =
		Url::parse(url_str).map_err(|e| WebhookError::InvalidUrl(format!("{}: {}", url_str, e)))?;

	// Check scheme - only HTTPS is allowed
	if parsed_url.scheme() != "https" {
		return Err(WebhookError::SchemeNotAllowed(
			parsed_url.scheme().to_string(),
		));
	}

	// Get the host
	let host = parsed_url
		.host_str()
		.ok_or_else(|| WebhookError::InvalidUrl("URL has no host".to_string()))?;

	// Check if host is an IP address directly
	// Note: host_str() returns IPv6 addresses with brackets (e.g., "[::1]")
	// so we need to strip them before parsing
	let host_for_parse = host
		.strip_prefix('[')
		.and_then(|s| s.strip_suffix(']'))
		.unwrap_or(host);

	if let Ok(ip) = host_for_parse.parse::<IpAddr>() {
		if is_blocked_ip(&ip) {
			return Err(WebhookError::BlockedIpAddress(ip.to_string()));
		}
		return Ok(parsed_url);
	}

	// For hostnames, we need to resolve DNS (synchronously check common patterns)
	// First check for localhost-like patterns
	let host_lower = host.to_lowercase();
	if host_lower == "localhost" || host_lower.ends_with(".localhost") {
		return Err(WebhookError::BlockedIpAddress("localhost".to_string()));
	}

	// Check for internal hostname patterns (common in cloud environments)
	if host_lower.ends_with(".internal") || host_lower.ends_with(".local") {
		return Err(WebhookError::BlockedIpAddress(format!(
			"internal hostname: {}",
			host
		)));
	}

	Ok(parsed_url)
}

/// Asynchronously resolve a hostname and validate all resolved IP addresses.
///
/// This function performs DNS resolution and checks each resolved IP address
/// against the blocked ranges.
///
/// # Arguments
///
/// * `url` - The parsed URL to validate
///
/// # Returns
///
/// `Ok(())` if all resolved IPs are safe, `Err(WebhookError)` if any IP is blocked
pub async fn validate_resolved_ips(url: &Url) -> Result<(), WebhookError> {
	let host = url
		.host_str()
		.ok_or_else(|| WebhookError::InvalidUrl("URL has no host".to_string()))?;

	// Skip DNS resolution for IP addresses (already validated)
	// Note: host_str() returns IPv6 addresses with brackets (e.g., "[::1]")
	let host_for_parse = host
		.strip_prefix('[')
		.and_then(|s| s.strip_suffix(']'))
		.unwrap_or(host);

	if host_for_parse.parse::<IpAddr>().is_ok() {
		return Ok(());
	}

	let port = url.port().unwrap_or(443);

	// Perform DNS resolution
	let addrs = tokio::net::lookup_host(format!("{}:{}", host, port))
		.await
		.map_err(|e| WebhookError::DnsResolutionFailed(format!("{}: {}", host, e)))?;

	// Check each resolved IP address
	for addr in addrs {
		if is_blocked_ip(&addr.ip()) {
			return Err(WebhookError::BlockedIpAddress(format!(
				"{} resolves to {}",
				host,
				addr.ip()
			)));
		}
	}

	Ok(())
}

/// Task status for webhook events
///
/// # Example
///
/// ```rust
/// use reinhardt_tasks::webhook::TaskStatus;
///
/// let status = TaskStatus::Success;
/// assert_eq!(status, TaskStatus::Success);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TaskStatus {
	/// Task completed successfully
	Success,
	/// Task failed with an error
	Failed,
	/// Task was cancelled
	Cancelled,
}

/// Webhook event payload
///
/// Contains all information about a completed task for webhook notification.
///
/// # Example
///
/// ```rust
/// use reinhardt_tasks::webhook::{WebhookEvent, TaskStatus};
/// use reinhardt_tasks::TaskId;
/// use chrono::Utc;
///
/// let now = Utc::now();
/// let event = WebhookEvent {
///     task_id: TaskId::new(),
///     task_name: "send_email".to_string(),
///     status: TaskStatus::Success,
///     result: Some("Email sent".to_string()),
///     error: None,
///     started_at: now - chrono::Duration::seconds(5),
///     completed_at: now,
///     duration_ms: 5000,
/// };
///
/// assert_eq!(event.status, TaskStatus::Success);
/// assert_eq!(event.duration_ms, 5000);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookEvent {
	/// Unique task identifier
	pub task_id: TaskId,
	/// Task name
	pub task_name: String,
	/// Task completion status
	pub status: TaskStatus,
	/// Task result (if successful)
	pub result: Option<String>,
	/// Error message (if failed)
	pub error: Option<String>,
	/// Task start time
	pub started_at: DateTime<Utc>,
	/// Task completion time
	pub completed_at: DateTime<Utc>,
	/// Task duration in milliseconds
	pub duration_ms: u64,
}

/// Retry configuration for webhook requests
///
/// # Example
///
/// ```rust
/// use reinhardt_tasks::webhook::RetryConfig;
/// use std::time::Duration;
///
/// let config = RetryConfig {
///     max_retries: 3,
///     initial_backoff: Duration::from_millis(100),
///     max_backoff: Duration::from_secs(10),
///     backoff_multiplier: 2.0,
/// };
///
/// assert_eq!(config.max_retries, 3);
/// assert_eq!(config.backoff_multiplier, 2.0);
/// ```
#[derive(Debug, Clone)]
pub struct RetryConfig {
	/// Maximum number of retry attempts
	pub max_retries: u32,
	/// Initial backoff duration
	pub initial_backoff: Duration,
	/// Maximum backoff duration
	pub max_backoff: Duration,
	/// Backoff multiplier for exponential backoff
	pub backoff_multiplier: f64,
}

impl Default for RetryConfig {
	fn default() -> Self {
		Self {
			max_retries: 3,
			initial_backoff: Duration::from_millis(100),
			max_backoff: Duration::from_secs(30),
			backoff_multiplier: 2.0,
		}
	}
}

/// Webhook configuration
///
/// # Example
///
/// ```rust
/// use reinhardt_tasks::webhook::{WebhookConfig, RetryConfig};
/// use std::time::Duration;
/// use std::collections::HashMap;
///
/// let mut headers = HashMap::new();
/// headers.insert("Authorization".to_string(), "Bearer token123".to_string());
///
/// let config = WebhookConfig {
///     url: "https://api.example.com/webhooks".to_string(),
///     method: "POST".to_string(),
///     headers,
///     timeout: Duration::from_secs(5),
///     retry_config: RetryConfig::default(),
/// };
///
/// assert_eq!(config.url, "https://api.example.com/webhooks");
/// assert_eq!(config.timeout, Duration::from_secs(5));
/// ```
#[derive(Debug, Clone)]
pub struct WebhookConfig {
	/// Webhook URL
	pub url: String,
	/// HTTP method (e.g., "POST", "PUT")
	pub method: String,
	/// Additional HTTP headers
	pub headers: HashMap<String, String>,
	/// Request timeout
	pub timeout: Duration,
	/// Retry configuration
	pub retry_config: RetryConfig,
}

impl Default for WebhookConfig {
	fn default() -> Self {
		Self {
			url: String::new(),
			method: "POST".to_string(),
			headers: HashMap::new(),
			timeout: Duration::from_secs(5),
			retry_config: RetryConfig::default(),
		}
	}
}

/// Trait for webhook senders
///
/// # Example
///
/// ```rust,no_run
/// use reinhardt_tasks::webhook::{WebhookSender, WebhookEvent, TaskStatus, HttpWebhookSender, WebhookConfig};
/// use reinhardt_tasks::TaskId;
/// use chrono::Utc;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let sender = HttpWebhookSender::new(WebhookConfig::default());
///
/// let now = Utc::now();
/// let event = WebhookEvent {
///     task_id: TaskId::new(),
///     task_name: "test_task".to_string(),
///     status: TaskStatus::Success,
///     result: Some("OK".to_string()),
///     error: None,
///     started_at: now,
///     completed_at: now,
///     duration_ms: 0,
/// };
///
/// sender.send(&event).await?;
/// # Ok(())
/// # }
/// ```
#[async_trait]
pub trait WebhookSender: Send + Sync {
	/// Send a webhook event
	async fn send(&self, event: &WebhookEvent) -> Result<(), WebhookError>;
}

/// HTTP webhook sender with retry logic
///
/// # Example
///
/// ```rust
/// use reinhardt_tasks::webhook::{HttpWebhookSender, WebhookConfig, RetryConfig};
/// use std::time::Duration;
///
/// let config = WebhookConfig {
///     url: "https://example.com/webhook".to_string(),
///     method: "POST".to_string(),
///     headers: Default::default(),
///     timeout: Duration::from_secs(5),
///     retry_config: RetryConfig::default(),
/// };
///
/// let sender = HttpWebhookSender::new(config);
/// ```
pub struct HttpWebhookSender {
	client: reqwest::Client,
	config: WebhookConfig,
}

impl HttpWebhookSender {
	/// Create a new HTTP webhook sender
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_tasks::webhook::{HttpWebhookSender, WebhookConfig};
	///
	/// let sender = HttpWebhookSender::new(WebhookConfig::default());
	/// ```
	pub fn new(config: WebhookConfig) -> Self {
		let client = reqwest::Client::builder()
			.timeout(config.timeout)
			.build()
			.unwrap_or_else(|_| reqwest::Client::new());

		Self { client, config }
	}

	/// Calculate backoff duration with exponential backoff and jitter
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_tasks::webhook::{HttpWebhookSender, WebhookConfig, RetryConfig};
	/// use std::time::Duration;
	///
	/// let config = WebhookConfig {
	///     url: "https://example.com".to_string(),
	///     method: "POST".to_string(),
	///     headers: Default::default(),
	///     timeout: Duration::from_secs(5),
	///     retry_config: RetryConfig::default(),
	/// };
	///
	/// let sender = HttpWebhookSender::new(config);
	/// let backoff = sender.calculate_backoff(2);
	/// assert!(backoff > Duration::from_millis(0));
	/// ```
	pub fn calculate_backoff(&self, retry_count: u32) -> Duration {
		let retry_config = &self.config.retry_config;

		// Calculate base backoff with exponential growth
		let backoff_ms = retry_config.initial_backoff.as_millis() as f64
			* retry_config.backoff_multiplier.powi(retry_count as i32);

		// Add jitter (±25%)
		let mut rng = rand::thread_rng();
		let jitter = rng.gen_range(-0.25..=0.25);
		let backoff_with_jitter = backoff_ms * (1.0 + jitter);

		// Cap at max backoff (AFTER jitter)
		let capped_backoff = backoff_with_jitter.min(retry_config.max_backoff.as_millis() as f64);

		Duration::from_millis(capped_backoff.max(0.0) as u64)
	}

	/// Send webhook request with retry logic
	async fn send_with_retry(&self, event: &WebhookEvent) -> Result<(), WebhookError> {
		let mut retry_count = 0;
		let max_retries = self.config.retry_config.max_retries;

		loop {
			match self.send_request(event).await {
				Ok(_) => return Ok(()),
				Err(e) => {
					if retry_count >= max_retries {
						return Err(WebhookError::MaxRetriesExceeded);
					}

					let backoff = self.calculate_backoff(retry_count);
					tracing::warn!(
						attempt = retry_count + 1,
						max_attempts = max_retries + 1,
						error = %e,
						backoff = ?backoff,
						"Webhook request failed, retrying"
					);

					// Wait before retrying to avoid tight retry loops
					tokio::time::sleep(backoff).await;
					retry_count += 1;
				}
			}
		}
	}

	/// Send a single webhook request
	async fn send_request(&self, event: &WebhookEvent) -> Result<(), WebhookError> {
		let json_body = serde_json::to_string(event)
			.map_err(|e| WebhookError::SerializationError(e.to_string()))?;

		let mut request = match self.config.method.to_uppercase().as_str() {
			"POST" => self.client.post(&self.config.url),
			"PUT" => self.client.put(&self.config.url),
			"PATCH" => self.client.patch(&self.config.url),
			_ => self.client.post(&self.config.url),
		};

		// Add headers
		for (key, value) in &self.config.headers {
			request = request.header(key, value);
		}

		// Send request
		let response = request
			.header("Content-Type", "application/json")
			.body(json_body)
			.send()
			.await
			.map_err(|e| WebhookError::RequestFailed(e.to_string()))?;

		// Check response status
		if !response.status().is_success() {
			return Err(WebhookError::RequestFailed(format!(
				"HTTP {}: {}",
				response.status(),
				response
					.text()
					.await
					.unwrap_or_else(|_| "No response body".to_string())
			)));
		}

		Ok(())
	}
}

#[async_trait]
impl WebhookSender for HttpWebhookSender {
	async fn send(&self, event: &WebhookEvent) -> Result<(), WebhookError> {
		// Validate URL for SSRF protection before making any requests
		let validated_url = validate_webhook_url(&self.config.url)?;
		validate_resolved_ips(&validated_url).await?;

		self.send_with_retry(event).await
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;
	use std::time::Duration;

	#[rstest]
	fn test_task_status_serialization() {
		// Arrange
		let status = TaskStatus::Success;

		// Act
		let json = serde_json::to_string(&status).unwrap();

		// Assert
		assert_eq!(json, r#""success""#);

		// Act
		let status: TaskStatus = serde_json::from_str(r#""failed""#).unwrap();

		// Assert
		assert_eq!(status, TaskStatus::Failed);
	}

	#[rstest]
	fn test_webhook_event_serialization() {
		// Arrange
		let now = Utc::now();
		let event = WebhookEvent {
			task_id: TaskId::new(),
			task_name: "test_task".to_string(),
			status: TaskStatus::Success,
			result: Some("OK".to_string()),
			error: None,
			started_at: now,
			completed_at: now,
			duration_ms: 1000,
		};

		// Act
		let json = serde_json::to_string(&event).unwrap();

		// Assert
		assert!(json.contains("test_task"));
		assert!(json.contains(r#""status":"success""#));

		// Act
		let deserialized: WebhookEvent = serde_json::from_str(&json).unwrap();

		// Assert
		assert_eq!(deserialized.task_name, "test_task");
		assert_eq!(deserialized.status, TaskStatus::Success);
	}

	#[rstest]
	fn test_retry_config_default() {
		// Arrange & Act
		let config = RetryConfig::default();

		// Assert
		assert_eq!(config.max_retries, 3);
		assert_eq!(config.initial_backoff, Duration::from_millis(100));
		assert_eq!(config.max_backoff, Duration::from_secs(30));
		assert_eq!(config.backoff_multiplier, 2.0);
	}

	#[rstest]
	fn test_webhook_config_default() {
		// Arrange & Act
		let config = WebhookConfig::default();

		// Assert
		assert_eq!(config.url, "");
		assert_eq!(config.method, "POST");
		assert_eq!(config.timeout, Duration::from_secs(5));
		assert!(config.headers.is_empty());
	}

	#[rstest]
	fn test_calculate_backoff() {
		// Arrange
		let config = WebhookConfig {
			url: "https://example.com".to_string(),
			method: "POST".to_string(),
			headers: HashMap::new(),
			timeout: Duration::from_secs(5),
			retry_config: RetryConfig {
				max_retries: 3,
				initial_backoff: Duration::from_millis(100),
				max_backoff: Duration::from_secs(10),
				backoff_multiplier: 2.0,
			},
		};
		let sender = HttpWebhookSender::new(config);

		// Act - test exponential backoff
		let backoff0 = sender.calculate_backoff(0);
		let backoff1 = sender.calculate_backoff(1);
		let backoff2 = sender.calculate_backoff(2);

		// Assert - verify exponential growth (accounting for jitter)
		assert!(backoff0.as_millis() >= 75 && backoff0.as_millis() <= 125); // ~100ms +/-25%
		assert!(backoff1.as_millis() >= 150 && backoff1.as_millis() <= 250); // ~200ms +/-25%
		assert!(backoff2.as_millis() >= 300 && backoff2.as_millis() <= 500); // ~400ms +/-25%

		// Act & Assert - test max backoff cap
		let backoff_large = sender.calculate_backoff(100);
		assert!(backoff_large <= Duration::from_secs(10));
	}

	#[rstest]
	fn test_webhook_error_display() {
		// Arrange & Act & Assert
		let error = WebhookError::RequestFailed("Connection timeout".to_string());
		assert_eq!(
			error.to_string(),
			"Webhook request failed: Connection timeout"
		);

		let error = WebhookError::MaxRetriesExceeded;
		assert_eq!(error.to_string(), "Max retries exceeded for webhook");

		let error = WebhookError::SerializationError("Invalid JSON".to_string());
		assert_eq!(
			error.to_string(),
			"Webhook serialization error: Invalid JSON"
		);
	}

	#[rstest]
	#[tokio::test]
	async fn test_http_webhook_sender_creation() {
		// Arrange & Act
		let config = WebhookConfig::default();
		let sender = HttpWebhookSender::new(config);

		// Assert
		assert_eq!(sender.config.method, "POST");
	}

	#[rstest]
	#[tokio::test]
	async fn test_webhook_event_creation() {
		// Arrange
		let now = Utc::now();
		let started = now - chrono::Duration::seconds(5);

		// Act
		let event = WebhookEvent {
			task_id: TaskId::new(),
			task_name: "test_task".to_string(),
			status: TaskStatus::Success,
			result: Some("Task completed successfully".to_string()),
			error: None,
			started_at: started,
			completed_at: now,
			duration_ms: 5000,
		};

		// Assert
		assert_eq!(event.task_name, "test_task");
		assert_eq!(event.status, TaskStatus::Success);
		assert!(event.result.is_some());
		assert!(event.error.is_none());
		assert_eq!(event.duration_ms, 5000);
	}

	#[rstest]
	#[tokio::test]
	async fn test_webhook_failed_event() {
		// Arrange
		let now = Utc::now();

		// Act
		let event = WebhookEvent {
			task_id: TaskId::new(),
			task_name: "failed_task".to_string(),
			status: TaskStatus::Failed,
			result: None,
			error: Some("Database connection failed".to_string()),
			started_at: now,
			completed_at: now,
			duration_ms: 100,
		};

		// Assert
		assert_eq!(event.status, TaskStatus::Failed);
		assert!(event.result.is_none());
		assert!(event.error.is_some());
		assert_eq!(
			event.error.unwrap(),
			"Database connection failed".to_string()
		);
	}

	// Integration test with mock HTTP server.
	// NOTE: These tests use send_with_retry directly because mockito servers
	// use HTTP on localhost, which is intentionally blocked by SSRF validation.
	// SSRF validation is tested separately below.
	#[rstest]
	#[tokio::test]
	async fn test_webhook_send_success() {
		// Arrange
		let mut server = mockito::Server::new_async().await;
		let mock = server
			.mock("POST", "/webhook")
			.with_status(200)
			.with_header("content-type", "application/json")
			.with_body(r#"{"status":"ok"}"#)
			.create_async()
			.await;

		let config = WebhookConfig {
			url: format!("{}/webhook", server.url()),
			method: "POST".to_string(),
			headers: HashMap::new(),
			timeout: Duration::from_secs(5),
			retry_config: RetryConfig {
				max_retries: 0,
				initial_backoff: Duration::from_millis(10),
				max_backoff: Duration::from_secs(1),
				backoff_multiplier: 2.0,
			},
		};

		let sender = HttpWebhookSender::new(config);

		let now = Utc::now();
		let event = WebhookEvent {
			task_id: TaskId::new(),
			task_name: "test_task".to_string(),
			status: TaskStatus::Success,
			result: Some("OK".to_string()),
			error: None,
			started_at: now,
			completed_at: now,
			duration_ms: 100,
		};

		// Act
		let result = sender.send_with_retry(&event).await;

		// Assert
		assert!(result.is_ok());
		mock.assert_async().await;
	}

	#[rstest]
	#[tokio::test]
	async fn test_webhook_send_retry_then_success() {
		// Arrange
		let mut server = mockito::Server::new_async().await;

		// First two requests fail, third succeeds
		let mock1 = server
			.mock("POST", "/webhook")
			.with_status(500)
			.expect(1)
			.create_async()
			.await;

		let mock2 = server
			.mock("POST", "/webhook")
			.with_status(503)
			.expect(1)
			.create_async()
			.await;

		let mock3 = server
			.mock("POST", "/webhook")
			.with_status(200)
			.expect(1)
			.create_async()
			.await;

		let config = WebhookConfig {
			url: format!("{}/webhook", server.url()),
			method: "POST".to_string(),
			headers: HashMap::new(),
			timeout: Duration::from_secs(5),
			retry_config: RetryConfig {
				max_retries: 3,
				initial_backoff: Duration::from_millis(10),
				max_backoff: Duration::from_secs(1),
				backoff_multiplier: 2.0,
			},
		};

		let sender = HttpWebhookSender::new(config);

		let now = Utc::now();
		let event = WebhookEvent {
			task_id: TaskId::new(),
			task_name: "test_task".to_string(),
			status: TaskStatus::Success,
			result: Some("OK".to_string()),
			error: None,
			started_at: now,
			completed_at: now,
			duration_ms: 100,
		};

		// Act
		let result = sender.send_with_retry(&event).await;

		// Assert
		assert!(result.is_ok());
		mock1.assert_async().await;
		mock2.assert_async().await;
		mock3.assert_async().await;
	}

	#[rstest]
	#[tokio::test]
	async fn test_webhook_send_max_retries_exceeded() {
		// Arrange
		let mut server = mockito::Server::new_async().await;

		// All requests fail
		let mock = server
			.mock("POST", "/webhook")
			.with_status(500)
			.expect(4) // Initial + 3 retries
			.create_async()
			.await;

		let config = WebhookConfig {
			url: format!("{}/webhook", server.url()),
			method: "POST".to_string(),
			headers: HashMap::new(),
			timeout: Duration::from_secs(5),
			retry_config: RetryConfig {
				max_retries: 3,
				initial_backoff: Duration::from_millis(10),
				max_backoff: Duration::from_secs(1),
				backoff_multiplier: 2.0,
			},
		};

		let sender = HttpWebhookSender::new(config);

		let now = Utc::now();
		let event = WebhookEvent {
			task_id: TaskId::new(),
			task_name: "test_task".to_string(),
			status: TaskStatus::Success,
			result: Some("OK".to_string()),
			error: None,
			started_at: now,
			completed_at: now,
			duration_ms: 100,
		};

		// Act
		let result = sender.send_with_retry(&event).await;

		// Assert
		assert!(result.is_err());
		assert!(matches!(
			result.unwrap_err(),
			WebhookError::MaxRetriesExceeded
		));
		mock.assert_async().await;
	}

	#[rstest]
	#[tokio::test]
	async fn test_webhook_custom_headers() {
		// Arrange
		let mut server = mockito::Server::new_async().await;

		let mock = server
			.mock("POST", "/webhook")
			.match_header("Authorization", "Bearer test-token")
			.match_header("X-Custom-Header", "custom-value")
			.with_status(200)
			.create_async()
			.await;

		let mut headers = HashMap::new();
		headers.insert("Authorization".to_string(), "Bearer test-token".to_string());
		headers.insert("X-Custom-Header".to_string(), "custom-value".to_string());

		let config = WebhookConfig {
			url: format!("{}/webhook", server.url()),
			method: "POST".to_string(),
			headers,
			timeout: Duration::from_secs(5),
			retry_config: RetryConfig {
				max_retries: 0,
				initial_backoff: Duration::from_millis(10),
				max_backoff: Duration::from_secs(1),
				backoff_multiplier: 2.0,
			},
		};

		let sender = HttpWebhookSender::new(config);

		let now = Utc::now();
		let event = WebhookEvent {
			task_id: TaskId::new(),
			task_name: "test_task".to_string(),
			status: TaskStatus::Success,
			result: Some("OK".to_string()),
			error: None,
			started_at: now,
			completed_at: now,
			duration_ms: 100,
		};

		// Act
		let result = sender.send_with_retry(&event).await;

		// Assert
		assert!(result.is_ok());
		mock.assert_async().await;
	}

	#[rstest]
	#[tokio::test]
	async fn test_webhook_retry_loop_sleeps_between_retries() {
		// Arrange - verify that the retry loop actually sleeps (using backoff delay)
		// between failed attempts, preventing a tight CPU-spinning retry loop.
		let mut server = mockito::Server::new_async().await;

		// All requests fail so we go through all retries
		let _mock = server
			.mock("POST", "/webhook")
			.with_status(500)
			.expect(3) // Initial + 2 retries
			.create_async()
			.await;

		let config = WebhookConfig {
			url: format!("{}/webhook", server.url()),
			method: "POST".to_string(),
			headers: HashMap::new(),
			timeout: Duration::from_secs(5),
			retry_config: RetryConfig {
				max_retries: 2,
				initial_backoff: Duration::from_millis(50),
				max_backoff: Duration::from_secs(1),
				backoff_multiplier: 2.0,
			},
		};

		let sender = HttpWebhookSender::new(config);

		let now = Utc::now();
		let event = WebhookEvent {
			task_id: TaskId::new(),
			task_name: "test_task".to_string(),
			status: TaskStatus::Success,
			result: None,
			error: None,
			started_at: now,
			completed_at: now,
			duration_ms: 0,
		};

		// Act - measure elapsed time to verify sleep actually occurs
		let start = std::time::Instant::now();
		let result = sender.send_with_retry(&event).await;
		let elapsed = start.elapsed();

		// Assert - with 2 retries at 50ms and 100ms backoff (plus jitter),
		// total sleep should be at least ~100ms. Without the sleep call,
		// elapsed would be near-zero (only network round-trip time).
		assert!(result.is_err());
		assert!(
			elapsed >= Duration::from_millis(80),
			"Expected at least 80ms delay from retry backoff sleep, got {:?}",
			elapsed
		);
	}

	// SSRF protection tests

	#[rstest]
	#[case("127.0.0.1", true)]
	#[case("127.0.0.2", true)]
	#[case("127.255.255.255", true)]
	#[case("10.0.0.1", true)]
	#[case("10.255.255.255", true)]
	#[case("172.16.0.1", true)]
	#[case("172.31.255.255", true)]
	#[case("192.168.0.1", true)]
	#[case("192.168.255.255", true)]
	#[case("169.254.169.254", true)]
	#[case("169.254.170.2", true)]
	#[case("::1", true)]
	#[case("fe80::1", true)]
	#[case("fc00::1", true)]
	#[case("8.8.8.8", false)]
	#[case("1.1.1.1", false)]
	#[case("203.0.113.1", false)]
	#[case("2001:db8::1", false)]
	fn test_is_blocked_ip(#[case] ip_str: &str, #[case] expected: bool) {
		// Arrange
		let ip: IpAddr = ip_str.parse().unwrap();

		// Act
		let result = is_blocked_ip(&ip);

		// Assert
		assert_eq!(
			result, expected,
			"IP {} should be blocked={}",
			ip_str, expected
		);
	}

	#[rstest]
	#[case("https://example.com/webhook", true)]
	#[case("https://api.example.com/hooks/123", true)]
	#[case("https://hooks.slack.com/services/T00/B00/xxx", true)]
	fn test_validate_webhook_url_accepts_valid_urls(#[case] url: &str, #[case] _valid: bool) {
		// Act
		let result = validate_webhook_url(url);

		// Assert
		assert!(
			result.is_ok(),
			"URL {} should be valid: {:?}",
			url,
			result.err()
		);
	}

	#[rstest]
	#[case("http://example.com/webhook", "SchemeNotAllowed")]
	#[case("ftp://example.com/file", "SchemeNotAllowed")]
	#[case("not-a-url", "InvalidUrl")]
	#[case("https://127.0.0.1/webhook", "BlockedIpAddress")]
	#[case("https://10.0.0.1/webhook", "BlockedIpAddress")]
	#[case("https://172.16.0.1/webhook", "BlockedIpAddress")]
	#[case("https://192.168.1.1/webhook", "BlockedIpAddress")]
	#[case("https://169.254.169.254/latest/meta-data/", "BlockedIpAddress")]
	#[case("https://[::1]/webhook", "BlockedIpAddress")]
	#[case("https://[fe80::1]/webhook", "BlockedIpAddress")]
	#[case("https://[fc00::1]/webhook", "BlockedIpAddress")]
	#[case("https://localhost/webhook", "BlockedIpAddress")]
	#[case("https://sub.localhost/webhook", "BlockedIpAddress")]
	#[case("https://service.internal/webhook", "BlockedIpAddress")]
	#[case("https://printer.local/webhook", "BlockedIpAddress")]
	fn test_validate_webhook_url_rejects_unsafe_urls(
		#[case] url: &str,
		#[case] expected_error: &str,
	) {
		// Act
		let result = validate_webhook_url(url);

		// Assert
		assert!(result.is_err(), "URL {} should be rejected", url);
		let err = result.unwrap_err();
		let err_name = match &err {
			WebhookError::InvalidUrl(_) => "InvalidUrl",
			WebhookError::SchemeNotAllowed(_) => "SchemeNotAllowed",
			WebhookError::BlockedIpAddress(_) => "BlockedIpAddress",
			WebhookError::DnsResolutionFailed(_) => "DnsResolutionFailed",
			_ => "Other",
		};
		assert_eq!(
			err_name, expected_error,
			"URL {} should produce {} error, got: {}",
			url, expected_error, err
		);
	}

	#[rstest]
	fn test_validate_webhook_url_blocks_cloud_metadata_endpoint() {
		// Arrange
		let metadata_urls = [
			"https://169.254.169.254/latest/meta-data/",
			"https://169.254.169.254/computeMetadata/v1/",
			"https://169.254.170.2/v2/credentials",
		];

		for url in &metadata_urls {
			// Act
			let result = validate_webhook_url(url);

			// Assert
			assert!(
				result.is_err(),
				"Cloud metadata URL {} should be blocked",
				url
			);
			assert!(
				matches!(result.unwrap_err(), WebhookError::BlockedIpAddress(_)),
				"Cloud metadata URL {} should produce BlockedIpAddress error",
				url
			);
		}
	}

	#[rstest]
	fn test_webhook_error_display_ssrf_variants() {
		// Arrange & Act & Assert
		let error = WebhookError::InvalidUrl("bad-url".to_string());
		assert_eq!(error.to_string(), "Invalid webhook URL: bad-url");

		let error = WebhookError::SchemeNotAllowed("http".to_string());
		assert_eq!(
			error.to_string(),
			"URL scheme not allowed: http. Only HTTPS is permitted for webhooks"
		);

		let error = WebhookError::BlockedIpAddress("127.0.0.1".to_string());
		assert_eq!(
			error.to_string(),
			"Webhook URL resolves to blocked IP address: 127.0.0.1"
		);

		let error = WebhookError::DnsResolutionFailed("bad.host".to_string());
		assert_eq!(
			error.to_string(),
			"DNS resolution failed for webhook URL host: bad.host"
		);
	}

	#[rstest]
	#[tokio::test]
	async fn test_send_rejects_http_url_via_ssrf_validation() {
		// Arrange
		let config = WebhookConfig {
			url: "http://example.com/webhook".to_string(),
			method: "POST".to_string(),
			headers: HashMap::new(),
			timeout: Duration::from_secs(5),
			retry_config: RetryConfig::default(),
		};
		let sender = HttpWebhookSender::new(config);
		let now = Utc::now();
		let event = WebhookEvent {
			task_id: TaskId::new(),
			task_name: "test_task".to_string(),
			status: TaskStatus::Success,
			result: None,
			error: None,
			started_at: now,
			completed_at: now,
			duration_ms: 0,
		};

		// Act
		let result = sender.send(&event).await;

		// Assert
		assert!(result.is_err());
		assert!(matches!(
			result.unwrap_err(),
			WebhookError::SchemeNotAllowed(_)
		));
	}

	#[rstest]
	#[tokio::test]
	async fn test_send_rejects_private_ip_via_ssrf_validation() {
		// Arrange
		let config = WebhookConfig {
			url: "https://192.168.1.1/webhook".to_string(),
			method: "POST".to_string(),
			headers: HashMap::new(),
			timeout: Duration::from_secs(5),
			retry_config: RetryConfig::default(),
		};
		let sender = HttpWebhookSender::new(config);
		let now = Utc::now();
		let event = WebhookEvent {
			task_id: TaskId::new(),
			task_name: "test_task".to_string(),
			status: TaskStatus::Success,
			result: None,
			error: None,
			started_at: now,
			completed_at: now,
			duration_ms: 0,
		};

		// Act
		let result = sender.send(&event).await;

		// Assert
		assert!(result.is_err());
		assert!(matches!(
			result.unwrap_err(),
			WebhookError::BlockedIpAddress(_)
		));
	}

	// Regression tests for #742: the retry loop MUST call tokio::time::sleep between
	// each failed attempt. Without the sleep, retries would spin at CPU speed and
	// flood the upstream server. The parametrized cases cover 1, 2, and 3 retries
	// with a fixed initial_backoff so elapsed time is predictable.

	// The minimum elapsed time accounts for jitter (±25%) on initial_backoff:
	//   case_1: 1 retry × 50ms × 0.75 = ~37ms  → assert ≥ 30ms
	//   case_2: 2 retries × (50ms + 100ms) × 0.75 ≈ 112ms → assert ≥ 80ms
	//   case_3: 3 retries × (50+100+200)ms × 0.75 ≈ 262ms → assert ≥ 200ms
	#[rstest]
	#[case(1, Duration::from_millis(30), Duration::from_millis(50))]
	#[case(2, Duration::from_millis(80), Duration::from_millis(50))]
	#[case(3, Duration::from_millis(200), Duration::from_millis(50))]
	#[tokio::test]
	async fn test_webhook_retry_sleep_is_called_between_attempts(
		#[case] max_retries: u32,
		#[case] min_elapsed: Duration,
		#[case] initial_backoff: Duration,
	) {
		// Arrange - all server responses fail so the full retry sequence is exercised.
		let mut server = mockito::Server::new_async().await;

		// Expect initial attempt + max_retries retries
		let _mock = server
			.mock("POST", "/webhook")
			.with_status(500)
			.expect((max_retries + 1) as usize)
			.create_async()
			.await;

		let config = WebhookConfig {
			url: format!("{}/webhook", server.url()),
			method: "POST".to_string(),
			headers: HashMap::new(),
			timeout: Duration::from_secs(5),
			retry_config: RetryConfig {
				max_retries,
				initial_backoff,
				max_backoff: Duration::from_secs(1),
				backoff_multiplier: 2.0,
			},
		};

		let sender = HttpWebhookSender::new(config);
		let now = Utc::now();
		let event = WebhookEvent {
			task_id: TaskId::new(),
			task_name: "regression_742".to_string(),
			status: TaskStatus::Success,
			result: None,
			error: None,
			started_at: now,
			completed_at: now,
			duration_ms: 0,
		};

		// Act - measure total elapsed time across all retries
		let start = std::time::Instant::now();
		let result = sender.send_with_retry(&event).await;
		let elapsed = start.elapsed();

		// Assert - at least one backoff sleep must have occurred, so elapsed time
		// must exceed min_elapsed. Without sleep the loop would complete in near-zero
		// wall time (only network round-trip overhead from mockito).
		assert!(
			result.is_err(),
			"expected MaxRetriesExceeded after all retries"
		);
		assert!(
			elapsed >= min_elapsed,
			"Regression #742: expected sleep between retries (>={:?}), got {:?}",
			min_elapsed,
			elapsed
		);
	}
}
