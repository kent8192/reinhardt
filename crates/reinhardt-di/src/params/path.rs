//! Path parameter extraction
//!
//! Extract typed values from URL path parameters.

use async_trait::async_trait;
use reinhardt_http::Request;
use serde::de::DeserializeOwned;
use std::fmt::{self, Debug};
use std::ops::Deref;

use super::{
	ParamContext, ParamError, ParamErrorContext, ParamResult, ParamType, extract::FromRequest,
};

/// Extract a single value from the URL path
///
/// # Example
///
/// ```rust
/// use reinhardt_di::params::Path;
///
/// let id = Path(42_i64);
/// let user_id: i64 = id.0; // or *id
/// assert_eq!(user_id, 42);
/// ```
pub struct Path<T>(pub T);

impl<T> Path<T> {
	/// Unwrap the Path and return the inner value
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_di::params::Path;
	///
	/// let path = Path(42i64);
	/// let inner = path.into_inner();
	/// assert_eq!(inner, 42);
	/// ```
	pub fn into_inner(self) -> T {
		self.0
	}
}

impl<T> Deref for Path<T> {
	type Target = T;

	fn deref(&self) -> &Self::Target {
		&self.0
	}
}

impl<T: Debug> Debug for Path<T> {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		self.0.fmt(f)
	}
}

impl<T: Clone> Clone for Path<T> {
	fn clone(&self) -> Self {
		Path(self.0.clone())
	}
}

// Macro to implement FromRequest for primitive types
// This allows extracting primitive types directly from path parameters
macro_rules! impl_path_from_str {
    ($($ty:ty),+ $(,)?) => {
        $(
            #[async_trait]
            impl FromRequest for Path<$ty> {
                async fn from_request(_req: &Request, ctx: &ParamContext) -> ParamResult<Self> {
                    // For primitive types, extract the single value directly
                    if ctx.path_params.len() != 1 {
                        return Err(ParamError::InvalidParameter(Box::new(
                            ParamErrorContext::new(
                                ParamType::Path,
                                format!(
                                    "Expected exactly 1 path parameter for primitive type, found {}",
                                    ctx.path_params.len()
                                ),
                            )
                            .with_expected_type::<$ty>(),
                        )));
                    }

                    let value = ctx.path_params.values().next().unwrap();
                    value.parse::<$ty>()
                        .map(Path)
                        .map_err(|e| {
                            ParamError::parse::<$ty>(
                                ParamType::Path,
                                format!("Failed to parse '{}' as {}: {}", value, stringify!($ty), e),
                                Box::new(std::io::Error::new(
                                    std::io::ErrorKind::InvalidData,
                                    e.to_string(),
                                )),
                            )
                        })
                }
            }
        )+
    };
}

// Implement for common primitive types
impl_path_from_str!(
	i8, i16, i32, i64, i128, isize, u8, u16, u32, u64, u128, usize, f32, f64, bool
);

// Implement for Uuid when the uuid feature is enabled
#[cfg(feature = "uuid")]
impl_path_from_str!(uuid::Uuid);

// Implementation for 2-tuple path parameters
// This enables extracting multiple path parameters like Path<(Uuid, Uuid)>
macro_rules! impl_path_tuple2_from_str {
    ($($t1:ty, $t2:ty);+ $(;)?) => {
        $(
            #[async_trait]
            impl FromRequest for Path<($t1, $t2)> {
                async fn from_request(_req: &Request, ctx: &ParamContext) -> ParamResult<Self> {
                    if ctx.path_params.len() != 2 {
                        return Err(ParamError::InvalidParameter(Box::new(
                            ParamErrorContext::new(
                                ParamType::Path,
                                format!(
                                    "Expected exactly 2 path parameters for tuple type, found {}",
                                    ctx.path_params.len()
                                ),
                            )
                            .with_expected_type::<($t1, $t2)>(),
                        )));
                    }

                    // Sort by key name to ensure deterministic extraction order
                    // regardless of HashMap iteration order
                    let mut sorted_params: Vec<_> = ctx.path_params.iter().collect();
                    sorted_params.sort_by_key(|(k, _)| k.clone());
                    let values: Vec<_> = sorted_params.into_iter().map(|(_, v)| v).collect();
                    if values.len() != 2 {
                        return Err(ParamError::InvalidParameter(Box::new(
                            ParamErrorContext::new(
                                ParamType::Path,
                                "Expected exactly 2 path parameters".to_string(),
                            )
                            .with_expected_type::<($t1, $t2)>(),
                        )));
                    }

                    let v1 = values[0].parse::<$t1>()
                        .map_err(|e| {
                            let ctx = ParamErrorContext::new(
                                ParamType::Path,
                                format!("Failed to parse '{}' as {}: {}", values[0], stringify!($t1), e),
                            )
                            .with_field("path[0]")
                            .with_expected_type::<$t1>()
                            .with_raw_value(values[0].as_str())
                            .with_source(Box::new(std::io::Error::new(
                                std::io::ErrorKind::InvalidData,
                                e.to_string(),
                            )));
                            ParamError::ParseError(Box::new(ctx))
                        })?;

                    let v2 = values[1].parse::<$t2>()
                        .map_err(|e| {
                            let ctx = ParamErrorContext::new(
                                ParamType::Path,
                                format!("Failed to parse '{}' as {}: {}", values[1], stringify!($t2), e),
                            )
                            .with_field("path[1]")
                            .with_expected_type::<$t2>()
                            .with_raw_value(values[1].as_str())
                            .with_source(Box::new(std::io::Error::new(
                                std::io::ErrorKind::InvalidData,
                                e.to_string(),
                            )));
                            ParamError::ParseError(Box::new(ctx))
                        })?;

                    Ok(Path((v1, v2)))
                }
            }
        )+
    };
}

// Common tuple combinations
impl_path_tuple2_from_str!(
	i64, i64;
	String, i64;
	i64, String;
	String, String
);

// Uuid tuple combinations when uuid feature is enabled
#[cfg(feature = "uuid")]
impl_path_tuple2_from_str!(
	uuid::Uuid, uuid::Uuid;
	uuid::Uuid, i64;
	i64, uuid::Uuid;
	uuid::Uuid, String;
	String, uuid::Uuid
);

// Special implementation for String (no parsing needed)
#[async_trait]
impl FromRequest for Path<String> {
	async fn from_request(_req: &Request, ctx: &ParamContext) -> ParamResult<Self> {
		if ctx.path_params.len() != 1 {
			return Err(ParamError::InvalidParameter(Box::new(
				ParamErrorContext::new(
					ParamType::Path,
					format!(
						"Expected exactly 1 path parameter for String, found {}",
						ctx.path_params.len()
					),
				)
				.with_expected_type::<String>(),
			)));
		}

		let value = ctx.path_params.values().next().unwrap().clone();
		Ok(Path(value))
	}
}

// Note: For complex types like enums, Vec, HashMap, etc., users should use
// a custom deserializer or validate that the type is not suitable for path parameters.
// We intentionally don't provide a generic DeserializeOwned impl to avoid conflicts
// with the FromStr-based implementations above.

/// PathStruct is a helper type for extracting structured path parameters
///
/// Use this when you need to extract multiple path parameters into a struct.
///
/// # Example
///
/// ```rust
/// use reinhardt_di::params::PathStruct;
/// # use serde::Deserialize;
/// #[derive(Deserialize)]
/// struct UserPath {
///     user_id: i64,
///     post_id: i64,
/// }
///
/// let user_path = UserPath { user_id: 123, post_id: 456 };
/// let path = PathStruct(user_path);
/// let user_id = path.user_id;
/// let post_id = path.post_id;
/// assert_eq!(user_id, 123);
/// assert_eq!(post_id, 456);
/// ```
pub struct PathStruct<T>(pub T);

impl<T> PathStruct<T> {
	/// Unwrap the PathStruct and return the inner value
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_di::params::PathStruct;
	/// use serde::Deserialize;
	///
	/// #[derive(Deserialize, Debug, PartialEq)]
	/// struct UserPath {
	///     user_id: i64,
	///     post_id: i64,
	/// }
	///
	/// let path = PathStruct(UserPath {
	///     user_id: 123,
	///     post_id: 456,
	/// });
	/// let inner = path.into_inner();
	/// assert_eq!(inner.user_id, 123);
	/// assert_eq!(inner.post_id, 456);
	/// ```
	pub fn into_inner(self) -> T {
		self.0
	}
}

impl<T> Deref for PathStruct<T> {
	type Target = T;
	fn deref(&self) -> &Self::Target {
		&self.0
	}
}

impl<T: Debug> Debug for PathStruct<T> {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		self.0.fmt(f)
	}
}

#[async_trait]
impl<T> FromRequest for PathStruct<T>
where
	T: DeserializeOwned + Send,
{
	async fn from_request(_req: &Request, ctx: &ParamContext) -> ParamResult<Self> {
		// Convert path params HashMap to URL-encoded format for deserialization
		// This enables proper type coercion from strings (e.g., "42" -> 42)
		let encoded = serde_urlencoded::to_string(&ctx.path_params).map_err(|e| {
			ParamError::ParseError(Box::new(
				ParamErrorContext::new(
					ParamType::Path,
					format!("Failed to encode path params: {}", e),
				)
				.with_expected_type::<T>()
				.with_source(Box::new(e)),
			))
		})?;

		serde_urlencoded::from_str(&encoded)
			.map(PathStruct)
			.map_err(|e| ParamError::url_encoding::<T>(ParamType::Path, e, Some(encoded.clone())))
	}
}

// Implement WithValidation trait for Path
#[cfg(feature = "validation")]
impl<T> super::validation::WithValidation for Path<T> {}

#[cfg(test)]
mod tests {
	use super::*;
	use std::collections::HashMap;

	#[tokio::test]
	async fn test_path_struct_params() {
		use bytes::Bytes;
		use hyper::{HeaderMap, Method, Version};
		use serde::Deserialize;

		#[derive(Debug, Deserialize, PartialEq)]
		struct PathParams {
			id: i64,
		}

		let mut params = HashMap::new();
		params.insert("id".to_string(), "42".to_string());

		let ctx = ParamContext::with_path_params(params);
		let req = Request::builder()
			.method(Method::GET)
			.uri("/test")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();

		let result = PathStruct::<PathParams>::from_request(&req, &ctx).await;
		assert!(result.is_ok());
		assert_eq!(result.unwrap().id, 42);
	}

	// Test primitive type extraction
	#[tokio::test]
	async fn test_path_primitive_i64() {
		use bytes::Bytes;
		use hyper::{HeaderMap, Method, Version};

		let mut params = HashMap::new();
		params.insert("id".to_string(), "42".to_string());

		let ctx = ParamContext::with_path_params(params);
		let req = Request::builder()
			.method(Method::GET)
			.uri("/test")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();

		let result = Path::<i64>::from_request(&req, &ctx).await;
		assert!(result.is_ok(), "Failed to extract i64: {:?}", result.err());
		assert_eq!(*result.unwrap(), 42);
	}

	#[tokio::test]
	async fn test_path_primitive_string() {
		use bytes::Bytes;
		use hyper::{HeaderMap, Method, Version};

		let mut params = HashMap::new();
		params.insert("name".to_string(), "foobar".to_string());

		let ctx = ParamContext::with_path_params(params);
		let req = Request::builder()
			.method(Method::GET)
			.uri("/test")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();

		let result = Path::<String>::from_request(&req, &ctx).await;
		assert!(
			result.is_ok(),
			"Failed to extract String: {:?}",
			result.err()
		);
		assert_eq!(*result.unwrap(), "foobar");
	}

	#[tokio::test]
	async fn test_path_primitive_f64() {
		use bytes::Bytes;
		use hyper::{HeaderMap, Method, Version};

		let mut params = HashMap::new();
		params.insert("price".to_string(), "19.99".to_string());

		let ctx = ParamContext::with_path_params(params);
		let req = Request::builder()
			.method(Method::GET)
			.uri("/test")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();

		let result = Path::<f64>::from_request(&req, &ctx).await;
		assert!(result.is_ok(), "Failed to extract f64: {:?}", result.err());
		assert_eq!(*result.unwrap(), 19.99);
	}

	#[tokio::test]
	async fn test_path_primitive_bool() {
		use bytes::Bytes;
		use hyper::{HeaderMap, Method, Version};

		let mut params = HashMap::new();
		params.insert("active".to_string(), "true".to_string());

		let ctx = ParamContext::with_path_params(params);
		let req = Request::builder()
			.method(Method::GET)
			.uri("/test")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();

		let result = Path::<bool>::from_request(&req, &ctx).await;
		assert!(result.is_ok(), "Failed to extract bool: {:?}", result.err());
		assert!(*result.unwrap());
	}

	#[tokio::test]
	async fn test_path_multiple_params_struct() {
		use bytes::Bytes;
		use hyper::{HeaderMap, Method, Version};
		use serde::Deserialize;

		#[derive(Debug, Deserialize, PartialEq)]
		struct MultiParams {
			user_id: i64,
			post_id: i64,
		}

		let mut params = HashMap::new();
		params.insert("user_id".to_string(), "123".to_string());
		params.insert("post_id".to_string(), "456".to_string());

		let ctx = ParamContext::with_path_params(params);
		let req = Request::builder()
			.method(Method::GET)
			.uri("/test")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();

		let result = PathStruct::<MultiParams>::from_request(&req, &ctx).await;
		let params = result.unwrap();
		assert_eq!(params.user_id, 123);
		assert_eq!(params.post_id, 456);
	}
}
