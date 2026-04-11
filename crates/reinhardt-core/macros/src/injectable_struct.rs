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
use quote::{format_ident, quote};
use syn::parse::Parser;
use syn::{Data, DeriveInput, Fields, Result, Type};

/// Check if `Clone` is already in a `#[derive(...)]` attribute
fn has_clone_derive(attrs: &[syn::Attribute]) -> bool {
	attrs.iter().any(|attr| {
		if !attr.path().is_ident("derive") {
			return false;
		}
		attr.parse_args_with(
			syn::punctuated::Punctuated::<syn::Path, syn::Token![,]>::parse_terminated,
		)
		.map(|paths| paths.iter().any(|p| p.is_ident("Clone")))
		.unwrap_or(false)
	})
}

/// Field information for processing
struct FieldInfo {
	name: syn::Ident,
	ty: Type,
	inject: bool,
	no_inject: Option<NoInjectOptions>,
	use_cache: bool,
	scope: InjectionScope,
}

/// Scope for struct-level `#[injectable]` registration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StructScope {
	Singleton,
	Request,
	Transient,
}

/// Parsed arguments for `#[injectable(scope = Singleton, prebuilt = true)]`
struct StructInjectableArgs {
	scope: Option<StructScope>,
	prebuilt: bool,
}

impl StructInjectableArgs {
	fn parse(args: proc_macro2::TokenStream) -> Result<Self> {
		if args.is_empty() {
			return Ok(Self {
				scope: None,
				prebuilt: false,
			});
		}

		let mut scope = None;
		let mut prebuilt = false;
		let mut seen_scope = false;
		let mut seen_prebuilt = false;

		let parsed = syn::punctuated::Punctuated::<syn::Meta, syn::Token![,]>::parse_terminated
			.parse2(args)?;

		for meta in &parsed {
			match meta {
				syn::Meta::NameValue(nv) if nv.path.is_ident("scope") => {
					if seen_scope {
						return Err(syn::Error::new_spanned(
							&nv.path,
							"duplicate argument: scope was already specified",
						));
					}
					seen_scope = true;
					if let syn::Expr::Path(expr_path) = &nv.value {
						let ident = expr_path.path.get_ident().ok_or_else(|| {
							syn::Error::new_spanned(
								&nv.value,
								"scope must be Singleton, Request, or Transient",
							)
						})?;
						scope = Some(match ident.to_string().as_str() {
							"Singleton" => StructScope::Singleton,
							"Request" => StructScope::Request,
							"Transient" => StructScope::Transient,
							_ => {
								return Err(syn::Error::new_spanned(
									ident,
									"scope must be Singleton, Request, or Transient",
								));
							}
						});
					} else {
						return Err(syn::Error::new_spanned(
							&nv.value,
							"scope must be Singleton, Request, or Transient",
						));
					}
				}
				syn::Meta::NameValue(nv) if nv.path.is_ident("prebuilt") => {
					if seen_prebuilt {
						return Err(syn::Error::new_spanned(
							&nv.path,
							"duplicate argument: prebuilt was already specified",
						));
					}
					seen_prebuilt = true;
					if let syn::Expr::Lit(expr_lit) = &nv.value {
						if let syn::Lit::Bool(lit_bool) = &expr_lit.lit {
							prebuilt = lit_bool.value;
						} else {
							return Err(syn::Error::new_spanned(
								&nv.value,
								"prebuilt must be true or false",
							));
						}
					} else {
						return Err(syn::Error::new_spanned(
							&nv.value,
							"prebuilt must be true or false",
						));
					}
				}
				_ => {
					return Err(syn::Error::new_spanned(
						meta,
						"unknown argument; expected scope or prebuilt",
					));
				}
			}
		}

		Ok(Self { scope, prebuilt })
	}

	fn scope_tokens(
		&self,
		di_crate: &proc_macro2::TokenStream,
	) -> Option<proc_macro2::TokenStream> {
		self.scope.map(|s| match s {
			StructScope::Singleton => quote! { #di_crate::DependencyScope::Singleton },
			StructScope::Request => quote! { #di_crate::DependencyScope::Request },
			StructScope::Transient => quote! { #di_crate::DependencyScope::Transient },
		})
	}
}

/// Implementation of the `#[injectable]` attribute macro for structs
///
/// Generates an `Injectable` trait implementation for structs with `#[inject]` fields.
/// Supports optional `scope` and `prebuilt` arguments for auto-registration.
pub(crate) fn injectable_struct_impl(
	args: proc_macro2::TokenStream,
	mut input: DeriveInput,
) -> Result<TokenStream> {
	let struct_args = StructInjectableArgs::parse(args)?;

	// prebuilt requires scope
	if struct_args.prebuilt && struct_args.scope.is_none() {
		return Err(syn::Error::new(
			proc_macro2::Span::call_site(),
			"prebuilt = true requires scope to be specified",
		));
	}

	// Remove #[injectable] attribute from the struct definition
	input
		.attrs
		.retain(|attr| !attr.path().is_ident("injectable"));

	let struct_name = &input.ident;
	let generics = &input.generics;
	let where_clause = &generics.where_clause;

	// Auto-derive Clone for DI-ready types (used by into_inner() and injectable_factory patterns)
	if !has_clone_derive(&input.attrs) {
		input.attrs.push(syn::parse_quote!(#[derive(Clone)]));
	}

	// Prebuilt mode: skip field validation and Injectable impl generation.
	// The struct is expected to have a manual Injectable impl and to be
	// manually registered in SingletonScope via set_arc() before resolution.
	// No inventory registration is emitted because the value is placed
	// into the scope cache explicitly (e.g., in configure_di()).
	if struct_args.prebuilt {
		return Ok(quote! { #input });
	}

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

	// Generate optional inventory registration when scope is specified.
	// Uses a named async fn instead of a closure because inventory::submit!
	// requires expressions valid in a static context.
	let registration = if let Some(scope_tokens) = struct_args.scope_tokens(&di_crate) {
		let type_name_str = struct_name.to_string();
		let factory_fn_name = syn::Ident::new(
			&format!("__injectable_factory_{}", struct_name),
			proc_macro2::Span::call_site(),
		);
		let register_fn_name = format_ident!("__reinhardt_register_{}", struct_name);
		quote! {
			#[allow(non_snake_case)]
			async fn #factory_fn_name(
				ctx: ::std::sync::Arc<#di_crate::InjectionContext>,
			) -> #di_crate::DiResult<#struct_name> {
				<#struct_name as #di_crate::Injectable>::inject(&ctx).await
			}

			#[allow(non_snake_case)]
			fn #register_fn_name(registry: &#di_crate::DependencyRegistry) {
				registry.register_async::<#struct_name, _, _>(#scope_tokens, #factory_fn_name);
				registry.register_type_name(
					::std::any::TypeId::of::<#struct_name>(),
					#type_name_str,
				);
			}

			#di_crate::inventory::submit! {
				#di_crate::DependencyRegistration::new::<#struct_name>(
					#type_name_str,
					#scope_tokens,
					#register_fn_name
				)
			}
		}
	} else {
		quote! {}
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

		#registration
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

#[cfg(test)]
mod tests {
	use super::*;
	use quote::quote;

	#[test]
	fn test_injectable_struct_no_args_unchanged() {
		// Arrange
		let args = quote! {};
		let input: DeriveInput = syn::parse2(quote! {
			struct Foo;
		})
		.unwrap();

		// Act
		let result = injectable_struct_impl(args, input);

		// Assert
		assert!(result.is_ok());
		let output = result.unwrap().to_string();
		// Should generate Injectable impl but no inventory::submit
		assert!(output.contains("Injectable"));
		assert!(!output.contains("inventory"));
	}

	#[test]
	fn test_injectable_struct_with_scope_generates_registration() {
		// Arrange
		let args = quote! { scope = Singleton };
		let input: DeriveInput = syn::parse2(quote! {
			struct Foo;
		})
		.unwrap();

		// Act
		let result = injectable_struct_impl(args, input);

		// Assert
		assert!(result.is_ok());
		let output = result.unwrap().to_string();
		assert!(output.contains("Injectable"));
		assert!(output.contains("inventory"));
		assert!(output.contains("DependencyRegistration"));
	}

	#[test]
	fn test_injectable_struct_prebuilt_skips_injectable_impl() {
		// Arrange
		let args = quote! { scope = Singleton, prebuilt = true };
		let input: DeriveInput = syn::parse2(quote! {
			struct Foo {
				name: String,
			}
		})
		.unwrap();

		// Act
		let result = injectable_struct_impl(args, input);

		// Assert
		assert!(result.is_ok());
		let output = result.unwrap().to_string();
		// Prebuilt mode emits only the struct definition
		assert!(!output.contains("Injectable for"));
		// No inventory registration -- value is manually placed in scope cache
		assert!(!output.contains("inventory"));
		assert!(!output.contains("DependencyRegistration"));
		// Should contain the struct definition
		assert!(output.contains("struct Foo"));
	}

	#[test]
	fn test_injectable_struct_prebuilt_emits_only_struct() {
		// Arrange
		let args = quote! { scope = Singleton, prebuilt = true };
		let input: DeriveInput = syn::parse2(quote! {
			struct MyService;
		})
		.unwrap();

		// Act
		let result = injectable_struct_impl(args, input);

		// Assert
		assert!(result.is_ok());
		let output = result.unwrap().to_string();
		// Prebuilt mode only emits the struct definition, no generated code
		assert!(output.contains("struct MyService"));
		assert!(!output.contains("Injectable for"));
		assert!(!output.contains("inventory"));
	}

	#[test]
	fn test_injectable_struct_prebuilt_without_scope_errors() {
		// Arrange
		let args = quote! { prebuilt = true };
		let input: DeriveInput = syn::parse2(quote! {
			struct Foo;
		})
		.unwrap();

		// Act
		let result = injectable_struct_impl(args, input);

		// Assert
		assert!(result.is_err());
		let err = result.unwrap_err().to_string();
		assert!(err.contains("scope"));
	}

	#[test]
	fn test_injectable_struct_duplicate_scope_errors() {
		// Arrange
		let args = quote! { scope = Singleton, scope = Request };
		let input: DeriveInput = syn::parse2(quote! {
			struct Foo;
		})
		.unwrap();

		// Act
		let result = injectable_struct_impl(args, input);

		// Assert
		assert!(result.is_err());
		let err = result.unwrap_err().to_string();
		assert!(err.contains("duplicate"));
	}

	#[test]
	fn test_injectable_struct_duplicate_prebuilt_errors() {
		// Arrange
		let args = quote! { scope = Singleton, prebuilt = true, prebuilt = false };
		let input: DeriveInput = syn::parse2(quote! {
			struct Foo;
		})
		.unwrap();

		// Act
		let result = injectable_struct_impl(args, input);

		// Assert
		assert!(result.is_err());
		let err = result.unwrap_err().to_string();
		assert!(err.contains("duplicate"));
	}

	#[test]
	fn test_injectable_struct_auto_derives_clone() {
		// Arrange
		let args = quote! {};
		let input: DeriveInput = syn::parse2(quote! {
			#[derive(Default)]
			struct Foo;
		})
		.unwrap();

		// Act
		let result = injectable_struct_impl(args, input);

		// Assert
		let output = result.unwrap();
		let file: syn::File = syn::parse2(output.clone()).expect("output should parse");
		let item_struct = file
			.items
			.iter()
			.find_map(|item| match item {
				syn::Item::Struct(s) if s.ident == "Foo" => Some(s),
				_ => None,
			})
			.expect("output should contain struct Foo");
		assert!(
			has_clone_derive(&item_struct.attrs),
			"Output should derive Clone: {output}"
		);
	}

	#[test]
	fn test_injectable_struct_skips_clone_when_already_derived() {
		// Arrange
		let args = quote! {};
		let input: DeriveInput = syn::parse2(quote! {
			#[derive(Clone, Default)]
			struct Foo;
		})
		.unwrap();

		// Act
		let result = injectable_struct_impl(args, input);

		// Assert
		let output = result.unwrap();
		let file: syn::File = syn::parse2(output.clone()).expect("output should parse");
		let item_struct = file
			.items
			.iter()
			.find_map(|item| match item {
				syn::Item::Struct(s) if s.ident == "Foo" => Some(s),
				_ => None,
			})
			.expect("output should contain struct Foo");
		let clone_count = item_struct
			.attrs
			.iter()
			.filter(|attr| attr.path().is_ident("derive"))
			.map(|attr| {
				attr.parse_args_with(
					syn::punctuated::Punctuated::<syn::Path, syn::Token![,]>::parse_terminated,
				)
				.map(|paths| paths.iter().filter(|p| p.is_ident("Clone")).count())
				.unwrap_or(0)
			})
			.sum::<usize>();
		assert_eq!(
			clone_count, 1,
			"Clone should appear exactly once in derive attributes: {output}"
		);
	}
}
