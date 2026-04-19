use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::{
    FnArg, ItemFn, LitStr, Pat, PatType, Result, Token,
    parse::{Parse, ParseStream, Parser},
};

use crate::crate_paths::get_reinhardt_crate;
use crate::routes::{InjectInfo, detect_inject_params, extract_url_params, to_resolver_trait_name};

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
        <Self as Parse>::parse.parse2(args)
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
    let meta_macro_ident =
        syn::Ident::new(&format!("__ws_url_resolver_meta_{fn_name}"), Span::call_site());
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
            &input.sig.fn_token,
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
            let pat = &p.pat;
            let ty = &p.ty;
            quote! { pub #pat: #ty }
        })
        .collect();

    let inject_field_clones: Vec<TokenStream> = inject_params
        .iter()
        .map(|p| {
            let pat = &p.pat;
            quote! { self.#pat.clone() }
        })
        .collect();

    let inject_pat_names: Vec<&Pat> = inject_params.iter().map(|p| p.pat.as_ref()).collect();

    // Consumer struct body
    let consumer_struct_body = if has_inject {
        quote! { { #(#inject_field_decls),* } }
    } else {
        quote! { ; }
    };

    // on_message call
    let on_message_call = if has_inject {
        quote! {
            #original_fn_ident(#(#non_inject_arg_pats,)* #(#inject_field_clones),*).await
        }
    } else {
        quote! { #original_fn_ident(#(#non_inject_arg_pats),*).await }
    };

    // DI factory impl (generated when #[inject] params present)
    let di_factory_impl = if has_inject {
        let di_crate = crate::crate_paths::get_reinhardt_di_crate();
        let resolve_stmts: Vec<TokenStream> = inject_params
            .iter()
            .map(|p| {
                let pat = &p.pat;
                let ty = &p.ty;
                if let Some(inner) =
                    crate::routes_registration::extract_depends_inner_type(ty)
                {
                    quote! {
                        let #pat: #ty =
                            #di_crate::Depends::<#inner>::resolve_from_registry(
                                &__di_ctx, true
                            ).await;
                    }
                } else {
                    quote! {
                        let #pat: #ty =
                            #di_crate::Depends::<#ty>::resolve(&__di_ctx, true).await;
                    }
                }
            })
            .collect();
        quote! {
            #[cfg(feature = "di")]
            #[#reinhardt_crate::async_trait]
            impl #ws_crate::WebSocketConsumerFactory for #consumer_ident {
                async fn build(ctx: &#ws_crate::InjectionContext) -> Self {
                    let __di_ctx = ctx.clone();
                    #(#resolve_stmts)*
                    Self { #(#inject_pat_names),* }
                }
            }
        }
    } else {
        quote! {}
    };

    // Factory function body
    let factory_body = if has_inject {
        quote! { unimplemented!("Use injectable_consumer() with DI context") }
    } else {
        quote! { #consumer_ident }
    };

    let path = &ws_args.path;
    let name_str = route_name.as_deref().unwrap_or(&fn_name_str);

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

        // WebSocketEndpointInfo impl (parallel to EndpointInfo)
        impl #ws_crate::WebSocketEndpointInfo for #consumer_ident {
            fn path() -> &'static str { #path }
            fn name() -> ::core::option::Option<&'static str> { ::core::option::Option::Some(#name_str) }
        }

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

        #di_factory_impl

        // inventory registration (parallel to EndpointMetadata)
        #[allow(unsafe_attr_outside_unsafe)]
        const _: () = {
            #ws_crate::inventory::submit! {
                #ws_crate::WebSocketEndpointMetadata {
                    path: #path,
                    name: #name_str,
                    fn_name: ::core::stringify!(#fn_name),
                    module_path: ::core::module_path!(),
                }
            }
        };

        // Factory function (parallel to fn login() -> LoginView)
        #fn_vis fn #fn_name() -> #consumer_ident {
            #factory_body
        }

        // URL resolver extension trait
        #url_resolver_tokens
    })
}
