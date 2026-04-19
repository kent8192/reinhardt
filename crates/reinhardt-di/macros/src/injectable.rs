//! Implementation of the `#[injectable]` macro

use crate::crate_paths::get_reinhardt_di_crate;
use crate::utils::MacroArgs;
use proc_macro2::TokenStream;
use quote::quote;
use syn::{DeriveInput, Fields, GenericArgument, PathArguments, Result, Type};

/// Check if a field has `#[inject]` attribute
fn has_inject_attr(field: &syn::Field) -> bool {
	field
		.attrs
		.iter()
		.any(|attr| attr.path().is_ident("inject"))
}

/// Check if a field has `#[no_inject]` attribute
fn has_no_inject_attr(field: &syn::Field) -> bool {
	field
		.attrs
		.iter()
		.any(|attr| attr.path().is_ident("no_inject"))
}

/// Check if field should use cache (always true by default)
fn should_use_cache(_field: &syn::Field) -> bool {
	// Always use cache for injected fields
	// Future: Could support per-field cache control with additional attributes
	true
}

/// Injection field type classification
#[derive(Debug, Clone)]
enum InjectionType {
	/// `Injected<T>` - required dependency (deprecated, use `Depends<T>`)
	Injected(Type),
	/// `OptionalInjected<T>` (= `Option<Injected<T>>`) - optional dependency (deprecated)
	OptionalInjected(Type),
	/// `Depends<T>` - required dependency (preferred)
	Depends(Type),
	/// `Option<Depends<T>>` - optional dependency (preferred)
	OptionalDepends(Type),
}

/// Check whether a type path likely originates from the `reinhardt_di` crate.
///
/// In proc-macro context, full path resolution is not available. This heuristic
/// inspects the path segments preceding the terminal identifier for known
/// `reinhardt_di` / `reinhardt` prefixes. When the type is referenced with a
/// bare identifier (single segment, e.g. `Injected<T>`), we accept it because
/// the user likely has `use reinhardt_di::Injected` in scope. Multi-segment
/// paths that do NOT contain a recognized prefix are rejected to prevent
/// accidental misclassification of user-defined types with the same name.
fn is_likely_reinhardt_di_path(
	segments: &syn::punctuated::Punctuated<syn::PathSegment, syn::token::PathSep>,
) -> bool {
	// Single-segment path (e.g. `Injected<T>`) — assume it was imported from
	// `reinhardt_di`. This is the normal usage pattern.
	if segments.len() <= 1 {
		return true;
	}

	// Multi-segment path: check that at least one prefix segment matches a known
	// reinhardt crate module name.
	let known_prefixes = ["reinhardt_di", "reinhardt", "crate"];
	segments.iter().rev().skip(1).any(|seg| {
		let ident_str = seg.ident.to_string();
		known_prefixes.contains(&ident_str.as_str())
	})
}

/// Extract inner type from `Injected<T>` or `OptionalInjected<T>`
///
/// Returns `Some(InjectionType)` if the type is a valid injection type,
/// `None` otherwise.
///
/// # Path qualification
///
/// This function matches on the terminal identifier (`Injected`,
/// `OptionalInjected`, `Option`) and additionally validates that multi-segment
/// paths contain a recognized `reinhardt_di` prefix. This prevents user-defined
/// types with the same leaf name from being misclassified as DI wrapper types.
/// Single-segment paths are accepted because the typical usage is via
/// `use reinhardt_di::Injected;`.
fn classify_injection_type(ty: &Type) -> Option<InjectionType> {
	if let Type::Path(type_path) = ty {
		let segments = &type_path.path.segments;
		if segments.is_empty() {
			return None;
		}

		let last_segment = segments.last()?;
		let ident = &last_segment.ident;

		// Check for Injected<T>
		if ident == "Injected"
			&& is_likely_reinhardt_di_path(segments)
			&& let PathArguments::AngleBracketed(args) = &last_segment.arguments
			&& let Some(GenericArgument::Type(inner_ty)) = args.args.first()
		{
			return Some(InjectionType::Injected(inner_ty.clone()));
		}

		// Check for OptionalInjected<T> (type alias for Option<Injected<T>>)
		// Also check for Option<Injected<T>> directly
		if ident == "OptionalInjected"
			&& is_likely_reinhardt_di_path(segments)
			&& let PathArguments::AngleBracketed(args) = &last_segment.arguments
			&& let Some(GenericArgument::Type(inner_ty)) = args.args.first()
		{
			return Some(InjectionType::OptionalInjected(inner_ty.clone()));
		}

		// Check for Depends<T>
		if ident == "Depends"
			&& is_likely_reinhardt_di_path(segments)
			&& let PathArguments::AngleBracketed(args) = &last_segment.arguments
			&& let Some(GenericArgument::Type(inner_ty)) = args.args.first()
		{
			return Some(InjectionType::Depends(inner_ty.clone()));
		}

		// Check for Option<Injected<T>> or Option<Depends<T>>
		if ident == "Option"
			&& let PathArguments::AngleBracketed(args) = &last_segment.arguments
			&& let Some(GenericArgument::Type(inner_ty)) = args.args.first()
			&& let Type::Path(inner_path) = inner_ty
			&& let Some(inner_seg) = inner_path.path.segments.last()
			&& let PathArguments::AngleBracketed(inner_args) = &inner_seg.arguments
			&& let Some(GenericArgument::Type(innermost_ty)) = inner_args.args.first()
		{
			// Check if inner type is Injected<T> (deprecated)
			if inner_seg.ident == "Injected"
				&& is_likely_reinhardt_di_path(&inner_path.path.segments)
			{
				return Some(InjectionType::OptionalInjected(innermost_ty.clone()));
			}

			// Check if inner type is Depends<T>
			if inner_seg.ident == "Depends"
				&& is_likely_reinhardt_di_path(&inner_path.path.segments)
			{
				return Some(InjectionType::OptionalDepends(innermost_ty.clone()));
			}
		}
	}

	None
}

/// Implementation of the `#[injectable]` attribute macro
///
/// This macro:
/// 1. Implements the `Injectable` trait for the struct (with field injection support)
/// 2. Registers the type with the global dependency registry using inventory
pub(crate) fn injectable_impl(args: TokenStream, input: DeriveInput) -> Result<TokenStream> {
	let struct_name = &input.ident;
	let generics = &input.generics;
	let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

	// Parse macro arguments (scope defaults to Request when omitted).
	// All injectable types are registered in the global registry to enable
	// resolution via ctx.resolve::<T>() and Depends<T>.
	let scope = if !args.is_empty() {
		let parsed_args: MacroArgs = syn::parse2(args)?;
		parsed_args.scope.unwrap_or(crate::utils::Scope::Request)
	} else {
		crate::utils::Scope::Request
	};

	// Validate that this is a struct and extract fields
	let fields = match &input.data {
		syn::Data::Struct(data_struct) => match &data_struct.fields {
			Fields::Named(fields) => Some(&fields.named),
			Fields::Unit => None, // Unit struct (no fields)
			Fields::Unnamed(_) => {
				return Err(syn::Error::new_spanned(
					struct_name,
					"#[injectable] cannot be applied to tuple structs",
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

	// Process fields: require explicit #[inject] or #[no_inject]
	let mut has_inject_fields = false;
	let mut inject_stmts = Vec::new();
	let mut field_inits = Vec::new();

	if let Some(fields) = fields {
		for field in fields {
			let name = &field.ident;
			let ty = &field.ty;
			let has_inject = has_inject_attr(field);
			let has_no_inject = has_no_inject_attr(field);

			// Validate: field must have either #[inject] or #[no_inject]
			if has_inject && has_no_inject {
				return Err(syn::Error::new_spanned(
					field,
					"Field cannot have both #[inject] and #[no_inject] attributes",
				));
			}

			if !has_inject && !has_no_inject {
				return Err(syn::Error::new_spanned(
					field,
					"Field must have either #[inject] or #[no_inject] attribute. Use #[inject] for dependency injection, or #[no_inject] for default initialization.",
				));
			}

			if has_inject {
				// Validate and classify the injection type
				let injection_type = classify_injection_type(ty).ok_or_else(|| {
					syn::Error::new_spanned(
						field,
						"#[inject] field must have type Depends<T>, Option<Depends<T>>, Injected<T>, or OptionalInjected<T>",
					)
				})?;

				// Inject this field
				has_inject_fields = true;
				let use_cache = should_use_cache(field);

				// Get dynamic crate path once per field
				let di_crate = get_reinhardt_di_crate();

				// Generate dependency resolution call based on injection type
				let resolve_call = match injection_type {
					InjectionType::Depends(inner_ty) => {
						if use_cache {
							quote! {
								#di_crate::Depends::<#inner_ty>::resolve(__di_ctx, true)
									.await?
							}
						} else {
							quote! {
								#di_crate::Depends::<#inner_ty>::resolve(__di_ctx, false)
									.await?
							}
						}
					}
					InjectionType::OptionalDepends(inner_ty) => {
						if use_cache {
							quote! {
								#di_crate::Depends::<#inner_ty>::resolve(__di_ctx, true)
									.await
									.ok()
							}
						} else {
							quote! {
								#di_crate::Depends::<#inner_ty>::resolve(__di_ctx, false)
									.await
									.ok()
							}
						}
					}
					// Deprecated: Injected<T> and OptionalInjected<T> are still supported
					// for backward compatibility but users should migrate to Depends<T>.
					InjectionType::Injected(inner_ty) => {
						if use_cache {
							quote! {
								{
									#[allow(deprecated)]
									let __v = #di_crate::Injected::<#inner_ty>::resolve(__di_ctx)
										.await?;
									__v
								}
							}
						} else {
							quote! {
								{
									#[allow(deprecated)]
									let __v = #di_crate::Injected::<#inner_ty>::resolve_uncached(__di_ctx)
										.await?;
									__v
								}
							}
						}
					}
					InjectionType::OptionalInjected(inner_ty) => {
						if use_cache {
							quote! {
								{
									#[allow(deprecated)]
									let __v = #di_crate::Injected::<#inner_ty>::resolve(__di_ctx)
										.await
										.ok();
									__v
								}
							}
						} else {
							quote! {
								{
									#[allow(deprecated)]
									let __v = #di_crate::Injected::<#inner_ty>::resolve_uncached(__di_ctx)
										.await
										.ok();
									__v
								}
							}
						}
					}
				};

				inject_stmts.push(quote! {
					let #name = #resolve_call;
				});
				field_inits.push(quote! { #name });
			} else {
				// Use Default::default() for fields marked with #[no_inject]
				field_inits.push(quote! { #name: Default::default() });
			}
		}
	}

	// Remove #[inject] and #[no_inject] attributes from fields to avoid "unknown attribute" errors
	let mut cleaned_input = input.clone();
	if let syn::Data::Struct(ref mut data_struct) = cleaned_input.data
		&& let syn::Fields::Named(ref mut fields_named) = data_struct.fields
	{
		for field in fields_named.named.iter_mut() {
			field.attrs.retain(|attr| {
				!attr.path().is_ident("inject") && !attr.path().is_ident("no_inject")
			});
		}
	}

	// Get dynamic crate path
	let di_crate = get_reinhardt_di_crate();

	// Generate the Injectable implementation
	let injectable_impl = if has_inject_fields {
		// With field injection
		quote! {
			#[async_trait::async_trait]
			impl #impl_generics #di_crate::Injectable for #struct_name #ty_generics #where_clause {
				async fn inject(__di_ctx: &#di_crate::InjectionContext) -> #di_crate::DiResult<Self> {
					#(#inject_stmts)*

					Ok(Self {
						#(#field_inits),*
					})
				}
			}
		}
	} else {
		// Without field injection - use Default::default()
		quote! {
			#[async_trait::async_trait]
			impl #impl_generics #di_crate::Injectable for #struct_name #ty_generics #where_clause {
				async fn inject(_ctx: &#di_crate::InjectionContext) -> #di_crate::DiResult<Self> {
					// Use Default::default() for types without field injection
					Ok(<Self as ::std::default::Default>::default())
				}
			}
		}
	};

	// Generate inventory registration for the global dependency registry.
	// All injectable types are always registered (scope defaults to Request).
	// Uses InjectableRegistration (const-constructible) instead of
	// DependencyRegistration (which uses Box and cannot be used in
	// inventory::submit!'s static context).
	// InjectableFactory<T> bypasses AsyncFactory's Fut: Sync bound by
	// implementing FactoryTrait directly via Injectable::inject.
	let scope_tokens = scope.into_tokens();
	let register_fn_name = syn::Ident::new(
		&format!("__register_injectable_{}", struct_name),
		proc_macro2::Span::call_site(),
	);

	let registration = quote! {
		fn #register_fn_name(registry: &#di_crate::DependencyRegistry) {
			registry.register::<#struct_name>(
				#scope_tokens,
				#di_crate::InjectableFactory::<#struct_name>::new(),
			);
			registry.register_qualified_type_name(
				::std::any::TypeId::of::<#struct_name>(),
				::std::any::type_name::<#struct_name>(),
			);
		}

		#di_crate::inventory::submit! {
			#di_crate::InjectableRegistration::new(
				#register_fn_name
			)
		}
	};

	// Combine cleaned struct, Injectable impl, and optional registration
	let expanded = quote! {
		#cleaned_input

		#injectable_impl

		#registration
	};

	Ok(expanded)
}
