//! Automatic schema generation from Rust types
//!
//! Provides traits and utilities for automatic OpenAPI schema generation
//! from Rust types, inspired by FastAPI/Pydantic.

use crate::openapi::Schema;

/// Trait for types that can generate OpenAPI schemas
///
/// This is the core trait for automatic schema generation.
/// Implement this manually or use `#[derive(Schema)]` macro.
///
/// # Example
///
/// ```rust,no_run
/// use reinhardt_rest::openapi::{ToSchema, Schema};
/// use utoipa::openapi::schema::{ObjectBuilder, SchemaType, Type};
///
/// struct User {
///     id: i64,
///     name: String,
/// }
///
/// impl ToSchema for User {
///     fn schema() -> Schema {
///         Schema::Object(
///             ObjectBuilder::new()
///                 .schema_type(SchemaType::Type(Type::Object))
///                 .property("id", Schema::Object(
///                     ObjectBuilder::new()
///                         .schema_type(SchemaType::Type(Type::Integer))
///                         .build()
///                 ))
///                 .property("name", Schema::Object(
///                     ObjectBuilder::new()
///                         .schema_type(SchemaType::Type(Type::String))
///                         .build()
///                 ))
///                 .required("id")
///                 .required("name")
///                 .build()
///         )
///     }
///
///     fn schema_name() -> Option<String> {
///         Some("User".to_string())
///     }
/// }
/// ```
pub trait ToSchema {
	/// Generate an OpenAPI schema for this type
	fn schema() -> Schema;

	/// Get the schema name (for $ref references)
	fn schema_name() -> Option<String> {
		None
	}
}

/// A complete schema object with metadata
/// This is an alias to utoipa's Schema for convenience
pub type SchemaObject = Schema;

// Implement ToSchema for common types
use utoipa::openapi::schema::{ObjectBuilder, SchemaType, Type};

impl ToSchema for String {
	fn schema() -> Schema {
		Schema::Object(
			ObjectBuilder::new()
				.schema_type(SchemaType::Type(Type::String))
				.build(),
		)
	}
}

impl ToSchema for &str {
	fn schema() -> Schema {
		Schema::Object(
			ObjectBuilder::new()
				.schema_type(SchemaType::Type(Type::String))
				.build(),
		)
	}
}

impl ToSchema for i8 {
	fn schema() -> Schema {
		Schema::Object(
			ObjectBuilder::new()
				.schema_type(SchemaType::Type(Type::Integer))
				.build(),
		)
	}
}

impl ToSchema for i16 {
	fn schema() -> Schema {
		Schema::Object(
			ObjectBuilder::new()
				.schema_type(SchemaType::Type(Type::Integer))
				.build(),
		)
	}
}

impl ToSchema for i32 {
	fn schema() -> Schema {
		Schema::Object(
			ObjectBuilder::new()
				.schema_type(SchemaType::Type(Type::Integer))
				.build(),
		)
	}
}

impl ToSchema for i64 {
	fn schema() -> Schema {
		Schema::Object(
			ObjectBuilder::new()
				.schema_type(SchemaType::Type(Type::Integer))
				.build(),
		)
	}
}

impl ToSchema for u8 {
	fn schema() -> Schema {
		Schema::Object(
			ObjectBuilder::new()
				.schema_type(SchemaType::Type(Type::Integer))
				.build(),
		)
	}
}

impl ToSchema for u16 {
	fn schema() -> Schema {
		Schema::Object(
			ObjectBuilder::new()
				.schema_type(SchemaType::Type(Type::Integer))
				.build(),
		)
	}
}

impl ToSchema for u32 {
	fn schema() -> Schema {
		Schema::Object(
			ObjectBuilder::new()
				.schema_type(SchemaType::Type(Type::Integer))
				.build(),
		)
	}
}

impl ToSchema for u64 {
	fn schema() -> Schema {
		Schema::Object(
			ObjectBuilder::new()
				.schema_type(SchemaType::Type(Type::Integer))
				.build(),
		)
	}
}

impl ToSchema for usize {
	fn schema() -> Schema {
		Schema::Object(
			ObjectBuilder::new()
				.schema_type(SchemaType::Type(Type::Integer))
				.build(),
		)
	}
}

impl ToSchema for isize {
	fn schema() -> Schema {
		Schema::Object(
			ObjectBuilder::new()
				.schema_type(SchemaType::Type(Type::Integer))
				.build(),
		)
	}
}

impl ToSchema for f32 {
	fn schema() -> Schema {
		Schema::Object(
			ObjectBuilder::new()
				.schema_type(SchemaType::Type(Type::Number))
				.build(),
		)
	}
}

impl ToSchema for f64 {
	fn schema() -> Schema {
		Schema::Object(
			ObjectBuilder::new()
				.schema_type(SchemaType::Type(Type::Number))
				.build(),
		)
	}
}

impl ToSchema for bool {
	fn schema() -> Schema {
		Schema::Object(
			ObjectBuilder::new()
				.schema_type(SchemaType::Type(Type::Boolean))
				.build(),
		)
	}
}

impl<T: ToSchema> ToSchema for Option<T> {
	fn schema() -> Schema {
		// Option<T> makes the field optional in parent object
		// The schema itself is the same as T
		T::schema()
	}
}

impl<T: ToSchema> ToSchema for Vec<T> {
	fn schema() -> Schema {
		use utoipa::openapi::schema::Array;
		Schema::Array(Array::new(T::schema()))
	}

	fn schema_name() -> Option<String> {
		T::schema_name().map(|name| format!("Array_{}", name))
	}
}

/// `HashMap<String, V>` support for OpenAPI schema generation
///
/// Generates an OpenAPI schema with `additionalProperties` for dictionary-like structures.
/// Keys are restricted to `String` type as per OpenAPI specification.
///
/// # Example
///
/// ```rust
/// use reinhardt_rest::openapi::ToSchema;
/// use std::collections::HashMap;
///
/// // Simple HashMap with primitive values
/// let schema = <HashMap<String, i32>>::schema();
/// // Verify it's a valid schema
/// match schema {
///     reinhardt_rest::openapi::Schema::Object(_) => {},
///     _ => panic!("Expected Object schema"),
/// }
/// ```
///
/// # Nested HashMaps
///
/// ```rust
/// use reinhardt_rest::openapi::ToSchema;
/// use std::collections::HashMap;
///
/// // Nested HashMaps are supported
/// let schema = <HashMap<String, HashMap<String, String>>>::schema();
/// match schema {
///     reinhardt_rest::openapi::Schema::Object(_) => {},
///     _ => panic!("Expected Object schema"),
/// }
/// ```
impl<V: ToSchema> ToSchema for std::collections::HashMap<String, V> {
	fn schema() -> Schema {
		Schema::Object(
			ObjectBuilder::new()
				.schema_type(SchemaType::Type(Type::Object))
				.additional_properties(Some(V::schema()))
				.build(),
		)
	}

	fn schema_name() -> Option<String> {
		V::schema_name()
			.map(|name| format!("HashMap_String_{}", name))
			.or(Some("HashMap_String_Value".into()))
	}
}

// DateTime<Utc> support for OpenAPI schema generation
impl ToSchema for chrono::DateTime<chrono::Utc> {
	fn schema() -> Schema {
		use utoipa::openapi::schema::{KnownFormat, SchemaFormat};
		Schema::Object(
			ObjectBuilder::new()
				.schema_type(SchemaType::Type(Type::String))
				.format(Some(SchemaFormat::KnownFormat(KnownFormat::DateTime)))
				.build(),
		)
	}
}

// Uuid support for OpenAPI schema generation
impl ToSchema for uuid::Uuid {
	fn schema() -> Schema {
		use utoipa::openapi::schema::{KnownFormat, SchemaFormat};
		Schema::Object(
			ObjectBuilder::new()
				.schema_type(SchemaType::Type(Type::String))
				.format(Some(SchemaFormat::KnownFormat(KnownFormat::Uuid)))
				.build(),
		)
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;
	use std::collections::HashMap;

	/// Custom test struct for HashMap value testing
	#[allow(dead_code)]
	struct User {
		id: i64,
		name: String,
	}

	impl ToSchema for User {
		fn schema() -> Schema {
			Schema::Object(
				ObjectBuilder::new()
					.schema_type(SchemaType::Type(Type::Object))
					.property("id", i64::schema())
					.property("name", String::schema())
					.required("id")
					.required("name")
					.build(),
			)
		}

		fn schema_name() -> Option<String> {
			Some("User".into())
		}
	}

	#[rstest]
	fn test_string_schema_returns_string_type() {
		// Arrange / Act
		let schema = String::schema();

		// Assert
		match schema {
			Schema::Object(obj) => {
				assert!(matches!(obj.schema_type, SchemaType::Type(Type::String)));
			}
			_ => panic!("Expected Object schema"),
		}
	}

	#[rstest]
	fn test_str_ref_schema_returns_string_type() {
		// Arrange / Act
		let schema = <&str>::schema();

		// Assert
		match schema {
			Schema::Object(obj) => {
				assert!(matches!(obj.schema_type, SchemaType::Type(Type::String)));
			}
			_ => panic!("Expected Object schema"),
		}
	}

	#[rstest]
	fn test_i32_schema_returns_integer_type() {
		// Arrange / Act
		let schema = i32::schema();

		// Assert
		match schema {
			Schema::Object(obj) => {
				assert!(matches!(obj.schema_type, SchemaType::Type(Type::Integer)));
			}
			_ => panic!("Expected Object schema"),
		}
	}

	#[rstest]
	fn test_bool_schema_returns_boolean_type() {
		// Arrange / Act
		let schema = bool::schema();

		// Assert
		match schema {
			Schema::Object(obj) => {
				assert!(matches!(obj.schema_type, SchemaType::Type(Type::Boolean)));
			}
			_ => panic!("Expected Object schema"),
		}
	}

	#[rstest]
	fn test_f64_schema_returns_number_type() {
		// Arrange / Act
		let schema = f64::schema();

		// Assert
		match schema {
			Schema::Object(obj) => {
				assert!(matches!(obj.schema_type, SchemaType::Type(Type::Number)));
			}
			_ => panic!("Expected Object schema"),
		}
	}

	#[rstest]
	fn test_vec_schema_produces_array_with_item_schema() {
		// Arrange / Act
		let schema = Vec::<String>::schema();

		// Assert: Vec<T> should produce an Array schema
		match schema {
			Schema::Array(arr) => {
				// Verify the item schema is String
				match &arr.items {
					utoipa::openapi::schema::ArrayItems::RefOrSchema(boxed) => match boxed.as_ref()
					{
						utoipa::openapi::RefOr::T(Schema::Object(item_obj)) => {
							assert!(matches!(
								item_obj.schema_type,
								SchemaType::Type(Type::String)
							));
						}
						_ => panic!("Expected Object schema for String items"),
					},
					_ => panic!("Expected RefOrSchema ArrayItems"),
				}
			}
			_ => panic!("Expected Array schema for Vec<String>"),
		}
	}

	#[rstest]
	fn test_option_schema_delegates_to_inner_type() {
		// Arrange / Act
		let option_schema = Option::<i32>::schema();
		let i32_schema = i32::schema();

		// Assert: Option<T> should delegate to T's schema
		// Both should produce Object schemas with Integer type
		match (option_schema, i32_schema) {
			(Schema::Object(opt_obj), Schema::Object(i32_obj)) => {
				assert!(
					matches!(opt_obj.schema_type, SchemaType::Type(Type::Integer)),
					"Option<i32> schema should be Integer type"
				);
				assert!(
					matches!(i32_obj.schema_type, SchemaType::Type(Type::Integer)),
					"i32 schema should be Integer type"
				);
			}
			_ => panic!("Expected Object schemas for both Option<i32> and i32"),
		}
	}

	#[rstest]
	fn test_hashmap_with_primitive_values() {
		let schema = <HashMap<String, i32>>::schema();

		match schema {
			Schema::Object(obj) => {
				// Verify it's an object type
				assert!(matches!(obj.schema_type, SchemaType::Type(Type::Object)));

				// Verify additional_properties is set
				assert!(obj.additional_properties.is_some());

				// Verify the additional_properties schema is for i32
				if let Some(additional_props) = &obj.additional_properties {
					match additional_props.as_ref() {
						utoipa::openapi::schema::AdditionalProperties::RefOr(ref_or) => {
							match ref_or {
								utoipa::openapi::RefOr::T(Schema::Object(inner)) => {
									assert!(matches!(
										inner.schema_type,
										SchemaType::Type(Type::Integer)
									));
								}
								_ => panic!("Expected Object schema for additional properties"),
							}
						}
						_ => panic!("Expected RefOr AdditionalProperties"),
					}
				}
			}
			_ => panic!("Expected Object schema"),
		}
	}

	#[rstest]
	fn test_hashmap_schema_name() {
		let schema_name = <HashMap<String, i32>>::schema_name();
		assert_eq!(schema_name, Some("HashMap_String_Value".into()));

		let schema_name_with_user = <HashMap<String, User>>::schema_name();
		assert_eq!(schema_name_with_user, Some("HashMap_String_User".into()));
	}

	#[rstest]
	fn test_hashmap_with_custom_struct() {
		let schema = <HashMap<String, User>>::schema();

		match schema {
			Schema::Object(obj) => {
				assert!(matches!(obj.schema_type, SchemaType::Type(Type::Object)));
				assert!(obj.additional_properties.is_some());

				// Verify the additional_properties contains User schema
				if let Some(additional_props) = &obj.additional_properties {
					match additional_props.as_ref() {
						utoipa::openapi::schema::AdditionalProperties::RefOr(ref_or) => {
							match ref_or {
								utoipa::openapi::RefOr::T(Schema::Object(inner)) => {
									assert!(matches!(
										inner.schema_type,
										SchemaType::Type(Type::Object)
									));
									// User schema should have properties
									assert!(inner.properties.contains_key("id"));
									assert!(inner.properties.contains_key("name"));
								}
								_ => panic!("Expected Object schema for User"),
							}
						}
						_ => panic!("Expected RefOr AdditionalProperties"),
					}
				}
			}
			_ => panic!("Expected Object schema"),
		}
	}

	#[rstest]
	fn test_nested_hashmap() {
		let schema = <HashMap<String, HashMap<String, String>>>::schema();

		match schema {
			Schema::Object(obj) => {
				assert!(matches!(obj.schema_type, SchemaType::Type(Type::Object)));
				assert!(obj.additional_properties.is_some());

				// Verify nested HashMap structure
				if let Some(additional_props) = &obj.additional_properties {
					match additional_props.as_ref() {
						utoipa::openapi::schema::AdditionalProperties::RefOr(ref_or) => {
							match ref_or {
								utoipa::openapi::RefOr::T(Schema::Object(inner)) => {
									// Inner should also be an object with additional_properties
									assert!(matches!(
										inner.schema_type,
										SchemaType::Type(Type::Object)
									));
									assert!(inner.additional_properties.is_some());

									// Verify innermost type is String
									if let Some(innermost) = &inner.additional_properties {
										match innermost.as_ref() {
                                            utoipa::openapi::schema::AdditionalProperties::RefOr(inner_ref_or) => {
                                                match inner_ref_or {
                                                    utoipa::openapi::RefOr::T(Schema::Object(innermost_obj)) => {
                                                        assert!(matches!(
                                                            innermost_obj.schema_type,
                                                            SchemaType::Type(Type::String)
                                                        ));
                                                    }
                                                    _ => panic!("Expected Object schema for String"),
                                                }
                                            }
                                            _ => panic!("Expected RefOr AdditionalProperties for innermost"),
                                        }
									}
								}
								_ => panic!("Expected Object schema for nested HashMap"),
							}
						}
						_ => panic!("Expected RefOr AdditionalProperties"),
					}
				}
			}
			_ => panic!("Expected Object schema"),
		}
	}

	#[rstest]
	fn test_hashmap_with_option_values() {
		let schema = <HashMap<String, Option<i32>>>::schema();

		match schema {
			Schema::Object(obj) => {
				assert!(matches!(obj.schema_type, SchemaType::Type(Type::Object)));
				assert!(obj.additional_properties.is_some());

				// Option<T> should return T's schema
				if let Some(additional_props) = &obj.additional_properties {
					match additional_props.as_ref() {
						utoipa::openapi::schema::AdditionalProperties::RefOr(ref_or) => {
							match ref_or {
								utoipa::openapi::RefOr::T(Schema::Object(inner)) => {
									assert!(matches!(
										inner.schema_type,
										SchemaType::Type(Type::Integer)
									));
								}
								_ => panic!("Expected Object schema for Option<i32>"),
							}
						}
						_ => panic!("Expected RefOr AdditionalProperties"),
					}
				}
			}
			_ => panic!("Expected Object schema"),
		}
	}

	#[rstest]
	fn test_hashmap_with_vec_values() {
		let schema = <HashMap<String, Vec<String>>>::schema();

		match schema {
			Schema::Object(obj) => {
				assert!(matches!(obj.schema_type, SchemaType::Type(Type::Object)));
				assert!(obj.additional_properties.is_some());

				// Verify additional_properties is an array
				if let Some(additional_props) = &obj.additional_properties {
					match additional_props.as_ref() {
						utoipa::openapi::schema::AdditionalProperties::RefOr(ref_or) => {
							match ref_or {
								utoipa::openapi::RefOr::T(Schema::Array(arr)) => {
									// Array items should be String - access via ArrayItems enum
									match &arr.items {
										utoipa::openapi::schema::ArrayItems::RefOrSchema(
											boxed_schema,
										) => match boxed_schema.as_ref() {
											utoipa::openapi::RefOr::T(Schema::Object(item_obj)) => {
												assert!(matches!(
													item_obj.schema_type,
													SchemaType::Type(Type::String)
												));
											}
											_ => panic!("Expected Object schema for String items"),
										},
										_ => panic!("Expected RefOrSchema ArrayItems"),
									}
								}
								_ => panic!("Expected Array schema for Vec"),
							}
						}
						_ => panic!("Expected RefOr AdditionalProperties"),
					}
				}
			}
			_ => panic!("Expected Object schema"),
		}
	}
}
