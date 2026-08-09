use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::{
	FnArg, ItemFn, LitStr, Pat, PatType, Result, Token,
	parse::{Parse, ParseStream},
};

use crate::crate_paths::get_reinhardt_crate;
use crate::injectable_common::generate_inject_resolver_expr;
use crate::routes::{detect_inject_params, extract_url_params, to_resolver_trait_name};

pub(crate) struct WebSocketArgs {
	pub path: String,
	pub name: Option<String>,
}

impl Parse for WebSocketArgs {
	fn parse(input: ParseStream) -> Result<Self> {
		// First argument: the path string literal (supports raw strings and escapes).
		let path_lit: LitStr = input.parse().map_err(|e| {
			syn::Error::new(
				e.span(),
				"#[websocket] expects a string path as first argument",
			)
		})?;
		let path = path_lit.value();

		let mut name: Option<String> = None;
		while !input.is_empty() {
			input.parse::<Token![,]>()?;
			if input.is_empty() {
				break;
			}
			let ident: syn::Ident = input.parse()?;
			input.parse::<Token![=]>()?;
			if ident == "name" {
				let lit: LitStr = input.parse()?;
				name = Some(lit.value());
			} else {
				return Err(syn::Error::new(
					ident.span(),
					format!("unknown #[websocket] argument `{}`", ident),
				));
			}
		}
		Ok(Self { path, name })
	}
}

impl WebSocketArgs {
	pub(crate) fn parse(args: TokenStream) -> Result<Self> {
		syn::parse2::<Self>(args)
	}
}

/// Generate URL resolver extension trait tokens for a WebSocket endpoint.
/// Parallel to `generate_url_resolver_tokens()` in routes.rs, but uses
/// `WebSocketUrlResolver` as supertrait and `__ws_url_resolver_*` naming.
pub(crate) fn generate_ws_url_resolver_tokens(
	route_name: &Option<String>,
	fn_name: &str,
	path: &str,
	reinhardt_crate: &TokenStream,
) -> TokenStream {
	let Some(name) = route_name.as_ref() else {
		return quote! {};
	};

	if path.contains('*') {
		return quote! {};
	}

	if syn::parse_str::<syn::Ident>(name).is_err() {
		let msg = format!(
			"WebSocket route name `{name}` is not a valid Rust identifier. \
             Route names used with url-resolver must be valid identifiers \
             (no hyphens, dots, or leading digits)."
		);
		return quote! { ::core::compile_error!(#msg); };
	}

	let trait_name_str = to_resolver_trait_name(name);
	let trait_ident = syn::Ident::new(&trait_name_str, Span::call_site());
	let method_ident = syn::Ident::new(name, Span::call_site());
	// Module/macro names use fn_name (parallel to HTTP __url_resolver_{fn_name})
	let resolver_mod_ident =
		syn::Ident::new(&format!("__ws_url_resolver_{fn_name}"), Span::call_site());
	let meta_macro_ident = syn::Ident::new(
		&format!("__ws_url_resolver_meta_{fn_name}"),
		Span::call_site(),
	);
	let params = extract_url_params(path);
	let doc_str = format!(
		"Resolve WebSocket URL for route `{}` (pattern: `{}`).",
		name, path
	);
	let param_strs: Vec<&str> = params.iter().map(|s| s.as_str()).collect();

	if params.is_empty() {
		quote! {
			#[doc(hidden)]
			pub mod #resolver_mod_ident {
				#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
				macro_rules! #meta_macro_ident {
					($callback:ident, $app:ident) => {
						$callback!($app, #method_ident, #name, );
					};
				}
				#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
				pub(crate) use #meta_macro_ident;

				#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
				#[doc = #doc_str]
				pub trait #trait_ident: #reinhardt_crate::WebSocketUrlResolver {
					#[doc = #doc_str]
					fn #method_ident(&self) -> String {
						self.resolve_ws_url(#name, &[])
					}
				}
				#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
				impl<T: #reinhardt_crate::WebSocketUrlResolver> #trait_ident for T {}
			}
			#[doc(hidden)]
			pub use #resolver_mod_ident::*;
		}
	} else {
		let param_idents: Vec<syn::Ident> = params
			.iter()
			.map(|p| syn::Ident::new(p, Span::call_site()))
			.collect();

		quote! {
			#[doc(hidden)]
			pub mod #resolver_mod_ident {
				#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
				macro_rules! #meta_macro_ident {
					($callback:ident, $app:ident) => {
						$callback!($app, #method_ident, #name, #(#param_strs),* );
					};
				}
				#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
				pub(crate) use #meta_macro_ident;

				#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
				#[doc = #doc_str]
				pub trait #trait_ident: #reinhardt_crate::WebSocketUrlResolver {
					#[doc = #doc_str]
					fn #method_ident(&self, #(#param_idents: &str),*) -> String {
						self.resolve_ws_url(#name, &[#((#param_strs, #param_idents)),*])
					}
				}
				#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
				impl<T: #reinhardt_crate::WebSocketUrlResolver> #trait_ident for T {}
			}
			#[doc(hidden)]
			pub use #resolver_mod_ident::*;
		}
	}
}

/// Main implementation for `#[websocket]` proc macro.
/// Parallel to `route_impl()` in routes.rs.
pub(crate) fn websocket_impl(args: TokenStream, mut input: ItemFn) -> Result<TokenStream> {
	// Handlers must be `async fn`; otherwise the generated `.await` in the
	// WebSocketConsumer impl produces a confusing type error.
	if input.sig.asyncness.is_none() {
		return Err(syn::Error::new_spanned(
			input.sig.fn_token,
			"`#[websocket]` handlers must be `async fn`",
		));
	}
	let reinhardt_crate = get_reinhardt_crate();
	let ws_crate = crate::crate_paths::get_reinhardt_websockets_crate();
	let ws_args = WebSocketArgs::parse(args)?;

	let fn_name = input.sig.ident.clone();
	let fn_name_str = fn_name.to_string();
	let fn_vis = input.vis.clone();

	let route_name = ws_args.name.clone().or_else(|| Some(fn_name_str.clone()));

	// Detect #[inject] params (same as route_impl)
	let inject_params = detect_inject_params(&input.sig.inputs);
	let has_inject = !inject_params.is_empty();

	// Consumer struct name: chat_ws → ChatWsConsumer
	let consumer_name_str =
		crate::pascal_case::to_pascal_case_with_suffix(&fn_name_str, "Consumer");
	let consumer_ident = syn::Ident::new(&consumer_name_str, Span::call_site());

	// Rename original function
	let original_fn_ident =
		syn::Ident::new(&format!("{}_original", fn_name_str), Span::call_site());
	input.sig.ident = original_fn_ident.clone();

	// Separate non-inject params (context + message) BEFORE stripping
	// #[inject] attributes — otherwise the filter below matches nothing and
	// injected params leak into the generated WebSocketConsumer trait impl.
	let non_inject_params: Vec<FnArg> = input
		.sig
		.inputs
		.iter()
		.filter(|arg| {
			if let FnArg::Typed(pt) = arg {
				!pt.attrs
					.iter()
					.any(crate::injectable_common::is_inject_attr)
			} else {
				true
			}
		})
		.cloned()
		.collect();
	let source_inputs = input.sig.inputs.clone();

	// Strip #[inject] attrs from parameters in the original fn
	for arg in input.sig.inputs.iter_mut() {
		if let FnArg::Typed(pt) = arg {
			pt.attrs
				.retain(|a| !crate::injectable_common::is_inject_attr(a));
		}
	}

	let non_inject_arg_pats: Vec<&Pat> = non_inject_params
		.iter()
		.filter_map(|arg| {
			if let FnArg::Typed(PatType { pat, .. }) = arg {
				Some(pat.as_ref())
			} else {
				None
			}
		})
		.collect();

	// Consumer struct fields for injected deps
	let inject_field_decls: Vec<TokenStream> = inject_params
		.iter()
		.map(|p| {
			let resolved_ident = &p.resolved_ident;
			let ty = &p.ty;
			quote! { pub #resolved_ident: #ty }
		})
		.collect();

	let inject_field_names: Vec<_> = inject_params.iter().map(|p| &p.resolved_ident).collect();

	// Consumer struct body
	let consumer_struct_body = if has_inject {
		quote! { { #(#inject_field_decls),* } }
	} else {
		quote! { ; }
	};

	// on_message call
	let on_message_call = if has_inject {
		let mut inject_args = inject_params.iter();
		let call_args: Vec<_> = source_inputs
			.iter()
			.filter_map(|arg| {
				let FnArg::Typed(pt) = arg else { return None };
				if pt
					.attrs
					.iter()
					.any(crate::injectable_common::is_inject_attr)
				{
					let resolved_ident = &inject_args
						.next()
						.expect("each injected argument must have detected metadata")
						.resolved_ident;
					Some(quote! { self.#resolved_ident.clone() })
				} else {
					let pat = &pt.pat;
					Some(quote! { #pat })
				}
			})
			.collect();
		quote! {
			#original_fn_ident(#(#call_args),*).await
		}
	} else {
		quote! { #original_fn_ident(#(#non_inject_arg_pats),*).await }
	};

	let path = &ws_args.path;
	let name_str = route_name.as_deref().unwrap_or(&fn_name_str);
	let source = quote! {
		::core::concat!(::core::module_path!(), "::", ::core::stringify!(#fn_name))
	};

	let (preflight_stmts, build_stmts) = if has_inject {
		let di_crate = quote! { #ws_crate::reinhardt_di };
		let resolve_exprs: Vec<TokenStream> = inject_params
			.iter()
			.map(|p| {
				let ty = &p.ty;
				generate_inject_resolver_expr(&di_crate, ty, quote! { &*__di_ctx }, true)
			})
			.collect();
		let preflight_stmts = inject_params
			.iter()
			.zip(&resolve_exprs)
			.map(|(param, resolve_expr)| {
				let ty = &param.ty;
				quote! {
					let _: #ty = #resolve_expr.map_err(|__error| {
						#ws_crate::ConsumerBuildError::new(
							#source,
							::core::any::type_name::<#ty>(),
							__error,
						)
					})?;
				}
			})
			.collect();
		let build_stmts = inject_params
			.iter()
			.zip(&resolve_exprs)
			.map(|(param, resolve_expr)| {
				let resolved_ident = &param.resolved_ident;
				let ty = &param.ty;
				quote! {
					let #resolved_ident: #ty = #resolve_expr.map_err(|__error| {
						#ws_crate::ConsumerBuildError::new(
							#source,
							::core::any::type_name::<#ty>(),
							__error,
						)
					})?;
				}
			})
			.collect();
		(preflight_stmts, build_stmts)
	} else {
		(Vec::new(), Vec::new())
	};

	let build_consumer = if has_inject {
		quote! { #consumer_ident { #(#inject_field_names),* } }
	} else {
		quote! { #consumer_ident }
	};

	let endpoint_ident =
		syn::Ident::new(&format!("{consumer_name_str}Endpoint"), Span::call_site());
	let endpoint_descriptor = has_inject.then(|| {
		quote! {
			#fn_vis struct #endpoint_ident;

			impl #ws_crate::WebSocketEndpointInfo for #endpoint_ident {
				fn path() -> &'static str { #path }
				fn name() -> ::core::option::Option<&'static str> {
					::core::option::Option::Some(#name_str)
				}
				fn consumer_key() -> #ws_crate::WebSocketConsumerKey {
					#consumer_ident::KEY
				}
			}
		}
	});
	let (selector_type, selector_value) = if has_inject {
		(quote! { #endpoint_ident }, quote! { #endpoint_ident })
	} else {
		(quote! { #consumer_ident }, quote! { #consumer_ident })
	};

	// URL resolver tokens
	let url_resolver_tokens = generate_ws_url_resolver_tokens(
		&Some(name_str.to_string()),
		&fn_name_str,
		path,
		&reinhardt_crate,
	);

	let non_inject_params_tokens: Vec<TokenStream> =
		non_inject_params.iter().map(|a| quote! { #a }).collect();

	Ok(quote! {
		// Renamed original function
		#fn_vis #input

		// Consumer struct (parallel to {FnName}View for HTTP)
		#fn_vis struct #consumer_ident #consumer_struct_body
		impl #consumer_ident {
			const KEY: #ws_crate::WebSocketConsumerKey = #ws_crate::WebSocketConsumerKey::new(
				::core::concat!(
					::core::module_path!(),
					"::",
					::core::stringify!(#consumer_ident),
				)
			);
		}

		// WebSocketEndpointInfo impl (parallel to EndpointInfo)
		impl #ws_crate::WebSocketEndpointInfo for #consumer_ident {
			fn path() -> &'static str { #path }
			fn name() -> ::core::option::Option<&'static str> { ::core::option::Option::Some(#name_str) }
			fn consumer_key() -> #ws_crate::WebSocketConsumerKey { Self::KEY }
		}

		#endpoint_descriptor

		// WebSocketConsumer impl — on_message delegates to original fn
		#[#reinhardt_crate::async_trait]
		impl #ws_crate::WebSocketConsumer for #consumer_ident {
			async fn on_connect(
				&self,
				_ctx: &mut #ws_crate::ConsumerContext,
			) -> #ws_crate::WebSocketResult<()> {
				::core::result::Result::Ok(())
			}
			async fn on_message(
				&self,
				#(#non_inject_params_tokens),*
			) -> #ws_crate::WebSocketResult<()> {
				#on_message_call
			}
			async fn on_disconnect(
				&self,
				_ctx: &mut #ws_crate::ConsumerContext,
			) -> #ws_crate::WebSocketResult<()> {
				::core::result::Result::Ok(())
			}
		}

		// Inventory registration (parallel to EndpointMetadata).
		#[allow(unsafe_attr_outside_unsafe)]
		const _: () = {
			fn __reinhardt_websocket_preflight(
				__di_ctx: ::std::sync::Arc<#ws_crate::InjectionContext>,
			) -> #ws_crate::ConsumerPreflightFuture {
				::std::boxed::Box::pin(async move {
					#(#preflight_stmts)*
					::core::result::Result::Ok(())
				})
			}

			fn __reinhardt_websocket_build(
				__di_ctx: ::std::sync::Arc<#ws_crate::InjectionContext>,
			) -> #ws_crate::ConsumerBuildFuture {
				::std::boxed::Box::pin(async move {
					#(#build_stmts)*
					let __consumer: ::std::boxed::Box<dyn #ws_crate::WebSocketConsumer> =
						::std::boxed::Box::new(#build_consumer);
					::core::result::Result::Ok(__consumer)
				})
			}

			#ws_crate::inventory::submit! {
				#ws_crate::WebSocketEndpointMetadata {
					path: #path,
					name: #name_str,
					fn_name: ::core::stringify!(#fn_name),
					module_path: ::core::module_path!(),
				}
			}

			#ws_crate::inventory::submit! {
				#ws_crate::WebSocketConsumerRegistration::new(
					#consumer_ident::KEY,
					#source,
					__reinhardt_websocket_preflight,
					__reinhardt_websocket_build,
				)
			}
		};

		// Route selector (parallel to fn login() -> LoginView).
		#fn_vis fn #fn_name() -> #selector_type {
			#selector_value
		}

		// URL resolver extension trait
		#url_resolver_tokens
	})
}

#[cfg(test)]
mod tests {
	use super::*;
	use syn::parse_quote;

	#[test]
	fn preserves_interleaved_argument_order() {
		let input: ItemFn = parse_quote! {
			async fn chat(first: String, #[inject] mut left: Service, last: usize, #[inject] mut right: Service) -> Result<(), Error> { Ok(()) }
		};
		let generated = websocket_impl(quote! { "/chat" }, input)
			.unwrap()
			.to_string();
		assert!(generated.contains("chat_original (first , self . __reinhardt_injected_0 . clone () , last , self . __reinhardt_injected_1 . clone ())"), "{generated}");
	}

	#[test]
	fn generates_fallible_injected_consumer_registration() {
		let input: ItemFn = parse_quote! {
			async fn chat(context: &mut ConsumerContext, message: Message, #[inject] service: Service) -> Result<(), Error> { Ok(()) }
		};
		let expanded = websocket_impl(quote! { "/chat" }, input)
			.unwrap()
			.to_string();

		assert!(!expanded.contains("expect ( \"websocket dependency injection failed\" )"));
		assert!(!expanded.contains("unimplemented !"));
		assert!(expanded.contains("WebSocketConsumerRegistration"));
		assert!(expanded.contains("ConsumerBuildError"));
	}
}
