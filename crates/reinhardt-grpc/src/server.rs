//! gRPC server configuration
//!
//! This module provides server-level configuration for gRPC services,
//! including message size limits, request timeouts, and connection limits
//! to prevent denial of service attacks.
//!
//! # Default Limits
//!
//! [`GrpcServerConfig`] provides sensible defaults for DoS protection:
//!
//! - **Message size**: 4MB for both encoding and decoding
//! - **Request timeout**: 30 seconds
//! - **Max concurrent connections**: 1000
//!
//! # Example
//!
//! ```rust
//! use reinhardt_grpc::server::GrpcServerConfig;
//! use std::time::Duration;
//!
//! // Use defaults
//! let config = GrpcServerConfig::default();
//! assert_eq!(config.max_decoding_message_size(), 4 * 1024 * 1024);
//! assert_eq!(config.request_timeout(), Duration::from_secs(30));
//! assert_eq!(config.max_concurrent_connections(), 1000);
//!
//! // Custom limits
//! let config = GrpcServerConfig::builder()
//!     .max_decoding_message_size(8 * 1024 * 1024)
//!     .request_timeout(Duration::from_secs(60))
//!     .max_concurrent_connections(500)
//!     .build();
//! ```
//!
//! # Tower Middleware Integration
//!
//! For rate limiting, use tower's middleware ecosystem with tonic. The
//! [`GrpcServerConfig`] values can be applied to a tonic server through
//! tower layers:
//!
//! ```rust,ignore
//! use tonic::transport::Server;
//! use tower::ServiceBuilder;
//! use tower::timeout::TimeoutLayer;
//! use tower::limit::ConcurrencyLimitLayer;
//! use reinhardt_grpc::server::GrpcServerConfig;
//!
//! let config = GrpcServerConfig::default();
//!
//! Server::builder()
//!     .layer(
//!         ServiceBuilder::new()
//!             .layer(TimeoutLayer::new(config.request_timeout()))
//!             .layer(ConcurrencyLimitLayer::new(config.max_concurrent_connections()))
//!             .into_inner(),
//!     )
//!     .add_service(my_service)
//!     .serve(addr)
//!     .await?;
//! ```

use std::time::Duration;

/// Default maximum decoding (incoming) message size: 4MB
const DEFAULT_MAX_DECODING_MESSAGE_SIZE: usize = 4 * 1024 * 1024;

/// Default maximum encoding (outgoing) message size: 4MB
const DEFAULT_MAX_ENCODING_MESSAGE_SIZE: usize = 4 * 1024 * 1024;

/// Default request timeout: 30 seconds
const DEFAULT_REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

/// Default maximum concurrent connections: 1000
const DEFAULT_MAX_CONCURRENT_CONNECTIONS: usize = 1000;

/// Configuration for gRPC server DoS protection.
///
/// This struct holds configuration for message size limits, request
/// timeouts, and connection limits. Setting appropriate values prevents
/// denial of service attacks from oversized messages, slow requests,
/// and connection floods.
///
/// Use [`GrpcServerConfig::builder()`] to construct with custom values,
/// or [`GrpcServerConfig::default()`] for sensible defaults.
///
/// # Defaults
///
/// | Setting | Default |
/// |---------|---------|
/// | `max_decoding_message_size` | 4 MB |
/// | `max_encoding_message_size` | 4 MB |
/// | `request_timeout` | 30 seconds |
/// | `max_concurrent_connections` | 1000 |
///
/// # Example
///
/// ```rust
/// use reinhardt_grpc::server::GrpcServerConfig;
/// use std::time::Duration;
///
/// let config = GrpcServerConfig::builder()
///     .max_decoding_message_size(2 * 1024 * 1024)
///     .request_timeout(Duration::from_secs(60))
///     .max_concurrent_connections(500)
///     .build();
///
/// assert_eq!(config.max_decoding_message_size(), 2 * 1024 * 1024);
/// assert_eq!(config.request_timeout(), Duration::from_secs(60));
/// assert_eq!(config.max_concurrent_connections(), 500);
/// ```
#[derive(Debug, Clone)]
pub struct GrpcServerConfig {
	max_decoding_message_size: usize,
	max_encoding_message_size: usize,
	request_timeout: Duration,
	max_concurrent_connections: usize,
}

impl GrpcServerConfig {
	/// Create a new builder for `GrpcServerConfig`.
	pub fn builder() -> GrpcServerConfigBuilder {
		GrpcServerConfigBuilder::default()
	}

	/// Returns the maximum decoding (incoming) message size in bytes.
	pub fn max_decoding_message_size(&self) -> usize {
		self.max_decoding_message_size
	}

	/// Returns the maximum encoding (outgoing) message size in bytes.
	pub fn max_encoding_message_size(&self) -> usize {
		self.max_encoding_message_size
	}

	/// Returns the maximum message size in bytes.
	///
	/// This is an alias for [`max_decoding_message_size`](Self::max_decoding_message_size),
	/// representing the largest message the server will accept from clients.
	pub fn max_message_size(&self) -> usize {
		self.max_decoding_message_size
	}

	/// Returns the request timeout duration.
	///
	/// Requests exceeding this duration will be cancelled with a
	/// `DeadlineExceeded` status. Apply this via tower's `TimeoutLayer`.
	pub fn request_timeout(&self) -> Duration {
		self.request_timeout
	}

	/// Returns the maximum number of concurrent connections allowed.
	///
	/// Apply this via tower's `ConcurrencyLimitLayer` to prevent
	/// connection flood attacks.
	pub fn max_concurrent_connections(&self) -> usize {
		self.max_concurrent_connections
	}
}

impl Default for GrpcServerConfig {
	fn default() -> Self {
		Self {
			max_decoding_message_size: DEFAULT_MAX_DECODING_MESSAGE_SIZE,
			max_encoding_message_size: DEFAULT_MAX_ENCODING_MESSAGE_SIZE,
			request_timeout: DEFAULT_REQUEST_TIMEOUT,
			max_concurrent_connections: DEFAULT_MAX_CONCURRENT_CONNECTIONS,
		}
	}
}

/// Builder for [`GrpcServerConfig`].
///
/// Uses the builder pattern to construct a `GrpcServerConfig` with
/// custom limits. All values default to the same as `GrpcServerConfig::default()`.
#[derive(Debug, Clone)]
pub struct GrpcServerConfigBuilder {
	max_decoding_message_size: usize,
	max_encoding_message_size: usize,
	request_timeout: Duration,
	max_concurrent_connections: usize,
}

impl GrpcServerConfigBuilder {
	/// Set the maximum decoding (incoming) message size in bytes.
	///
	/// This limits the maximum size of a protobuf message that the
	/// server will accept from clients. Messages exceeding this limit
	/// will be rejected with a `ResourceExhausted` status.
	pub fn max_decoding_message_size(mut self, size: usize) -> Self {
		self.max_decoding_message_size = size;
		self
	}

	/// Set the maximum encoding (outgoing) message size in bytes.
	///
	/// This limits the maximum size of a protobuf message that the
	/// server will send to clients.
	pub fn max_encoding_message_size(mut self, size: usize) -> Self {
		self.max_encoding_message_size = size;
		self
	}

	/// Set the maximum message size in bytes.
	///
	/// This is a convenience method that sets both decoding and encoding
	/// limits to the same value.
	pub fn max_message_size(mut self, size: usize) -> Self {
		self.max_decoding_message_size = size;
		self.max_encoding_message_size = size;
		self
	}

	/// Set the request timeout duration.
	///
	/// Requests exceeding this duration will be cancelled. Apply this
	/// via tower's `TimeoutLayer` when building the server.
	pub fn request_timeout(mut self, timeout: Duration) -> Self {
		self.request_timeout = timeout;
		self
	}

	/// Set the maximum number of concurrent connections.
	///
	/// Apply this via tower's `ConcurrencyLimitLayer` when building
	/// the server.
	pub fn max_concurrent_connections(mut self, max: usize) -> Self {
		self.max_concurrent_connections = max;
		self
	}

	/// Build the `GrpcServerConfig`.
	pub fn build(self) -> GrpcServerConfig {
		GrpcServerConfig {
			max_decoding_message_size: self.max_decoding_message_size,
			max_encoding_message_size: self.max_encoding_message_size,
			request_timeout: self.request_timeout,
			max_concurrent_connections: self.max_concurrent_connections,
		}
	}
}

impl Default for GrpcServerConfigBuilder {
	fn default() -> Self {
		Self {
			max_decoding_message_size: DEFAULT_MAX_DECODING_MESSAGE_SIZE,
			max_encoding_message_size: DEFAULT_MAX_ENCODING_MESSAGE_SIZE,
			request_timeout: DEFAULT_REQUEST_TIMEOUT,
			max_concurrent_connections: DEFAULT_MAX_CONCURRENT_CONNECTIONS,
		}
	}
}

/// Trait for applying message size limits to tonic-generated gRPC service servers.
///
/// Tonic generates service server structs (e.g., `GreeterServer<T>`) that have
/// `max_decoding_message_size` and `max_encoding_message_size` methods. This
/// trait provides a unified way to apply [`GrpcServerConfig`] limits to any
/// such service.
///
/// # Example
///
/// ```rust,ignore
/// use reinhardt_grpc::server::{GrpcServerConfig, MessageSizeLimiter};
///
/// let config = GrpcServerConfig::default();
/// let service = MyServiceServer::new(my_impl).apply_message_size_limits(&config);
/// ```
pub trait MessageSizeLimiter: Sized {
	/// Apply message size limits from the given configuration.
	fn apply_message_size_limits(self, config: &GrpcServerConfig) -> Self;
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;

	#[rstest]
	fn default_config_has_4mb_limits() {
		// Arrange
		let expected_size = 4 * 1024 * 1024;

		// Act
		let config = GrpcServerConfig::default();

		// Assert
		assert_eq!(config.max_decoding_message_size(), expected_size);
		assert_eq!(config.max_encoding_message_size(), expected_size);
	}

	#[rstest]
	fn default_config_has_30s_request_timeout() {
		// Arrange & Act
		let config = GrpcServerConfig::default();

		// Assert
		assert_eq!(config.request_timeout(), Duration::from_secs(30));
	}

	#[rstest]
	fn default_config_has_1000_max_connections() {
		// Arrange & Act
		let config = GrpcServerConfig::default();

		// Assert
		assert_eq!(config.max_concurrent_connections(), 1000);
	}

	#[rstest]
	fn max_message_size_returns_decoding_size() {
		// Arrange
		let config = GrpcServerConfig::builder()
			.max_decoding_message_size(2 * 1024 * 1024)
			.build();

		// Act & Assert
		assert_eq!(config.max_message_size(), 2 * 1024 * 1024);
		assert_eq!(
			config.max_message_size(),
			config.max_decoding_message_size()
		);
	}

	#[rstest]
	fn builder_default_matches_default_config() {
		// Arrange
		let default_config = GrpcServerConfig::default();

		// Act
		let builder_config = GrpcServerConfig::builder().build();

		// Assert
		assert_eq!(
			builder_config.max_decoding_message_size(),
			default_config.max_decoding_message_size()
		);
		assert_eq!(
			builder_config.max_encoding_message_size(),
			default_config.max_encoding_message_size()
		);
		assert_eq!(
			builder_config.request_timeout(),
			default_config.request_timeout()
		);
		assert_eq!(
			builder_config.max_concurrent_connections(),
			default_config.max_concurrent_connections()
		);
	}

	#[rstest]
	fn builder_sets_custom_decoding_limit() {
		// Arrange
		let custom_size = 8 * 1024 * 1024; // 8MB

		// Act
		let config = GrpcServerConfig::builder()
			.max_decoding_message_size(custom_size)
			.build();

		// Assert
		assert_eq!(config.max_decoding_message_size(), custom_size);
		// Encoding should remain default
		assert_eq!(
			config.max_encoding_message_size(),
			DEFAULT_MAX_ENCODING_MESSAGE_SIZE
		);
	}

	#[rstest]
	fn builder_sets_custom_encoding_limit() {
		// Arrange
		let custom_size = 16 * 1024 * 1024; // 16MB

		// Act
		let config = GrpcServerConfig::builder()
			.max_encoding_message_size(custom_size)
			.build();

		// Assert
		assert_eq!(
			config.max_decoding_message_size(),
			DEFAULT_MAX_DECODING_MESSAGE_SIZE
		);
		assert_eq!(config.max_encoding_message_size(), custom_size);
	}

	#[rstest]
	fn builder_sets_both_limits() {
		// Arrange
		let decoding_size = 2 * 1024 * 1024; // 2MB
		let encoding_size = 8 * 1024 * 1024; // 8MB

		// Act
		let config = GrpcServerConfig::builder()
			.max_decoding_message_size(decoding_size)
			.max_encoding_message_size(encoding_size)
			.build();

		// Assert
		assert_eq!(config.max_decoding_message_size(), decoding_size);
		assert_eq!(config.max_encoding_message_size(), encoding_size);
	}

	#[rstest]
	fn builder_sets_max_message_size_for_both() {
		// Arrange
		let size = 2 * 1024 * 1024; // 2MB

		// Act
		let config = GrpcServerConfig::builder().max_message_size(size).build();

		// Assert
		assert_eq!(config.max_decoding_message_size(), size);
		assert_eq!(config.max_encoding_message_size(), size);
	}

	#[rstest]
	fn builder_sets_custom_request_timeout() {
		// Arrange & Act
		let config = GrpcServerConfig::builder()
			.request_timeout(Duration::from_secs(60))
			.build();

		// Assert
		assert_eq!(config.request_timeout(), Duration::from_secs(60));
	}

	#[rstest]
	fn builder_sets_custom_max_concurrent_connections() {
		// Arrange & Act
		let config = GrpcServerConfig::builder()
			.max_concurrent_connections(500)
			.build();

		// Assert
		assert_eq!(config.max_concurrent_connections(), 500);
	}

	#[rstest]
	fn builder_sets_all_custom_values() {
		// Arrange
		let decoding = 2 * 1024 * 1024;
		let encoding = 8 * 1024 * 1024;
		let timeout = Duration::from_secs(60);
		let max_conns = 500;

		// Act
		let config = GrpcServerConfig::builder()
			.max_decoding_message_size(decoding)
			.max_encoding_message_size(encoding)
			.request_timeout(timeout)
			.max_concurrent_connections(max_conns)
			.build();

		// Assert
		assert_eq!(config.max_decoding_message_size(), decoding);
		assert_eq!(config.max_encoding_message_size(), encoding);
		assert_eq!(config.request_timeout(), timeout);
		assert_eq!(config.max_concurrent_connections(), max_conns);
	}

	#[rstest]
	fn config_clone_preserves_values() {
		// Arrange
		let config = GrpcServerConfig::builder()
			.max_decoding_message_size(1024)
			.max_encoding_message_size(2048)
			.request_timeout(Duration::from_secs(45))
			.max_concurrent_connections(200)
			.build();

		// Act
		let cloned = config.clone();

		// Assert
		assert_eq!(
			cloned.max_decoding_message_size(),
			config.max_decoding_message_size()
		);
		assert_eq!(
			cloned.max_encoding_message_size(),
			config.max_encoding_message_size()
		);
		assert_eq!(cloned.request_timeout(), config.request_timeout());
		assert_eq!(
			cloned.max_concurrent_connections(),
			config.max_concurrent_connections()
		);
	}

	#[rstest]
	fn builder_allows_zero_size() {
		// Arrange & Act
		let config = GrpcServerConfig::builder()
			.max_decoding_message_size(0)
			.max_encoding_message_size(0)
			.build();

		// Assert
		assert_eq!(config.max_decoding_message_size(), 0);
		assert_eq!(config.max_encoding_message_size(), 0);
	}

	// Test that MessageSizeLimiter trait can be implemented for a mock service
	struct MockService {
		max_decoding: Option<usize>,
		max_encoding: Option<usize>,
	}

	impl MockService {
		fn new() -> Self {
			Self {
				max_decoding: None,
				max_encoding: None,
			}
		}
	}

	impl MessageSizeLimiter for MockService {
		fn apply_message_size_limits(mut self, config: &GrpcServerConfig) -> Self {
			self.max_decoding = Some(config.max_decoding_message_size());
			self.max_encoding = Some(config.max_encoding_message_size());
			self
		}
	}

	#[rstest]
	fn message_size_limiter_applies_config() {
		// Arrange
		let config = GrpcServerConfig::builder()
			.max_decoding_message_size(1024 * 1024)
			.max_encoding_message_size(2 * 1024 * 1024)
			.build();
		let service = MockService::new();

		// Act
		let service = service.apply_message_size_limits(&config);

		// Assert
		assert_eq!(service.max_decoding, Some(1024 * 1024));
		assert_eq!(service.max_encoding, Some(2 * 1024 * 1024));
	}

	#[rstest]
	fn message_size_limiter_applies_defaults() {
		// Arrange
		let config = GrpcServerConfig::default();
		let service = MockService::new();

		// Act
		let service = service.apply_message_size_limits(&config);

		// Assert
		assert_eq!(
			service.max_decoding,
			Some(DEFAULT_MAX_DECODING_MESSAGE_SIZE)
		);
		assert_eq!(
			service.max_encoding,
			Some(DEFAULT_MAX_ENCODING_MESSAGE_SIZE)
		);
	}
}
