//! Builder pattern for creating association proxies

use crate::proxy::AssociationProxy;
use std::marker::PhantomData;

/// Type alias for getter function
pub type GetterFn<T, U> = fn(&T) -> Result<U, crate::proxy::ProxyError>;

/// Type alias for setter function
pub type SetterFn<T, U> = fn(&mut T, U) -> Result<(), crate::proxy::ProxyError>;

/// Type alias for validator function
pub type ValidatorFn<U> = fn(&U) -> Result<(), crate::proxy::ProxyError>;

/// Builder for creating association proxies with fluent API
///
/// ## Example
///
/// ```rust
/// # use reinhardt_urls::proxy::ProxyBuilder;
/// # #[derive(Clone)]
/// # struct UserKeyword { keyword: String }
/// # impl UserKeyword {
/// #     fn new(keyword: String) -> Self { UserKeyword { keyword } }
/// # }
/// let proxy = ProxyBuilder::new()
///     .relationship("user_keywords")
///     .attribute("keyword")
///     .creator(|keyword: String| UserKeyword::new(keyword))
///     .build();
/// assert_eq!(proxy.relationship, "user_keywords");
/// assert_eq!(proxy.attribute, "keyword");
/// ```
pub struct ProxyBuilder<T, U> {
	name: Option<String>,
	relationship: Option<String>,
	attribute: Option<String>,
	creator: Option<fn(U) -> T>,
	getter: Option<GetterFn<T, U>>,
	setter: Option<SetterFn<T, U>>,
	validator: Option<ValidatorFn<U>>,
	transform: Option<fn(U) -> U>,
	_phantom: PhantomData<(T, U)>,
}

impl<T, U> Default for ProxyBuilder<T, U> {
	fn default() -> Self {
		Self::new()
	}
}

impl<T, U> ProxyBuilder<T, U> {
	/// Create a new proxy builder without a name
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_urls::proxy::ProxyBuilder;
	///
	/// let builder: ProxyBuilder<(), ()> = ProxyBuilder::new();
	/// // Builder is ready to configure
	/// ```
	pub fn new() -> Self {
		Self {
			name: None,
			relationship: None,
			attribute: None,
			creator: None,
			getter: None,
			setter: None,
			validator: None,
			transform: None,
			_phantom: PhantomData,
		}
	}

	/// Create a new proxy builder with a name
	///
	/// This is useful for creating named proxy aliases.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_urls::proxy::ProxyBuilder;
	///
	/// let builder: ProxyBuilder<(), ()> = ProxyBuilder::with_name("keyword_strings");
	/// ```
	pub fn with_name(name: &str) -> Self {
		Self {
			name: Some(name.to_string()),
			relationship: None,
			attribute: None,
			creator: None,
			getter: None,
			setter: None,
			validator: None,
			transform: None,
			_phantom: PhantomData,
		}
	}
	/// Set the relationship name
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_urls::proxy::ProxyBuilder;
	///
	/// let builder: ProxyBuilder<(), ()> = ProxyBuilder::new()
	///     .relationship("posts");
	/// // Builder now has relationship set
	/// ```
	pub fn relationship(mut self, name: &str) -> Self {
		self.relationship = Some(name.to_string());
		self
	}
	/// Set the attribute name
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_urls::proxy::ProxyBuilder;
	///
	/// let builder: ProxyBuilder<(), ()> = ProxyBuilder::new()
	///     .relationship("posts")
	///     .attribute("title");
	/// // Builder now has both relationship and attribute set
	/// ```
	pub fn attribute(mut self, name: &str) -> Self {
		self.attribute = Some(name.to_string());
		self
	}
	/// Set the creator function
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_urls::proxy::ProxyBuilder;
	///
	/// fn create_item(value: i32) -> String { format!("item_{}", value) }
	///
	/// let builder = ProxyBuilder::new()
	///     .relationship("items")
	///     .attribute("value")
	///     .creator(create_item);
	/// // Builder now has creator function set
	/// ```
	pub fn creator(mut self, creator: fn(U) -> T) -> Self {
		self.creator = Some(creator);
		self
	}

	/// Set the relationship name (alias for `relationship()`)
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_urls::proxy::ProxyBuilder;
	///
	/// let builder: ProxyBuilder<(), ()> = ProxyBuilder::new()
	///     .for_relationship("posts");
	/// ```
	pub fn for_relationship(self, name: &str) -> Self {
		self.relationship(name)
	}

	/// Set a custom getter function
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_urls::proxy::ProxyBuilder;
	///
	/// fn custom_getter(_obj: &()) -> Result<(), reinhardt_urls::proxy::ProxyError> {
	///     Ok(())
	/// }
	///
	/// let builder: ProxyBuilder<(), ()> = ProxyBuilder::new()
	///     .for_relationship("data")
	///     .attribute("value")
	///     .with_getter(custom_getter);
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
	/// use reinhardt_urls::proxy::ProxyBuilder;
	///
	/// fn custom_setter(_obj: &mut (), _value: ()) -> Result<(), reinhardt_urls::proxy::ProxyError> {
	///     Ok(())
	/// }
	///
	/// let builder: ProxyBuilder<(), ()> = ProxyBuilder::new()
	///     .for_relationship("data")
	///     .attribute("value")
	///     .with_setter(custom_setter);
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
	/// use reinhardt_urls::proxy::{ProxyBuilder, ProxyError};
	///
	/// fn validate_value(_value: &()) -> Result<(), ProxyError> {
	///     Ok(())
	/// }
	///
	/// let builder: ProxyBuilder<(), ()> = ProxyBuilder::new()
	///     .for_relationship("data")
	///     .attribute("value")
	///     .with_validator(validate_value);
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
	/// use reinhardt_urls::proxy::ProxyBuilder;
	///
	/// fn transform_value(value: ()) -> () {
	///     value
	/// }
	///
	/// let builder: ProxyBuilder<(), ()> = ProxyBuilder::new()
	///     .for_relationship("data")
	///     .attribute("value")
	///     .with_transform(transform_value);
	/// ```
	pub fn with_transform(mut self, transform: fn(U) -> U) -> Self {
		self.transform = Some(transform);
		self
	}

	/// Check if custom accessors (getter/setter) are configured
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_urls::proxy::ProxyBuilder;
	///
	/// fn custom_getter(_obj: &()) -> Result<(), reinhardt_urls::proxy::ProxyError> {
	///     Ok(())
	/// }
	///
	/// let builder: ProxyBuilder<(), ()> = ProxyBuilder::new()
	///     .with_getter(custom_getter);
	/// assert!(builder.has_custom_accessors());
	/// ```
	pub fn has_custom_accessors(&self) -> bool {
		self.getter.is_some() || self.setter.is_some()
	}

	/// Check if a validator is configured
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_urls::proxy::{ProxyBuilder, ProxyError};
	///
	/// fn validate(_value: &()) -> Result<(), ProxyError> {
	///     Ok(())
	/// }
	///
	/// let builder: ProxyBuilder<(), ()> = ProxyBuilder::new()
	///     .with_validator(validate);
	/// assert!(builder.has_validator());
	/// ```
	pub fn has_validator(&self) -> bool {
		self.validator.is_some()
	}

	/// Check if a transform function is configured
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_urls::proxy::ProxyBuilder;
	///
	/// fn transform(value: ()) -> () { value }
	///
	/// let builder: ProxyBuilder<(), ()> = ProxyBuilder::new()
	///     .with_transform(transform);
	/// assert!(builder.has_transform());
	/// ```
	pub fn has_transform(&self) -> bool {
		self.transform.is_some()
	}

	/// Get the builder name if set
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_urls::proxy::ProxyBuilder;
	///
	/// let builder: ProxyBuilder<(), ()> = ProxyBuilder::with_name("my_alias");
	/// assert_eq!(builder.name(), Some("my_alias"));
	/// ```
	pub fn name(&self) -> Option<&str> {
		self.name.as_deref()
	}

	/// Get the relationship name
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_urls::proxy::ProxyBuilder;
	///
	/// let builder: ProxyBuilder<(), ()> = ProxyBuilder::new()
	///     .relationship("posts");
	/// assert_eq!(builder.get_relationship(), Some("posts"));
	/// ```
	pub fn get_relationship(&self) -> Option<&str> {
		self.relationship.as_deref()
	}

	/// Get the attribute name
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_urls::proxy::ProxyBuilder;
	///
	/// let builder: ProxyBuilder<(), ()> = ProxyBuilder::new()
	///     .attribute("title");
	/// assert_eq!(builder.get_attribute(), Some("title"));
	/// ```
	pub fn get_attribute(&self) -> Option<&str> {
		self.attribute.as_deref()
	}

	/// Build the association proxy
	///
	/// # Panics
	///
	/// Panics if relationship or attribute is not set
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_urls::proxy::ProxyBuilder;
	///
	/// let proxy: reinhardt_urls::proxy::AssociationProxy<(), ()> = ProxyBuilder::new()
	///     .relationship("posts")
	///     .attribute("title")
	///     .build();
	/// assert_eq!(proxy.relationship, "posts");
	/// assert_eq!(proxy.attribute, "title");
	/// ```
	pub fn build(self) -> AssociationProxy<T, U> {
		let relationship = self.relationship.expect("Relationship must be set");
		let attribute = self.attribute.expect("Attribute must be set");

		let mut proxy = AssociationProxy::new(&relationship, &attribute);
		if let Some(creator) = self.creator {
			proxy = proxy.with_creator(creator);
		}
		proxy
	}
	/// Build the association proxy, returning None if configuration is incomplete
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_urls::proxy::ProxyBuilder;
	///
	/// // Complete configuration
	/// let proxy: Option<reinhardt_urls::proxy::AssociationProxy<(), ()>> = ProxyBuilder::new()
	///     .relationship("posts")
	///     .attribute("title")
	///     .try_build();
	/// assert!(proxy.is_some());
	///
	/// // Incomplete configuration
	/// let incomplete: Option<reinhardt_urls::proxy::AssociationProxy<(), ()>> = ProxyBuilder::new()
	///     .relationship("posts")
	///     .try_build();
	/// assert!(incomplete.is_none());
	/// ```
	pub fn try_build(self) -> Option<AssociationProxy<T, U>> {
		let relationship = self.relationship?;
		let attribute = self.attribute?;

		let mut proxy = AssociationProxy::new(&relationship, &attribute);
		if let Some(creator) = self.creator {
			proxy = proxy.with_creator(creator);
		}
		Some(proxy)
	}
}

/// Helper function to create a simple association proxy
///
/// ## Example
///
/// ```rust,no_run
/// # use reinhardt_urls::proxy::{AssociationProxy, association_proxy};
/// # struct UserKeyword;
/// let proxy: AssociationProxy<UserKeyword, String> = association_proxy("user_keywords", "keyword");
/// ```
pub fn association_proxy<T, U>(relationship: &str, attribute: &str) -> AssociationProxy<T, U> {
	AssociationProxy::new(relationship, attribute)
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_proxy_builder_basic_unit() {
		let proxy: AssociationProxy<(), ()> = ProxyBuilder::new()
			.relationship("rel")
			.attribute("attr")
			.build();

		assert_eq!(proxy.relationship, "rel");
		assert_eq!(proxy.attribute, "attr");
	}
}
