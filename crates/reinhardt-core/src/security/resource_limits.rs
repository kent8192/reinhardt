//! Resource limits configuration for preventing denial-of-service attacks
//!
//! Provides configurable limits for request sizes, connection counts,
//! and other resources to prevent resource exhaustion attacks.

/// Default maximum body size: 10 MB
const DEFAULT_MAX_BODY_SIZE: usize = 10 * 1024 * 1024;
/// Default maximum header count
const DEFAULT_MAX_HEADER_COUNT: usize = 100;
/// Default maximum header size: 8 KB
const DEFAULT_MAX_HEADER_SIZE: usize = 8 * 1024;
/// Default maximum query parameters
const DEFAULT_MAX_QUERY_PARAMS: usize = 100;
/// Default maximum form fields
const DEFAULT_MAX_FORM_FIELDS: usize = 1_000;
/// Default maximum upload size: 50 MB
const DEFAULT_MAX_UPLOAD_SIZE: usize = 50 * 1024 * 1024;
/// Default maximum JSON nesting depth
const DEFAULT_MAX_JSON_DEPTH: usize = 32;
/// Default maximum URL length
const DEFAULT_MAX_URL_LENGTH: usize = 2_048;
/// Default maximum connections
const DEFAULT_MAX_CONNECTIONS: usize = 10_000;
/// Default maximum request rate per second
const DEFAULT_MAX_REQUEST_RATE: u32 = 100;

/// Resource limits configuration
///
/// Configurable limits for various request and connection parameters
/// to prevent resource exhaustion and denial-of-service attacks.
///
/// # Examples
///
/// ```
/// use reinhardt_core::security::resource_limits::ResourceLimits;
///
/// // Use sensible defaults
/// let limits = ResourceLimits::default();
/// assert_eq!(limits.max_body_size(), Some(10 * 1024 * 1024));
///
/// // Use strict limits for sensitive endpoints
/// let strict = ResourceLimits::strict();
/// assert!(strict.max_body_size().unwrap() < limits.max_body_size().unwrap());
///
/// // Customize with builder pattern
/// let custom = ResourceLimits::default()
///     .with_max_body_size(5 * 1024 * 1024)
///     .with_max_json_depth(16);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceLimits {
	max_body_size: Option<usize>,
	max_header_count: Option<usize>,
	max_header_size: Option<usize>,
	max_query_params: Option<usize>,
	max_form_fields: Option<usize>,
	max_upload_size: Option<usize>,
	max_json_depth: Option<usize>,
	max_url_length: Option<usize>,
	max_connections: Option<usize>,
	max_request_rate: Option<u32>,
}

impl Default for ResourceLimits {
	fn default() -> Self {
		Self {
			max_body_size: Some(DEFAULT_MAX_BODY_SIZE),
			max_header_count: Some(DEFAULT_MAX_HEADER_COUNT),
			max_header_size: Some(DEFAULT_MAX_HEADER_SIZE),
			max_query_params: Some(DEFAULT_MAX_QUERY_PARAMS),
			max_form_fields: Some(DEFAULT_MAX_FORM_FIELDS),
			max_upload_size: Some(DEFAULT_MAX_UPLOAD_SIZE),
			max_json_depth: Some(DEFAULT_MAX_JSON_DEPTH),
			max_url_length: Some(DEFAULT_MAX_URL_LENGTH),
			max_connections: Some(DEFAULT_MAX_CONNECTIONS),
			max_request_rate: Some(DEFAULT_MAX_REQUEST_RATE),
		}
	}
}

impl ResourceLimits {
	/// Create resource limits with conservative (strict) values.
	///
	/// Suitable for sensitive endpoints or public-facing APIs.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::security::resource_limits::ResourceLimits;
	///
	/// let limits = ResourceLimits::strict();
	/// assert_eq!(limits.max_body_size(), Some(1024 * 1024));
	/// assert_eq!(limits.max_json_depth(), Some(10));
	/// ```
	pub fn strict() -> Self {
		Self {
			max_body_size: Some(1024 * 1024), // 1 MB
			max_header_count: Some(50),
			max_header_size: Some(4 * 1024), // 4 KB
			max_query_params: Some(20),
			max_form_fields: Some(50),
			max_upload_size: Some(5 * 1024 * 1024), // 5 MB
			max_json_depth: Some(10),
			max_url_length: Some(1024),
			max_connections: Some(1_000),
			max_request_rate: Some(30),
		}
	}

	/// Create resource limits with generous (relaxed) values.
	///
	/// Suitable for internal APIs or trusted clients.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::security::resource_limits::ResourceLimits;
	///
	/// let limits = ResourceLimits::relaxed();
	/// assert_eq!(limits.max_body_size(), Some(100 * 1024 * 1024));
	/// ```
	pub fn relaxed() -> Self {
		Self {
			max_body_size: Some(100 * 1024 * 1024), // 100 MB
			max_header_count: Some(500),
			max_header_size: Some(64 * 1024), // 64 KB
			max_query_params: Some(500),
			max_form_fields: Some(10_000),
			max_upload_size: Some(500 * 1024 * 1024), // 500 MB
			max_json_depth: Some(128),
			max_url_length: Some(8_192),
			max_connections: Some(100_000),
			max_request_rate: Some(1_000),
		}
	}

	/// Create resource limits with no restrictions.
	///
	/// **Warning:** Only use this for testing or fully trusted environments.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::security::resource_limits::ResourceLimits;
	///
	/// let limits = ResourceLimits::unlimited();
	/// assert_eq!(limits.max_body_size(), None);
	/// assert_eq!(limits.max_connections(), None);
	/// ```
	pub fn unlimited() -> Self {
		Self {
			max_body_size: None,
			max_header_count: None,
			max_header_size: None,
			max_query_params: None,
			max_form_fields: None,
			max_upload_size: None,
			max_json_depth: None,
			max_url_length: None,
			max_connections: None,
			max_request_rate: None,
		}
	}

	// Builder methods

	/// Set the maximum request body size in bytes.
	pub fn with_max_body_size(mut self, size: usize) -> Self {
		self.max_body_size = Some(size);
		self
	}

	/// Set the maximum number of headers.
	pub fn with_max_header_count(mut self, count: usize) -> Self {
		self.max_header_count = Some(count);
		self
	}

	/// Set the maximum header size in bytes.
	pub fn with_max_header_size(mut self, size: usize) -> Self {
		self.max_header_size = Some(size);
		self
	}

	/// Set the maximum number of query parameters.
	pub fn with_max_query_params(mut self, count: usize) -> Self {
		self.max_query_params = Some(count);
		self
	}

	/// Set the maximum number of form fields.
	pub fn with_max_form_fields(mut self, count: usize) -> Self {
		self.max_form_fields = Some(count);
		self
	}

	/// Set the maximum upload size in bytes.
	pub fn with_max_upload_size(mut self, size: usize) -> Self {
		self.max_upload_size = Some(size);
		self
	}

	/// Set the maximum JSON nesting depth.
	pub fn with_max_json_depth(mut self, depth: usize) -> Self {
		self.max_json_depth = Some(depth);
		self
	}

	/// Set the maximum URL length.
	pub fn with_max_url_length(mut self, length: usize) -> Self {
		self.max_url_length = Some(length);
		self
	}

	/// Set the maximum number of connections.
	pub fn with_max_connections(mut self, count: usize) -> Self {
		self.max_connections = Some(count);
		self
	}

	/// Set the maximum request rate per second.
	pub fn with_max_request_rate(mut self, rate: u32) -> Self {
		self.max_request_rate = Some(rate);
		self
	}

	// Getter methods

	/// Get the maximum body size limit.
	pub fn max_body_size(&self) -> Option<usize> {
		self.max_body_size
	}

	/// Get the maximum header count limit.
	pub fn max_header_count(&self) -> Option<usize> {
		self.max_header_count
	}

	/// Get the maximum header size limit.
	pub fn max_header_size(&self) -> Option<usize> {
		self.max_header_size
	}

	/// Get the maximum query params limit.
	pub fn max_query_params(&self) -> Option<usize> {
		self.max_query_params
	}

	/// Get the maximum form fields limit.
	pub fn max_form_fields(&self) -> Option<usize> {
		self.max_form_fields
	}

	/// Get the maximum upload size limit.
	pub fn max_upload_size(&self) -> Option<usize> {
		self.max_upload_size
	}

	/// Get the maximum JSON depth limit.
	pub fn max_json_depth(&self) -> Option<usize> {
		self.max_json_depth
	}

	/// Get the maximum URL length limit.
	pub fn max_url_length(&self) -> Option<usize> {
		self.max_url_length
	}

	/// Get the maximum connections limit.
	pub fn max_connections(&self) -> Option<usize> {
		self.max_connections
	}

	/// Get the maximum request rate limit.
	pub fn max_request_rate(&self) -> Option<u32> {
		self.max_request_rate
	}

	// Check methods

	/// Check if a body size is within limits.
	///
	/// Returns `Ok(())` if the size is within limits or if no limit is set.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::security::resource_limits::ResourceLimits;
	///
	/// let limits = ResourceLimits::default();
	/// assert!(limits.check_body_size(1024).is_ok());
	/// assert!(limits.check_body_size(100 * 1024 * 1024).is_err());
	/// ```
	pub fn check_body_size(&self, actual: usize) -> Result<(), LimitExceeded> {
		if let Some(limit) = self.max_body_size
			&& actual > limit
		{
			return Err(LimitExceeded::BodyTooLarge { limit, actual });
		}
		Ok(())
	}

	/// Check if a header count is within limits.
	pub fn check_header_count(&self, actual: usize) -> Result<(), LimitExceeded> {
		if let Some(limit) = self.max_header_count
			&& actual > limit
		{
			return Err(LimitExceeded::TooManyHeaders { limit, actual });
		}
		Ok(())
	}

	/// Check if a header size is within limits.
	pub fn check_header_size(&self, actual: usize) -> Result<(), LimitExceeded> {
		if let Some(limit) = self.max_header_size
			&& actual > limit
		{
			return Err(LimitExceeded::HeaderTooLarge { limit, actual });
		}
		Ok(())
	}

	/// Check if a query params count is within limits.
	pub fn check_query_params(&self, actual: usize) -> Result<(), LimitExceeded> {
		if let Some(limit) = self.max_query_params
			&& actual > limit
		{
			return Err(LimitExceeded::TooManyQueryParams { limit, actual });
		}
		Ok(())
	}

	/// Check if a form fields count is within limits.
	pub fn check_form_fields(&self, actual: usize) -> Result<(), LimitExceeded> {
		if let Some(limit) = self.max_form_fields
			&& actual > limit
		{
			return Err(LimitExceeded::TooManyFormFields { limit, actual });
		}
		Ok(())
	}

	/// Check if an upload size is within limits.
	pub fn check_upload_size(&self, actual: usize) -> Result<(), LimitExceeded> {
		if let Some(limit) = self.max_upload_size
			&& actual > limit
		{
			return Err(LimitExceeded::UploadTooLarge { limit, actual });
		}
		Ok(())
	}

	/// Check if a JSON depth is within limits.
	pub fn check_json_depth(&self, actual: usize) -> Result<(), LimitExceeded> {
		if let Some(limit) = self.max_json_depth
			&& actual > limit
		{
			return Err(LimitExceeded::JsonTooDeep { limit, actual });
		}
		Ok(())
	}

	/// Check if a URL length is within limits.
	pub fn check_url_length(&self, actual: usize) -> Result<(), LimitExceeded> {
		if let Some(limit) = self.max_url_length
			&& actual > limit
		{
			return Err(LimitExceeded::UrlTooLong { limit, actual });
		}
		Ok(())
	}
}

/// Error type for resource limit violations
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LimitExceeded {
	/// Request body exceeds the configured limit
	#[error("body too large: {actual} bytes exceeds limit of {limit} bytes")]
	BodyTooLarge {
		/// Configured limit
		limit: usize,
		/// Actual size
		actual: usize,
	},

	/// Too many headers in the request
	#[error("too many headers: {actual} exceeds limit of {limit}")]
	TooManyHeaders {
		/// Configured limit
		limit: usize,
		/// Actual count
		actual: usize,
	},

	/// Individual header too large
	#[error("header too large: {actual} bytes exceeds limit of {limit} bytes")]
	HeaderTooLarge {
		/// Configured limit
		limit: usize,
		/// Actual size
		actual: usize,
	},

	/// Too many query parameters
	#[error("too many query parameters: {actual} exceeds limit of {limit}")]
	TooManyQueryParams {
		/// Configured limit
		limit: usize,
		/// Actual count
		actual: usize,
	},

	/// Too many form fields
	#[error("too many form fields: {actual} exceeds limit of {limit}")]
	TooManyFormFields {
		/// Configured limit
		limit: usize,
		/// Actual count
		actual: usize,
	},

	/// Upload exceeds size limit
	#[error("upload too large: {actual} bytes exceeds limit of {limit} bytes")]
	UploadTooLarge {
		/// Configured limit
		limit: usize,
		/// Actual size
		actual: usize,
	},

	/// JSON nesting too deep
	#[error("JSON too deeply nested: depth {actual} exceeds limit of {limit}")]
	JsonTooDeep {
		/// Configured limit
		limit: usize,
		/// Actual depth
		actual: usize,
	},

	/// URL too long
	#[error("URL too long: {actual} characters exceeds limit of {limit}")]
	UrlTooLong {
		/// Configured limit
		limit: usize,
		/// Actual length
		actual: usize,
	},

	/// Too many concurrent connections
	#[error("too many connections: limit is {limit}")]
	TooManyConnections {
		/// Configured limit
		limit: usize,
	},

	/// Request rate exceeded
	#[error("rate limit exceeded: limit is {limit} requests per second")]
	RateLimitExceeded {
		/// Configured limit
		limit: u32,
	},
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;

	#[rstest]
	fn default_limits_have_expected_values() {
		// Arrange
		let limits = ResourceLimits::default();

		// Act & Assert
		assert_eq!(limits.max_body_size(), Some(10 * 1024 * 1024));
		assert_eq!(limits.max_header_count(), Some(100));
		assert_eq!(limits.max_header_size(), Some(8 * 1024));
		assert_eq!(limits.max_query_params(), Some(100));
		assert_eq!(limits.max_form_fields(), Some(1_000));
		assert_eq!(limits.max_upload_size(), Some(50 * 1024 * 1024));
		assert_eq!(limits.max_json_depth(), Some(32));
		assert_eq!(limits.max_url_length(), Some(2_048));
		assert_eq!(limits.max_connections(), Some(10_000));
		assert_eq!(limits.max_request_rate(), Some(100));
	}

	#[rstest]
	fn strict_limits_are_more_restrictive_than_default() {
		// Arrange
		let default = ResourceLimits::default();
		let strict = ResourceLimits::strict();

		// Act & Assert
		assert!(strict.max_body_size().unwrap() < default.max_body_size().unwrap());
		assert!(strict.max_header_count().unwrap() < default.max_header_count().unwrap());
		assert!(strict.max_header_size().unwrap() < default.max_header_size().unwrap());
		assert!(strict.max_query_params().unwrap() < default.max_query_params().unwrap());
		assert!(strict.max_form_fields().unwrap() < default.max_form_fields().unwrap());
		assert!(strict.max_upload_size().unwrap() < default.max_upload_size().unwrap());
		assert!(strict.max_json_depth().unwrap() < default.max_json_depth().unwrap());
		assert!(strict.max_url_length().unwrap() < default.max_url_length().unwrap());
		assert!(strict.max_connections().unwrap() < default.max_connections().unwrap());
		assert!(strict.max_request_rate().unwrap() < default.max_request_rate().unwrap());
	}

	#[rstest]
	fn strict_limits_have_expected_values() {
		// Arrange
		let strict = ResourceLimits::strict();

		// Act & Assert
		assert_eq!(strict.max_body_size(), Some(1024 * 1024));
		assert_eq!(strict.max_header_count(), Some(50));
		assert_eq!(strict.max_header_size(), Some(4 * 1024));
		assert_eq!(strict.max_query_params(), Some(20));
		assert_eq!(strict.max_form_fields(), Some(50));
		assert_eq!(strict.max_upload_size(), Some(5 * 1024 * 1024));
		assert_eq!(strict.max_json_depth(), Some(10));
		assert_eq!(strict.max_url_length(), Some(1024));
		assert_eq!(strict.max_connections(), Some(1_000));
		assert_eq!(strict.max_request_rate(), Some(30));
	}

	#[rstest]
	fn relaxed_limits_are_more_generous_than_default() {
		// Arrange
		let default = ResourceLimits::default();
		let relaxed = ResourceLimits::relaxed();

		// Act & Assert
		assert!(relaxed.max_body_size().unwrap() > default.max_body_size().unwrap());
		assert!(relaxed.max_header_count().unwrap() > default.max_header_count().unwrap());
		assert!(relaxed.max_header_size().unwrap() > default.max_header_size().unwrap());
		assert!(relaxed.max_query_params().unwrap() > default.max_query_params().unwrap());
		assert!(relaxed.max_form_fields().unwrap() > default.max_form_fields().unwrap());
		assert!(relaxed.max_upload_size().unwrap() > default.max_upload_size().unwrap());
		assert!(relaxed.max_json_depth().unwrap() > default.max_json_depth().unwrap());
		assert!(relaxed.max_url_length().unwrap() > default.max_url_length().unwrap());
		assert!(relaxed.max_connections().unwrap() > default.max_connections().unwrap());
		assert!(relaxed.max_request_rate().unwrap() > default.max_request_rate().unwrap());
	}

	#[rstest]
	fn relaxed_limits_have_expected_values() {
		// Arrange
		let relaxed = ResourceLimits::relaxed();

		// Act & Assert
		assert_eq!(relaxed.max_body_size(), Some(100 * 1024 * 1024));
		assert_eq!(relaxed.max_header_count(), Some(500));
		assert_eq!(relaxed.max_header_size(), Some(64 * 1024));
		assert_eq!(relaxed.max_query_params(), Some(500));
		assert_eq!(relaxed.max_form_fields(), Some(10_000));
		assert_eq!(relaxed.max_upload_size(), Some(500 * 1024 * 1024));
		assert_eq!(relaxed.max_json_depth(), Some(128));
		assert_eq!(relaxed.max_url_length(), Some(8_192));
		assert_eq!(relaxed.max_connections(), Some(100_000));
		assert_eq!(relaxed.max_request_rate(), Some(1_000));
	}

	#[rstest]
	fn unlimited_has_no_limits() {
		// Arrange
		let unlimited = ResourceLimits::unlimited();

		// Act & Assert
		assert_eq!(unlimited.max_body_size(), None);
		assert_eq!(unlimited.max_header_count(), None);
		assert_eq!(unlimited.max_header_size(), None);
		assert_eq!(unlimited.max_query_params(), None);
		assert_eq!(unlimited.max_form_fields(), None);
		assert_eq!(unlimited.max_upload_size(), None);
		assert_eq!(unlimited.max_json_depth(), None);
		assert_eq!(unlimited.max_url_length(), None);
		assert_eq!(unlimited.max_connections(), None);
		assert_eq!(unlimited.max_request_rate(), None);
	}

	#[rstest]
	fn builder_overrides_individual_limits() {
		// Arrange & Act
		let limits = ResourceLimits::default()
			.with_max_body_size(5 * 1024 * 1024)
			.with_max_header_count(200)
			.with_max_header_size(16 * 1024)
			.with_max_query_params(50)
			.with_max_form_fields(500)
			.with_max_upload_size(25 * 1024 * 1024)
			.with_max_json_depth(16)
			.with_max_url_length(4_096)
			.with_max_connections(5_000)
			.with_max_request_rate(200);

		// Assert
		assert_eq!(limits.max_body_size(), Some(5 * 1024 * 1024));
		assert_eq!(limits.max_header_count(), Some(200));
		assert_eq!(limits.max_header_size(), Some(16 * 1024));
		assert_eq!(limits.max_query_params(), Some(50));
		assert_eq!(limits.max_form_fields(), Some(500));
		assert_eq!(limits.max_upload_size(), Some(25 * 1024 * 1024));
		assert_eq!(limits.max_json_depth(), Some(16));
		assert_eq!(limits.max_url_length(), Some(4_096));
		assert_eq!(limits.max_connections(), Some(5_000));
		assert_eq!(limits.max_request_rate(), Some(200));
	}

	#[rstest]
	fn builder_can_chain_from_strict() {
		// Arrange & Act
		let limits = ResourceLimits::strict().with_max_body_size(2 * 1024 * 1024);

		// Assert
		assert_eq!(limits.max_body_size(), Some(2 * 1024 * 1024));
		// Other strict values remain unchanged
		assert_eq!(limits.max_header_count(), Some(50));
	}

	#[rstest]
	fn check_body_size_within_limit() {
		// Arrange
		let limits = ResourceLimits::default();

		// Act
		let result = limits.check_body_size(1024);

		// Assert
		assert!(result.is_ok());
	}

	#[rstest]
	fn check_body_size_at_exact_limit() {
		// Arrange
		let limits = ResourceLimits::default();

		// Act
		let result = limits.check_body_size(10 * 1024 * 1024);

		// Assert
		assert!(result.is_ok());
	}

	#[rstest]
	fn check_body_size_exceeds_limit() {
		// Arrange
		let limits = ResourceLimits::default();
		let over_limit = 10 * 1024 * 1024 + 1;

		// Act
		let result = limits.check_body_size(over_limit);

		// Assert
		assert_eq!(
			result,
			Err(LimitExceeded::BodyTooLarge {
				limit: 10 * 1024 * 1024,
				actual: over_limit,
			})
		);
	}

	#[rstest]
	fn check_body_size_unlimited_allows_any() {
		// Arrange
		let limits = ResourceLimits::unlimited();

		// Act
		let result = limits.check_body_size(usize::MAX);

		// Assert
		assert!(result.is_ok());
	}

	#[rstest]
	fn check_header_count_within_limit() {
		// Arrange
		let limits = ResourceLimits::default();

		// Act
		let result = limits.check_header_count(50);

		// Assert
		assert!(result.is_ok());
	}

	#[rstest]
	fn check_header_count_exceeds_limit() {
		// Arrange
		let limits = ResourceLimits::default();

		// Act
		let result = limits.check_header_count(101);

		// Assert
		assert_eq!(
			result,
			Err(LimitExceeded::TooManyHeaders {
				limit: 100,
				actual: 101,
			})
		);
	}

	#[rstest]
	fn check_header_size_within_limit() {
		// Arrange
		let limits = ResourceLimits::default();

		// Act
		let result = limits.check_header_size(4 * 1024);

		// Assert
		assert!(result.is_ok());
	}

	#[rstest]
	fn check_header_size_exceeds_limit() {
		// Arrange
		let limits = ResourceLimits::default();

		// Act
		let result = limits.check_header_size(8 * 1024 + 1);

		// Assert
		assert_eq!(
			result,
			Err(LimitExceeded::HeaderTooLarge {
				limit: 8 * 1024,
				actual: 8 * 1024 + 1,
			})
		);
	}

	#[rstest]
	fn check_query_params_within_limit() {
		// Arrange
		let limits = ResourceLimits::default();

		// Act
		let result = limits.check_query_params(50);

		// Assert
		assert!(result.is_ok());
	}

	#[rstest]
	fn check_query_params_exceeds_limit() {
		// Arrange
		let limits = ResourceLimits::default();

		// Act
		let result = limits.check_query_params(101);

		// Assert
		assert_eq!(
			result,
			Err(LimitExceeded::TooManyQueryParams {
				limit: 100,
				actual: 101,
			})
		);
	}

	#[rstest]
	fn check_form_fields_within_limit() {
		// Arrange
		let limits = ResourceLimits::default();

		// Act
		let result = limits.check_form_fields(500);

		// Assert
		assert!(result.is_ok());
	}

	#[rstest]
	fn check_form_fields_exceeds_limit() {
		// Arrange
		let limits = ResourceLimits::default();

		// Act
		let result = limits.check_form_fields(1_001);

		// Assert
		assert_eq!(
			result,
			Err(LimitExceeded::TooManyFormFields {
				limit: 1_000,
				actual: 1_001,
			})
		);
	}

	#[rstest]
	fn check_upload_size_within_limit() {
		// Arrange
		let limits = ResourceLimits::default();

		// Act
		let result = limits.check_upload_size(25 * 1024 * 1024);

		// Assert
		assert!(result.is_ok());
	}

	#[rstest]
	fn check_upload_size_exceeds_limit() {
		// Arrange
		let limits = ResourceLimits::default();
		let over_limit = 50 * 1024 * 1024 + 1;

		// Act
		let result = limits.check_upload_size(over_limit);

		// Assert
		assert_eq!(
			result,
			Err(LimitExceeded::UploadTooLarge {
				limit: 50 * 1024 * 1024,
				actual: over_limit,
			})
		);
	}

	#[rstest]
	fn check_json_depth_within_limit() {
		// Arrange
		let limits = ResourceLimits::default();

		// Act
		let result = limits.check_json_depth(16);

		// Assert
		assert!(result.is_ok());
	}

	#[rstest]
	fn check_json_depth_exceeds_limit() {
		// Arrange
		let limits = ResourceLimits::default();

		// Act
		let result = limits.check_json_depth(33);

		// Assert
		assert_eq!(
			result,
			Err(LimitExceeded::JsonTooDeep {
				limit: 32,
				actual: 33,
			})
		);
	}

	#[rstest]
	fn check_url_length_within_limit() {
		// Arrange
		let limits = ResourceLimits::default();

		// Act
		let result = limits.check_url_length(1_024);

		// Assert
		assert!(result.is_ok());
	}

	#[rstest]
	fn check_url_length_exceeds_limit() {
		// Arrange
		let limits = ResourceLimits::default();

		// Act
		let result = limits.check_url_length(2_049);

		// Assert
		assert_eq!(
			result,
			Err(LimitExceeded::UrlTooLong {
				limit: 2_048,
				actual: 2_049,
			})
		);
	}

	#[rstest]
	fn unlimited_check_methods_all_pass() {
		// Arrange
		let limits = ResourceLimits::unlimited();

		// Act & Assert
		assert!(limits.check_body_size(usize::MAX).is_ok());
		assert!(limits.check_header_count(usize::MAX).is_ok());
		assert!(limits.check_header_size(usize::MAX).is_ok());
		assert!(limits.check_query_params(usize::MAX).is_ok());
		assert!(limits.check_form_fields(usize::MAX).is_ok());
		assert!(limits.check_upload_size(usize::MAX).is_ok());
		assert!(limits.check_json_depth(usize::MAX).is_ok());
		assert!(limits.check_url_length(usize::MAX).is_ok());
	}

	#[rstest]
	fn limit_exceeded_display_body_too_large() {
		// Arrange
		let err = LimitExceeded::BodyTooLarge {
			limit: 1024,
			actual: 2048,
		};

		// Act
		let msg = err.to_string();

		// Assert
		assert_eq!(
			msg,
			"body too large: 2048 bytes exceeds limit of 1024 bytes"
		);
	}

	#[rstest]
	fn limit_exceeded_display_too_many_headers() {
		// Arrange
		let err = LimitExceeded::TooManyHeaders {
			limit: 100,
			actual: 150,
		};

		// Act
		let msg = err.to_string();

		// Assert
		assert_eq!(msg, "too many headers: 150 exceeds limit of 100");
	}

	#[rstest]
	fn limit_exceeded_display_rate_limit() {
		// Arrange
		let err = LimitExceeded::RateLimitExceeded { limit: 100 };

		// Act
		let msg = err.to_string();

		// Assert
		assert_eq!(msg, "rate limit exceeded: limit is 100 requests per second");
	}

	#[rstest]
	fn limit_exceeded_display_too_many_connections() {
		// Arrange
		let err = LimitExceeded::TooManyConnections { limit: 10_000 };

		// Act
		let msg = err.to_string();

		// Assert
		assert_eq!(msg, "too many connections: limit is 10000");
	}

	#[rstest]
	fn resource_limits_implements_clone() {
		// Arrange
		let limits = ResourceLimits::default().with_max_body_size(5 * 1024 * 1024);

		// Act
		let cloned = limits.clone();

		// Assert
		assert_eq!(limits, cloned);
	}

	#[rstest]
	fn resource_limits_debug_format() {
		// Arrange
		let limits = ResourceLimits::strict();

		// Act
		let debug = format!("{:?}", limits);

		// Assert
		assert!(debug.contains("ResourceLimits"));
		assert!(debug.contains("max_body_size"));
	}

	#[rstest]
	fn check_at_exact_boundary_passes() {
		// Arrange
		let limits = ResourceLimits::default();

		// Act & Assert - exact boundary values should pass
		assert!(limits.check_body_size(10 * 1024 * 1024).is_ok());
		assert!(limits.check_header_count(100).is_ok());
		assert!(limits.check_header_size(8 * 1024).is_ok());
		assert!(limits.check_query_params(100).is_ok());
		assert!(limits.check_form_fields(1_000).is_ok());
		assert!(limits.check_upload_size(50 * 1024 * 1024).is_ok());
		assert!(limits.check_json_depth(32).is_ok());
		assert!(limits.check_url_length(2_048).is_ok());
	}

	#[rstest]
	fn check_one_over_boundary_fails() {
		// Arrange
		let limits = ResourceLimits::default();

		// Act & Assert - one over boundary should fail
		assert!(limits.check_body_size(10 * 1024 * 1024 + 1).is_err());
		assert!(limits.check_header_count(101).is_err());
		assert!(limits.check_header_size(8 * 1024 + 1).is_err());
		assert!(limits.check_query_params(101).is_err());
		assert!(limits.check_form_fields(1_001).is_err());
		assert!(limits.check_upload_size(50 * 1024 * 1024 + 1).is_err());
		assert!(limits.check_json_depth(33).is_err());
		assert!(limits.check_url_length(2_049).is_err());
	}

	#[rstest]
	fn check_zero_always_passes() {
		// Arrange
		let limits = ResourceLimits::strict();

		// Act & Assert - zero should always pass
		assert!(limits.check_body_size(0).is_ok());
		assert!(limits.check_header_count(0).is_ok());
		assert!(limits.check_header_size(0).is_ok());
		assert!(limits.check_query_params(0).is_ok());
		assert!(limits.check_form_fields(0).is_ok());
		assert!(limits.check_upload_size(0).is_ok());
		assert!(limits.check_json_depth(0).is_ok());
		assert!(limits.check_url_length(0).is_ok());
	}

	#[rstest]
	fn presets_ordering_strict_lt_default_lt_relaxed() {
		// Arrange
		let strict = ResourceLimits::strict();
		let default = ResourceLimits::default();
		let relaxed = ResourceLimits::relaxed();

		// Act & Assert
		assert!(strict.max_body_size().unwrap() < default.max_body_size().unwrap());
		assert!(default.max_body_size().unwrap() < relaxed.max_body_size().unwrap());
		assert!(strict.max_connections().unwrap() < default.max_connections().unwrap());
		assert!(default.max_connections().unwrap() < relaxed.max_connections().unwrap());
		assert!(strict.max_request_rate().unwrap() < default.max_request_rate().unwrap());
		assert!(default.max_request_rate().unwrap() < relaxed.max_request_rate().unwrap());
	}

	#[rstest]
	fn limit_exceeded_equality() {
		// Arrange
		let err1 = LimitExceeded::BodyTooLarge {
			limit: 1024,
			actual: 2048,
		};
		let err2 = LimitExceeded::BodyTooLarge {
			limit: 1024,
			actual: 2048,
		};
		let err3 = LimitExceeded::BodyTooLarge {
			limit: 1024,
			actual: 4096,
		};

		// Act & Assert
		assert_eq!(err1, err2);
		assert_ne!(err1, err3);
	}
}
