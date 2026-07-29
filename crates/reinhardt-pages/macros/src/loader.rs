use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{
	FnArg, Ident, ItemFn, Pat, PatTupleStruct, PatType, Path, ReturnType, Token, Type,
	parse_macro_input, punctuated::Punctuated,
};

use crate::crate_paths::get_reinhardt_pages_crate;

#[derive(Clone, Copy)]
enum LoaderInputKind {
	Path,
	Query,
}

struct LoaderArg {
	kind: Option<LoaderInputKind>,
	name: Option<Ident>,
	value_type: Option<Type>,
}

struct LoaderSignature {
	args: Vec<LoaderArg>,
	data: Type,
	error: Type,
}

/// Expands `#[loader]` into the original async function, a same-name marker
/// module, and an inventory registration consumed by pages navigation.
pub(crate) fn loader_impl(args: TokenStream, input: TokenStream) -> TokenStream {
	if !args.is_empty() {
		return syn::Error::new(
			proc_macro2::Span::call_site(),
			"#[loader] does not accept arguments",
		)
		.to_compile_error()
		.into();
	}
	let input = parse_macro_input!(input as ItemFn);
	match expand_loader(input) {
		Ok(expanded) => expanded.into(),
		Err(error) => error.to_compile_error().into(),
	}
}

fn expand_loader(input: ItemFn) -> syn::Result<proc_macro2::TokenStream> {
	if input.sig.asyncness.is_none() {
		return Err(syn::Error::new_spanned(
			input.sig.fn_token,
			"#[loader] functions must be async",
		));
	}
	if !input.sig.generics.params.is_empty() || input.sig.generics.where_clause.is_some() {
		return Err(syn::Error::new_spanned(
			&input.sig.generics,
			"#[loader] functions must not be generic",
		));
	}
	let signature = parse_signature(&input.sig.inputs, &input.sig.output)?;
	let pages_crate = get_reinhardt_pages_crate();
	let function_name = input.sig.ident.clone();
	let visibility = input.vis.clone();
	let attrs = input.attrs.clone();
	let sig = input.sig.clone();
	let block = input.block.clone();
	let module_name = function_name.clone();
	let executor_name = format_ident!("__execute", span = function_name.span());
	let fetcher_name = format_ident!("__fetch", span = function_name.span());
	let hydrator_name = format_ident!("__hydrate", span = function_name.span());
	let query_seeder_name = format_ident!("__seed_query", span = function_name.span());
	let input_specs = signature.args.iter().filter_map(|arg| {
		let kind = arg.kind?;
		let name = arg
			.name
			.as_ref()
			.expect("loader extractor has a name")
			.to_string();
		Some(match kind {
			LoaderInputKind::Path => {
				quote! { #pages_crate::router::loader::LoaderInputSpec::path(#name) }
			}
			LoaderInputKind::Query => {
				quote! { #pages_crate::router::loader::LoaderInputSpec::query(#name) }
			}
		})
	});
	let extraction = signature.args.iter().map(|arg| {
		let Some(kind) = arg.kind else {
			return quote! {};
		};
		let name = arg.name.as_ref().expect("loader extractor has a name");
		let value_type = arg.value_type.as_ref().expect("loader extractor has a value type");
		let key = name.to_string();
		let extractor = match kind {
			LoaderInputKind::Path => quote! {
				#pages_crate::router::request::PathParam::<#value_type>::extract(&__context, #key)
			},
			LoaderInputKind::Query => quote! {
				#pages_crate::router::request::QueryParam::<#value_type>::extract(&__context, #key)
			},
		};
		quote! {
			let #name = #extractor
				.map_err(|error| #pages_crate::router::loader::RouteLoaderError::with_status(error.to_string(), 400))?
				.into_inner();
		}
	});
	let call_args = signature
		.args
		.iter()
		.map(|arg| match (&arg.kind, &arg.name) {
			(Some(LoaderInputKind::Path), Some(name)) => quote! { #pages_crate::Path(#name) },
			(Some(LoaderInputKind::Query), Some(name)) => quote! { #pages_crate::Query(#name) },
			(None, Some(_name)) => {
				quote! { #pages_crate::CancellationToken(__cancellation.clone()) }
			}
			_ => quote! {},
		});
	let data = &signature.data;
	let error = &signature.error;
	let route_loader_id = quote! {
		#pages_crate::router::RouteLoaderId::new(concat!(module_path!(), "::", stringify!(#function_name)))
	};

	Ok(quote! {
		#(#attrs)*
		#visibility #sig #block

		#visibility mod #module_name {
			use super::*;

			// The lowercase marker name is part of the stable same-name loader API.
			#[allow(non_camel_case_types)]
			pub struct marker;

			pub const INPUTS: &'static [#pages_crate::router::loader::LoaderInputSpec] = &[
				#(#input_specs,)*
			];

			impl #pages_crate::RouteLoader for marker {
				type Data = #data;
				type Error = #error;
				const ID: #pages_crate::router::RouteLoaderId = #route_loader_id;
			}

			fn #fetcher_name(
				__context: #pages_crate::router::request::RouteContext,
				__cancellation: #pages_crate::CancellationHandle,
			) -> ::std::pin::Pin<Box<dyn ::std::future::Future<Output = ::std::result::Result<#data, #pages_crate::RouteLoaderError>> + 'static>> {
				Box::pin(async move {
					#(#extraction)*
					super::#function_name(#(#call_args),*).await.map_err(Into::into)
				})
			}

			fn #executor_name(
				__context: &#pages_crate::router::request::RouteContext,
				__cancellation: #pages_crate::CancellationHandle,
				__consumer: #pages_crate::router::loader_registry::LoaderConsumer,
			) -> #pages_crate::router::loader_registry::LoaderFuture {
				let __context = __context.clone();
				Box::pin(async move {
					let __fetcher = {
						let __fetch_context = __context.clone();
						move |__query_cancellation: #pages_crate::CancellationHandle| {
							#fetcher_name(__fetch_context.clone(), __query_cancellation)
						}
					};
					#pages_crate::router::loader::acquire_loader_query::<#data>(
						<marker as #pages_crate::RouteLoader>::ID,
						&__context,
						INPUTS,
						__cancellation,
						__consumer,
						__fetcher,
					)
					.await
				})
			}

			fn #hydrator_name(
				__value: &#pages_crate::__private::serde_json::Value,
			) -> ::core::result::Result<
				#pages_crate::router::loader::PreparedLoader,
				#pages_crate::RouteLoaderError,
			> {
				#pages_crate::router::loader::PreparedLoader::from_serialized::<#data>(
					<marker as #pages_crate::RouteLoader>::ID,
					__value,
				)
			}

			fn #query_seeder_name(
				__client: &#pages_crate::QueryClient,
				__context: &#pages_crate::router::request::RouteContext,
				__hydration: &#pages_crate::HydrationContext,
			) -> ::core::result::Result<(), #pages_crate::RouteLoaderError> {
				let __context = __context.clone();
				let __fetcher = {
					let __fetch_context = __context.clone();
					move |__query_cancellation: #pages_crate::CancellationHandle| {
						#fetcher_name(__fetch_context.clone(), __query_cancellation)
					}
				};
				#pages_crate::router::loader::seed_loader_query::<#data>(
					__client,
					<marker as #pages_crate::RouteLoader>::ID,
					&__context,
					INPUTS,
					__hydration,
					__fetcher,
				)
			}

			#pages_crate::__private::inventory::submit! {
				#pages_crate::router::loader_registry::LoaderRegistration {
					id: <marker as #pages_crate::RouteLoader>::ID,
					inputs: INPUTS,
					execute: #executor_name,
				}
			}

			#pages_crate::__private::inventory::submit! {
				#pages_crate::router::loader_registry::LoaderHydrationRegistration::new(
					<marker as #pages_crate::RouteLoader>::ID,
					#hydrator_name,
				)
			}

			#pages_crate::__private::inventory::submit! {
				#pages_crate::router::loader_registry::LoaderQueryHydrationRegistration::new(
					<marker as #pages_crate::RouteLoader>::ID,
					#query_seeder_name,
				)
			}
		}
	})
}

fn parse_signature(
	inputs: &Punctuated<FnArg, Token![,]>,
	output: &ReturnType,
) -> syn::Result<LoaderSignature> {
	let (data, error) = parse_result_output(output)?;
	let mut args = Vec::new();
	let mut cancellation_seen = false;
	for input in inputs {
		let FnArg::Typed(PatType { pat, ty, .. }) = input else {
			return Err(syn::Error::new_spanned(
				input,
				"#[loader] functions do not support receiver arguments",
			));
		};
		if is_cancellation_type(ty) {
			if cancellation_seen {
				return Err(syn::Error::new_spanned(
					input,
					"#[loader] accepts at most one CancellationToken extractor",
				));
			}
			let name = cancellation_binding(pat)?;
			cancellation_seen = true;
			args.push(LoaderArg {
				kind: None,
				name: Some(name),
				value_type: None,
			});
			continue;
		}
		let (kind, name, value_type) = parse_input_extractor(pat, ty)?;
		args.push(LoaderArg {
			kind: Some(kind),
			name: Some(name),
			value_type: Some(value_type),
		});
	}
	Ok(LoaderSignature { args, data, error })
}

fn parse_result_output(output: &ReturnType) -> syn::Result<(Type, Type)> {
	let ReturnType::Type(_, output_type) = output else {
		return Err(syn::Error::new_spanned(
			output,
			"#[loader] functions must return Result<Data, Error>",
		));
	};
	let Type::Path(type_path) = &**output_type else {
		return Err(syn::Error::new_spanned(
			output_type,
			"#[loader] functions must return direct Result<Data, Error>",
		));
	};
	let segment = type_path.path.segments.last().ok_or_else(|| {
		syn::Error::new_spanned(
			output_type,
			"#[loader] functions must return direct Result<Data, Error>",
		)
	})?;
	if segment.ident != "Result" {
		return Err(syn::Error::new_spanned(
			segment,
			"#[loader] functions must return Result<Data, Error>",
		));
	}
	let syn::PathArguments::AngleBracketed(arguments) = &segment.arguments else {
		return Err(syn::Error::new_spanned(
			segment,
			"#[loader] Result must specify Data and Error types",
		));
	};
	if arguments.args.len() != 2 {
		return Err(syn::Error::new_spanned(
			arguments,
			"#[loader] Result must specify exactly Data and Error types",
		));
	}
	let mut types = arguments.args.iter().filter_map(|argument| match argument {
		syn::GenericArgument::Type(ty) => Some(ty.clone()),
		_ => None,
	});
	let Some(data) = types.next() else {
		return Err(syn::Error::new_spanned(
			arguments,
			"#[loader] Result Data must be a type",
		));
	};
	let Some(error) = types.next() else {
		return Err(syn::Error::new_spanned(
			arguments,
			"#[loader] Result Error must be a type",
		));
	};
	Ok((data, error))
}

fn parse_input_extractor(pat: &Pat, ty: &Type) -> syn::Result<(LoaderInputKind, Ident, Type)> {
	let Pat::TupleStruct(PatTupleStruct { path, elems, .. }) = pat else {
		return Err(syn::Error::new_spanned(
			pat,
			"#[loader] inputs must use Path(name): Path<T> or Query(name): Query<T>",
		));
	};
	if elems.len() != 1 {
		return Err(syn::Error::new_spanned(
			elems,
			"#[loader] extractor must bind one identifier",
		));
	}
	let Pat::Ident(binding) = &elems[0] else {
		return Err(syn::Error::new_spanned(
			&elems[0],
			"#[loader] extractor must bind a plain identifier",
		));
	};
	let source = path_last_ident(path)
		.ok_or_else(|| syn::Error::new_spanned(path, "expected Path or Query extractor"))?;
	let kind = match source.to_string().as_str() {
		"Path" => LoaderInputKind::Path,
		"Query" => LoaderInputKind::Query,
		_ => {
			return Err(syn::Error::new_spanned(
				path,
				"#[loader] supports only Path and Query extractors",
			));
		}
	};
	let value_type = extractor_inner_type(ty, source.to_string().as_str())?;
	Ok((kind, binding.ident.clone(), value_type))
}

fn cancellation_binding(pat: &Pat) -> syn::Result<Ident> {
	let Pat::TupleStruct(PatTupleStruct { elems, .. }) = pat else {
		return Err(syn::Error::new_spanned(
			pat,
			"CancellationToken must use tuple destructuring: CancellationToken(token)",
		));
	};
	if elems.len() != 1 {
		return Err(syn::Error::new_spanned(
			elems,
			"CancellationToken must bind one identifier",
		));
	}
	let Pat::Ident(binding) = &elems[0] else {
		return Err(syn::Error::new_spanned(
			&elems[0],
			"CancellationToken must bind a plain identifier",
		));
	};
	Ok(binding.ident.clone())
}

fn is_cancellation_type(ty: &Type) -> bool {
	let Type::Path(type_path) = ty else {
		return false;
	};
	path_last_ident(&type_path.path).is_some_and(|ident| ident == "CancellationToken")
}

fn path_last_ident(path: &Path) -> Option<&Ident> {
	path.segments.last().map(|segment| &segment.ident)
}

fn extractor_inner_type(ty: &Type, expected: &str) -> syn::Result<Type> {
	let Type::Path(type_path) = ty else {
		return Err(syn::Error::new_spanned(ty, "expected Path<T> or Query<T>"));
	};
	let segment = type_path
		.path
		.segments
		.last()
		.ok_or_else(|| syn::Error::new_spanned(ty, "expected Path<T> or Query<T>"))?;
	if segment.ident != expected {
		return Err(syn::Error::new_spanned(
			ty,
			"extractor pattern and argument type must match",
		));
	}
	let syn::PathArguments::AngleBracketed(arguments) = &segment.arguments else {
		return Err(syn::Error::new_spanned(ty, "expected Path<T> or Query<T>"));
	};
	let Some(syn::GenericArgument::Type(inner)) = arguments.args.first() else {
		return Err(syn::Error::new_spanned(ty, "expected Path<T> or Query<T>"));
	};
	Ok(inner.clone())
}
