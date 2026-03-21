//! Limit/offset based pagination implementation

use crate::exception::{Error, Result};
use async_trait::async_trait;

use super::core::{AsyncPaginator, PaginatedResponse, Paginator, SchemaParameter};

/// Limit/offset based pagination
///
/// Example URLs:
/// - `http://api.example.org/accounts/?limit=100`
/// - `http://api.example.org/accounts/?offset=400&limit=100`
#[derive(Debug, Clone)]
pub struct LimitOffsetPagination {
	/// Default limit (page size)
	pub default_limit: usize,
	/// Query parameter name for limit
	pub limit_query_param: String,
	/// Query parameter name for offset
	pub offset_query_param: String,
	/// Maximum allowed limit
	pub max_limit: Option<usize>,
}

impl Default for LimitOffsetPagination {
	fn default() -> Self {
		Self {
			default_limit: 10,
			limit_query_param: "limit".to_string(),
			offset_query_param: "offset".to_string(),
			max_limit: None,
		}
	}
}

impl LimitOffsetPagination {
	/// Creates a new LimitOffsetPagination with default settings
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::pagination::LimitOffsetPagination;
	///
	/// let paginator = LimitOffsetPagination::new();
	/// assert_eq!(paginator.default_limit, 10);
	/// assert_eq!(paginator.limit_query_param, "limit");
	/// assert_eq!(paginator.offset_query_param, "offset");
	/// ```
	pub fn new() -> Self {
		Self::default()
	}
	/// Sets the default limit (page size) for pagination
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::pagination::LimitOffsetPagination;
	///
	/// let paginator = LimitOffsetPagination::new().default_limit(25);
	/// assert_eq!(paginator.default_limit, 25);
	/// ```
	pub fn default_limit(mut self, limit: usize) -> Self {
		self.default_limit = limit;
		self
	}
	/// Sets the maximum allowed limit
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::pagination::LimitOffsetPagination;
	///
	/// let paginator = LimitOffsetPagination::new()
	///     .default_limit(10)
	///     .max_limit(100);
	/// assert_eq!(paginator.max_limit, Some(100));
	/// ```
	pub fn max_limit(mut self, limit: usize) -> Self {
		self.max_limit = Some(limit);
		self
	}

	fn parse_positive_int(value: &str) -> Result<usize> {
		value
			.parse::<usize>()
			.map_err(|_| Error::Validation(format!("Invalid number: {}", value)))
	}

	/// Parse limit and offset from URL query parameters
	fn parse_params(&self, params: &str, _base_url: &str) -> Result<(usize, usize)> {
		// Try to parse as URL first, otherwise treat as query string
		let query_string = if params.starts_with("http") || params.starts_with('/') {
			if let Ok(url) = url::Url::parse(params)
				.or_else(|_| url::Url::parse(&format!("http://localhost{}", params)))
			{
				url.query().unwrap_or("").to_string()
			} else {
				params.to_string()
			}
		} else {
			params.to_string()
		};

		let mut limit = self.default_limit;
		let mut offset = 0;

		// Parse query parameters
		for pair in query_string.split('&') {
			let parts: Vec<&str> = pair.split('=').collect();
			if parts.len() == 2 {
				let key = parts[0];
				let value = parts[1];

				if key == self.limit_query_param {
					limit = Self::parse_positive_int(value)?;
					// Reject zero limit to prevent infinite next-link loop
					if limit == 0 {
						return Err(Error::InvalidLimit(format!(
							"{} must be greater than zero (got {})",
							self.limit_query_param, limit
						)));
					}
					// Apply max_limit if configured
					if let Some(max) = self.max_limit
						&& limit > max
					{
						return Err(Error::InvalidLimit(format!(
							"Limit {} exceeds maximum {}",
							limit, max
						)));
					}
				} else if key == self.offset_query_param {
					offset = Self::parse_positive_int(value)?;
				}
			}
		}

		Ok((limit, offset))
	}

	fn build_url(&self, base_url: &str, offset: usize, limit: usize) -> String {
		let url = super::parse_base_url(base_url);

		let mut new_url = url.clone();
		new_url
			.query_pairs_mut()
			.clear()
			.append_pair(&self.offset_query_param, &offset.to_string())
			.append_pair(&self.limit_query_param, &limit.to_string());

		// Copy other query parameters
		for (key, value) in url.query_pairs() {
			if key != self.offset_query_param && key != self.limit_query_param {
				new_url.query_pairs_mut().append_pair(&key, &value);
			}
		}

		new_url.to_string()
	}
}

#[async_trait]
impl Paginator for LimitOffsetPagination {
	fn paginate<T: Clone + Send + Sync>(
		&self,
		items: &[T],
		params: Option<&str>,
		base_url: &str,
	) -> Result<PaginatedResponse<T>> {
		// Parse query parameters from URL or params string
		let (limit, offset) = if let Some(param_str) = params {
			self.parse_params(param_str, base_url)?
		} else {
			(self.default_limit, 0)
		};

		let total_count = items.len();

		// Validate offset
		if offset > total_count {
			return Ok(PaginatedResponse {
				count: total_count,
				next: None,
				previous: None,
				results: vec![],
			});
		}

		// Calculate slice bounds
		let start = offset;
		let end = std::cmp::min(start + limit, total_count);

		// Get results
		let results = items[start..end].to_vec();

		// Build next/previous links
		let next = if end < total_count {
			Some(self.build_url(base_url, offset + limit, limit))
		} else {
			None
		};

		let previous = if offset > 0 {
			let prev_offset = offset.saturating_sub(limit);
			Some(self.build_url(base_url, prev_offset, limit))
		} else {
			None
		};

		Ok(PaginatedResponse {
			count: total_count,
			next,
			previous,
			results,
		})
	}

	fn get_schema_parameters(&self) -> Vec<SchemaParameter> {
		vec![
			SchemaParameter {
				name: self.limit_query_param.clone(),
				required: false,
				location: "query".to_string(),
				description: "Number of results to return per page.".to_string(),
				schema_type: "integer".to_string(),
			},
			SchemaParameter {
				name: self.offset_query_param.clone(),
				required: false,
				location: "query".to_string(),
				description: "The initial index from which to return the results.".to_string(),
				schema_type: "integer".to_string(),
			},
		]
	}
}

#[async_trait]
impl AsyncPaginator for LimitOffsetPagination {
	async fn apaginate<T: Clone + Send + Sync>(
		&self,
		items: &[T],
		params: Option<&str>,
		base_url: &str,
	) -> Result<PaginatedResponse<T>> {
		// For in-memory operations, just call the sync version
		self.paginate(items, params, base_url)
	}

	fn get_schema_parameters(&self) -> Vec<SchemaParameter> {
		Paginator::get_schema_parameters(self)
	}
}

#[cfg(test)]
mod tests {
	use rstest::rstest;

	use super::*;

	#[rstest]
	#[case("not a valid url at all \x00\x01")]
	#[case("://missing-scheme")]
	#[case("")]
	fn build_url_does_not_panic_with_malformed_base_url(#[case] malformed_url: &str) {
		// Arrange
		let paginator = LimitOffsetPagination::new();
		let items: Vec<i32> = (0..20).collect();

		// Act
		let result = paginator.paginate(&items, None, malformed_url);

		// Assert
		assert!(
			result.is_ok(),
			"paginate should not panic with malformed URL: {malformed_url:?}"
		);
	}
}
