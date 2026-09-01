use std::collections::HashSet;

use convert_case::{Case, Casing};
use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{
	FnArg, Ident, ItemFn, LitStr, Pat, PatTupleStruct, PatType, ReturnType, Token, Type,
	Visibility,
	parse::{Parse, ParseStream},
	parse_macro_input,
	punctuated::Punctuated,
};

use crate::crate_paths::get_reinhardt_pages_crate;
use crate::type_utils::option_inner_type;

struct ComponentArgs {
	path: LitStr,
	name: LitStr,
	loader: Option<syn::Path>,
	navigation_guard: Option<syn::Path>,
}

struct LayoutArgs {
	path: LitStr,
	name: LitStr,
	loader: Option<syn::Path>,
	navigation_guard: Option<syn::Path>,
}

const COMPONENT_ARGS_EXPECTED: &str = "expected #[component(\"/path/\", name = \"name\")]";
const LAYOUT_ARGS_EXPECTED: &str = "expected #[layout(\"/path/\", name = \"name\")]";

impl Parse for ComponentArgs {
	fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
		let (path, name, loader, navigation_guard) =
			parse_route_macro_args(input, "component", COMPONENT_ARGS_EXPECTED)?;
		Ok(Self {
			path,
			name,
			loader,
			navigation_guard,
		})
	}
}

impl Parse for LayoutArgs {
	fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
		let (path, name, loader, navigation_guard) =
			parse_route_macro_args(input, "layout", LAYOUT_ARGS_EXPECTED)?;
		Ok(Self {
			path,
			name,
			loader,
			navigation_guard,
		})
	}
}

fn parse_route_macro_args(
	input: ParseStream<'_>,
	macro_name: &str,
	expected: &str,
) -> syn::Result<(LitStr, LitStr, Option<syn::Path>, Option<syn::Path>)> {
	let path: LitStr = input.parse()?;
	if input.is_empty() {
		return Err(input.error(expected));
	}
	input.parse::<Token![,]>()?;
	if input.peek(LitStr) {
		let name: LitStr = input.parse()?;
		return Err(syn::Error::new(
			name.span(),
			"expected named route argument `name = \"...\"`; positional route names are no longer supported",
		));
	}
	let mut name = None;
	let mut loader = None;
	let mut navigation_guard = None;
	loop {
		let key: Ident = input.parse()?;
		let key_name = key.to_string();
		if key_name != "name" && key_name != "loader" && key_name != "navigation_guard" {
			return Err(syn::Error::new(
				key.span(),
				"expected route option `name = \"...\"`, `loader = path`, or `navigation_guard = path`",
			));
		}
		input.parse::<Token![=]>()?;
		match key_name.as_str() {
			"name" => {
				if name.is_some() {
					return Err(syn::Error::new_spanned(
						key,
						"duplicate route option `name`",
					));
				}
				if !input.peek(LitStr) {
					return Err(input.error(format!(
						"expected string literal route name in #[{macro_name}(\"/path/\", name = \"name\")]"
					)));
				}
				name = Some(input.parse()?);
			}
			"loader" => {
				if loader.is_some() {
					return Err(syn::Error::new_spanned(
						key,
						"duplicate route option `loader`",
					));
				}
				loader = Some(input.parse()?);
			}
			"navigation_guard" => {
				if navigation_guard.is_some() {
					return Err(syn::Error::new_spanned(
						key,
						"duplicate route option `navigation_guard`",
					));
				}
				navigation_guard = Some(input.parse()?);
			}
			_ => unreachable!("route option was validated above"),
		}
		if input.is_empty() {
			break;
		}
		input.parse::<Token![,]>()?;
		// A trailing comma is accepted for consistency with Rust attributes.
		if input.is_empty() {
			break;
		}
	}
	let Some(name) = name else {
		return Err(input.error(expected));
	};
	Ok((path, name, loader, navigation_guard))
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Source {
	Path,
	Query,
	Loader,
}

struct ExtractedArg {
	source: Source,
	name: Ident,
	ty: Type,
	extractor_ty: Type,
}

struct LayoutFunctionArgs {
	extracted: Vec<ExtractedArg>,
	outlet_name: Ident,
	outlet_ty: Type,
}

struct LoaderBinding<'a> {
	path: &'a syn::Path,
	argument: &'a ExtractedArg,
}

pub(crate) fn component_impl(args: TokenStream, input: TokenStream) -> TokenStream {
	let args = parse_macro_input!(args as ComponentArgs);
	let input = parse_macro_input!(input as ItemFn);
	expand_component(args, input)
		.unwrap_or_else(|e| e.to_compile_error())
		.into()
}

pub(crate) fn layout_impl(args: TokenStream, input: TokenStream) -> TokenStream {
	let args = parse_macro_input!(args as LayoutArgs);
	let input = parse_macro_input!(input as ItemFn);
	expand_layout(args, input)
		.unwrap_or_else(|e| e.to_compile_error())
		.into()
}

fn expand_component(args: ComponentArgs, input: ItemFn) -> syn::Result<proc_macro2::TokenStream> {
	if input.sig.asyncness.is_some() {
		return Err(syn::Error::new_spanned(
			input.sig.asyncness,
			"#[component] functions must not be async",
		));
	}
	if !input.sig.generics.params.is_empty() || input.sig.generics.where_clause.is_some() {
		return Err(syn::Error::new_spanned(
			input.sig.generics,
			"#[component] functions must not be generic",
		));
	}
	match &input.sig.output {
		ReturnType::Type(_, ty) if is_page_type(ty) => {}
		_ => {
			return Err(syn::Error::new_spanned(
				&input.sig,
				"#[component] functions must return Page",
			));
		}
	}

	let pages_crate = get_reinhardt_pages_crate();
	let ComponentArgs {
		path,
		name: route_name,
		loader: loader_option,
		navigation_guard: navigation_guard_option,
	} = args;
	let fn_name = input.sig.ident.clone();
	let component_name = fn_name.to_string().to_case(Case::Pascal);
	let component_ident = format_ident!("{component_name}", span = fn_name.span());
	let props_ident = format_ident!("{}Props", component_name, span = fn_name.span());
	let original_ident = format_ident!("__{}_component_body", fn_name, span = fn_name.span());
	let output = input.sig.output.clone();
	let vis = input.vis.clone();
	let field_vis = field_visibility_tokens(&vis);
	let attrs = input.attrs.clone();
	let block = input.block.clone();
	let args_info = parse_args(&input.sig.inputs)?;
	let loader_binding =
		resolve_loader_binding(&args_info, loader_option.as_ref(), "#[component]")?;
	let loader_path = loader_binding.as_ref().map(|binding| binding.path);
	let loader_argument = loader_binding.as_ref().map(|binding| binding.argument);
	let loader_type = loader_argument.map(|argument| &argument.ty);
	let loaded_value_ident = format_ident!("__{}_loader_value", fn_name, span = fn_name.span());
	let loader_id = loader_path.map(|path| {
		quote! { <#path::marker as #pages_crate::RouteLoader>::ID }
	});
	let loader_id_method = loader_id.as_ref().map_or_else(
		|| quote! { ::core::option::Option::None },
		|id| quote! { ::core::option::Option::Some(#id) },
	);
	let navigation_guard_id = navigation_guard_option.map(|path| {
		quote! { <#path::marker as #pages_crate::NavigationGuard>::ID }
	});
	let navigation_guard_id_method = navigation_guard_id.as_ref().map_or_else(
		|| quote! { ::core::option::Option::None },
		|id| quote! { ::core::option::Option::Some(#id) },
	);
	let navigation_guard_metadata = navigation_guard_id.as_ref().map_or_else(
		|| quote! {},
		|id| {
			quote! {
				#pages_crate::__private::inventory::submit! {
					#pages_crate::__private::reinhardt_urls::routers::client_router::ComponentNavigationGuardMetadata {
						path: #path,
						name: #route_name,
						navigation_guard_id: #id,
					}
				}
			}
		},
	);
	let loader_assertion = loader_binding.as_ref().map_or_else(
		|| quote! {},
		|binding| {
			let assertion_name =
				format_ident!("__{}_assert_loader_data", fn_name, span = fn_name.span());
			let path = binding.path;
			let data = &binding.argument.ty;
			quote! {
				fn #assertion_name<M, T>()
				where
					M: #pages_crate::RouteLoader<Data = T>,
					T: ::core::clone::Clone
						+ #pages_crate::__private::serde::Serialize
						+ #pages_crate::__private::serde::de::DeserializeOwned
						+ 'static,
				{
				}
				const _: fn() = || #assertion_name::<#path::marker, #data>();
			}
		},
	);
	let loader_load = match (loader_type, loader_id.as_ref()) {
		(Some(data), Some(id)) => quote! {
			let #loaded_value_ident = #pages_crate::router::loader::active_loader_store()
				.expect("a loaded route must render inside a LoaderStore scope")
				.get::<#data>(#id)
				.map(|value| value.0)
				.unwrap_or_else(|error| panic!("{error}"));
		},
		_ => quote! {},
	};

	let props_fields = args_info
		.iter()
		.filter(|arg| arg.source != Source::Loader)
		.map(|arg| {
			let name = &arg.name;
			let ty = &arg.ty;
			quote! { #field_vis #name: #ty }
		});
	let destructure_fields = args_info
		.iter()
		.filter(|arg| arg.source != Source::Loader)
		.map(|arg| &arg.name);
	let original_inputs = args_info.iter().map(|arg| {
		let name = &arg.name;
		let ty = &arg.ty;
		quote! { #name: #ty }
	});
	let call_args = args_info.iter().map(|arg| {
		if arg.source == Source::Loader {
			quote! { #loaded_value_ident }
		} else {
			let name = &arg.name;
			quote! { #name }
		}
	});
	let from_request_fields = args_info
		.iter()
		.filter(|arg| arg.source != Source::Loader)
		.map(|arg| {
			let name = &arg.name;
			let key = name.to_string();
			let ty = &arg.ty;
			match arg.source {
				Source::Path => quote! {
					#name: #pages_crate::router::request::PathParam::<#ty>::extract(ctx, #key)?.into_inner()
				},
				Source::Query => {
					if let Some(inner) = option_inner_type(ty) {
						quote! {
							#name: #pages_crate::router::request::OptionalQueryParam::<#inner>::extract(ctx, #key)?.into_inner()
						}
					} else {
						quote! {
							#name: #pages_crate::router::request::QueryParam::<#ty>::extract(ctx, #key)?.into_inner()
						}
					}
				}
				Source::Loader => unreachable!("loader inputs are not props fields"),
			}
		});
	let extractor_type_aliases = args_info.iter().enumerate().map(|(index, arg)| {
		let alias = format_ident!(
			"__{}PropsExtractor{}",
			component_name,
			index,
			span = fn_name.span()
		);
		let extractor_ty = &arg.extractor_ty;
		quote! {
			// Keep the extractor type from the original signature referenced after
			// the attribute macro rewrites the function into a props argument.
			#[allow(non_camel_case_types, dead_code)]
			type #alias = #extractor_ty;
		}
	});

	let fn_name_literal = fn_name.to_string();
	let props_type_literal = props_ident.to_string();
	let component_name_literal = component_ident.to_string();

	Ok(quote! {
		#loader_assertion

		#[derive(#pages_crate::__private::bon::Builder)]
		#[builder(crate = #pages_crate::__private::bon)]
		#vis struct #props_ident {
			#(#props_fields,)*
		}

		#(#extractor_type_aliases)*

		impl #pages_crate::router::request::FromRequest for #props_ident {
			fn from_request(
				ctx: &#pages_crate::router::request::RouteContext,
			) -> ::std::result::Result<Self, #pages_crate::router::request::ExtractError> {
				::std::result::Result::Ok(Self {
					#(#from_request_fields,)*
				})
			}
		}

		impl #pages_crate::__private::reinhardt_urls::routers::client_router::ComponentInfo
			for #props_ident
		{
			fn path() -> &'static str { #path }
			fn name() -> &'static str { #route_name }
			fn component_name() -> &'static str { #component_name_literal }
			fn function_name() -> &'static str { #fn_name_literal }
			fn props_type_name() -> &'static str { #props_type_literal }
			fn loader_id() -> ::core::option::Option<#pages_crate::RouteLoaderId> {
				#loader_id_method
			}
			fn navigation_guard_id() -> ::core::option::Option<#pages_crate::NavigationGuardId> {
				#navigation_guard_id_method
			}
		}

		#pages_crate::__private::inventory::submit! {
			#pages_crate::__private::reinhardt_urls::routers::client_router::ComponentMetadata {
				path: #path,
				name: #route_name,
				component_name: #component_name_literal,
				function_name: #fn_name_literal,
				props_type_name: #props_type_literal,
				module_path: ::core::module_path!(),
				loader_id: #loader_id_method,
			}
		}

		#navigation_guard_metadata

		fn #original_ident(#(#original_inputs,)*) #output {
			#block
		}

		#(#attrs)*
		#vis fn #fn_name(props: #props_ident) #output {
			let #props_ident { #(#destructure_fields,)* } = props;
			#loader_load
			#original_ident(#(#call_args,)*)
		}
	})
}

fn expand_layout(args: LayoutArgs, input: ItemFn) -> syn::Result<proc_macro2::TokenStream> {
	if input.sig.asyncness.is_some() {
		return Err(syn::Error::new_spanned(
			input.sig.asyncness,
			"#[layout] functions must not be async",
		));
	}
	if !input.sig.generics.params.is_empty() || input.sig.generics.where_clause.is_some() {
		return Err(syn::Error::new_spanned(
			input.sig.generics,
			"#[layout] functions must not be generic",
		));
	}
	match &input.sig.output {
		ReturnType::Type(_, ty) if is_page_type(ty) => {}
		_ => {
			return Err(syn::Error::new_spanned(
				&input.sig,
				"#[layout] functions must return Page",
			));
		}
	}

	let pages_crate = get_reinhardt_pages_crate();
	let LayoutArgs {
		path,
		name: route_name,
		loader: loader_option,
		navigation_guard: navigation_guard_option,
	} = args;
	let fn_name = input.sig.ident.clone();
	let component_name = fn_name.to_string().to_case(Case::Pascal);
	let component_ident = format_ident!("{component_name}", span = fn_name.span());
	let props_ident = format_ident!("{}Props", component_name, span = fn_name.span());
	let original_ident = format_ident!("__{}_layout_body", fn_name, span = fn_name.span());
	let output = input.sig.output.clone();
	let vis = input.vis.clone();
	let field_vis = field_visibility_tokens(&vis);
	let attrs = input.attrs.clone();
	let block = input.block.clone();
	let args_info = parse_layout_args(&input.sig.inputs)?;
	let extracted_args = &args_info.extracted;
	let outlet_name = &args_info.outlet_name;
	let outlet_ty = &args_info.outlet_ty;
	let loader_binding =
		resolve_loader_binding(extracted_args, loader_option.as_ref(), "#[layout]")?;
	let loader_path = loader_binding.as_ref().map(|binding| binding.path);
	let loader_argument = loader_binding.as_ref().map(|binding| binding.argument);
	let loader_type = loader_argument.map(|argument| &argument.ty);
	let loaded_value_ident = format_ident!("__{}_loader_value", fn_name, span = fn_name.span());
	let loader_id = loader_path.map(|path| {
		quote! { <#path::marker as #pages_crate::RouteLoader>::ID }
	});
	let loader_id_method = loader_id.as_ref().map_or_else(
		|| quote! { ::core::option::Option::None },
		|id| quote! { ::core::option::Option::Some(#id) },
	);
	let navigation_guard_id = navigation_guard_option.map(|path| {
		quote! { <#path::marker as #pages_crate::NavigationGuard>::ID }
	});
	let navigation_guard_id_method = navigation_guard_id.as_ref().map_or_else(
		|| quote! { ::core::option::Option::None },
		|id| quote! { ::core::option::Option::Some(#id) },
	);
	let navigation_guard_metadata = navigation_guard_id.as_ref().map_or_else(
		|| quote! {},
		|id| {
			quote! {
				#pages_crate::__private::inventory::submit! {
					#pages_crate::__private::reinhardt_urls::routers::client_router::LayoutNavigationGuardMetadata {
						path: #path,
						name: #route_name,
						navigation_guard_id: #id,
					}
				}
			}
		},
	);
	let loader_assertion = loader_binding.as_ref().map_or_else(
		|| quote! {},
		|binding| {
			let assertion_name =
				format_ident!("__{}_assert_loader_data", fn_name, span = fn_name.span());
			let path = binding.path;
			let data = &binding.argument.ty;
			quote! {
				fn #assertion_name<M, T>()
				where
					M: #pages_crate::RouteLoader<Data = T>,
					T: ::core::clone::Clone
						+ #pages_crate::__private::serde::Serialize
						+ #pages_crate::__private::serde::de::DeserializeOwned
						+ 'static,
				{
				}
				const _: fn() = || #assertion_name::<#path::marker, #data>();
			}
		},
	);
	let loader_load = match (loader_type, loader_id.as_ref()) {
		(Some(data), Some(id)) => quote! {
			let #loaded_value_ident = #pages_crate::router::loader::active_loader_store()
				.expect("a loaded route must render inside a LoaderStore scope")
				.get::<#data>(#id)
				.map(|value| value.0)
				.unwrap_or_else(|error| panic!("{error}"));
		},
		_ => quote! {},
	};

	let props_fields = extracted_args
		.iter()
		.filter(|arg| arg.source != Source::Loader)
		.map(|arg| {
			let name = &arg.name;
			let ty = &arg.ty;
			quote! { #field_vis #name: #ty }
		});
	let destructure_fields = extracted_args
		.iter()
		.filter(|arg| arg.source != Source::Loader)
		.map(|arg| &arg.name);
	let original_inputs = extracted_args.iter().map(|arg| {
		let name = &arg.name;
		let ty = &arg.ty;
		quote! { #name: #ty }
	});
	let call_args = extracted_args.iter().map(|arg| {
		if arg.source == Source::Loader {
			quote! { #loaded_value_ident }
		} else {
			let name = &arg.name;
			quote! { #name }
		}
	});
	let from_request_fields = extracted_args
		.iter()
		.filter(|arg| arg.source != Source::Loader)
		.map(|arg| {
			let name = &arg.name;
			let key = name.to_string();
			let ty = &arg.ty;
			match arg.source {
				Source::Path => quote! {
					#name: #pages_crate::router::request::PathParam::<#ty>::extract(ctx, #key)?.into_inner()
				},
				Source::Query => {
					if let Some(inner) = option_inner_type(ty) {
						quote! {
							#name: #pages_crate::router::request::OptionalQueryParam::<#inner>::extract(ctx, #key)?.into_inner()
						}
					} else {
						quote! {
							#name: #pages_crate::router::request::QueryParam::<#ty>::extract(ctx, #key)?.into_inner()
						}
					}
				}
				Source::Loader => unreachable!("loader inputs are not props fields"),
			}
		});
	let extractor_type_aliases = extracted_args.iter().enumerate().map(|(index, arg)| {
		let alias = format_ident!(
			"__{}PropsExtractor{}",
			component_name,
			index,
			span = fn_name.span()
		);
		let extractor_ty = &arg.extractor_ty;
		quote! {
			// Keep the extractor type from the original signature referenced after
			// the attribute macro rewrites the function into a props argument.
			#[allow(non_camel_case_types, dead_code)]
			type #alias = #extractor_ty;
		}
	});

	let fn_name_literal = fn_name.to_string();
	let props_type_literal = props_ident.to_string();
	let component_name_literal = component_ident.to_string();

	Ok(quote! {
		#loader_assertion

		#[derive(#pages_crate::__private::bon::Builder)]
		#[builder(crate = #pages_crate::__private::bon)]
		#vis struct #props_ident {
			#(#props_fields,)*
			#field_vis #outlet_name: #pages_crate::component::Outlet,
		}

		#(#extractor_type_aliases)*

		impl #pages_crate::__private::reinhardt_urls::routers::client_router::FromLayoutRequest
			for #props_ident
		{
			fn from_layout_request(
				ctx: &#pages_crate::router::request::RouteContext,
				outlet: #pages_crate::component::Outlet,
			) -> ::std::result::Result<Self, #pages_crate::router::request::ExtractError> {
				::std::result::Result::Ok(Self {
					#(#from_request_fields,)*
					#outlet_name: outlet,
				})
			}
		}

		impl #pages_crate::__private::reinhardt_urls::routers::client_router::LayoutInfo
			for #props_ident
		{
			fn path() -> &'static str { #path }
			fn name() -> &'static str { #route_name }
			fn component_name() -> &'static str { #component_name_literal }
			fn function_name() -> &'static str { #fn_name_literal }
			fn props_type_name() -> &'static str { #props_type_literal }
			fn loader_id() -> ::core::option::Option<#pages_crate::RouteLoaderId> {
				#loader_id_method
			}
			fn navigation_guard_id() -> ::core::option::Option<#pages_crate::NavigationGuardId> {
				#navigation_guard_id_method
			}
		}

		#pages_crate::__private::inventory::submit! {
			#pages_crate::__private::reinhardt_urls::routers::client_router::LayoutMetadata {
				path: #path,
				name: #route_name,
				component_name: #component_name_literal,
				function_name: #fn_name_literal,
				props_type_name: #props_type_literal,
				module_path: ::core::module_path!(),
				loader_id: #loader_id_method,
			}
		}

		#navigation_guard_metadata

		fn #original_ident(#(#original_inputs,)* #outlet_name: #outlet_ty) #output {
			#block
		}

		#(#attrs)*
		#vis fn #fn_name(props: #props_ident) #output {
			let #props_ident { #(#destructure_fields,)* #outlet_name } = props;
			#loader_load
			#original_ident(#(#call_args,)* #outlet_name)
		}
	})
}

fn field_visibility_tokens(vis: &Visibility) -> proc_macro2::TokenStream {
	match vis {
		Visibility::Inherited => quote! {},
		_ => quote! { #vis },
	}
}

fn is_page_type(ty: &Type) -> bool {
	let Type::Path(type_path) = ty else {
		return false;
	};
	type_path
		.path
		.segments
		.last()
		.is_some_and(|segment| segment.ident == "Page")
}

fn parse_args(inputs: &Punctuated<FnArg, Token![,]>) -> syn::Result<Vec<ExtractedArg>> {
	let mut seen = HashSet::new();
	let mut out = Vec::new();

	for input in inputs {
		let FnArg::Typed(PatType { pat, ty, .. }) = input else {
			return Err(syn::Error::new_spanned(
				input,
				"#[component] does not support receiver arguments",
			));
		};

		let arg = parse_extractor_arg(pat, ty, "#[component]")?;
		let name = arg.name.clone();
		if !seen.insert(name.to_string()) {
			return Err(syn::Error::new_spanned(
				&name,
				format!("duplicate component props field `{name}`"),
			));
		}
		out.push(arg);
	}

	Ok(out)
}

fn resolve_loader_binding<'a>(
	args: &'a [ExtractedArg],
	loader_path: Option<&'a syn::Path>,
	macro_name: &str,
) -> syn::Result<Option<LoaderBinding<'a>>> {
	let loader_args = args
		.iter()
		.filter(|argument| argument.source == Source::Loader)
		.collect::<Vec<_>>();
	if loader_args.len() > 1 {
		return Err(syn::Error::new_spanned(
			loader_args[1].name.clone(),
			format!("{macro_name} functions accept exactly one Loader<T> input"),
		));
	}
	match (loader_path, loader_args.first().copied()) {
		(Some(path), Some(argument)) => Ok(Some(LoaderBinding { path, argument })),
		(Some(_), None) => Err(syn::Error::new(
			proc_macro2::Span::call_site(),
			format!("{macro_name} route binds a loader but has no Loader<T> input"),
		)),
		(None, Some(argument)) => Err(syn::Error::new_spanned(
			argument.name.clone(),
			format!("{macro_name} Loader<T> input requires a `loader = ...` route option"),
		)),
		(None, None) => Ok(None),
	}
}

fn parse_layout_args(inputs: &Punctuated<FnArg, Token![,]>) -> syn::Result<LayoutFunctionArgs> {
	let mut seen = HashSet::new();
	let mut extracted = Vec::new();
	let mut outlet = None;

	for input in inputs {
		let FnArg::Typed(PatType { pat, ty, .. }) = input else {
			return Err(syn::Error::new_spanned(
				input,
				"#[layout] does not support receiver arguments",
			));
		};

		if is_outlet_type(ty) {
			let Pat::Ident(pat_ident) = &**pat else {
				return Err(syn::Error::new_spanned(
					pat,
					"#[layout] Outlet parameter must bind a plain identifier",
				));
			};
			let name = pat_ident.ident.clone();
			if outlet.is_some() {
				return Err(syn::Error::new_spanned(
					&name,
					"#[layout] functions must accept exactly one Outlet parameter",
				));
			}
			if !seen.insert(name.to_string()) {
				return Err(syn::Error::new_spanned(
					&name,
					format!("duplicate layout props field `{name}`"),
				));
			}
			outlet = Some((name, (**ty).clone()));
			continue;
		}

		let arg = parse_extractor_arg(pat, ty, "#[layout]")?;
		let name = arg.name.clone();
		if !seen.insert(name.to_string()) {
			return Err(syn::Error::new_spanned(
				&name,
				format!("duplicate layout props field `{name}`"),
			));
		}
		extracted.push(arg);
	}

	let Some((outlet_name, outlet_ty)) = outlet else {
		return Err(syn::Error::new_spanned(
			inputs,
			"#[layout] functions must accept exactly one Outlet parameter",
		));
	};

	Ok(LayoutFunctionArgs {
		extracted,
		outlet_name,
		outlet_ty,
	})
}

fn parse_extractor_arg(pat: &Pat, ty: &Type, macro_name: &str) -> syn::Result<ExtractedArg> {
	let Pat::TupleStruct(PatTupleStruct { path, elems, .. }) = pat else {
		return Err(syn::Error::new_spanned(
			pat,
			format!(
				"{macro_name} arguments must use extractor destructuring such as Path(id): Path<i64>"
			),
		));
	};

	if elems.len() != 1 {
		return Err(syn::Error::new_spanned(
			elems,
			"extractor destructuring must bind exactly one identifier",
		));
	}

	let Pat::Ident(pat_ident) = &elems[0] else {
		return Err(syn::Error::new_spanned(
			&elems[0],
			"extractor destructuring must bind an identifier",
		));
	};
	let name = pat_ident.ident.clone();
	let source_ident = path
		.segments
		.last()
		.map(|seg| seg.ident.to_string())
		.ok_or_else(|| syn::Error::new_spanned(path, "expected Path or Query extractor"))?;
	let source = match source_ident.as_str() {
		"Path" => Source::Path,
		"Query" => Source::Query,
		"Loader" => Source::Loader,
		_ => {
			return Err(syn::Error::new_spanned(
				path,
				"expected Path(...), Query(...), or Loader(...) extractor",
			));
		}
	};

	let inner_ty = extractor_inner_type(ty, source_ident.as_str())?;
	Ok(ExtractedArg {
		source,
		name,
		ty: inner_ty,
		extractor_ty: ty.clone(),
	})
}

fn is_outlet_type(ty: &Type) -> bool {
	let Type::Path(type_path) = ty else {
		return false;
	};
	type_path
		.path
		.segments
		.last()
		.is_some_and(|segment| segment.ident == "Outlet")
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
	let syn::PathArguments::AngleBracketed(args) = &segment.arguments else {
		return Err(syn::Error::new_spanned(ty, "expected Path<T> or Query<T>"));
	};
	let Some(syn::GenericArgument::Type(inner)) = args.args.first() else {
		return Err(syn::Error::new_spanned(ty, "expected Path<T> or Query<T>"));
	};
	Ok(inner.clone())
}
