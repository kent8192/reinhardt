use proc_macro2::TokenStream;
use quote::quote;
use syn::{ItemStruct, Result, parse::Parse, parse::ParseStream};

use crate::crate_paths::{get_inventory_crate, get_reinhardt_commands_crate};

/// Parsed arguments for `#[hook(on = <target>)]`.
struct HookArgs {
	target: syn::Ident,
}

impl Parse for HookArgs {
	fn parse(input: ParseStream) -> Result<Self> {
		let on_ident: syn::Ident = input.parse()?;
		if on_ident != "on" {
			return Err(syn::Error::new(on_ident.span(), "expected `on`"));
		}
		let _eq: syn::Token![=] = input.parse()?;
		let target: syn::Ident = input.parse()?;
		Ok(HookArgs { target })
	}
}

/// Implementation for `#[hook(on = runserver)]`.
///
/// Generates `inventory::submit!` registration for the annotated struct.
/// The struct must implement the corresponding hook trait.
pub(crate) fn hook_impl(args: TokenStream, input: ItemStruct) -> Result<TokenStream> {
	let args: HookArgs = syn::parse2(args)?;
	let struct_name = &input.ident;
	let type_name_str = struct_name.to_string();

	// Validate: only unit structs (no fields) are supported
	if !input.fields.is_empty() {
		return Err(syn::Error::new_spanned(
			&input.fields,
			"#[hook] can only be applied to unit structs (structs without fields)",
		));
	}

	// Validate: no generic parameters
	if !input.generics.params.is_empty() {
		return Err(syn::Error::new_spanned(
			&input.generics,
			"#[hook] does not support generic structs",
		));
	}

	match args.target.to_string().as_str() {
		"runserver" => {
			let inventory_crate = get_inventory_crate();
			let commands_crate = get_reinhardt_commands_crate();

			Ok(quote! {
				#input

				#inventory_crate::submit! {
					#commands_crate::RunserverHookRegistration::__macro_new(
						|| Box::new(#struct_name),
						#type_name_str,
					)
				}
			})
		}
		other => Err(syn::Error::new(
			args.target.span(),
			format!(
				"unknown hook target `{}`. Expected one of: runserver",
				other
			),
		)),
	}
}
