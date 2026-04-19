use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::{
    FnArg, ItemFn, LitStr, Pat,
    parse::{Parse, ParseStream},
    Token,
};

use crate::crate_paths::get_inventory_crate;

// ─────────────────────────────────────────────────────────
// #[producer] macro
// ─────────────────────────────────────────────────────────

struct ProducerArgs {
    topic: String,
    name: String,
}

impl Parse for ProducerArgs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut topic = None;
        let mut name = None;
        while !input.is_empty() {
            let ident: syn::Ident = input.parse()?;
            let _: Token![=] = input.parse()?;
            let value: LitStr = input.parse()?;
            match ident.to_string().as_str() {
                "topic" => topic = Some(value.value()),
                "name" => name = Some(value.value()),
                other => {
                    return Err(syn::Error::new(
                        ident.span(),
                        format!("unknown argument `{other}`"),
                    ))
                }
            }
            if input.peek(Token![,]) {
                let _: Token![,] = input.parse()?;
            }
        }
        Ok(ProducerArgs {
            topic: topic.ok_or_else(|| {
                syn::Error::new(Span::call_site(), "missing `topic` argument")
            })?,
            name: name.ok_or_else(|| {
                syn::Error::new(Span::call_site(), "missing `name` argument")
            })?,
        })
    }
}

pub(crate) fn producer_impl(args: TokenStream, input: ItemFn) -> syn::Result<TokenStream> {
    let args: ProducerArgs = syn::parse2(args)?;
    let topic = &args.topic;
    let name = &args.name;

    // `#[producer]` generates a wrapper that awaits the inner fn, so the
    // annotated function must be `async`. Reject non-async fns with a clear
    // compile error instead of silently emitting `.await` on a non-future.
    if input.sig.asyncness.is_none() {
        return Err(syn::Error::new_spanned(
            &input.sig.fn_token,
            "#[producer] can only be applied to `async fn`",
        ));
    }

    let fn_name = &input.sig.ident;
    let fn_vis = &input.vis;
    let fn_sig = &input.sig;
    let fn_block = &input.block;
    let fn_attrs: Vec<_> = input.attrs.iter().filter(|a| !a.path().is_ident("producer")).collect();

    // Inner function name
    let inner_fn_name = syn::Ident::new(
        &format!("__producer_inner_{fn_name}"),
        fn_name.span(),
    );

    // Collect non-self, non-inject parameters for argument forwarding
    let call_args: Vec<TokenStream> = input
        .sig
        .inputs
        .iter()
        .filter_map(|arg| {
            if let FnArg::Typed(pat_type) = arg {
                // Skip #[inject] parameters (they are resolved by DI, not forwarded)
                let is_inject = pat_type
                    .attrs
                    .iter()
                    .any(|a| a.path().is_ident("inject"));
                if is_inject {
                    return None;
                }
                if let Pat::Ident(id) = pat_type.pat.as_ref() {
                    return Some(quote! { #id });
                }
            }
            None
        })
        .collect();

    // Build inner function signature: drop #[inject] parameters entirely so
    // the inner fn's arity matches `call_args` (which also filters them out).
    // Without this, the wrapper would call `inner(ctx, msg)` against a fn that
    // expects `(ctx, svc, msg)`, producing an arity mismatch.
    let inner_inputs: Vec<_> = input
        .sig
        .inputs
        .iter()
        .filter_map(|arg| {
            if let FnArg::Typed(pt) = arg {
                let is_inject = pt.attrs.iter().any(|a| a.path().is_ident("inject"));
                if is_inject {
                    return None;
                }
                let mut pt = pt.clone();
                pt.attrs.retain(|a| !a.path().is_ident("inject"));
                Some(FnArg::Typed(pt))
            } else {
                Some(arg.clone())
            }
        })
        .collect();

    let mut inner_sig = input.sig.clone();
    inner_sig.ident = inner_fn_name.clone();
    inner_sig.inputs = syn::punctuated::Punctuated::from_iter(inner_inputs);

    // Metadata module for ResolvedUrls generation
    let resolver_mod = syn::Ident::new(
        &format!("__streaming_resolver_{fn_name}"),
        Span::call_site(),
    );
    let meta_macro = syn::Ident::new(
        &format!("__streaming_resolver_meta_{fn_name}"),
        Span::call_site(),
    );
    let method_ident = syn::Ident::new(name, Span::call_site());

    let streaming_crate = quote! { ::reinhardt_streaming };
    let inventory = get_inventory_crate();

    Ok(quote! {
        // Metadata module (consumed by streaming_routes! and #[routes])
        #[doc(hidden)]
        pub mod #resolver_mod {
            #[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
            #[allow(unused_macros)]
            macro_rules! #meta_macro {
                ($callback:ident, $app:ident) => {
                    $callback!($app, #method_ident, #name, #topic);
                };
            }
            #[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
            pub(crate) use #meta_macro;
        }
        #[doc(hidden)]
        pub use #resolver_mod::*;

        // Inner function with original body
        #(#fn_attrs)*
        #[allow(non_snake_case)]
        #[doc(hidden)]
        #fn_vis #inner_sig {
            #fn_block
        }

        // Public wrapper that auto-publishes the return value
        #(#fn_attrs)*
        #fn_vis #fn_sig {
            let __result = #inner_fn_name(#(#call_args),*).await;
            if let Ok(ref __payload) = __result {
                if let Some(__producer) = #streaming_crate::global_producer() {
                    let _ = __producer.send(#topic, __payload).await;
                }
            }
            __result
        }

        // Register handler metadata so `resolve_streaming_topic(name)` can
        // find the corresponding topic at runtime.
        #[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
        #inventory::submit! {
            #streaming_crate::StreamingHandlerMetadata {
                name: #name,
                topic: #topic,
                kind: #streaming_crate::StreamingHandlerKind::Producer,
                group: ::core::option::Option::None,
                module_path: ::core::module_path!(),
            }
        }
    })
}

// ─────────────────────────────────────────────────────────
// #[consumer] macro
// ─────────────────────────────────────────────────────────

struct ConsumerArgs {
    topic: String,
    group: String,
    name: String,
}

impl Parse for ConsumerArgs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut topic = None;
        let mut group = None;
        let mut name = None;
        while !input.is_empty() {
            let ident: syn::Ident = input.parse()?;
            let _: Token![=] = input.parse()?;
            let value: LitStr = input.parse()?;
            match ident.to_string().as_str() {
                "topic" => topic = Some(value.value()),
                "group" => group = Some(value.value()),
                "name" => name = Some(value.value()),
                other => {
                    return Err(syn::Error::new(
                        ident.span(),
                        format!("unknown argument `{other}`"),
                    ))
                }
            }
            if input.peek(Token![,]) {
                let _: Token![,] = input.parse()?;
            }
        }
        Ok(ConsumerArgs {
            topic: topic.ok_or_else(|| {
                syn::Error::new(Span::call_site(), "missing `topic` argument")
            })?,
            group: group.ok_or_else(|| {
                syn::Error::new(Span::call_site(), "missing `group` argument")
            })?,
            name: name.ok_or_else(|| {
                syn::Error::new(Span::call_site(), "missing `name` argument")
            })?,
        })
    }
}

pub(crate) fn consumer_impl(args: TokenStream, input: ItemFn) -> syn::Result<TokenStream> {
    let args: ConsumerArgs = syn::parse2(args)?;
    let topic = &args.topic;
    let group = &args.group;
    let name = &args.name;

    // Consumers are invoked by async workers, so the annotated function must
    // be `async fn`. Reject non-async inputs with a clear compile error.
    if input.sig.asyncness.is_none() {
        return Err(syn::Error::new_spanned(
            &input.sig.fn_token,
            "#[consumer] can only be applied to `async fn`",
        ));
    }

    let fn_name = &input.sig.ident;
    let fn_vis = &input.vis;
    let fn_attrs: Vec<_> = input.attrs.iter().filter(|a| !a.path().is_ident("consumer")).collect();
    let fn_sig = &input.sig;
    let fn_block = &input.block;

    let resolver_mod = syn::Ident::new(
        &format!("__streaming_resolver_{fn_name}"),
        Span::call_site(),
    );
    let meta_macro = syn::Ident::new(
        &format!("__streaming_resolver_meta_{fn_name}"),
        Span::call_site(),
    );
    let method_ident = syn::Ident::new(name, Span::call_site());

    let streaming_crate = quote! { ::reinhardt_streaming };
    let inventory = get_inventory_crate();

    Ok(quote! {
        // Metadata module (consumed by streaming_routes! and #[routes])
        #[doc(hidden)]
        pub mod #resolver_mod {
            #[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
            #[allow(unused_macros)]
            macro_rules! #meta_macro {
                ($callback:ident, $app:ident) => {
                    $callback!($app, #method_ident, #name, #topic);
                };
            }
            #[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
            pub(crate) use #meta_macro;
        }
        #[doc(hidden)]
        pub use #resolver_mod::*;

        // Consumer function is kept as-is; consumer workers call it externally
        #(#fn_attrs)*
        #fn_vis #fn_sig {
            #fn_block
        }

        // Register handler metadata so `resolve_streaming_topic(name)` can
        // find the corresponding topic at runtime.
        #[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
        #inventory::submit! {
            #streaming_crate::StreamingHandlerMetadata {
                name: #name,
                topic: #topic,
                kind: #streaming_crate::StreamingHandlerKind::Consumer,
                group: ::core::option::Option::Some(#group),
                module_path: ::core::module_path!(),
            }
        }
    })
}

