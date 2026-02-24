//! HyperlinkedModelSerializer - Django REST Framework inspired hyperlinked serialization
//!
//! This module provides HyperlinkedModelSerializer that automatically generates
//! URL fields for model instances, enabling HATEOAS-style REST APIs.

use crate::{Serializer, SerializerError};
use reinhardt_db::orm::Model;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::marker::PhantomData;
use std::sync::Arc;

/// Trait for URL reversal functionality
///
/// Implement this trait to provide URL reversal for HyperlinkedModelSerializer.
/// This abstraction allows integration with reinhardt-routers without creating
/// circular dependencies.
///
/// # Example
///
/// ```rust,no_run,ignore
/// use std::collections::HashMap;
/// use reinhardt_rest::serializers::UrlReverser;
/// use reinhardt_urls::routers::UrlReverser as RouterUrlReverser;
///
/// impl UrlReverser for RouterUrlReverser {
///     fn reverse(&self, name: &str, params: &HashMap<String, String>) -> Result<String, String> {
///         self.reverse(name, params).map_err(|e| e.to_string())
///     }
/// }
/// ```
pub trait UrlReverser: Send + Sync {
	/// Reverse a URL name to a path with parameters
	///
	/// # Arguments
	///
	/// * `name` - The route name
	/// * `params` - Map of parameter names to values
	///
	/// # Returns
	///
	/// The resolved URL path or an error message
	fn reverse(&self, name: &str, params: &HashMap<String, String>) -> Result<String, String>;
}

/// HyperlinkedModelSerializer provides automatic URL generation for ORM models
///
/// Inspired by Django REST Framework's HyperlinkedModelSerializer, this extends
/// ModelSerializer by automatically adding a URL field that points to the detail
/// view of the instance.
///
/// # Examples
///
/// ```no_run
/// # use reinhardt_rest::serializers::{HyperlinkedModelSerializer, UrlReverser, Serializer};
/// # use reinhardt_db::orm::Model;
/// # use serde::{Serialize, Deserialize};
/// # use std::sync::Arc;
/// # use std::collections::HashMap;
/// #
/// # #[derive(Debug, Clone, Serialize, Deserialize)]
/// # struct User {
/// #     id: Option<i64>,
/// #     username: String,
/// #     email: String,
/// # }
/// #
/// # impl Model for User {
/// #     type PrimaryKey = i64;
/// #     type Fields = UserFields;
/// #     fn table_name() -> &'static str { "users" }
/// #     fn primary_key(&self) -> Option<Self::PrimaryKey> { self.id }
/// #     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
/// #     fn new_fields() -> Self::Fields { UserFields }
/// # }
/// # #[derive(Clone)]
/// # struct UserFields;
/// # impl reinhardt_db::orm::FieldSelector for UserFields {
/// #     fn with_alias(self, _alias: &str) -> Self { self }
/// # }
/// #
/// # struct MyUrlReverser;
/// # impl UrlReverser for MyUrlReverser {
/// #     fn reverse(&self, name: &str, params: &HashMap<String, String>) -> Result<String, String> {
/// #         Ok(format!("/users/{}/", params.get("id").unwrap()))
/// #     }
/// # }
/// #
/// # fn example() {
/// let reverser: Arc<dyn UrlReverser> = Arc::new(MyUrlReverser);
/// let serializer = HyperlinkedModelSerializer::<User>::new("user-detail", Some(reverser));
///
/// let user = User {
///     id: Some(1),
///     username: "alice".to_string(),
///     email: "alice@example.com".to_string(),
/// };
///
/// // The serialized JSON will include a "url" field
/// let json = serializer.serialize(&user).unwrap();
/// // Output: {"id":1,"username":"alice","email":"alice@example.com","url":"/users/1/"}
/// # }
/// ```
pub struct HyperlinkedModelSerializer<M: Model> {
	view_name: String,
	url_field_name: String,
	url_reverser: Option<Arc<dyn UrlReverser>>,
	_phantom: PhantomData<M>,
}

impl<M: Model> HyperlinkedModelSerializer<M> {
	/// Create a new HyperlinkedModelSerializer instance
	///
	/// # Arguments
	///
	/// * `view_name` - The name of the detail view for URL generation
	/// * `url_reverser` - Optional URL reverser for proper URL resolution. If None,
	///   falls back to simple path-based URL generation.
	///
	/// # Examples
	///
	/// ```
	/// # use reinhardt_rest::serializers::{HyperlinkedModelSerializer, UrlReverser};
	/// # use reinhardt_db::orm::Model;
	/// # use serde::{Serialize, Deserialize};
	/// # use std::sync::Arc;
	/// # use std::collections::HashMap;
	/// #
	/// # #[derive(Debug, Clone, Serialize, Deserialize)]
	/// # struct User {
	/// #     id: Option<i64>,
	/// #     username: String,
	/// # }
	/// #
	/// # impl Model for User {
	/// #     type PrimaryKey = i64;
	/// #     type Fields = UserFields;
	/// #     fn table_name() -> &'static str { "users" }
	/// #     fn primary_key(&self) -> Option<Self::PrimaryKey> { self.id }
	/// #     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
	/// #     fn new_fields() -> Self::Fields { UserFields }
	/// # }
	/// # #[derive(Clone)]
	/// # struct UserFields;
	/// # impl reinhardt_db::orm::FieldSelector for UserFields {
	/// #     fn with_alias(self, _alias: &str) -> Self { self }
	/// # }
	/// #
	/// # struct MyUrlReverser;
	/// # impl UrlReverser for MyUrlReverser {
	/// #     fn reverse(&self, name: &str, _params: &HashMap<String, String>) -> Result<String, String> {
	/// #         Ok(format!("/users/{}/", name))
	/// #     }
	/// # }
	/// let reverser: Arc<dyn UrlReverser> = Arc::new(MyUrlReverser);
	/// let serializer = HyperlinkedModelSerializer::<User>::new("user-detail", Some(reverser));
	/// ```
	pub fn new(view_name: impl Into<String>, url_reverser: Option<Arc<dyn UrlReverser>>) -> Self {
		Self {
			view_name: view_name.into(),
			url_field_name: String::from("url"),
			url_reverser,
			_phantom: PhantomData,
		}
	}

	/// Set a custom name for the URL field (default: "url")
	///
	/// # Examples
	///
	/// ```
	/// # use reinhardt_rest::serializers::HyperlinkedModelSerializer;
	/// # use reinhardt_db::orm::Model;
	/// # use serde::{Serialize, Deserialize};
	/// #
	/// # #[derive(Debug, Clone, Serialize, Deserialize)]
	/// # struct User {
	/// #     id: Option<i64>,
	/// #     username: String,
	/// # }
	/// #
	/// # impl Model for User {
	/// #     type PrimaryKey = i64;
	/// #     type Fields = UserFields;
	/// #     fn table_name() -> &'static str { "users" }
	/// #     fn primary_key(&self) -> Option<Self::PrimaryKey> { self.id }
	/// #     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
	/// #     fn new_fields() -> Self::Fields { UserFields }
	/// # }
	/// # #[derive(Clone)]
	/// # struct UserFields;
	/// # impl reinhardt_db::orm::FieldSelector for UserFields {
	/// #     fn with_alias(self, _alias: &str) -> Self { self }
	/// # }
	/// let serializer = HyperlinkedModelSerializer::<User>::new("user-detail", None)
	///     .url_field_name("self_link");
	/// ```
	pub fn url_field_name(mut self, name: impl Into<String>) -> Self {
		self.url_field_name = name.into();
		self
	}

	/// Generate URL for a model instance
	///
	/// Uses reinhardt_urls::routers::reverse() for proper URL resolution if a reverser
	/// is provided, otherwise falls back to simple path-based URL generation.
	fn get_url(&self, instance: &M) -> Result<String, SerializerError>
	where
		M::PrimaryKey: serde::Serialize,
	{
		if let Some(pk) = instance.primary_key() {
			let pk_str = serde_json::to_string(&pk)
				.map_err(|e| SerializerError::Other {
					message: format!("Primary key serialization error: {}", e),
				})?
				.trim_matches('"')
				.to_string();

			// Use URL reverser if available for proper URL resolution
			if let Some(reverser) = &self.url_reverser {
				let mut params = HashMap::new();
				params.insert(M::primary_key_field().to_string(), pk_str);

				reverser
					.reverse(&self.view_name, &params)
					.map_err(|e| SerializerError::Other {
						message: format!("URL reversal error for view '{}': {}", self.view_name, e),
					})
			} else {
				// Fallback to simple path-based URL generation
				Ok(format!(
					"/{}/{}/{}",
					M::table_name(),
					self.view_name,
					pk_str
				))
			}
		} else {
			Err(SerializerError::Other {
				message: String::from("Instance has no primary key"),
			})
		}
	}
}

impl<M> Serializer for HyperlinkedModelSerializer<M>
where
	M: Model,
	M::PrimaryKey: serde::Serialize,
{
	type Input = M;
	type Output = String;

	fn serialize(&self, input: &Self::Input) -> Result<Self::Output, SerializerError> {
		// 1. Serialize the model to JSON value
		let mut value: Value = serde_json::to_value(input).map_err(|e| SerializerError::Other {
			message: format!("Serialization error: {}", e),
		})?;

		// 2. Add URL field
		if let Value::Object(ref mut map) = value {
			let url = self.get_url(input)?;
			map.insert(self.url_field_name.clone(), json!(url));
		}

		// 3. Convert to JSON string
		serde_json::to_string(&value).map_err(|e| SerializerError::Other {
			message: format!("Serialization error: {}", e),
		})
	}

	fn deserialize(&self, output: &Self::Output) -> Result<Self::Input, SerializerError> {
		// Deserialize from JSON, ignoring the URL field
		serde_json::from_str(output).map_err(|e| SerializerError::Other {
			message: format!("Deserialization error: {}", e),
		})
	}
}
