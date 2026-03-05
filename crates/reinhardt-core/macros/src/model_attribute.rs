//! Attribute macro implementation for `#[model(...)]`

use crate::crate_paths::{get_reinhardt_crate, get_reinhardt_orm_crate};
use proc_macro2::TokenStream;
use quote::quote;
use syn::{Attribute, Field, ItemStruct, Result, Type};

/// Extract target type from ForeignKeyField<T> or OneToOneField<T>
fn extract_fk_target_type(ty: &Type) -> Option<&Type> {
	if let Type::Path(type_path) = ty
		&& let Some(last_segment) = type_path.path.segments.last()
		&& (last_segment.ident == "ForeignKeyField" || last_segment.ident == "OneToOneField")
		&& let syn::PathArguments::AngleBracketed(args) = &last_segment.arguments
		&& let Some(syn::GenericArgument::Type(inner_ty)) = args.args.first()
	{
		return Some(inner_ty);
	}
	None
}

pub(crate) fn model_attribute_impl(
	args: TokenStream,
	mut input: ItemStruct,
) -> Result<TokenStream> {
	// Get dynamic crate paths for code generation
	let reinhardt = get_reinhardt_crate();
	let orm_crate = get_reinhardt_orm_crate();

	// Check if #[derive(Model)] already exists (avoid double processing)
	// Parse derive tokens properly instead of fragile string matching
	let has_derive_model = input.attrs.iter().any(|attr| {
		if attr.path().is_ident("derive")
			&& let syn::Meta::List(meta_list) = &attr.meta
		{
			// Parse the token stream as a punctuated list of paths
			if let Ok(paths) = meta_list.parse_args_with(
				syn::punctuated::Punctuated::<syn::Path, syn::Token![,]>::parse_terminated,
			) {
				return paths.iter().any(|path| {
					// Match exact "Model" or paths ending in "Model" (e.g., reinhardt::macros::Model)
					path.segments.last().is_some_and(|seg| seg.ident == "Model")
				});
			}
			return false;
		}
		false
	});

	if has_derive_model {
		// Already has #[derive(Model)], just return input unchanged
		// The derive macro will read #[model(...)] helper attribute
		return Ok(quote! { #input });
	}

	/// Check if a specific trait is already in `#[derive(...)]` attributes
	fn has_derive_trait(attrs: &[Attribute], trait_name: &str) -> bool {
		attrs.iter().any(|attr| {
			if attr.path().is_ident("derive")
				&& let syn::Meta::List(meta_list) = &attr.meta
			{
				// Parse the token stream as a punctuated list of paths
				if let Ok(paths) = meta_list.parse_args_with(
					syn::punctuated::Punctuated::<syn::Path, syn::Token![,]>::parse_terminated,
				) {
					return paths.iter().any(|path| {
						path.segments
							.last()
							.is_some_and(|seg| seg.ident == trait_name)
					});
				}
				return false;
			}
			false
		})
	}

	/// Check if field has `#[rel(foreign_key, ...)]` or `#[rel(one_to_one, ...)]` attribute
	fn has_fk_or_one_to_one_rel(attrs: &[Attribute]) -> bool {
		attrs.iter().any(|attr| {
			if attr.path().is_ident("rel")
				&& let syn::Meta::List(meta_list) = &attr.meta
			{
				let tokens_str = meta_list.tokens.to_string();
				return tokens_str.contains("foreign_key") || tokens_str.contains("one_to_one");
			}
			false
		})
	}

	// Collect existing field names to avoid duplicates
	let existing_field_names: std::collections::HashSet<String> =
		if let syn::Fields::Named(ref fields) = input.fields {
			fields
				.named
				.iter()
				.filter_map(|f| f.ident.as_ref().map(|i| i.to_string()))
				.collect()
		} else {
			std::collections::HashSet::new()
		};

	// Validate that users have not manually defined _id fields for relationships
	if let syn::Fields::Named(ref fields) = input.fields {
		for field in fields.named.iter() {
			if has_fk_or_one_to_one_rel(&field.attrs)
				&& let Some(field_name) = &field.ident
			{
				let id_field_name_str = format!("{}_id", field_name);
				if existing_field_names.contains(&id_field_name_str) {
					return Err(syn::Error::new_spanned(
						field,
						format!(
							"Field '{}' must not be manually defined. It will be auto-generated from the '{}' relationship field.",
							id_field_name_str, field_name
						),
					));
				}
			}
		}
	}

	// Collect FK fields and generate corresponding _id fields
	// Only generate _id fields for fields with #[rel(foreign_key, ...)] or #[rel(one_to_one, ...)]
	let mut fk_id_fields: Vec<Field> = Vec::new();
	// Track generated _id field names to prevent duplicates
	let mut generated_id_field_names: std::collections::HashSet<String> =
		std::collections::HashSet::new();

	if let syn::Fields::Named(ref fields) = input.fields {
		for field in fields.named.iter() {
			// Check if this field has #[rel(foreign_key, ...)] or #[rel(one_to_one, ...)]
			if has_fk_or_one_to_one_rel(&field.attrs)
				&& let Some(field_name) = &field.ident
				&& let Some(target_ty) = extract_fk_target_type(&field.ty)
			{
				let id_field_name_str = format!("{}_id", field_name);

				// Only add if not already defined by user OR already generated
				if !existing_field_names.contains(&id_field_name_str)
					&& !generated_id_field_names.contains(&id_field_name_str)
				{
					let id_field_name = syn::Ident::new(&id_field_name_str, field_name.span());

					// Generate _id field with the target model's PrimaryKey type
					// The type will be resolved at compile time via Model trait
					let new_field: Field = syn::parse_quote! {
						#[serde(default)]
						#id_field_name: <#target_ty as #orm_crate::Model>::PrimaryKey
					};

					fk_id_fields.push(new_field);
					generated_id_field_names.insert(id_field_name_str);
				}
			}
		}
	}

	// Process struct fields to add #[serde(skip)] to ManyToMany and ForeignKey fields
	if let syn::Fields::Named(ref mut fields) = input.fields {
		for field in fields.named.iter_mut() {
			// Check if this field has #[rel(many_to_many, ...)] attribute
			let has_many_to_many = field.attrs.iter().any(|attr| {
				if attr.path().is_ident("rel") {
					// Parse the attribute to check for many_to_many
					if let syn::Meta::List(meta_list) = &attr.meta {
						let tokens_str = meta_list.tokens.to_string();
						return tokens_str.contains("many_to_many");
					}
				}
				false
			});

			// Check if this is a ForeignKey or OneToOne field
			let is_fk_field = extract_fk_target_type(&field.ty).is_some();

			if has_many_to_many || is_fk_field {
				// Check if #[serde(skip)] already exists
				let has_serde_skip = field.attrs.iter().any(|attr| {
					if attr.path().is_ident("serde")
						&& let syn::Meta::List(meta_list) = &attr.meta
					{
						let tokens_str = meta_list.tokens.to_string();
						return tokens_str.contains("skip");
					}
					false
				});

				// Add #[serde(skip)] if not already present
				if !has_serde_skip {
					let serde_skip_attr: Attribute = syn::parse_quote! { #[serde(skip)] };
					field.attrs.push(serde_skip_attr);
				}
			}
		}

		// Add generated _id fields to the struct
		for fk_field in fk_id_fields {
			fields.named.push(fk_field);
		}
	}

	// Create a #[model_config(...)] helper attribute with the arguments
	// Using model_config instead of model to avoid name collision with the attribute macro
	let config_attr: Attribute = if args.is_empty() {
		syn::parse_quote! { #[model_config] }
	} else {
		syn::parse_quote! { #[model_config(#args)] }
	};

	// Build derive attribute with Model derive macro
	// Model must be first for proper attribute processing
	// Use reinhardt::macros::Model for hierarchical imports
	// (reinhardt::Model refers to the trait, not the derive macro)
	let model_path = quote!(#reinhardt::macros::Model);

	// Check which common traits need to be added
	// Note: Eq and Hash are NOT included by default because:
	// - Not all types implement Eq/Hash (e.g., f64, f32)
	// - Most models don't need these traits for database operations
	// - Users can manually add them when needed
	// Note: Serialize and Deserialize are NOT included by default because:
	// - They require serde to be in scope at the call site
	// - The derive attribute's scope doesn't inherit the caller's imports
	// - Users should manually add #[derive(Serialize, Deserialize)] when needed
	let required_traits = ["Debug", "Clone", "PartialEq"];

	let mut additional_traits = Vec::new();
	for &trait_name in &required_traits {
		if !has_derive_trait(&input.attrs, trait_name) {
			additional_traits.push(trait_name);
		}
	}

	// Find existing derive attribute to merge with, or create a new one
	// This prevents duplicate trait errors when user already has #[derive(...)]
	let existing_derive_idx = input.attrs.iter().position(|attr| {
		attr.path().is_ident("derive") && matches!(&attr.meta, syn::Meta::List(_))
	});

	if let Some(idx) = existing_derive_idx {
		// Merge Model and additional traits into the existing derive attribute
		// Only add traits that are not already present in the existing derive
		if let syn::Meta::List(ref meta_list) = input.attrs[idx].meta {
			let existing_tokens = &meta_list.tokens;
			// Build the new derive attribute with Model first, then additional traits, then existing
			let new_derive_attr: Attribute = if additional_traits.is_empty() {
				// No additional traits needed, just add Model before existing
				syn::parse_quote! { #[derive(#model_path, #existing_tokens)] }
			} else {
				// Add Model first, then only the additional traits not already present
				let traits_str = additional_traits.join(", ");
				let tokens: TokenStream = traits_str
					.parse()
					.expect("Failed to parse derive traits - this is a bug");
				syn::parse_quote! { #[derive(#model_path, #tokens, #existing_tokens)] }
			};
			input.attrs[idx] = new_derive_attr;
		}
	} else {
		// No existing derive attribute, create a new one
		let derive_attr: Attribute = if additional_traits.is_empty() {
			syn::parse_quote! { #[derive(#model_path)] }
		} else {
			let traits_str = additional_traits.join(", ");
			let tokens: TokenStream = traits_str
				.parse()
				.expect("Failed to parse derive traits - this is a bug");
			syn::parse_quote! { #[derive(#model_path, #tokens)] }
		};
		// Insert at the beginning to ensure Model is processed first
		input.attrs.insert(0, derive_attr);
	}

	// Add the helper attribute AFTER the derive
	// Position depends on whether we merged into existing derive or created new one
	let config_insert_pos = if let Some(idx) = existing_derive_idx {
		// Merged into existing derive, insert after it
		idx + 1
	} else {
		// Created new derive at position 0, insert at position 1
		1
	};
	input.attrs.insert(config_insert_pos, config_attr);

	// Note: We don't generate auto-imports here because:
	// 1. Each #[model] usage would generate duplicate imports in the same module
	// 2. The Model derive macro uses absolute paths (::reinhardt::db::orm::Model etc.)
	// 3. derive(Serialize, Deserialize) doesn't require explicit use statements
	// Users should import serde traits themselves if needed for non-derive usage

	Ok(quote! { #input })
}
