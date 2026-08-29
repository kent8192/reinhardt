//! Attribute macro implementation for `#[dto]`
//!
//! Emits shared `Validate` derives for DTOs used by native/server and wasm/client
//! builds while normalizing legacy native-only `Validate` derives. The optional
//! `schema` argument adds native-only OpenAPI schema support. See the public-facing
//! rustdoc on `crate::dto` in `lib.rs` for the user-facing contract.

use crate::crate_paths::{get_reinhardt_crate, get_reinhardt_rest_crate};
use proc_macro2::TokenStream;
use quote::quote;
use syn::{
	Attribute, Data, DeriveInput, Fields, Meta, Path, Result, Token, parse::Parser, parse_quote,
	punctuated::Punctuated,
};

pub(crate) fn dto_impl(args: TokenStream, mut input: DeriveInput) -> Result<TokenStream> {
	let with_schema = if args.is_empty() {
		false
	} else {
		let option: syn::Ident = syn::parse2(args.clone()).map_err(|_| {
			syn::Error::new_spanned(args.clone(), "#[dto] accepts only the `schema` option")
		})?;
		if option != "schema" {
			return Err(syn::Error::new_spanned(
				option,
				"#[dto] accepts only the `schema` option",
			));
		}
		true
	};

	let reinhardt = get_reinhardt_crate();
	let schema_path: Path = parse_quote!(#reinhardt::rest::openapi::Schema);
	let direct_rest_schema_path = if with_schema {
		get_reinhardt_rest_crate().map(|rest| parse_quote!(#rest::openapi::Schema))
	} else {
		None
	};

	match &mut input.data {
		Data::Struct(struct_data) => match &mut struct_data.fields {
			Fields::Named(fields) => {
				for field in &mut fields.named {
					for attr in &mut field.attrs {
						if attr.path().is_ident("schema") {
							*attr = wrap_in_cfg_attr_native(attr);
						}
					}
				}
			}
			Fields::Unnamed(_) | Fields::Unit => {
				return Err(syn::Error::new_spanned(
					&input.ident,
					"#[dto] requires a struct with named fields",
				));
			}
		},
		Data::Enum(_) | Data::Union(_) => {
			return Err(syn::Error::new_spanned(
				&input.ident,
				"#[dto] can only be applied to structs",
			));
		}
	}

	let has_unconditional_validate =
		find_unconditional_derive(&input.attrs, is_validate_derive)?.is_some();
	remove_native_validate_derives(&mut input.attrs, "Validate")?;
	if !has_unconditional_validate {
		let new_attr: Attribute = parse_quote!(#[derive(#reinhardt::Validate)]);
		input.attrs.push(new_attr);
	}

	let first_schema_attr = input
		.attrs
		.iter()
		.position(|attr| attr.path().is_ident("schema"));
	let has_unconditional_schema = if with_schema {
		find_unconditional_derive(&input.attrs, |path| {
			is_schema_derive(path, &schema_path, direct_rest_schema_path.as_ref())
		})?
	} else {
		None
	};
	if with_schema
		&& let Some(attr) = find_unconditional_derive(&input.attrs, |path| path.is_ident("Schema"))?
	{
		return Err(syn::Error::new_spanned(
			attr,
			"#[dto(schema)] cannot be combined with `#[derive(Schema)]`. \
			 Remove the derive because #[dto(schema)] emits it for native builds.",
		));
	}

	let needs_schema = with_schema
		&& has_unconditional_schema.is_none()
		&& !has_native_derive(&input.attrs, |path| {
			is_schema_derive(path, &schema_path, direct_rest_schema_path.as_ref())
		})?;
	if needs_schema {
		let new_attr: Attribute =
			parse_quote!(#[cfg_attr(native, derive(#reinhardt::rest::openapi::Schema))]);
		if let Some(index) = first_schema_attr {
			input.attrs.insert(index, new_attr);
		} else {
			input.attrs.push(new_attr);
		}
	}

	for attr in &mut input.attrs {
		if attr.path().is_ident("schema") {
			*attr = wrap_in_cfg_attr_native(attr);
		}
	}

	let to_schema_import = if with_schema {
		quote! {
			#[cfg(native)]
			// The generated Schema implementation uses trait-method syntax at module scope.
			#[allow(unused_imports)]
			use #reinhardt::rest::openapi::ToSchema as _;
		}
	} else {
		quote! {}
	};

	Ok(quote! {
		#input
		#to_schema_import
	})
}

fn wrap_in_cfg_attr_native(attr: &Attribute) -> Attribute {
	let meta = &attr.meta;
	parse_quote!(#[cfg_attr(native, #meta)])
}

/// Returns the first unconditional derive matching `matches` on `attrs`, if any.
fn find_unconditional_derive<F>(attrs: &[Attribute], matches: F) -> Result<Option<&Attribute>>
where
	F: Fn(&Path) -> bool,
{
	for attr in attrs {
		if !attr.path().is_ident("derive") {
			continue;
		}
		let Meta::List(list) = &attr.meta else {
			continue;
		};
		let derives =
			Punctuated::<Path, Token![,]>::parse_terminated.parse2(list.tokens.clone())?;
		if derives.iter().any(&matches) {
			return Ok(Some(attr));
		}
	}
	Ok(None)
}

/// Returns true if `attrs` already contains a matching native derive.
fn has_native_derive<F>(attrs: &[Attribute], matches: F) -> Result<bool>
where
	F: Fn(&Path) -> bool,
{
	for attr in attrs {
		if !attr.path().is_ident("cfg_attr") {
			continue;
		}
		let Meta::List(list) = &attr.meta else {
			continue;
		};
		let nested = Punctuated::<Meta, Token![,]>::parse_terminated.parse2(list.tokens.clone())?;
		let mut iter = nested.iter();
		let Some(first) = iter.next() else {
			continue;
		};
		if !matches!(first, Meta::Path(path) if path.is_ident("native")) {
			continue;
		}
		for inner in iter {
			let Meta::List(inner_list) = inner else {
				continue;
			};
			if !inner_list.path.is_ident("derive") {
				continue;
			}
			let derives = Punctuated::<Path, Token![,]>::parse_terminated
				.parse2(inner_list.tokens.clone())?;
			if derives.iter().any(&matches) {
				return Ok(true);
			}
		}
	}
	Ok(false)
}

fn remove_native_validate_derives(attrs: &mut Vec<Attribute>, trait_name: &str) -> Result<()> {
	let mut normalized = Vec::with_capacity(attrs.len());

	for attr in attrs.drain(..) {
		if !attr.path().is_ident("cfg_attr") {
			normalized.push(attr);
			continue;
		}
		let Meta::List(list) = &attr.meta else {
			normalized.push(attr);
			continue;
		};
		let nested = Punctuated::<Meta, Token![,]>::parse_terminated.parse2(list.tokens.clone())?;
		let mut iter = nested.into_iter();
		let Some(first) = iter.next() else {
			normalized.push(attr);
			continue;
		};
		if !matches!(&first, Meta::Path(path) if path.is_ident("native")) {
			normalized.push(attr);
			continue;
		}

		let mut rebuilt = Punctuated::<Meta, Token![,]>::new();
		rebuilt.push(first);
		for inner in iter {
			let Meta::List(inner_list) = inner else {
				rebuilt.push(inner);
				continue;
			};
			if !inner_list.path.is_ident("derive") {
				rebuilt.push(Meta::List(inner_list));
				continue;
			}

			let derives = Punctuated::<Path, Token![,]>::parse_terminated
				.parse2(inner_list.tokens.clone())?;
			let mut filtered = Punctuated::<Path, Token![,]>::new();
			for derive in derives {
				if derive
					.segments
					.last()
					.is_some_and(|segment| segment.ident == trait_name)
				{
					continue;
				}
				filtered.push(derive);
			}
			if !filtered.is_empty() {
				let derive_meta: Meta = parse_quote!(derive(#filtered));
				rebuilt.push(derive_meta);
			}
		}

		if rebuilt.len() > 1 {
			let new_attr: Attribute = parse_quote!(#[cfg_attr(#rebuilt)]);
			normalized.push(new_attr);
		}
	}

	*attrs = normalized;
	Ok(())
}

fn is_schema_derive(
	path: &Path,
	schema_path: &Path,
	direct_rest_schema_path: Option<&Path>,
) -> bool {
	path.is_ident("Schema")
		|| path.segments == schema_path.segments
		|| direct_rest_schema_path.is_some_and(|candidate| path.segments == candidate.segments)
}

fn is_validate_derive(path: &Path) -> bool {
	path.segments
		.last()
		.is_some_and(|segment| segment.ident == "Validate")
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn schema_derive_matching_rejects_unrelated_paths() {
		let expected: Path = parse_quote!(::reinhardt::rest::openapi::Schema);
		let unqualified: Path = parse_quote!(Schema);
		let qualified: Path = parse_quote!(reinhardt::rest::openapi::Schema);
		let direct_rest: Path = parse_quote!(reinhardt_rest::openapi::Schema);
		let unrelated: Path = parse_quote!(other_crate::Schema);

		assert!(is_schema_derive(
			&unqualified,
			&expected,
			Some(&direct_rest)
		));
		assert!(is_schema_derive(&qualified, &expected, Some(&direct_rest)));
		assert!(is_schema_derive(
			&direct_rest,
			&expected,
			Some(&direct_rest)
		));
		assert!(!is_schema_derive(&direct_rest, &expected, None));
		assert!(!is_schema_derive(&unrelated, &expected, Some(&direct_rest)));
	}

	#[test]
	fn validate_derive_matching_accepts_qualified_paths() {
		let unqualified: Path = parse_quote!(Validate);
		let qualified: Path = parse_quote!(reinhardt_macros::Validate);
		let unrelated: Path = parse_quote!(other_crate::Schema);

		assert!(is_validate_derive(&unqualified));
		assert!(is_validate_derive(&qualified));
		assert!(!is_validate_derive(&unrelated));
	}
}
