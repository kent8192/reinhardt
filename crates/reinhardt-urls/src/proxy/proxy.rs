//! Core association proxy implementation

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::marker::PhantomData;

use crate::proxy::builder::{GetterFn, SetterFn, ValidatorFn};
use crate::proxy::{ProxyError, ProxyResult};

/// Association proxy for transparent access to related object attributes
///
/// ## Example
///
/// ```rust,no_run
/// # use reinhardt_urls::proxy::AssociationProxy;
/// # #[derive(Clone)]
/// # struct UserKeyword;
/// # #[derive(Clone)]
/// # struct User;
/// # #[tokio::main]
/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
/// # let user = User;
/// // Access keyword names through user_keywords relationship
/// let proxy: AssociationProxy<UserKeyword, String> = AssociationProxy::new("user_keywords", "keyword");
/// // let names = proxy.get_collection(&user).await?;
/// # Ok(())
/// # }
/// ```
pub struct AssociationProxy<T, U> {
	/// Optional name/alias for this proxy
	pub name: Option<String>,

	/// Name of the relationship attribute
	pub relationship: String,

	/// Name of the attribute on the related object
	pub attribute: String,

	/// Optional creator function for new associations
	pub creator: Option<fn(U) -> T>,

	/// Optional custom getter function
	pub getter: Option<GetterFn<T, U>>,

	/// Optional custom setter function
	pub setter: Option<SetterFn<T, U>>,

	/// Optional validator function
	pub validator: Option<ValidatorFn<U>>,

	/// Optional transform function
	pub transform: Option<fn(U) -> U>,

	/// Phantom data for type parameters
	_phantom: PhantomData<(T, U)>,
}

impl<T, U> AssociationProxy<T, U> {
	/// Create a new association proxy
	///
	/// # Arguments
	///
	/// * `relationship` - Name of the relationship to traverse
	/// * `attribute` - Name of the attribute to access on related objects
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_urls::proxy::AssociationProxy;
	///
	/// let proxy: AssociationProxy<(), ()> = AssociationProxy::new("user_keywords", "keyword");
	/// assert_eq!(proxy.relationship, "user_keywords");
	/// assert_eq!(proxy.attribute, "keyword");
	/// ```
	pub fn new(relationship: &str, attribute: &str) -> Self {
		Self {
			name: None,
			relationship: relationship.to_string(),
			attribute: attribute.to_string(),
			creator: None,
			getter: None,
			setter: None,
			validator: None,
			transform: None,
			_phantom: PhantomData,
		}
	}
	/// Set a creator function for new associations
	///
	/// The creator function is called when adding new items to the association.
	/// It should create an association object from the target value.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_urls::proxy::AssociationProxy;
	///
	/// fn create_association(value: String) -> i32 { 42 }
	///
	/// let proxy = AssociationProxy::new("items", "value")
	///     .with_creator(create_association);
	/// assert!(proxy.creator.is_some());
	/// ```
	pub fn with_creator(mut self, creator: fn(U) -> T) -> Self {
		self.creator = Some(creator);
		self
	}

	/// Set a custom getter function
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_urls::proxy::AssociationProxy;
	///
	/// fn custom_getter(_obj: &()) -> Result<(), reinhardt_urls::proxy::ProxyError> {
	///     Ok(())
	/// }
	///
	/// let proxy = AssociationProxy::new("data", "value")
	///     .with_getter(custom_getter);
	/// assert!(proxy.getter.is_some());
	/// ```
	pub fn with_getter(mut self, getter: fn(&T) -> Result<U, crate::proxy::ProxyError>) -> Self {
		self.getter = Some(getter);
		self
	}

	/// Set a custom setter function
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_urls::proxy::AssociationProxy;
	///
	/// fn custom_setter(_obj: &mut (), _value: ()) -> Result<(), reinhardt_urls::proxy::ProxyError> {
	///     Ok(())
	/// }
	///
	/// let proxy = AssociationProxy::new("data", "value")
	///     .with_setter(custom_setter);
	/// assert!(proxy.setter.is_some());
	/// ```
	pub fn with_setter(
		mut self,
		setter: fn(&mut T, U) -> Result<(), crate::proxy::ProxyError>,
	) -> Self {
		self.setter = Some(setter);
		self
	}

	/// Set a validator function
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_urls::proxy::{AssociationProxy, ProxyError};
	///
	/// fn validate_value(_value: &()) -> Result<(), ProxyError> {
	///     Ok(())
	/// }
	///
	/// let proxy: AssociationProxy<(), ()> = AssociationProxy::new("data", "value")
	///     .with_validator(validate_value);
	/// assert!(proxy.validator.is_some());
	/// ```
	pub fn with_validator(
		mut self,
		validator: fn(&U) -> Result<(), crate::proxy::ProxyError>,
	) -> Self {
		self.validator = Some(validator);
		self
	}

	/// Set a transform function
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_urls::proxy::AssociationProxy;
	///
	/// fn transform_value(value: ()) -> () {
	///     value
	/// }
	///
	/// let proxy: AssociationProxy<(), ()> = AssociationProxy::new("data", "value")
	///     .with_transform(transform_value);
	/// assert!(proxy.transform.is_some());
	/// ```
	pub fn with_transform(mut self, transform: fn(U) -> U) -> Self {
		self.transform = Some(transform);
		self
	}

	/// Get the proxy name if set
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_urls::proxy::AssociationProxy;
	///
	/// let mut proxy: AssociationProxy<(), ()> = AssociationProxy::new("rel", "attr");
	/// proxy.name = Some("my_proxy".to_string());
	/// assert_eq!(proxy.name(), Some("my_proxy"));
	/// ```
	pub fn name(&self) -> Option<&str> {
		self.name.as_deref()
	}

	/// Get the relationship name
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_urls::proxy::AssociationProxy;
	///
	/// let proxy: AssociationProxy<(), ()> = AssociationProxy::new("posts", "title");
	/// assert_eq!(proxy.relationship(), "posts");
	/// ```
	pub fn relationship(&self) -> &str {
		&self.relationship
	}

	/// Get the attribute name
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_urls::proxy::AssociationProxy;
	///
	/// let proxy: AssociationProxy<(), ()> = AssociationProxy::new("posts", "title");
	/// assert_eq!(proxy.attribute(), "title");
	/// ```
	pub fn attribute(&self) -> &str {
		&self.attribute
	}

	/// Check if custom accessors (getter/setter) are configured
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_urls::proxy::AssociationProxy;
	///
	/// fn custom_getter(_obj: &()) -> Result<(), reinhardt_urls::proxy::ProxyError> {
	///     Ok(())
	/// }
	///
	/// let proxy = AssociationProxy::new("data", "value")
	///     .with_getter(custom_getter);
	/// assert!(proxy.has_custom_accessors());
	/// ```
	pub fn has_custom_accessors(&self) -> bool {
		self.getter.is_some() || self.setter.is_some()
	}

	/// Check if a validator is configured
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_urls::proxy::{AssociationProxy, ProxyError};
	///
	/// fn validate(_value: &()) -> Result<(), ProxyError> {
	///     Ok(())
	/// }
	///
	/// let proxy: AssociationProxy<(), ()> = AssociationProxy::new("data", "value")
	///     .with_validator(validate);
	/// assert!(proxy.has_validator());
	/// ```
	pub fn has_validator(&self) -> bool {
		self.validator.is_some()
	}

	/// Check if a transform function is configured
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_urls::proxy::AssociationProxy;
	///
	/// fn transform(value: () ) -> () { value }
	///
	/// let proxy: AssociationProxy<(), ()> = AssociationProxy::new("data", "value")
	///     .with_transform(transform);
	/// assert!(proxy.has_transform());
	/// ```
	pub fn has_transform(&self) -> bool {
		self.transform.is_some()
	}
}

/// Trait for accessing proxy targets
#[async_trait]
pub trait ProxyAccessor<T> {
	/// Get the target value(s) from the source object
	async fn get(&self, source: &T) -> ProxyResult<ProxyTarget>;

	/// Set the target value(s) on the source object
	async fn set(&self, source: &mut T, value: ProxyTarget) -> ProxyResult<()>;
}

/// Represents the target of a proxy operation
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ProxyTarget {
	/// Single scalar value
	Scalar(ScalarValue),

	/// Collection of values
	Collection(Vec<ScalarValue>),

	/// No value (None)
	None,
}

/// Scalar value types supported by proxies
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum ScalarValue {
	/// A string value.
	String(String),
	/// A 64-bit integer value.
	Integer(i64),
	/// A 64-bit floating-point value.
	Float(f64),
	/// A boolean value.
	Boolean(bool),
	/// A null (absent) value.
	Null,
}

impl From<String> for ScalarValue {
	fn from(s: String) -> Self {
		ScalarValue::String(s)
	}
}

impl From<&str> for ScalarValue {
	fn from(s: &str) -> Self {
		ScalarValue::String(s.to_string())
	}
}

impl From<i64> for ScalarValue {
	fn from(i: i64) -> Self {
		ScalarValue::Integer(i)
	}
}

impl From<f64> for ScalarValue {
	fn from(f: f64) -> Self {
		ScalarValue::Float(f)
	}
}

impl From<bool> for ScalarValue {
	fn from(b: bool) -> Self {
		ScalarValue::Boolean(b)
	}
}

impl ScalarValue {
	/// Try to convert to String
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_urls::proxy::ScalarValue;
	///
	/// let value = ScalarValue::String("hello".to_string());
	/// assert_eq!(value.as_string().unwrap(), "hello");
	///
	/// let int_value = ScalarValue::Integer(42);
	/// assert!(int_value.as_string().is_err());
	/// ```
	pub fn as_string(&self) -> ProxyResult<String> {
		match self {
			ScalarValue::String(s) => Ok(s.clone()),
			_ => Err(ProxyError::TypeMismatch {
				expected: "String".to_string(),
				actual: format!("{:?}", self),
			}),
		}
	}
	/// Try to convert to i64
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_urls::proxy::ScalarValue;
	///
	/// let value = ScalarValue::Integer(42);
	/// assert_eq!(value.as_integer().unwrap(), 42);
	///
	/// let str_value = ScalarValue::String("test".to_string());
	/// assert!(str_value.as_integer().is_err());
	/// ```
	pub fn as_integer(&self) -> ProxyResult<i64> {
		match self {
			ScalarValue::Integer(i) => Ok(*i),
			_ => Err(ProxyError::TypeMismatch {
				expected: "Integer".to_string(),
				actual: format!("{:?}", self),
			}),
		}
	}
	/// Try to convert to f64
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_urls::proxy::ScalarValue;
	///
	/// let value = ScalarValue::Float(3.15);
	/// assert_eq!(value.as_float().unwrap(), 3.15);
	///
	/// let bool_value = ScalarValue::Boolean(true);
	/// assert!(bool_value.as_float().is_err());
	/// ```
	pub fn as_float(&self) -> ProxyResult<f64> {
		match self {
			ScalarValue::Float(f) => Ok(*f),
			_ => Err(ProxyError::TypeMismatch {
				expected: "Float".to_string(),
				actual: format!("{:?}", self),
			}),
		}
	}
	/// Try to convert to bool
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_urls::proxy::ScalarValue;
	///
	/// let value = ScalarValue::Boolean(true);
	/// assert!(value.as_boolean().unwrap());
	///
	/// let int_value = ScalarValue::Integer(1);
	/// assert!(int_value.as_boolean().is_err());
	/// ```
	pub fn as_boolean(&self) -> ProxyResult<bool> {
		match self {
			ScalarValue::Boolean(b) => Ok(*b),
			_ => Err(ProxyError::TypeMismatch {
				expected: "Boolean".to_string(),
				actual: format!("{:?}", self),
			}),
		}
	}
	/// Check if value is null
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_urls::proxy::ScalarValue;
	///
	/// let null_value = ScalarValue::Null;
	/// assert!(null_value.is_null());
	///
	/// let string_value = ScalarValue::String("test".to_string());
	/// assert!(!string_value.is_null());
	/// ```
	pub fn is_null(&self) -> bool {
		matches!(self, ScalarValue::Null)
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_proxy_scalar_conversions_unit() {
		let s = ScalarValue::String("test".to_string());
		assert_eq!(s.as_string().unwrap(), "test");

		let i = ScalarValue::Integer(42);
		assert_eq!(i.as_integer().unwrap(), 42);

		let f = ScalarValue::Float(3.15);
		assert_eq!(f.as_float().unwrap(), 3.15);

		let b = ScalarValue::Boolean(true);
		assert!(b.as_boolean().unwrap());
	}

	#[test]
	fn test_proxy_scalar_type_mismatch_unit() {
		let s = ScalarValue::String("test".to_string());
		assert!(s.as_integer().is_err());
	}
}
