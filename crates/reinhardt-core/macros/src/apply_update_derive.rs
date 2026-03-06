//! Derive macro implementation for `ApplyUpdate`

use crate::crate_paths::get_reinhardt_crate;
use proc_macro2::TokenStream;
use quote::quote;
use syn::parse::Parse;
use syn::{DeriveInput, Fields, Result, Type};

/// Check if a type is `Option<T>` and return the inner type
fn extract_option_inner(ty: &Type) -> Option<&Type> {
	if let Type::Path(type_path) = ty
		&& let Some(last_segment) = type_path.path.segments.last()
		&& last_segment.ident == "Option"
		&& let syn::PathArguments::AngleBracketed(args) = &last_segment.arguments
		&& let Some(syn::GenericArgument::Type(inner_ty)) = args.args.first()
	{
		return Some(inner_ty);
	}
	None
}

/// Parse target types from `#[apply_update_config(target(Type1, Type2))]`
fn parse_target_types(input: &DeriveInput) -> Result<Vec<syn::Path>> {
	for attr in &input.attrs {
		if attr.path().is_ident("apply_update_config") {
			// Parse as `target(Type1, Type2)`
			let mut targets = Vec::new();
			attr.parse_nested_meta(|meta| {
				if meta.path.is_ident("target") {
					let content;
					syn::parenthesized!(content in meta.input);
					let paths = content.parse_terminated(syn::Path::parse, syn::Token![,])?;
					targets.extend(paths);
					Ok(())
				} else {
					Err(meta.error("expected `target(...)`"))
				}
			})?;
			return Ok(targets);
		}
	}
	Err(syn::Error::new_spanned(
		&input.ident,
		"#[apply_update(...)] requires `target(...)` argument",
	))
}

/// Field configuration parsed from attributes
struct FieldConfig {
	skip: bool,
	rename: Option<syn::Ident>,
}

/// Parse field-level `#[apply_update(...)]` attributes
fn parse_field_config(field: &syn::Field) -> Result<FieldConfig> {
	let mut config = FieldConfig {
		skip: false,
		rename: None,
	};

	for attr in &field.attrs {
		if attr.path().is_ident("apply_update") {
			attr.parse_nested_meta(|meta| {
				if meta.path.is_ident("skip") {
					config.skip = true;
					Ok(())
				} else if meta.path.is_ident("rename") {
					let value = meta.value()?;
					let lit: syn::LitStr = value.parse()?;
					config.rename = Some(syn::Ident::new(&lit.value(), lit.span()));
					Ok(())
				} else {
					Err(meta.error("expected `skip` or `rename = \"...\"`"))
				}
			})?;
		}
	}

	Ok(config)
}

pub(crate) fn apply_update_derive_impl(input: DeriveInput) -> Result<TokenStream> {
	let reinhardt = get_reinhardt_crate();
	let struct_name = &input.ident;
	let targets = parse_target_types(&input)?;

	let fields = match &input.data {
		syn::Data::Struct(data) => match &data.fields {
			Fields::Named(named) => &named.named,
			_ => {
				return Err(syn::Error::new_spanned(
					struct_name,
					"ApplyUpdate can only be derived for structs with named fields",
				));
			}
		},
		_ => {
			return Err(syn::Error::new_spanned(
				struct_name,
				"ApplyUpdate can only be derived for structs",
			));
		}
	};

	let mut impls = Vec::new();

	for target in &targets {
		let mut assignments = Vec::new();

		for field in fields.iter() {
			let field_name = field.ident.as_ref().expect("named field");
			let config = parse_field_config(field)?;

			if config.skip {
				continue;
			}

			let target_field = config.rename.as_ref().unwrap_or(field_name);

			if extract_option_inner(&field.ty).is_some() {
				// Option<T> field: only apply when Some
				assignments.push(quote! {
					if let Some(v) = self.#field_name {
						target.#target_field = v;
					}
				});
			} else {
				// Non-Option field: always apply
				assignments.push(quote! {
					target.#target_field = self.#field_name;
				});
			}
		}

		impls.push(quote! {
			impl #reinhardt::ApplyUpdate<#target> for #struct_name {
				fn apply_to(self, target: &mut #target) {
					#(#assignments)*
				}
			}
		});
	}

	Ok(quote! { #(#impls)* })
}
