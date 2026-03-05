//! Derive macro for OrmReflectable trait
//!
//! Provides automatic implementation of the OrmReflectable trait for structs,
//! enabling reflection-based field and relationship access for association proxies.

use crate::crate_paths::get_reinhardt_proxy_crate;
use proc_macro2::TokenStream;
use quote::quote;
use syn::{Data, DeriveInput, Fields, Type, parse_macro_input};

/// Field classification result
enum FieldInfo {
	/// Regular field (Integer, String, Float, Boolean)
	Field {
		name: syn::Ident,
		field_type: String,
	},
	/// Collection relationship (`Vec<T>`)
	CollectionRelationship { name: syn::Ident },
	/// Scalar relationship (`Option<T>`)
	ScalarRelationship { name: syn::Ident },
	/// Field to ignore
	Ignored,
}

/// Implementation of the OrmReflectable derive macro
pub(crate) fn orm_reflectable_derive_impl(
	input: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
	let input = parse_macro_input!(input as DeriveInput);
	let struct_name = &input.ident;

	// Only support structs with named fields
	let fields = match &input.data {
		Data::Struct(data_struct) => match &data_struct.fields {
			Fields::Named(fields) => &fields.named,
			_ => {
				return syn::Error::new_spanned(
					struct_name,
					"OrmReflectable can only be derived for structs with named fields",
				)
				.to_compile_error()
				.into();
			}
		},
		_ => {
			return syn::Error::new_spanned(
				struct_name,
				"OrmReflectable can only be derived for structs",
			)
			.to_compile_error()
			.into();
		}
	};

	// Classify all fields
	let mut regular_fields = Vec::new();
	let mut collection_relationships = Vec::new();
	let mut scalar_relationships = Vec::new();

	for field in fields {
		match classify_field(field) {
			FieldInfo::Field { name, field_type } => {
				regular_fields.push((name, field_type));
			}
			FieldInfo::CollectionRelationship { name } => {
				collection_relationships.push(name);
			}
			FieldInfo::ScalarRelationship { name } => {
				scalar_relationships.push(name);
			}
			FieldInfo::Ignored => {
				// Skip ignored fields
			}
		}
	}

	// Get dynamic crate path
	let proxy_crate = get_reinhardt_proxy_crate();

	// Generate method implementations
	let clone_relationship_impl =
		generate_clone_relationship(&collection_relationships, &scalar_relationships);
	let get_relationship_mut_impl =
		generate_get_relationship_mut(&collection_relationships, &scalar_relationships);
	let get_field_value_impl = generate_get_field_value(&regular_fields, &proxy_crate);
	let set_field_value_impl = generate_set_field_value(&regular_fields, &proxy_crate);

	// Generate the impl block
	let expanded = quote! {
		impl #proxy_crate::orm_integration::OrmReflectable for #struct_name {
			fn clone_relationship(&self, name: &str) -> Option<Box<dyn std::any::Any + 'static>> {
				#clone_relationship_impl
			}

			fn get_relationship_mut_ref(&mut self, name: &str) -> Option<&mut dyn std::any::Any> {
				#get_relationship_mut_impl
			}

			fn get_field_value(&self, name: &str) -> Option<#proxy_crate::ScalarValue> {
				#get_field_value_impl
			}

			fn set_field_value(&mut self, name: &str, value: #proxy_crate::ScalarValue) -> #proxy_crate::ProxyResult<()> {
				#set_field_value_impl
			}
		}
	};

	expanded.into()
}

/// Classify a field as regular field, relationship, or ignored
fn classify_field(field: &syn::Field) -> FieldInfo {
	let Some(field_name) = field.ident.clone() else {
		// Unnamed fields (e.g. tuple structs) are not supported
		return FieldInfo::Ignored;
	};

	// 1. Check for #[orm_ignore] attribute
	if has_attribute(&field.attrs, "orm_ignore") {
		return FieldInfo::Ignored;
	}

	// 2. Check for #[orm_field(type = "...")] attribute
	if let Some(field_type) = get_orm_field_type(&field.attrs) {
		return FieldInfo::Field {
			name: field_name,
			field_type,
		};
	}

	// 3. Check for #[orm_relationship(type = "...")] attribute
	if let Some(rel_type) = get_orm_relationship_type(&field.attrs) {
		return match rel_type.as_str() {
			"collection" => FieldInfo::CollectionRelationship { name: field_name },
			"scalar" => FieldInfo::ScalarRelationship { name: field_name },
			_ => FieldInfo::Ignored,
		};
	}

	// 4. Type inference
	if let Type::Path(type_path) = &field.ty
		&& let Some(last_segment) = type_path.path.segments.last()
	{
		let type_name = last_segment.ident.to_string();

		// Vec<T> or Vec<Box<T>> → Collection
		if type_name == "Vec" {
			return FieldInfo::CollectionRelationship { name: field_name };
		}

		// Option<T> → Check if T is primitive or custom type
		if type_name == "Option"
			&& let syn::PathArguments::AngleBracketed(args) = &last_segment.arguments
			&& let Some(syn::GenericArgument::Type(inner_ty)) = args.args.first()
		{
			// If inner type is not primitive, treat as Scalar relationship
			if !is_primitive_type(inner_ty) {
				return FieldInfo::ScalarRelationship { name: field_name };
			}
			// If Option<primitive>, it's still a regular field
		}
	}

	// 5. Check if primitive type → Regular field
	if let Some(field_type) = infer_field_type(&field.ty) {
		return FieldInfo::Field {
			name: field_name,
			field_type,
		};
	}

	// 6. Unknown type → Ignore
	FieldInfo::Ignored
}

/// Check if field has specific attribute
fn has_attribute(attrs: &[syn::Attribute], name: &str) -> bool {
	attrs.iter().any(|attr| attr.path().is_ident(name))
}

/// Get field type from `#[orm_field(type = "Integer")]` attribute
fn get_orm_field_type(attrs: &[syn::Attribute]) -> Option<String> {
	for attr in attrs {
		if attr.path().is_ident("orm_field")
			&& let Ok(meta) = attr.parse_args::<syn::Meta>()
			&& let syn::Meta::NameValue(nv) = meta
			&& nv.path.is_ident("type")
			&& let syn::Expr::Lit(expr_lit) = &nv.value
			&& let syn::Lit::Str(lit_str) = &expr_lit.lit
		{
			return Some(lit_str.value());
		}
	}
	None
}

/// Get relationship type from `#[orm_relationship(type = "collection")]` attribute
fn get_orm_relationship_type(attrs: &[syn::Attribute]) -> Option<String> {
	for attr in attrs {
		if attr.path().is_ident("orm_relationship")
			&& let Ok(meta) = attr.parse_args::<syn::Meta>()
			&& let syn::Meta::NameValue(nv) = meta
			&& nv.path.is_ident("type")
			&& let syn::Expr::Lit(expr_lit) = &nv.value
			&& let syn::Lit::Str(lit_str) = &expr_lit.lit
		{
			return Some(lit_str.value());
		}
	}
	None
}

/// Check if type is a primitive type
fn is_primitive_type(ty: &Type) -> bool {
	if let Type::Path(type_path) = ty {
		if let Some(segment) = type_path.path.segments.last() {
			let type_name = segment.ident.to_string();
			matches!(
				type_name.as_str(),
				"i8" | "i16"
					| "i32" | "i64" | "i128"
					| "u8" | "u16" | "u32"
					| "u64" | "u128"
					| "f32" | "f64" | "bool"
					| "char" | "String"
					| "str"
			)
		} else {
			false
		}
	} else {
		false
	}
}

/// Infer field type from Rust type
fn infer_field_type(ty: &Type) -> Option<String> {
	if let Type::Path(type_path) = ty
		&& let Some(segment) = type_path.path.segments.last()
	{
		let type_name = segment.ident.to_string();
		return match type_name.as_str() {
			"i8" | "i16" | "i32" | "i64" | "i128" | "u8" | "u16" | "u32" | "u64" | "u128" => {
				Some("Integer".to_string())
			}
			"f32" | "f64" => Some("Float".to_string()),
			"bool" => Some("Boolean".to_string()),
			"String" | "str" => Some("String".to_string()),
			_ => None,
		};
	}
	None
}

/// Generate clone_relationship method implementation
fn generate_clone_relationship(collections: &[syn::Ident], scalars: &[syn::Ident]) -> TokenStream {
	let collection_arms: Vec<_> = collections
		.iter()
		.map(|name| {
			let name_str = name.to_string();
			quote! {
				#name_str => Some(Box::new(self.#name.clone()) as Box<dyn std::any::Any + 'static>),
			}
		})
		.collect();

	let scalar_arms: Vec<_> = scalars
		.iter()
		.map(|name| {
			let name_str = name.to_string();
			quote! {
				#name_str => Some(Box::new(self.#name.clone()) as Box<dyn std::any::Any + 'static>),
			}
		})
		.collect();

	quote! {
		match name {
			#(#collection_arms)*
			#(#scalar_arms)*
			_ => None,
		}
	}
}

/// Generate get_relationship_mut_ref method implementation
fn generate_get_relationship_mut(
	collections: &[syn::Ident],
	scalars: &[syn::Ident],
) -> TokenStream {
	let collection_arms: Vec<_> = collections
		.iter()
		.map(|name| {
			let name_str = name.to_string();
			quote! {
				#name_str => Some(&mut self.#name as &mut dyn std::any::Any),
			}
		})
		.collect();

	let scalar_arms: Vec<_> = scalars
		.iter()
		.map(|name| {
			let name_str = name.to_string();
			quote! {
				#name_str => Some(&mut self.#name as &mut dyn std::any::Any),
			}
		})
		.collect();

	quote! {
		match name {
			#(#collection_arms)*
			#(#scalar_arms)*
			_ => None,
		}
	}
}

/// Generate get_field_value method implementation
fn generate_get_field_value(
	fields: &[(syn::Ident, String)],
	proxy_crate: &TokenStream,
) -> TokenStream {
	let arms: Vec<_> = fields
		.iter()
		.map(|(name, field_type)| {
			let name_str = name.to_string();
			let conversion = match field_type.as_str() {
				"Integer" => quote! { #proxy_crate::ScalarValue::Integer(self.#name as i64) },
				"String" => quote! { #proxy_crate::ScalarValue::String(self.#name.clone()) },
				"Float" => quote! { #proxy_crate::ScalarValue::Float(self.#name as f64) },
				"Boolean" => quote! { #proxy_crate::ScalarValue::Boolean(self.#name) },
				_ => quote! { #proxy_crate::ScalarValue::Null },
			};
			quote! {
				#name_str => Some(#conversion),
			}
		})
		.collect();

	quote! {
		match name {
			#(#arms)*
			_ => None,
		}
	}
}

/// Generate set_field_value method implementation
fn generate_set_field_value(
	fields: &[(syn::Ident, String)],
	proxy_crate: &TokenStream,
) -> TokenStream {
	let arms: Vec<_> = fields
		.iter()
		.map(|(name, field_type)| {
			let name_str = name.to_string();
			let conversion = match field_type.as_str() {
				"Integer" => quote! {
					self.#name = value.as_integer()? as _;
					Ok(())
				},
				"String" => quote! {
					self.#name = value.as_string()?;
					Ok(())
				},
				"Float" => quote! {
					self.#name = value.as_float()? as _;
					Ok(())
				},
				"Boolean" => quote! {
					self.#name = value.as_boolean()?;
					Ok(())
				},
				_ => quote! {
					Err(#proxy_crate::ProxyError::AttributeNotFound(name.to_string()))
				},
			};
			quote! {
				#name_str => {
					#conversion
				},
			}
		})
		.collect();

	quote! {
		match name {
			#(#arms)*
			_ => Err(#proxy_crate::ProxyError::AttributeNotFound(name.to_string())),
		}
	}
}
