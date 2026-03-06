//! Attribute macro implementation for `#[apply_update(...)]`

use crate::crate_paths::get_reinhardt_crate;
use proc_macro2::TokenStream;
use quote::quote;
use syn::{Attribute, ItemStruct, Result};

pub(crate) fn apply_update_attribute_impl(
	args: TokenStream,
	mut input: ItemStruct,
) -> Result<TokenStream> {
	let reinhardt = get_reinhardt_crate();

	// Check if #[derive(ApplyUpdate)] already exists (avoid double processing)
	let has_derive_apply_update = input.attrs.iter().any(|attr| {
		if attr.path().is_ident("derive")
			&& let syn::Meta::List(meta_list) = &attr.meta
		{
			if let Ok(paths) = meta_list.parse_args_with(
				syn::punctuated::Punctuated::<syn::Path, syn::Token![,]>::parse_terminated,
			) {
				return paths.iter().any(|path| {
					path.segments
						.last()
						.is_some_and(|seg| seg.ident == "ApplyUpdate")
				});
			}
			return false;
		}
		false
	});

	if has_derive_apply_update {
		// Already has #[derive(ApplyUpdate)], just return input unchanged
		return Ok(quote! { #input });
	}

	// Create #[apply_update_config(...)] helper attribute with the arguments
	let config_attr: Attribute = if args.is_empty() {
		syn::parse_quote! { #[apply_update_config] }
	} else {
		syn::parse_quote! { #[apply_update_config(#args)] }
	};

	// Build derive attribute with ApplyUpdate derive macro
	let apply_update_path = quote!(#reinhardt::macros::ApplyUpdate);

	// Find existing derive attribute to merge with, or create a new one
	let existing_derive_idx = input.attrs.iter().position(|attr| {
		attr.path().is_ident("derive") && matches!(&attr.meta, syn::Meta::List(_))
	});

	if let Some(idx) = existing_derive_idx {
		// Merge ApplyUpdate into the existing derive attribute
		if let syn::Meta::List(ref meta_list) = input.attrs[idx].meta {
			let existing_tokens = &meta_list.tokens;
			let new_derive_attr: Attribute =
				syn::parse_quote! { #[derive(#apply_update_path, #existing_tokens)] };
			input.attrs[idx] = new_derive_attr;
		}
	} else {
		// No existing derive attribute, create a new one
		let derive_attr: Attribute = syn::parse_quote! { #[derive(#apply_update_path)] };
		input.attrs.insert(0, derive_attr);
	}

	// Add the helper attribute AFTER the derive
	let config_insert_pos = if let Some(idx) = existing_derive_idx {
		idx + 1
	} else {
		1
	};
	input.attrs.insert(config_insert_pos, config_attr);

	Ok(quote! { #input })
}
