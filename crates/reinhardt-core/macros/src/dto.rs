//! Attribute macro implementation for `#[dto]`
//!
//! Absorbs the `cfg_attr(native, ...)` boilerplate required for DTOs shared
//! between native (server) and wasm (client) builds. See the public-facing
//! rustdoc on `crate::dto` in `lib.rs` for the user-facing contract.

use crate::crate_paths::get_reinhardt_crate;
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
	let first_schema_attr = input
		.attrs
		.iter()
		.position(|attr| attr.path().is_ident("schema"));

	for attr in &mut input.attrs {
		if attr.path().is_ident("schema") {
			*attr = wrap_in_cfg_attr_native(attr);
		}
	}

	let fields = match &mut input.data {
		Data::Struct(s) => match &mut s.fields {
			Fields::Named(f) => Some(&mut f.named),
			Fields::Unnamed(f) => Some(&mut f.unnamed),
			Fields::Unit => None,
		},
		Data::Enum(_) | Data::Union(_) => {
			return Err(syn::Error::new_spanned(
				&input.ident,
				"#[dto] can only be applied to structs",
			));
		}
	};

	if let Some(fields) = fields {
		for field in fields.iter_mut() {
			for attr in field.attrs.iter_mut() {
				if attr.path().is_ident("validate") || attr.path().is_ident("schema") {
					*attr = wrap_in_cfg_attr_native(attr);
				}
			}
		}
	}

	// Reject unconditional `#[derive(Validate)]` upfront. `Validate` lives
	// behind the `native` cfg, so an unconditional derive cannot resolve on wasm
	// builds and would duplicate the macro's `cfg_attr(native, derive(...))` on
	// native builds.
	if let Some(attr) = find_unconditional_derive(&input.attrs, is_validate_derive)? {
		return Err(syn::Error::new_spanned(
			attr,
			"#[dto] cannot be combined with unconditional `#[derive(Validate)]`. \
			 Remove the derive so #[dto] can emit it as `cfg_attr(native, ...)` for you, \
			 or replace it with `#[cfg_attr(native, derive(Validate))]`.",
		));
	}
	if with_schema
		&& let Some(attr) =
			find_unconditional_derive(&input.attrs, |path| is_schema_derive(path, &schema_path))?
	{
		return Err(syn::Error::new_spanned(
			attr,
			"#[dto(schema)] cannot be combined with `#[derive(Schema)]`. \
			 Remove the derive because #[dto(schema)] emits it for native builds.",
		));
	}

	let needs_validate = !has_native_derive(&input.attrs, is_validate_derive)?;
	let needs_schema = with_schema
		&& !has_native_derive(&input.attrs, |path| is_schema_derive(path, &schema_path))?;

	let mut derives: Punctuated<Path, Token![,]> = Punctuated::new();
	if needs_validate {
		derives.push(parse_quote!(#reinhardt::Validate));
	}
	if needs_schema {
		derives.push(parse_quote!(#reinhardt::rest::openapi::Schema));
	}

	if !derives.is_empty() {
		let new_attr: Attribute = parse_quote!(#[cfg_attr(native, derive(#derives))]);
		if let Some(index) = first_schema_attr {
			input.attrs.insert(index, new_attr);
		} else {
			input.attrs.push(new_attr);
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
/// Used to detect derives that would clash with the macro-emitted
/// `cfg_attr(native, derive(...))`.
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
///
/// Only inspects the `native` cfg branch — unconditional `#[derive(TraitName)]`
/// is handled separately by `find_unconditional_derive` and reported as an error.
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
		// First arg must be the `native` predicate (bare `native` Path).
		if !matches!(first, Meta::Path(p) if p.is_ident("native")) {
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

fn is_schema_derive(path: &Path, schema_path: &Path) -> bool {
	path.is_ident("Schema") || path.segments == schema_path.segments
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
		let unrelated: Path = parse_quote!(other_crate::Schema);

		assert!(is_schema_derive(&unqualified, &expected));
		assert!(is_schema_derive(&qualified, &expected));
		assert!(!is_schema_derive(&unrelated, &expected));
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
