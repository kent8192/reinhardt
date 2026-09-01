use proc_macro::TokenStream;
use quote::quote;
use syn::{FnArg, ItemFn, PatType, ReturnType, Type, parse_macro_input};

use crate::crate_paths::get_reinhardt_pages_crate;

/// Expands `#[navigation_guard]` into a marker and registry entry.
pub(crate) fn navigation_guard_impl(args: TokenStream, input: TokenStream) -> TokenStream {
	if !args.is_empty() {
		return syn::Error::new(
			proc_macro2::Span::call_site(),
			"#[navigation_guard] does not accept arguments",
		)
		.to_compile_error()
		.into();
	}

	let input = parse_macro_input!(input as ItemFn);
	match expand_navigation_guard(input) {
		Ok(expanded) => expanded.into(),
		Err(error) => error.to_compile_error().into(),
	}
}

fn expand_navigation_guard(input: ItemFn) -> syn::Result<proc_macro2::TokenStream> {
	if input.sig.asyncness.is_none() {
		return Err(syn::Error::new_spanned(
			input.sig.fn_token,
			"#[navigation_guard] functions must be async",
		));
	}
	if !input.sig.generics.params.is_empty() || input.sig.generics.where_clause.is_some() {
		return Err(syn::Error::new_spanned(
			&input.sig.generics,
			"#[navigation_guard] functions must not be generic",
		));
	}
	if input.sig.inputs.len() != 1 {
		return Err(syn::Error::new_spanned(
			&input.sig.inputs,
			"#[navigation_guard] functions must accept exactly one NavigationContext",
		));
	}
	let FnArg::Typed(PatType { ty, .. }) = &input.sig.inputs[0] else {
		return Err(syn::Error::new_spanned(
			&input.sig.inputs[0],
			"#[navigation_guard] functions must accept exactly one NavigationContext",
		));
	};
	if !is_named_type(ty, "NavigationContext") {
		return Err(syn::Error::new_spanned(
			ty,
			"#[navigation_guard] argument must be NavigationContext",
		));
	}
	if !is_navigation_result(&input.sig.output) {
		return Err(syn::Error::new_spanned(
			&input.sig.output,
			"#[navigation_guard] functions must return Result<NavigationDecision, NavigationGuardError>",
		));
	}

	let pages_crate = get_reinhardt_pages_crate();
	let function_name = &input.sig.ident;
	let visibility = &input.vis;
	let attrs = &input.attrs;
	let sig = &input.sig;
	let block = &input.block;

	Ok(quote! {
		#(#attrs)*
		#visibility #sig #block

		#visibility mod #function_name {
			use super::*;

			// The marker intentionally shares the guard function path and therefore uses a lowercase type name.
			#[allow(non_camel_case_types)]
			pub struct marker;

			impl #pages_crate::NavigationGuard for marker {
				const ID: #pages_crate::NavigationGuardId =
					#pages_crate::NavigationGuardId::new(concat!(
						module_path!(), "::", stringify!(#function_name),
					));
			}

			fn __execute(
				context: #pages_crate::NavigationContext,
			) -> #pages_crate::router::navigation_guard_registry::NavigationGuardFuture {
				Box::pin(super::#function_name(context))
			}

			#pages_crate::__private::inventory::submit! {
				#pages_crate::router::navigation_guard_registry::NavigationGuardRegistration {
					id: <marker as #pages_crate::NavigationGuard>::ID,
					execute: __execute,
				}
			}
		}
	})
}

fn is_named_type(ty: &Type, expected: &str) -> bool {
	let Type::Path(type_path) = ty else {
		return false;
	};
	type_path.path.segments.last().is_some_and(|segment| {
		segment.ident == expected && matches!(segment.arguments, syn::PathArguments::None)
	})
}

fn is_navigation_result(output: &ReturnType) -> bool {
	let ReturnType::Type(_, ty) = output else {
		return false;
	};
	let Type::Path(type_path) = &**ty else {
		return false;
	};
	let Some(segment) = type_path.path.segments.last() else {
		return false;
	};
	if segment.ident != "Result" {
		return false;
	}
	let syn::PathArguments::AngleBracketed(arguments) = &segment.arguments else {
		return false;
	};
	if arguments.args.len() != 2 {
		return false;
	}
	let mut types = arguments.args.iter().filter_map(|arg| match arg {
		syn::GenericArgument::Type(ty) => Some(ty),
		_ => None,
	});
	types
		.next()
		.is_some_and(|ty| is_named_type(ty, "NavigationDecision"))
		&& types
			.next()
			.is_some_and(|ty| is_named_type(ty, "NavigationGuardError"))
}
