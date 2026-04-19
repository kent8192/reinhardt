//! Query parameter extraction

use async_trait::async_trait;
use reinhardt_http::Request;
use serde::de::DeserializeOwned;
use std::fmt::{self, Debug};
use std::ops::Deref;

use super::{ParamContext, ParamError, ParamResult, ParamType, extract::FromRequest};

#[cfg(feature = "multi-value-arrays")]
use std::collections::HashMap;

/// Extract query parameters from the URL
///
/// With the `multi-value-arrays` feature (enabled by default), repeated query
/// parameters are properly parsed into vectors. For example, `?q=5&q=6` will be
/// parsed as `vec![5, 6]` when the target field type is `Vec<T>`.
///
/// # Example
///
/// ```rust
/// use reinhardt_di::params::Query;
/// # use serde::Deserialize;
/// #[derive(Deserialize)]
/// struct Pagination {
///     page: Option<i32>,
///     per_page: Option<i32>,
/// }
///
/// let pagination = Pagination { page: Some(2), per_page: Some(25) };
/// let query = Query(pagination);
/// let page = query.page.unwrap_or(1);
/// let per_page = query.per_page.unwrap_or(10);
/// assert_eq!(page, 2);
/// assert_eq!(per_page, 25);
/// ```
///
/// # Multi-value Parameters
///
/// ```rust
/// use reinhardt_di::params::Query;
/// # use serde::Deserialize;
/// #[derive(Deserialize)]
/// struct SearchQuery {
///     q: Vec<i64>,  // Supports repeated keys: ?q=5&q=6
/// }
///
/// let search = SearchQuery { q: vec![5, 6] };
/// let query = Query(search);
/// assert_eq!(query.q, vec![5, 6]);
/// ```
pub struct Query<T>(pub T);

impl<T> Query<T> {
	/// Unwrap the Query and return the inner value
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_di::params::Query;
	/// use serde::Deserialize;
	///
	/// #[derive(Deserialize, Debug, PartialEq)]
	/// struct Pagination {
	///     page: i32,
	///     per_page: i32,
	/// }
	///
	/// let query = Query(Pagination { page: 1, per_page: 10 });
	/// let inner = query.into_inner();
	/// assert_eq!(inner.page, 1);
	/// assert_eq!(inner.per_page, 10);
	/// ```
	pub fn into_inner(self) -> T {
		self.0
	}
}

impl<T> Deref for Query<T> {
	type Target = T;

	fn deref(&self) -> &Self::Target {
		&self.0
	}
}

impl<T: Debug> Debug for Query<T> {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		self.0.fmt(f)
	}
}

impl<T: Clone> Clone for Query<T> {
	fn clone(&self) -> Self {
		Query(self.0.clone())
	}
}

#[cfg(feature = "multi-value-arrays")]
/// Parse query string supporting multiple values for the same key
/// Converts q=5&q=6 into {"q": ["5", "6"]}
fn parse_query_multi_value(query_string: &str) -> HashMap<String, Vec<String>> {
	let mut result: HashMap<String, Vec<String>> = HashMap::new();

	for (key, value) in form_urlencoded::parse(query_string.as_bytes()) {
		result
			.entry(key.into_owned())
			.or_default()
			.push(value.into_owned());
	}

	result
}

#[cfg(feature = "multi-value-arrays")]
/// Convert a string value to the most appropriate JSON value type
/// Tries number types first, then falls back to string
fn string_to_json_value(s: &str) -> serde_json::Value {
	// Try parsing as integer
	if let Ok(i) = s.parse::<i64>() {
		return serde_json::Value::Number(i.into());
	}
	// Try parsing as float
	if let Ok(f) = s.parse::<f64>()
		&& let Some(num) = serde_json::Number::from_f64(f)
	{
		return serde_json::Value::Number(num);
	}
	// Try parsing as boolean
	if let Ok(b) = s.parse::<bool>() {
		return serde_json::Value::Bool(b);
	}
	// Fall back to string
	serde_json::Value::String(s.to_string())
}

#[cfg(feature = "multi-value-arrays")]
/// Convert multi-value map to JSON for deserialization
/// This allows serde to properly deserialize arrays and type coercion
fn multi_value_to_json_value(multi_map: &HashMap<String, Vec<String>>) -> serde_json::Value {
	let mut result = serde_json::Map::new();

	for (key, values) in multi_map {
		let value = if values.len() == 1 {
			// Single value: convert to appropriate type
			string_to_json_value(&values[0])
		} else {
			// Multiple values: use as array with type conversion
			serde_json::Value::Array(values.iter().map(|v| string_to_json_value(v)).collect())
		};
		result.insert(key.clone(), value);
	}

	serde_json::Value::Object(result)
}

#[async_trait]
impl<T> FromRequest for Query<T>
where
	T: DeserializeOwned + Send,
{
	async fn from_request(req: &Request, _ctx: &ParamContext) -> ParamResult<Self> {
		// Extract query string from request
		let query_string = req.uri.query().unwrap_or("");

		// Deserialize query string to T
		// If multi-value-arrays feature is enabled, parse repeated parameters as arrays
		// (e.g., q=5&q=6 -> vec![5, 6])
		#[cfg(feature = "multi-value-arrays")]
		let result = {
			let multi_map = parse_query_multi_value(query_string);
			let json_value = multi_value_to_json_value(&multi_map);

			serde_json::from_value(json_value).map(Query).map_err(|e| {
				let raw_value = if query_string.is_empty() {
					None
				} else {
					Some(query_string.to_string())
				};
				let mut ctx = super::ParamErrorContext::new(ParamType::Query, e.to_string())
					.with_expected_type::<T>()
					.with_source(Box::new(e));
				if let Some(raw) = raw_value {
					ctx = ctx.with_raw_value(raw);
				}
				ParamError::InvalidParameter(Box::new(ctx))
			})
		};

		#[cfg(not(feature = "multi-value-arrays"))]
		let result = serde_urlencoded::from_str(query_string)
			.map(Query)
			.map_err(|e| {
				let raw_value = if query_string.is_empty() {
					None
				} else {
					Some(query_string.to_string())
				};
				ParamError::url_encoding::<T>(ParamType::Query, e, raw_value)
			});

		result
	}
}

// Implement WithValidation trait for Query
#[cfg(feature = "validation")]
impl<T> super::validation::WithValidation for Query<T> {}

impl<T> super::has_inner::HasInner for Query<T> {
	type Inner = T;

	fn inner_ref(&self) -> &T {
		&self.0
	}

	fn into_inner(self) -> T {
		self.0
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use serde::Deserialize;

	// Allow dead_code: fields are accessed via Deserialize derive, not directly in code
	#[allow(dead_code)]
	#[derive(Debug, Deserialize, PartialEq)]
	struct TestQuery {
		page: Option<i32>,
		limit: Option<i32>,
		search: Option<String>,
	}
}
