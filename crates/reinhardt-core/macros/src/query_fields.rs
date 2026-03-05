//! Procedural macro for generating type-safe field lookups
//!
//! This macro automatically generates field accessor methods for models,
//! enabling compile-time validated field lookups.

use crate::crate_paths::get_reinhardt_orm_crate;
use proc_macro2::TokenStream;
use quote::quote;
use syn::{Data, DeriveInput, Fields, Type};
/// Implementation of the `QueryFields` derive macro
///
/// This function is used internally by the `#[derive(QueryFields)]` macro.
/// Users should not call this function directly.
pub(crate) fn derive_query_fields_impl(input: DeriveInput) -> syn::Result<TokenStream> {
	let struct_name = &input.ident;
	let orm_crate = get_reinhardt_orm_crate();

	// Extract struct fields
	let fields = match &input.data {
		Data::Struct(data) => match &data.fields {
			Fields::Named(fields) => &fields.named,
			_ => {
				return Err(syn::Error::new_spanned(
					struct_name,
					"QueryFields can only be derived for structs with named fields",
				));
			}
		},
		_ => {
			return Err(syn::Error::new_spanned(
				struct_name,
				"QueryFields can only be derived for structs",
			));
		}
	};

	// Generate field accessor methods
	let field_methods: Vec<TokenStream> = fields
		.iter()
		.map(|field| {
			let field_name = field
				.ident
				.as_ref()
				.ok_or_else(|| syn::Error::new_spanned(field, "expected named field"))?;
			let field_name_str = field_name.to_string();
			let field_type = &field.ty;

			// Map Rust types to field lookup types
			let lookup_type = map_type_to_lookup_type(field_type, &orm_crate);

			Ok(quote! {
				#[doc = concat!("Field accessor for `", #field_name_str, "`")]
				pub fn #field_name() -> #orm_crate::query_fields::Field<#struct_name, #lookup_type> {
					#orm_crate::query_fields::Field::new(vec![#field_name_str])
				}
			})
		})
		.collect::<syn::Result<Vec<_>>>()?;

	// Generate the impl block
	Ok(quote! {
		impl #struct_name {
			#(#field_methods)*
		}
	})
}

/// Map Rust types to field lookup types
///
/// Handles sophisticated type mapping including:
/// - Primitive types (String, i32, i64, f32, f64, bool)
/// - DateTime types (DateTime, Date)
/// - Generic types (`Option<T>`, `Vec<T>`, `HashMap<K,V>`, `HashSet<T>`)
/// - ORM relationship types (ForeignKey, OneToOneField, ManyToManyField)
/// - Complex nested structures (`Option<Vec<T>>`, `Result<T, E>`)
/// - Custom types with full path qualification
fn map_type_to_lookup_type(ty: &Type, orm_crate: &TokenStream) -> TokenStream {
	match ty {
		Type::Path(type_path) => {
			let Some(last_segment) = type_path.path.segments.last() else {
				// Empty path segments - use type as-is
				return quote! { #ty };
			};
			let type_ident = &last_segment.ident;
			let type_name = type_ident.to_string();

			match type_name.as_str() {
				// Primitive types
				"String" | "str" => quote! { String },
				"i8" | "i16" | "i32" => quote! { i32 },
				"i64" | "i128" | "isize" => quote! { i64 },
				"u8" | "u16" | "u32" => quote! { i32 },
				"u64" | "u128" | "usize" => quote! { i64 },
				"f32" => quote! { f32 },
				"f64" => quote! { f64 },
				"bool" => quote! { bool },

				// DateTime types
				"DateTime" => quote! { #orm_crate::query_fields::DateTime },
				"Date" => quote! { #orm_crate::query_fields::Date },
				"NaiveDateTime" => quote! { #orm_crate::query_fields::DateTime },
				"NaiveDate" => quote! { #orm_crate::query_fields::Date },

				// Generic collection types
				"Option" => handle_option_type(last_segment, orm_crate),
				"Vec" => handle_vec_type(last_segment, orm_crate),
				"HashMap" => handle_hashmap_type(last_segment, orm_crate),
				"HashSet" => handle_hashset_type(last_segment, orm_crate),
				"BTreeMap" => handle_btreemap_type(last_segment, orm_crate),
				"BTreeSet" => handle_btreeset_type(last_segment, orm_crate),

				// Result type
				"Result" => handle_result_type(last_segment, orm_crate),

				// Box, Arc, Rc
				"Box" | "Arc" | "Rc" => handle_pointer_type(last_segment, orm_crate),

				// ORM relationship types
				"ForeignKey" => handle_foreign_key_type(last_segment, orm_crate),
				"OneToOneField" => handle_one_to_one_type(last_segment, orm_crate),
				"ManyToManyField" => handle_many_to_many_type(last_segment, orm_crate),

				// Custom types - use full path
				_ => {
					// Check if this is a qualified path (e.g., chrono::DateTime)
					if type_path.path.segments.len() > 1 {
						// Use the full path for qualified types
						let full_path = &type_path.path;
						quote! { #full_path }
					} else {
						// For unqualified custom types, use as-is
						quote! { #type_ident }
					}
				}
			}
		}
		Type::Reference(type_ref) => {
			// Handle reference types by extracting the inner type
			map_type_to_lookup_type(&type_ref.elem, orm_crate)
		}
		Type::Array(type_array) => {
			// Handle array types: [T; N]
			let elem_type = map_type_to_lookup_type(&type_array.elem, orm_crate);
			quote! { Vec<#elem_type> }
		}
		Type::Slice(type_slice) => {
			// Handle slice types: &[T]
			let elem_type = map_type_to_lookup_type(&type_slice.elem, orm_crate);
			quote! { Vec<#elem_type> }
		}
		Type::Tuple(type_tuple) => {
			// Handle tuple types
			if type_tuple.elems.is_empty() {
				// Unit type ()
				quote! { () }
			} else {
				// For non-empty tuples, use as-is
				quote! { #ty }
			}
		}
		_ => quote! { #ty },
	}
}

/// Handle `Option<T>` type mapping
fn handle_option_type(segment: &syn::PathSegment, orm_crate: &TokenStream) -> TokenStream {
	if let syn::PathArguments::AngleBracketed(args) = &segment.arguments
		&& let Some(syn::GenericArgument::Type(inner_type)) = args.args.first()
	{
		let inner_lookup = map_type_to_lookup_type(inner_type, orm_crate);
		return quote! { Option<#inner_lookup> };
	}
	quote! { Option<()> }
}

/// Handle `Vec<T>` type mapping
fn handle_vec_type(segment: &syn::PathSegment, orm_crate: &TokenStream) -> TokenStream {
	if let syn::PathArguments::AngleBracketed(args) = &segment.arguments
		&& let Some(syn::GenericArgument::Type(inner_type)) = args.args.first()
	{
		let inner_lookup = map_type_to_lookup_type(inner_type, orm_crate);
		return quote! { Vec<#inner_lookup> };
	}
	quote! { Vec<()> }
}

/// Handle `HashMap<K, V>` type mapping
fn handle_hashmap_type(segment: &syn::PathSegment, orm_crate: &TokenStream) -> TokenStream {
	if let syn::PathArguments::AngleBracketed(args) = &segment.arguments {
		let mut args_iter = args.args.iter();
		if let (
			Some(syn::GenericArgument::Type(key_type)),
			Some(syn::GenericArgument::Type(value_type)),
		) = (args_iter.next(), args_iter.next())
		{
			let key_lookup = map_type_to_lookup_type(key_type, orm_crate);
			let value_lookup = map_type_to_lookup_type(value_type, orm_crate);
			return quote! { ::std::collections::HashMap<#key_lookup, #value_lookup> };
		}
	}
	quote! { ::std::collections::HashMap<(), ()> }
}

/// Handle `HashSet<T>` type mapping
fn handle_hashset_type(segment: &syn::PathSegment, orm_crate: &TokenStream) -> TokenStream {
	if let syn::PathArguments::AngleBracketed(args) = &segment.arguments
		&& let Some(syn::GenericArgument::Type(inner_type)) = args.args.first()
	{
		let inner_lookup = map_type_to_lookup_type(inner_type, orm_crate);
		return quote! { ::std::collections::HashSet<#inner_lookup> };
	}
	quote! { ::std::collections::HashSet<()> }
}

/// Handle `BTreeMap<K, V>` type mapping
fn handle_btreemap_type(segment: &syn::PathSegment, orm_crate: &TokenStream) -> TokenStream {
	if let syn::PathArguments::AngleBracketed(args) = &segment.arguments {
		let mut args_iter = args.args.iter();
		if let (
			Some(syn::GenericArgument::Type(key_type)),
			Some(syn::GenericArgument::Type(value_type)),
		) = (args_iter.next(), args_iter.next())
		{
			let key_lookup = map_type_to_lookup_type(key_type, orm_crate);
			let value_lookup = map_type_to_lookup_type(value_type, orm_crate);
			return quote! { ::std::collections::BTreeMap<#key_lookup, #value_lookup> };
		}
	}
	quote! { ::std::collections::BTreeMap<(), ()> }
}

/// Handle `BTreeSet<T>` type mapping
fn handle_btreeset_type(segment: &syn::PathSegment, orm_crate: &TokenStream) -> TokenStream {
	if let syn::PathArguments::AngleBracketed(args) = &segment.arguments
		&& let Some(syn::GenericArgument::Type(inner_type)) = args.args.first()
	{
		let inner_lookup = map_type_to_lookup_type(inner_type, orm_crate);
		return quote! { ::std::collections::BTreeSet<#inner_lookup> };
	}
	quote! { ::std::collections::BTreeSet<()> }
}

/// Handle `Result<T, E>` type mapping
fn handle_result_type(segment: &syn::PathSegment, orm_crate: &TokenStream) -> TokenStream {
	if let syn::PathArguments::AngleBracketed(args) = &segment.arguments {
		let mut args_iter = args.args.iter();
		if let (
			Some(syn::GenericArgument::Type(ok_type)),
			Some(syn::GenericArgument::Type(err_type)),
		) = (args_iter.next(), args_iter.next())
		{
			let ok_lookup = map_type_to_lookup_type(ok_type, orm_crate);
			let err_lookup = map_type_to_lookup_type(err_type, orm_crate);
			return quote! { Result<#ok_lookup, #err_lookup> };
		}
	}
	quote! { Result<(), ()> }
}

/// Handle `Box<T>`, `Arc<T>`, `Rc<T>` type mapping
fn handle_pointer_type(segment: &syn::PathSegment, orm_crate: &TokenStream) -> TokenStream {
	if let syn::PathArguments::AngleBracketed(args) = &segment.arguments
		&& let Some(syn::GenericArgument::Type(inner_type)) = args.args.first()
	{
		// For pointer types, extract the inner type directly
		return map_type_to_lookup_type(inner_type, orm_crate);
	}
	quote! { () }
}

/// Handle ForeignKey type mapping
fn handle_foreign_key_type(segment: &syn::PathSegment, orm_crate: &TokenStream) -> TokenStream {
	if let syn::PathArguments::AngleBracketed(args) = &segment.arguments
		&& let Some(syn::GenericArgument::Type(related_model)) = args.args.first()
	{
		// Extract the related model type
		let model_lookup = map_type_to_lookup_type(related_model, orm_crate);
		return quote! { #model_lookup };
	}
	// If no generic arguments, use i64 (primary key type)
	quote! { i64 }
}

/// Handle OneToOneField type mapping
fn handle_one_to_one_type(segment: &syn::PathSegment, orm_crate: &TokenStream) -> TokenStream {
	if let syn::PathArguments::AngleBracketed(args) = &segment.arguments
		&& let Some(syn::GenericArgument::Type(related_model)) = args.args.first()
	{
		let model_lookup = map_type_to_lookup_type(related_model, orm_crate);
		return quote! { #model_lookup };
	}
	quote! { i64 }
}

/// Handle ManyToManyField type mapping
fn handle_many_to_many_type(segment: &syn::PathSegment, orm_crate: &TokenStream) -> TokenStream {
	if let syn::PathArguments::AngleBracketed(args) = &segment.arguments
		&& let Some(syn::GenericArgument::Type(related_model)) = args.args.first()
	{
		let model_lookup = map_type_to_lookup_type(related_model, orm_crate);
		// ManyToMany returns a collection of the related model
		return quote! { Vec<#model_lookup> };
	}
	quote! { Vec<i64> }
}

#[cfg(test)]
mod tests {
	use super::*;
	use syn::parse_quote;

	// Helper function to get a mock orm_crate for testing
	fn mock_orm_crate() -> TokenStream {
		quote! { ::reinhardt_orm }
	}

	#[test]
	fn test_derive_query_fields() {
		let input: DeriveInput = parse_quote! {
			struct User {
				id: i64,
				email: String,
				age: i32,
				created_at: DateTime,
			}
		};

		let output = derive_query_fields_impl(input).expect("derive should succeed");
		let output_str = output.to_string();

		// Verify that field accessor methods are generated
		assert!(output_str.contains("pub fn id"));
		assert!(output_str.contains("pub fn email"));
		assert!(output_str.contains("pub fn age"));
		assert!(output_str.contains("pub fn created_at"));
	}

	#[test]
	fn test_primitive_type_mapping() {
		let orm_crate = mock_orm_crate();

		let ty: Type = parse_quote! { String };
		let result = map_type_to_lookup_type(&ty, &orm_crate);
		assert_eq!(result.to_string(), "String");

		let ty: Type = parse_quote! { i32 };
		let result = map_type_to_lookup_type(&ty, &orm_crate);
		assert_eq!(result.to_string(), "i32");

		let ty: Type = parse_quote! { i64 };
		let result = map_type_to_lookup_type(&ty, &orm_crate);
		assert_eq!(result.to_string(), "i64");

		let ty: Type = parse_quote! { f64 };
		let result = map_type_to_lookup_type(&ty, &orm_crate);
		assert_eq!(result.to_string(), "f64");

		let ty: Type = parse_quote! { bool };
		let result = map_type_to_lookup_type(&ty, &orm_crate);
		assert_eq!(result.to_string(), "bool");
	}

	#[test]
	fn test_option_type_mapping() {
		let orm_crate = mock_orm_crate();

		let ty: Type = parse_quote! { Option<String> };
		let result = map_type_to_lookup_type(&ty, &orm_crate);
		assert_eq!(result.to_string(), "Option < String >");

		let ty: Type = parse_quote! { Option<i64> };
		let result = map_type_to_lookup_type(&ty, &orm_crate);
		assert_eq!(result.to_string(), "Option < i64 >");

		// Nested Option
		let ty: Type = parse_quote! { Option<Option<String>> };
		let result = map_type_to_lookup_type(&ty, &orm_crate);
		assert_eq!(result.to_string(), "Option < Option < String > >");
	}

	#[test]
	fn test_vec_type_mapping() {
		let orm_crate = mock_orm_crate();

		let ty: Type = parse_quote! { Vec<String> };
		let result = map_type_to_lookup_type(&ty, &orm_crate);
		assert_eq!(result.to_string(), "Vec < String >");

		let ty: Type = parse_quote! { Vec<i64> };
		let result = map_type_to_lookup_type(&ty, &orm_crate);
		assert_eq!(result.to_string(), "Vec < i64 >");

		// Complex nested type
		let ty: Type = parse_quote! { Vec<Option<String>> };
		let result = map_type_to_lookup_type(&ty, &orm_crate);
		assert_eq!(result.to_string(), "Vec < Option < String > >");
	}

	#[test]
	fn test_hashmap_type_mapping() {
		let orm_crate = mock_orm_crate();

		let ty: Type = parse_quote! { HashMap<String, i64> };
		let result = map_type_to_lookup_type(&ty, &orm_crate);
		assert_eq!(
			result.to_string(),
			":: std :: collections :: HashMap < String , i64 >"
		);

		let ty: Type = parse_quote! { HashMap<i64, Vec<String>> };
		let result = map_type_to_lookup_type(&ty, &orm_crate);
		assert_eq!(
			result.to_string(),
			":: std :: collections :: HashMap < i64 , Vec < String > >"
		);
	}

	#[test]
	fn test_hashset_type_mapping() {
		let orm_crate = mock_orm_crate();

		let ty: Type = parse_quote! { HashSet<String> };
		let result = map_type_to_lookup_type(&ty, &orm_crate);
		assert_eq!(
			result.to_string(),
			":: std :: collections :: HashSet < String >"
		);

		let ty: Type = parse_quote! { HashSet<i64> };
		let result = map_type_to_lookup_type(&ty, &orm_crate);
		assert_eq!(
			result.to_string(),
			":: std :: collections :: HashSet < i64 >"
		);
	}

	#[test]
	fn test_btreemap_type_mapping() {
		let orm_crate = mock_orm_crate();

		let ty: Type = parse_quote! { BTreeMap<String, i64> };
		let result = map_type_to_lookup_type(&ty, &orm_crate);
		assert_eq!(
			result.to_string(),
			":: std :: collections :: BTreeMap < String , i64 >"
		);
	}

	#[test]
	fn test_btreeset_type_mapping() {
		let orm_crate = mock_orm_crate();

		let ty: Type = parse_quote! { BTreeSet<String> };
		let result = map_type_to_lookup_type(&ty, &orm_crate);
		assert_eq!(
			result.to_string(),
			":: std :: collections :: BTreeSet < String >"
		);
	}

	#[test]
	fn test_result_type_mapping() {
		let orm_crate = mock_orm_crate();

		let ty: Type = parse_quote! { Result<String, i32> };
		let result = map_type_to_lookup_type(&ty, &orm_crate);
		assert_eq!(result.to_string(), "Result < String , i32 >");

		let ty: Type = parse_quote! { Result<Vec<String>, String> };
		let result = map_type_to_lookup_type(&ty, &orm_crate);
		assert_eq!(result.to_string(), "Result < Vec < String > , String >");
	}

	#[test]
	fn test_pointer_type_mapping() {
		let orm_crate = mock_orm_crate();

		let ty: Type = parse_quote! { Box<String> };
		let result = map_type_to_lookup_type(&ty, &orm_crate);
		assert_eq!(result.to_string(), "String");

		let ty: Type = parse_quote! { Arc<i64> };
		let result = map_type_to_lookup_type(&ty, &orm_crate);
		assert_eq!(result.to_string(), "i64");

		let ty: Type = parse_quote! { Rc<Vec<String>> };
		let result = map_type_to_lookup_type(&ty, &orm_crate);
		assert_eq!(result.to_string(), "Vec < String >");
	}

	#[test]
	fn test_datetime_type_mapping() {
		let orm_crate = mock_orm_crate();

		let ty: Type = parse_quote! { DateTime };
		let result = map_type_to_lookup_type(&ty, &orm_crate);
		assert_eq!(
			result.to_string(),
			":: reinhardt_orm :: query_fields :: DateTime"
		);

		let ty: Type = parse_quote! { Date };
		let result = map_type_to_lookup_type(&ty, &orm_crate);
		assert_eq!(
			result.to_string(),
			":: reinhardt_orm :: query_fields :: Date"
		);

		let ty: Type = parse_quote! { chrono::DateTime<chrono::Utc> };
		let result = map_type_to_lookup_type(&ty, &orm_crate);
		assert_eq!(
			result.to_string(),
			":: reinhardt_orm :: query_fields :: DateTime"
		);
	}

	#[test]
	fn test_reference_type_mapping() {
		let orm_crate = mock_orm_crate();

		let ty: Type = parse_quote! { &str };
		let result = map_type_to_lookup_type(&ty, &orm_crate);
		assert_eq!(result.to_string(), "String");

		let ty: Type = parse_quote! { &String };
		let result = map_type_to_lookup_type(&ty, &orm_crate);
		assert_eq!(result.to_string(), "String");

		let ty: Type = parse_quote! { &i64 };
		let result = map_type_to_lookup_type(&ty, &orm_crate);
		assert_eq!(result.to_string(), "i64");
	}

	#[test]
	fn test_array_type_mapping() {
		let orm_crate = mock_orm_crate();

		let ty: Type = parse_quote! { [i32; 10] };
		let result = map_type_to_lookup_type(&ty, &orm_crate);
		assert_eq!(result.to_string(), "Vec < i32 >");

		let ty: Type = parse_quote! { [String; 5] };
		let result = map_type_to_lookup_type(&ty, &orm_crate);
		assert_eq!(result.to_string(), "Vec < String >");
	}

	#[test]
	fn test_slice_type_mapping() {
		let orm_crate = mock_orm_crate();

		let ty: Type = parse_quote! { &[i32] };
		let result = map_type_to_lookup_type(&ty, &orm_crate);
		assert_eq!(result.to_string(), "Vec < i32 >");

		let ty: Type = parse_quote! { &[String] };
		let result = map_type_to_lookup_type(&ty, &orm_crate);
		assert_eq!(result.to_string(), "Vec < String >");
	}

	#[test]
	fn test_complex_nested_types() {
		let orm_crate = mock_orm_crate();

		// Option<Vec<HashMap<String, i64>>>
		let ty: Type = parse_quote! { Option<Vec<HashMap<String, i64>>> };
		let result = map_type_to_lookup_type(&ty, &orm_crate);
		assert_eq!(
			result.to_string(),
			"Option < Vec < :: std :: collections :: HashMap < String , i64 > > >"
		);

		// Result<Option<String>, Vec<String>>
		let ty: Type = parse_quote! { Result<Option<String>, Vec<String>> };
		let result = map_type_to_lookup_type(&ty, &orm_crate);
		assert_eq!(
			result.to_string(),
			"Result < Option < String > , Vec < String > >"
		);

		// Arc<Option<Vec<String>>>
		let ty: Type = parse_quote! { Arc<Option<Vec<String>>> };
		let result = map_type_to_lookup_type(&ty, &orm_crate);
		assert_eq!(result.to_string(), "Option < Vec < String > >");
	}

	#[test]
	fn test_custom_type_mapping() {
		let orm_crate = mock_orm_crate();

		// Simple custom type
		let ty: Type = parse_quote! { CustomType };
		let result = map_type_to_lookup_type(&ty, &orm_crate);
		assert_eq!(result.to_string(), "CustomType");

		// Qualified custom type
		let ty: Type = parse_quote! { my_crate::CustomType };
		let result = map_type_to_lookup_type(&ty, &orm_crate);
		assert_eq!(result.to_string(), "my_crate :: CustomType");

		// Fully qualified custom type
		let ty: Type = parse_quote! { crate::models::User };
		let result = map_type_to_lookup_type(&ty, &orm_crate);
		assert_eq!(result.to_string(), "crate :: models :: User");
	}

	#[test]
	fn test_integer_type_normalization() {
		let orm_crate = mock_orm_crate();

		// Smaller integers map to i32
		let ty: Type = parse_quote! { i8 };
		let result = map_type_to_lookup_type(&ty, &orm_crate);
		assert_eq!(result.to_string(), "i32");

		let ty: Type = parse_quote! { i16 };
		let result = map_type_to_lookup_type(&ty, &orm_crate);
		assert_eq!(result.to_string(), "i32");

		let ty: Type = parse_quote! { u8 };
		let result = map_type_to_lookup_type(&ty, &orm_crate);
		assert_eq!(result.to_string(), "i32");

		let ty: Type = parse_quote! { u16 };
		let result = map_type_to_lookup_type(&ty, &orm_crate);
		assert_eq!(result.to_string(), "i32");

		let ty: Type = parse_quote! { u32 };
		let result = map_type_to_lookup_type(&ty, &orm_crate);
		assert_eq!(result.to_string(), "i32");

		// Larger integers map to i64
		let ty: Type = parse_quote! { i128 };
		let result = map_type_to_lookup_type(&ty, &orm_crate);
		assert_eq!(result.to_string(), "i64");

		let ty: Type = parse_quote! { u64 };
		let result = map_type_to_lookup_type(&ty, &orm_crate);
		assert_eq!(result.to_string(), "i64");

		let ty: Type = parse_quote! { usize };
		let result = map_type_to_lookup_type(&ty, &orm_crate);
		assert_eq!(result.to_string(), "i64");
	}
}
