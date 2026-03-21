//! Path parameter extraction for typed route handlers.
//!
//! This module provides typed parameter extraction from URL paths,
//! similar to backend's `Path<T>` extractor.

use std::collections::HashMap;
use std::ops::Deref;

use super::error::PathError;

/// Context for parameter extraction.
///
/// Contains both named parameters (for backward compatibility)
/// and ordered parameter values (for tuple extraction).
#[derive(Debug, Clone)]
pub struct ParamContext {
	/// Named parameters extracted from the path.
	#[allow(dead_code)] // Reserved for future named parameter access
	pub(crate) params: HashMap<String, String>,
	/// Parameter values in the order they appear in the pattern.
	///
	/// This guarantees that tuple extraction works correctly by index,
	/// matching the order of parameters in the URL pattern.
	pub(crate) param_values: Vec<String>,
}

impl ParamContext {
	/// Creates a new parameter context.
	pub fn new(params: HashMap<String, String>, param_values: Vec<String>) -> Self {
		Self {
			params,
			param_values,
		}
	}

	/// Returns the number of parameters.
	pub fn len(&self) -> usize {
		self.param_values.len()
	}

	/// Returns whether there are no parameters.
	pub fn is_empty(&self) -> bool {
		self.param_values.is_empty()
	}
}

/// Trait for extracting typed values from path parameters.
///
/// This trait is similar to the backend's `FromRequest` trait,
/// but simplified for client-side routing (no async required).
pub trait FromPath: Sized {
	/// Extracts Self from the parameter context.
	///
	/// # Errors
	///
	/// Returns [`PathError::CountMismatch`] if the number of parameters doesn't match.
	/// Returns [`PathError::ParseError`] if parameter parsing fails.
	fn from_path(ctx: &ParamContext) -> Result<Self, PathError>;
}

/// Single path parameter extractor.
///
/// This is the primary type for extracting path parameters from URLs.
/// Use multiple `Path<T>` arguments for routes with multiple parameters.
///
/// # Example
///
/// ```ignore
/// use reinhardt_urls::routers::client_router::Path;
///
/// // Single parameter
/// fn user_detail(Path(id): Path<i64>) -> View {
///     user_page(id)
/// }
///
/// // Multiple parameters (use multiple Path arguments)
/// fn post_detail(Path(user_id): Path<i64>, Path(post_id): Path<i64>) -> View {
///     post_page(user_id, post_id)
/// }
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Path<T>(pub T);

impl<T> Path<T> {
	/// Unwraps the inner value.
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

impl<T> AsRef<T> for Path<T> {
	fn as_ref(&self) -> &T {
		&self.0
	}
}

// Macro for implementing FromPath for primitive types
macro_rules! impl_from_path_for_primitive {
	($($ty:ty => $type_name:expr),* $(,)?) => {
		$(
			impl FromPath for $ty {
				fn from_path(ctx: &ParamContext) -> Result<Self, PathError> {
					if ctx.param_values.len() != 1 {
						return Err(PathError::CountMismatch {
							expected: 1,
							actual: ctx.param_values.len(),
						});
					}

					ctx.param_values[0]
						.parse::<$ty>()
						.map_err(|e| PathError::ParseError {
							param_index: Some(0),
							param_type: $type_name,
							raw_value: ctx.param_values[0].clone(),
							source: format!("{}", e),
						})
				}
			}
		)*
	};
}

// Implement FromPath for common primitive types
impl_from_path_for_primitive! {
	i32 => "i32",
	i64 => "i64",
	u32 => "u32",
	u64 => "u64",
	bool => "bool",
}

// Special implementation for String (no parsing needed)
impl FromPath for String {
	fn from_path(ctx: &ParamContext) -> Result<Self, PathError> {
		if ctx.param_values.len() != 1 {
			return Err(PathError::CountMismatch {
				expected: 1,
				actual: ctx.param_values.len(),
			});
		}

		Ok(ctx.param_values[0].clone())
	}
}

// Implementation for Path<T>
impl<T: FromPath> FromPath for Path<T> {
	fn from_path(ctx: &ParamContext) -> Result<Self, PathError> {
		T::from_path(ctx).map(Path)
	}
}

/// Trait for extracting a single value at a specific index from path parameters.
///
/// This is used internally to support the multi-argument `Path<T>` style:
/// `|Path(user_id): Path<Uuid>, Path(post_id): Path<i64>|`
pub trait SingleFromPath: Sized {
	/// Extracts a single value at the given index.
	fn from_path_at(ctx: &ParamContext, index: usize) -> Result<Self, PathError>;
}

// Implement SingleFromPath for types that implement FromStr
impl<T> SingleFromPath for T
where
	T: std::str::FromStr,
	T::Err: std::fmt::Display,
{
	fn from_path_at(ctx: &ParamContext, index: usize) -> Result<Self, PathError> {
		if index >= ctx.param_values.len() {
			return Err(PathError::CountMismatch {
				expected: index + 1,
				actual: ctx.param_values.len(),
			});
		}

		ctx.param_values[index]
			.parse::<T>()
			.map_err(|e| PathError::ParseError {
				param_index: Some(index),
				param_type: std::any::type_name::<T>(),
				raw_value: ctx.param_values[index].clone(),
				source: format!("{}", e),
			})
	}
}

// Helper macro for parsing tuple elements
macro_rules! parse_tuple_element {
	($ctx:expr, $idx:expr, $ty:ty) => {{
		if $idx >= $ctx.param_values.len() {
			return Err(PathError::CountMismatch {
				expected: $idx + 1,
				actual: $ctx.param_values.len(),
			});
		}

		$ctx.param_values[$idx]
			.parse::<$ty>()
			.map_err(|e| PathError::ParseError {
				param_index: Some($idx),
				param_type: std::any::type_name::<$ty>(),
				raw_value: $ctx.param_values[$idx].clone(),
				source: format!("{}", e),
			})?
	}};
}

// Macro for implementing FromPath for tuples
macro_rules! impl_from_path_for_tuple {
	($($idx:tt => $ty:ident),+ $(,)?) => {
		impl<$($ty),+> FromPath for ($($ty,)+)
		where
			$($ty: std::str::FromStr,)+
			$(<$ty as std::str::FromStr>::Err: std::fmt::Display,)+
		{
			fn from_path(ctx: &ParamContext) -> Result<Self, PathError> {
				let expected_count = [$($idx),+].len();
				if ctx.param_values.len() != expected_count {
					return Err(PathError::CountMismatch {
						expected: expected_count,
						actual: ctx.param_values.len(),
					});
				}

				Ok((
					$(parse_tuple_element!(ctx, $idx, $ty),)+
				))
			}
		}
	};
}

// Implement FromPath for tuples of 2 to 6 elements
impl_from_path_for_tuple!(0 => A, 1 => B);
impl_from_path_for_tuple!(0 => A, 1 => B, 2 => C);
impl_from_path_for_tuple!(0 => A, 1 => B, 2 => C, 3 => D);
impl_from_path_for_tuple!(0 => A, 1 => B, 2 => C, 3 => D, 4 => E);
impl_from_path_for_tuple!(0 => A, 1 => B, 2 => C, 3 => D, 4 => E, 5 => F);

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_param_context_new() {
		let mut params = HashMap::new();
		params.insert("id".to_string(), "42".to_string());
		let param_values = vec!["42".to_string()];

		let ctx = ParamContext::new(params.clone(), param_values.clone());

		assert_eq!(ctx.params, params);
		assert_eq!(ctx.param_values, param_values);
		assert_eq!(ctx.len(), 1);
		assert!(!ctx.is_empty());
	}

	#[test]
	fn test_param_context_empty() {
		let ctx = ParamContext::new(HashMap::new(), Vec::new());

		assert_eq!(ctx.len(), 0);
		assert!(ctx.is_empty());
	}

	#[test]
	fn test_path_deref() {
		let params = Path(42i64);
		assert_eq!(*params, 42);
	}

	#[test]
	fn test_path_into_inner() {
		let params = Path("hello".to_string());
		assert_eq!(params.into_inner(), "hello");
	}

	#[test]
	fn test_path_as_ref() {
		let params = Path(42i64);
		let value: &i64 = params.as_ref();
		assert_eq!(*value, 42);
	}

	// FromPath implementation tests
	#[test]
	fn test_from_path_i32() {
		let mut params = HashMap::new();
		params.insert("id".to_string(), "42".to_string());
		let ctx = ParamContext::new(params, vec!["42".to_string()]);

		let result = i32::from_path(&ctx);
		assert!(result.is_ok());
		assert_eq!(result.unwrap(), 42);
	}

	#[test]
	fn test_from_path_i64() {
		let ctx = ParamContext::new(HashMap::new(), vec!["9223372036854775807".to_string()]);

		let result = i64::from_path(&ctx);
		assert!(result.is_ok());
		assert_eq!(result.unwrap(), 9223372036854775807);
	}

	#[test]
	fn test_from_path_u32() {
		let ctx = ParamContext::new(HashMap::new(), vec!["42".to_string()]);

		let result = u32::from_path(&ctx);
		assert!(result.is_ok());
		assert_eq!(result.unwrap(), 42);
	}

	#[test]
	fn test_from_path_u64() {
		let ctx = ParamContext::new(HashMap::new(), vec!["18446744073709551615".to_string()]);

		let result = u64::from_path(&ctx);
		assert!(result.is_ok());
		assert_eq!(result.unwrap(), 18446744073709551615);
	}

	#[test]
	fn test_from_path_bool() {
		let ctx_true = ParamContext::new(HashMap::new(), vec!["true".to_string()]);
		let result = bool::from_path(&ctx_true);
		assert!(result.is_ok());
		assert!(result.unwrap());

		let ctx_false = ParamContext::new(HashMap::new(), vec!["false".to_string()]);
		let result = bool::from_path(&ctx_false);
		assert!(result.is_ok());
		assert!(!result.unwrap());
	}

	#[test]
	fn test_from_path_string() {
		let ctx = ParamContext::new(HashMap::new(), vec!["hello".to_string()]);

		let result = String::from_path(&ctx);
		assert!(result.is_ok());
		assert_eq!(result.unwrap(), "hello");
	}

	#[test]
	fn test_from_path_parse_error() {
		let ctx = ParamContext::new(HashMap::new(), vec!["not_a_number".to_string()]);

		let result = i32::from_path(&ctx);
		assert!(result.is_err());

		match result {
			Err(PathError::ParseError {
				param_index,
				param_type,
				raw_value,
				..
			}) => {
				assert_eq!(param_index, Some(0));
				assert_eq!(param_type, "i32");
				assert_eq!(raw_value, "not_a_number");
			}
			_ => panic!("Expected ParseError"),
		}
	}

	#[test]
	fn test_from_path_count_mismatch() {
		let ctx = ParamContext::new(HashMap::new(), vec!["42".to_string(), "43".to_string()]);

		let result = i32::from_path(&ctx);
		assert!(result.is_err());

		match result {
			Err(PathError::CountMismatch { expected, actual }) => {
				assert_eq!(expected, 1);
				assert_eq!(actual, 2);
			}
			_ => panic!("Expected CountMismatch"),
		}
	}

	#[test]
	fn test_path_from_path() {
		let ctx = ParamContext::new(HashMap::new(), vec!["42".to_string()]);

		let result = Path::<i32>::from_path(&ctx);
		assert!(result.is_ok());
		assert_eq!(result.unwrap().0, 42);
	}

	// Tuple FromPath implementation tests
	#[test]
	fn test_from_path_tuple_2() {
		let ctx = ParamContext::new(HashMap::new(), vec!["42".to_string(), "hello".to_string()]);

		let result = <(i32, String)>::from_path(&ctx);
		assert!(result.is_ok());
		let (a, b) = result.unwrap();
		assert_eq!(a, 42);
		assert_eq!(b, "hello");
	}

	#[test]
	fn test_from_path_tuple_3() {
		let ctx = ParamContext::new(
			HashMap::new(),
			vec!["42".to_string(), "true".to_string(), "100".to_string()],
		);

		let result = <(i32, bool, u32)>::from_path(&ctx);
		assert!(result.is_ok());
		let (a, b, c) = result.unwrap();
		assert_eq!(a, 42);
		assert!(b);
		assert_eq!(c, 100);
	}

	#[test]
	fn test_from_path_tuple_mixed_types() {
		let ctx = ParamContext::new(
			HashMap::new(),
			vec![
				"123".to_string(),
				"456".to_string(),
				"test".to_string(),
				"true".to_string(),
			],
		);

		let result = <(i64, u64, String, bool)>::from_path(&ctx);
		assert!(result.is_ok());
		let (a, b, c, d) = result.unwrap();
		assert_eq!(a, 123);
		assert_eq!(b, 456);
		assert_eq!(c, "test");
		assert!(d);
	}

	#[test]
	fn test_from_path_tuple_count_mismatch() {
		let ctx = ParamContext::new(HashMap::new(), vec!["42".to_string()]);

		let result = <(i32, String)>::from_path(&ctx);
		assert!(result.is_err());

		match result {
			Err(PathError::CountMismatch { expected, actual }) => {
				assert_eq!(expected, 2);
				assert_eq!(actual, 1);
			}
			_ => panic!("Expected CountMismatch"),
		}
	}

	#[test]
	fn test_from_path_tuple_parse_error() {
		let ctx = ParamContext::new(
			HashMap::new(),
			vec!["not_a_number".to_string(), "hello".to_string()],
		);

		let result = <(i32, String)>::from_path(&ctx);
		assert!(result.is_err());

		match result {
			Err(PathError::ParseError {
				param_index,
				raw_value,
				..
			}) => {
				assert_eq!(param_index, Some(0));
				assert_eq!(raw_value, "not_a_number");
			}
			_ => panic!("Expected ParseError"),
		}
	}

	#[test]
	fn test_from_path_tuple_6_elements() {
		let ctx = ParamContext::new(
			HashMap::new(),
			vec![
				"1".to_string(),
				"2".to_string(),
				"3".to_string(),
				"4".to_string(),
				"5".to_string(),
				"6".to_string(),
			],
		);

		let result = <(i32, i32, i32, i32, i32, i32)>::from_path(&ctx);
		assert!(result.is_ok());
		let (a, b, c, d, e, f) = result.unwrap();
		assert_eq!((a, b, c, d, e, f), (1, 2, 3, 4, 5, 6));
	}
}
