//! Injectable attribute macro for structs
//!
//! Provides `#[injectable]` attribute macro that generates `Injectable` trait
//! implementation for structs with `#[inject]` fields.

use crate::crate_paths::{get_async_trait_crate, get_reinhardt_di_crate};
use crate::injectable_common::{
	DefaultValue, InjectionScope, NoInjectOptions, is_inject_attr, is_no_inject_attr,
	parse_inject_options, parse_no_inject_options,
};
use proc_macro2::TokenStream;
use quote::quote;
use syn::{Data, DeriveInput, Fields, Result, Type};

/// Field information for processing
struct FieldInfo {
	name: syn::Ident,
	ty: Type,
	inject: bool,
	no_inject: Option<NoInjectOptions>,
	use_cache: bool,
	scope: InjectionScope,
}

/// Implementation of the `#[injectable]` attribute macro for structs
///
/// Generates an `Injectable` trait implementation for structs with `#[inject]` fields.
pub(crate) fn injectable_struct_impl(mut input: DeriveInput) -> Result<TokenStream> {
	// Remove #[injectable] attribute from the struct definition
	input
		.attrs
		.retain(|attr| !attr.path().is_ident("injectable"));

	let struct_name = &input.ident;
	let generics = &input.generics;
	let where_clause = &generics.where_clause;

	// Only support structs
	let fields = match &mut input.data {
		Data::Struct(data_struct) => match &mut data_struct.fields {
			Fields::Named(fields) => Some(&mut fields.named),
			Fields::Unit => None, // Unit struct: struct Foo;
			Fields::Unnamed(_) => {
				return Err(syn::Error::new_spanned(
					struct_name,
					"#[injectable] does not support tuple structs",
				));
			}
		},
		_ => {
			return Err(syn::Error::new_spanned(
				struct_name,
				"#[injectable] can only be applied to structs",
			));
		}
	};

	// Process all fields (if any) and remove #[inject] and #[no_inject] attributes
	let mut field_infos = Vec::new();
	if let Some(fields) = fields {
		for field in fields.iter_mut() {
			let name = field
				.ident
				.clone()
				.ok_or_else(|| syn::Error::new_spanned(&*field, "Field must have a name"))?;
			let ty = field.ty.clone();

			let inject = field.attrs.iter().any(is_inject_attr);
			let no_inject_opts = parse_no_inject_options(&field.attrs);

			// Validation: Error if both attributes are present
			if inject && no_inject_opts.is_some() {
				return Err(syn::Error::new_spanned(
					&*field,
					"Field cannot have both #[inject] and #[no_inject] attributes",
				));
			}

			// Validation: Error if neither attribute is present
			if !inject && no_inject_opts.is_none() {
				return Err(syn::Error::new_spanned(
					&*field,
					"Field must have either #[inject] or #[no_inject] attribute. Use #[inject] for dependency injection, or #[no_inject] for default initialization.",
				));
			}

			// #[no_inject] without default value -> must be Option<T>
			if let Some(ref opts) = no_inject_opts
				&& matches!(opts.default, DefaultValue::None)
			{
				validate_option_type(&ty, &*field)?;
			}

			let options = if inject {
				parse_inject_options(&field.attrs)
			} else {
				Default::default()
			};

			// Remove #[inject] and #[no_inject] attributes from the field
			field
				.attrs
				.retain(|attr| !is_inject_attr(attr) && !is_no_inject_attr(attr));

			field_infos.push(FieldInfo {
				name,
				ty,
				inject,
				no_inject: no_inject_opts,
				use_cache: options.use_cache,
				scope: options.scope,
			});
		}
	}

	// Get dynamic crate paths
	let di_crate = get_reinhardt_di_crate();
	// Fixes #791: Use dynamic resolution instead of hardcoded ::async_trait
	let async_trait = get_async_trait_crate();

	// Generate injection code for #[inject] fields
	let mut inject_stmts = Vec::new();
	for field_info in &field_infos {
		if field_info.inject {
			let name = &field_info.name;
			let ty = &field_info.ty;
			let use_cache = field_info.use_cache;

			let resolve_call = match field_info.scope {
				InjectionScope::Singleton => {
					quote! {
						{
							// Check singleton cache first
							if let Some(cached) = __di_ctx.singleton_scope().get::<#ty>() {
								(*cached).clone()
							} else {
								let __injected = if #use_cache {
									#di_crate::Injected::<#ty>::resolve(__di_ctx).await
								} else {
									#di_crate::Injected::<#ty>::resolve_uncached(__di_ctx).await
								}
								.map_err(|e| {
									tracing::debug!(
										field = stringify!(#name),
										target_type = stringify!(#struct_name),
										"dependency injection resolution failed"
									);
									e
								})?;
								let value = (*__injected).clone();
								__di_ctx.singleton_scope().set(value.clone());
								value
							}
						}
					}
				}
				InjectionScope::Request => {
					quote! {
						{
							let __injected = if #use_cache {
								#di_crate::Injected::<#ty>::resolve(__di_ctx).await
							} else {
								#di_crate::Injected::<#ty>::resolve_uncached(__di_ctx).await
							}
							.map_err(|e| {
								tracing::debug!(
									field = stringify!(#name),
									target_type = stringify!(#struct_name),
									"dependency injection resolution failed"
								);
								e
							})?;
							(*__injected).clone()
						}
					}
				}
			};

			inject_stmts.push(quote! {
				let #name = #resolve_call;
			});
		}
	}

	// Generate field initialization
	let mut field_inits = Vec::new();
	for field_info in &field_infos {
		let name = &field_info.name;
		if field_info.inject {
			// Use the injected value
			field_inits.push(quote! { #name });
		} else if let Some(ref no_inject_opts) = field_info.no_inject {
			// Use #[no_inject] default value
			let init_expr = match &no_inject_opts.default {
				DefaultValue::DefaultTrait => {
					quote! { #name: Default::default() }
				}
				DefaultValue::Expression(expr) => {
					quote! { #name: #expr }
				}
				DefaultValue::None => {
					quote! { #name: None }
				}
			};
			field_inits.push(init_expr);
		} else {
			// Should not reach here due to validation
			unreachable!("Field must have either #[inject] or #[no_inject]");
		}
	}

	// Generate the Injectable implementation
	let struct_init = if field_infos.is_empty() {
		// Unit struct: struct Foo;
		quote! { Self }
	} else {
		// Named fields struct
		quote! {
			Self {
				#(#field_inits),*
			}
		}
	};

	// Keep the original struct definition and add Injectable implementation
	let expanded = quote! {
		#input

		#[#async_trait::async_trait]
		impl #generics #di_crate::Injectable for #struct_name #generics #where_clause {
			async fn inject(__di_ctx: &#di_crate::InjectionContext)
				-> #di_crate::DiResult<Self>
			{
				#(#inject_stmts)*

				Ok(#struct_init)
			}
		}
	};

	Ok(expanded)
}

/// Validate that a type is `Option<T>`
fn validate_option_type(ty: &Type, field: &syn::Field) -> Result<()> {
	if let Type::Path(type_path) = ty
		&& let Some(segment) = type_path.path.segments.last()
		&& segment.ident == "Option"
	{
		return Ok(());
	}

	Err(syn::Error::new_spanned(
		field,
		"Field with #[no_inject] but no default value must have type Option<T>",
	))
}
