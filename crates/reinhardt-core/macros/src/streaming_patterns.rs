use proc_macro2::{Span, TokenStream, TokenTree};
use quote::quote;
use syn::{ItemFn, parse2};

/// Parse the first argument to `#[streaming_patterns(InstalledApp::Orders)]`.
///
/// Returns the app label as a lowercase `Ident` (e.g., `orders` from `InstalledApp::Orders`).
fn parse_app_label(args: TokenStream) -> syn::Result<syn::Ident> {
    // Accept either a path (`InstalledApp::Orders`) or a bare ident (`Orders`).
    // In both cases, take the last segment and lowercase it.
    let path: syn::Path = syn::parse2(args)?;
    let last = path
        .segments
        .last()
        .ok_or_else(|| syn::Error::new(Span::call_site(), "expected app path like InstalledApp::Orders"))?;
    let label_str = last.ident.to_string().to_lowercase();
    Ok(syn::Ident::new(&label_str, last.ident.span()))
}

/// Flatten a function body into a flat token list for scanning.
fn flatten_body(func: &ItemFn) -> Vec<TokenTree> {
    fn recurse(tokens: impl IntoIterator<Item = TokenTree>) -> Vec<TokenTree> {
        let mut result = Vec::new();
        for tt in tokens {
            match &tt {
                TokenTree::Group(group) => {
                    result.push(tt.clone());
                    result.extend(recurse(group.stream()));
                }
                _ => result.push(tt),
            }
        }
        result
    }
    func.block
        .stmts
        .iter()
        .flat_map(|stmt| {
            let tokens: TokenStream = quote! { #stmt };
            recurse(tokens)
        })
        .collect()
}

/// Scan flattened tokens for `streaming_routes![ident, ...]` and extract handler idents.
///
/// Pattern: Ident("streaming_routes") Punct('!') Group(bracket or paren)
fn extract_streaming_handler_idents(tokens: &[TokenTree]) -> Vec<syn::Ident> {
    let mut idents = Vec::new();
    let mut i = 0;
    while i < tokens.len() {
        if i + 2 < tokens.len()
            && matches!(&tokens[i], TokenTree::Ident(id) if id == "streaming_routes")
            && matches!(&tokens[i + 1], TokenTree::Punct(p) if p.as_char() == '!')
            && matches!(&tokens[i + 2], TokenTree::Group(g)
                if g.delimiter() == proc_macro2::Delimiter::Bracket
                    || g.delimiter() == proc_macro2::Delimiter::Parenthesis)
        {
            if let TokenTree::Group(group) = &tokens[i + 2] {
                let inner: Vec<TokenTree> = group.stream().into_iter().collect();
                for tt in &inner {
                    if let TokenTree::Ident(id) = tt {
                        idents.push(id.clone());
                    }
                }
            }
            i += 3;
            continue;
        }
        i += 1;
    }
    idents
}

pub(crate) fn streaming_patterns_impl(
    args: TokenStream,
    input: TokenStream,
) -> syn::Result<TokenStream> {
    let func: ItemFn = parse2(input.clone())?;
    let app_label = parse_app_label(args.clone())?;

    // Pascal-case app label for struct name: "orders" → "Orders"
    let app_label_pascal = {
        let s = app_label.to_string();
        let mut c = s.chars();
        let upper = match c.next() {
            None => String::new(),
            Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        };
        syn::Ident::new(&upper, app_label.span())
    };

    let urls_struct_name = syn::Ident::new(
        &format!("{app_label_pascal}StreamingUrls"),
        app_label.span(),
    );

    let body_tokens = flatten_body(&func);
    let handler_idents = extract_streaming_handler_idents(&body_tokens);

    if handler_idents.is_empty() {
        return Err(syn::Error::new(
            Span::call_site(),
            "no streaming handlers found — ensure the function body contains `streaming_routes![handler1, handler2]`",
        ));
    }

    // For each handler `create_order`:
    // - resolver module: `__streaming_resolver_create_order`
    // - meta macro:      `__streaming_resolver_meta_create_order`
    let resolver_mods: Vec<syn::Ident> = handler_idents
        .iter()
        .map(|id| syn::Ident::new(&format!("__streaming_resolver_{id}"), id.span()))
        .collect();

    let meta_macros: Vec<syn::Ident> = handler_idents
        .iter()
        .map(|id| syn::Ident::new(&format!("__streaming_resolver_meta_{id}"), id.span()))
        .collect();

    // Callback macro name for this app
    let gen_method_macro = syn::Ident::new(
        &format!("__gen_{app_label}_streaming_method"),
        Span::call_site(),
    );

    Ok(quote! {
        // Preserve the original function
        #func

        // ── streaming_resolvers module ──────────────────────────────────────────
        // Contains __for_each_streaming_resolver! and re-exports per-handler meta macros.
        #[doc(hidden)]
        pub mod streaming_resolvers {
            // Re-export each handler's resolver metadata macros into this module
            #(
                #[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
                pub use super::#resolver_mods::*;
            )*

            // Dispatch macro: calls `$base :: __streaming_resolver_meta_<fn>!($callback, $app)`
            // for every handler registered in this app's streaming_routes.
            #[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
            macro_rules! __for_each_streaming_resolver {
                ($callback:ident, $app:ident, $($base:tt)+) => {
                    #(
                        $($base)+ :: #meta_macros ! ($callback, $app);
                    )*
                };
            }
            #[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
            pub(crate) use __for_each_streaming_resolver;
        }

        // ── Per-app streaming URL struct ────────────────────────────────────────
        /// Per-app streaming handler topic resolver for the `#app_label_pascal` app.
        ///
        /// Access via `ResolvedUrls::streaming()::#app_label()`.
        #[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
        pub struct #urls_struct_name<'a> {
            _marker: ::core::marker::PhantomData<&'a ()>,
        }

        // Callback macro: invoked once per handler to add a method to the struct above.
        // Signature matches __streaming_resolver_meta_<fn>! output:
        //   ($app_label, $method_ident, $name_literal, $topic_literal)
        #[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
        #[allow(unused_macros)]
        macro_rules! #gen_method_macro {
            ($app_label_unused:ident, $method:ident, $name:literal, $topic:literal) => {
                impl #urls_struct_name<'_> {
                    /// Returns the Kafka topic name for this streaming handler.
                    pub fn $method(&self) -> &'static str {
                        $topic
                    }
                }
            };
        }

        // Expand __for_each_streaming_resolver! to populate struct methods via callback.
        #[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
        streaming_resolvers::__for_each_streaming_resolver!(
            #gen_method_macro, #app_label,
            streaming_resolvers
        );

        // ── Add per-app accessor to StreamingRef ────────────────────────────────
        // StreamingRef is generated by #[routes] inside `pub mod __url_resolver_support`.
        // This inherent impl requires #[routes] to be present in the same crate.
        #[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
        impl<'a> crate::__url_resolver_support::StreamingRef<'a> {
            pub fn #app_label(&self) -> #urls_struct_name<'a> {
                #urls_struct_name { _marker: ::core::marker::PhantomData }
            }
        }
    })
}
