//! Proc macros for reinhardt-taggit
//!
//! This crate provides the `#[taggable]` attribute macro for zero-boilerplate tagging.

use proc_macro::TokenStream;
use quote::quote;
use syn::{Data, DeriveInput, Fields, Type, parse_macro_input};

/// Attribute macro to make a model taggable
///
/// This macro automatically implements the `Taggable` trait for the annotated struct,
/// providing `content_type_name()` and `object_id()` methods.
///
/// # Usage
///
/// ```rust,ignore
/// use reinhardt_taggit::prelude::*;
///
/// #[model(app_label = "myapp", table_name = "foods")]
/// #[taggable]
/// pub struct Food {
///     #[field(primary_key = true)]
///     pub id: Option<i64>,
///
///     #[field(max_length = 255)]
///     pub name: String,
/// }
/// ```
///
/// # Requirements
///
/// - The struct must have a field named `id` of type `i64` or `Option<i64>`
/// - The struct must be annotated with `#[model(...)]`
///
/// # Generated Code
///
/// The macro generates:
/// 1. `Taggable` trait implementation with `content_type_name()` and `object_id()`
#[proc_macro_attribute]
pub fn taggable(_attr: TokenStream, item: TokenStream) -> TokenStream {
	let input = parse_macro_input!(item as DeriveInput);
	let name = &input.ident;

	// Ensure it's a struct
	let Data::Struct(data_struct) = &input.data else {
		return syn::Error::new_spanned(&input, "#[taggable] can only be applied to structs")
			.to_compile_error()
			.into();
	};

	// Find the `id` field and determine if it's Option<i64> or i64
	let id_is_option = match &data_struct.fields {
		Fields::Named(fields) => fields
			.named
			.iter()
			.find(|f| f.ident.as_ref().is_some_and(|id| id == "id"))
			.map(|f| is_option_type(&f.ty))
			.unwrap_or(true), // Default to Option if id not found
		_ => true,
	};

	// Generate object_id() body based on id field type
	let object_id_body = if id_is_option {
		quote! { self.id.unwrap_or(0) }
	} else {
		quote! { self.id }
	};

	let expanded = quote! {
		#input

		#[automatically_derived]
		impl ::reinhardt_taggit::Taggable for #name {
			fn content_type_name() -> &'static str {
				stringify!(#name)
			}

			fn object_id(&self) -> i64 {
				#object_id_body
			}
		}
	};

	TokenStream::from(expanded)
}

/// Check if a type is `Option<T>`
fn is_option_type(ty: &Type) -> bool {
	if let Type::Path(type_path) = ty
		&& let Some(segment) = type_path.path.segments.last()
	{
		return segment.ident == "Option";
	}
	false
}
