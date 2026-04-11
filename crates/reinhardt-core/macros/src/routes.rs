//! HTTP method route macros

use crate::crate_paths::{
	get_async_trait_crate, get_reinhardt_core_crate, get_reinhardt_di_crate,
	get_reinhardt_http_crate, get_reinhardt_params_crate,
};
use crate::injectable_common::{InjectOptions, is_inject_attr, parse_inject_options};
use crate::path_macro;
use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::{
	Error, Expr, ExprLit, FnArg, ItemFn, Lit, LitStr, Meta, Pat, PatType, Result, Token, Type,
	parse::Parser, punctuated::Punctuated, spanned::Spanned,
};

/// Options for route macros
#[derive(Clone, Default)]
struct RouteOptions {
	/// Enable DI functionality with `use_inject = true`
	use_inject: bool,
	/// Route name for URL reversal
	name: Option<String>,
	/// Enable automatic validation with `pre_validate = true`
	///
	/// When enabled, extracted parameters implementing `reinhardt_core::validators::Validate`
	/// are automatically validated before the handler is called.
	/// Extractors used with this option must implement `Deref` to the inner type
	/// (e.g., `Json<T>` derefs to `T`), as validation is performed on the dereferenced value.
	/// Returns HTTP 400 with JSON error details on validation failure.
	pre_validate: bool,
}

/// Information about parameter extractors
#[derive(Clone)]
struct ExtractorInfo {
	pat: Box<Pat>,
	ty: Box<Type>,
	extractor_name: String,
}

/// Information about `#[inject]` parameters
#[derive(Clone)]
struct InjectInfo {
	pat: Box<Pat>,
	ty: Box<Type>,
	options: InjectOptions,
}

/// Validate a route path at compile time
fn validate_route_path(path: &str, span: Span) -> Result<()> {
	path_macro::parse_and_validate(path)
		.map(|_| ())
		.map_err(|e| Error::new(span, format!("Invalid route path: {}", e)))
}

/// Convert snake_case function name to PascalCase + View suffix
fn fn_name_to_view_type(fn_name: &str) -> String {
	let pascal_case: String = fn_name
		.split('_')
		.map(|word| {
			let mut chars = word.chars();
			match chars.next() {
				Some(first) => first.to_uppercase().chain(chars).collect::<String>(),
				None => String::new(),
			}
		})
		.collect();
	format!("{}View", pascal_case)
}

/// Detect whether parameters contain extractors
fn detect_extractors(inputs: &Punctuated<FnArg, Token![,]>) -> Vec<ExtractorInfo> {
	let mut extractors = Vec::new();

	for input in inputs {
		if let FnArg::Typed(pat_type) = input {
			// Skip parameters with #[inject] attribute
			if pat_type.attrs.iter().any(is_inject_attr) {
				continue;
			}

			if let Type::Path(type_path) = &*pat_type.ty
				&& let Some(segment) = type_path.path.segments.last()
			{
				let type_name = segment.ident.to_string();
				if matches!(
					type_name.as_str(),
					"Path"
						| "Json" | "Query" | "Header"
						| "Cookie" | "Form"
						| "Body" | "HeaderNamed"
						| "CookieNamed"
				) {
					extractors.push(ExtractorInfo {
						pat: pat_type.pat.clone(),
						ty: pat_type.ty.clone(),
						extractor_name: type_name,
					});
				}
			}
		}
	}

	extractors
}

/// Extract request body information from function parameters
///
/// Detects body-consuming extractors (Json<T>, Form<T>, Body<T>) and extracts:
/// - Type name T as string (e.g., "CreateUserRequest")
/// - Content-Type based on extractor type
///
/// Returns None if no body-consuming extractor is found.
fn extract_request_body_info(inputs: &Punctuated<FnArg, Token![,]>) -> Option<(String, String)> {
	for input in inputs {
		if let FnArg::Typed(pat_type) = input {
			// Skip parameters with #[inject] attribute
			if pat_type.attrs.iter().any(is_inject_attr) {
				continue;
			}

			if let Type::Path(type_path) = &*pat_type.ty
				&& let Some(segment) = type_path.path.segments.last()
			{
				let type_name = segment.ident.to_string();

				// Check for body-consuming extractors
				if matches!(type_name.as_str(), "Json" | "Form" | "Body") {
					// Extract generic argument T
					if let syn::PathArguments::AngleBracketed(args) = &segment.arguments
						&& let Some(syn::GenericArgument::Type(inner_type)) = args.args.first()
					{
						// Convert inner type to string
						let body_type_str = quote!(#inner_type).to_string();

						// Determine content type based on extractor
						let content_type = match type_name.as_str() {
							"Json" => "application/json",
							"Form" => "application/x-www-form-urlencoded",
							"Body" => "application/octet-stream",
							_ => "application/octet-stream",
						};

						return Some((body_type_str, content_type.to_string()));
					}
				}
			}
		}
	}

	None
}

/// Detect parameters with `#[inject]` attribute
fn detect_inject_params(inputs: &Punctuated<FnArg, Token![,]>) -> Vec<InjectInfo> {
	let mut inject_params = Vec::new();

	for input in inputs {
		if let FnArg::Typed(PatType { attrs, pat, ty, .. }) = input {
			let has_inject = attrs.iter().any(is_inject_attr);

			if has_inject {
				let options = parse_inject_options(attrs);
				inject_params.push(InjectInfo {
					pat: pat.clone(),
					ty: ty.clone(),
					options,
				});
			}
		}
	}

	inject_params
}

/// Validate duplication of body-consuming extractors
fn validate_extractors(extractors: &[ExtractorInfo]) -> Result<()> {
	let body_consuming_types = ["Json", "Form", "Body"];
	let body_extractors: Vec<_> = extractors
		.iter()
		.filter(|ext| body_consuming_types.contains(&ext.extractor_name.as_str()))
		.collect();

	if body_extractors.len() > 1 {
		let names: Vec<_> = body_extractors
			.iter()
			.map(|e| e.extractor_name.as_str())
			.collect();
		return Err(Error::new(
			Span::call_site(),
			format!(
				"Cannot use multiple body-consuming extractors: {}. Request body can only be read once.",
				names.join(", ")
			),
		));
	}

	Ok(())
}

/// Convert `Option<String>` to TokenStream for `Option<&'static str>` literal
fn option_to_lit(opt: &Option<String>) -> TokenStream {
	match opt {
		Some(s) => quote! { Some(#s) },
		None => quote! { None },
	}
}

/// Result of auth parameter detection for a handler.
struct AuthDetection {
	/// The detected protection level for the endpoint.
	protection: AuthProtectionKind,
	/// The stringified guard expression for OpenAPI `x-guard`, if any.
	guard_description: Option<String>,
}

/// Protection level detected from handler parameter types.
///
/// This mirrors `reinhardt_core::endpoint::AuthProtection` but lives in the
/// macro crate where we cannot take a dependency on `reinhardt-auth`.
enum AuthProtectionKind {
	Protected,
	Optional,
	Public,
	None,
}

/// Detect auth protection level from a list of type strings.
///
/// Each string is the stringified token stream of a parameter type.
/// The first matching rule wins (rules are checked in priority order):
///
/// - Contains `"Guard"` → `Protected`; also captures `guard_description`
/// - Contains `"AuthUser"` → `Protected`
/// - Contains both `"Option"` and `"AuthInfo"` → `Optional`
/// - Contains `"AuthInfo"` (alone) → `Protected`
/// - Contains `"Public"` → `Public`
/// - Otherwise → `None`
fn detect_auth_from_type_strings(type_strings: &[String]) -> AuthDetection {
	// Collect guard expression strings for guard_description
	let mut guard_desc: Option<String> = None;
	let mut found_protected = false;
	let mut found_optional = false;
	let mut found_public = false;

	for ty_str in type_strings {
		// Guard<...> or guard!(...) — highest priority
		if ty_str.contains("Guard") || ty_str.contains("guard") {
			found_protected = true;
			// Capture the guard description for OpenAPI x-guard metadata.
			// Use the full type string as the description.
			if guard_desc.is_none() {
				guard_desc = Some(ty_str.clone());
			}
			continue;
		}

		if ty_str.contains("AuthUser") {
			found_protected = true;
			continue;
		}

		// Option<AuthInfo<...>> → Optional
		if ty_str.contains("Option") && ty_str.contains("AuthInfo") {
			found_optional = true;
			continue;
		}

		// Bare AuthInfo<...> → Protected
		if ty_str.contains("AuthInfo") {
			found_protected = true;
			continue;
		}

		if ty_str.contains("Public") {
			found_public = true;
		}
	}

	// Priority: Protected > Optional > Public > None
	if found_protected {
		AuthDetection {
			protection: AuthProtectionKind::Protected,
			guard_description: guard_desc,
		}
	} else if found_optional {
		AuthDetection {
			protection: AuthProtectionKind::Optional,
			guard_description: None,
		}
	} else if found_public {
		AuthDetection {
			protection: AuthProtectionKind::Public,
			guard_description: None,
		}
	} else {
		AuthDetection {
			protection: AuthProtectionKind::None,
			guard_description: None,
		}
	}
}

/// Detect auth protection from extractor and inject parameter types.
///
/// Inspects all parameter types (both regular extractors and `#[inject]` params)
/// to determine the endpoint's auth protection level and guard description.
fn detect_auth_protection(
	extractors: &[ExtractorInfo],
	inject_params: &[InjectInfo],
) -> AuthDetection {
	let type_strings: Vec<String> = extractors
		.iter()
		.map(|e| {
			let ty = &e.ty;
			quote!(#ty).to_string()
		})
		.chain(inject_params.iter().map(|p| {
			let ty = &p.ty;
			quote!(#ty).to_string()
		}))
		.collect();
	detect_auth_from_type_strings(&type_strings)
}

/// Detect auth protection from raw function inputs (for simple routes without extractors/inject).
fn detect_auth_protection_from_inputs(
	inputs: &syn::punctuated::Punctuated<FnArg, Token![,]>,
) -> AuthDetection {
	let type_strings: Vec<String> = inputs
		.iter()
		.filter_map(|arg| {
			if let FnArg::Typed(pat_type) = arg {
				let ty = &pat_type.ty;
				Some(quote!(#ty).to_string())
			} else {
				None
			}
		})
		.collect();
	detect_auth_from_type_strings(&type_strings)
}

/// Convert `AuthDetection` into the `auth_protection` and `guard_description` token streams.
fn auth_detection_to_tokens(
	detection: &AuthDetection,
	core_crate: &TokenStream,
) -> (TokenStream, TokenStream) {
	let protection_ts = match detection.protection {
		AuthProtectionKind::Protected => {
			quote! { #core_crate::endpoint::AuthProtection::Protected }
		}
		AuthProtectionKind::Optional => quote! { #core_crate::endpoint::AuthProtection::Optional },
		AuthProtectionKind::Public => quote! { #core_crate::endpoint::AuthProtection::Public },
		AuthProtectionKind::None => quote! { #core_crate::endpoint::AuthProtection::None },
	};
	let guard_desc_ts = match &detection.guard_description {
		Some(s) => quote! { Some(#s) },
		None => quote! { None },
	};
	(protection_ts, guard_desc_ts)
}

/// Generate wrapper function with both extractors and inject params
fn generate_wrapper_with_both(
	original_fn: &ItemFn,
	extractors: &[ExtractorInfo],
	inject_params: &[InjectInfo],
	options: &RouteOptions,
) -> (TokenStream, TokenStream) {
	let di_crate = get_reinhardt_di_crate();
	let core_crate = get_reinhardt_core_crate();
	let params_crate = get_reinhardt_params_crate();

	let fn_name = &original_fn.sig.ident;
	let original_fn_name = quote::format_ident!("{}_original", fn_name);
	let fn_attrs: Vec<_> = original_fn
		.attrs
		.iter()
		.filter(|attr| !attr.path().is_ident("inject"))
		.collect();
	let output = &original_fn.sig.output;
	let fn_block = &original_fn.block;
	let asyncness = &original_fn.sig.asyncness;

	// Build original function parameters (without #[inject] attributes)
	let original_inputs: Vec<_> = original_fn
		.sig
		.inputs
		.iter()
		.map(|arg| {
			if let FnArg::Typed(pat_type) = arg {
				let mut pat_type = pat_type.clone();
				pat_type.attrs.retain(|attr| !is_inject_attr(attr));
				FnArg::Typed(pat_type)
			} else {
				arg.clone()
			}
		})
		.collect();

	// Generate DI context extraction
	let di_context_extraction = if !inject_params.is_empty() {
		quote! {
			let __shared_ctx = req.get_di_context::<::std::sync::Arc<#di_crate::InjectionContext>>()
				.ok_or_else(|| #core_crate::exception::Error::Internal(
					"DI context not set. Ensure the router is configured with .with_di_context()".to_string()
				))?;
			let __di_request = req.clone_for_di();
			let __di_ctx = ::std::sync::Arc::new((*__shared_ctx).fork_for_request(__di_request));
			let __resolve_ctx = #di_crate::resolve_context::ResolveContext {
				root: ::std::sync::Arc::clone(&__shared_ctx),
				current: ::std::sync::Arc::clone(&__di_ctx),
			};
		}
	} else {
		quote! {}
	};

	// Generate injection calls
	let injection_calls: Vec<_> = inject_params
		.iter()
		.map(|param| {
			let pat = &param.pat;
			let ty = &param.ty;
			let use_cache = param.options.use_cache;

			if use_cache {
				quote! {
					let #pat: #ty = #di_crate::Injected::<#ty>::resolve(&__di_ctx)
						.await
						.map_err(#core_crate::exception::Error::from)?
						.into_inner();
				}
			} else {
				quote! {
					let #pat: #ty = #di_crate::Injected::<#ty>::resolve_uncached(&__di_ctx)
						.await
						.map_err(#core_crate::exception::Error::from)?
						.into_inner();
				}
			}
		})
		.collect();

	// Build call arguments for inject params (shared between both paths)
	let inject_args: Vec<_> = inject_params.iter().map(|param| &param.pat).collect();

	// Generate extractor calls and validation differently based on pre_validate.
	// When pre_validate = true and a destructuring pattern like `Json(body)` is used,
	// extracting directly into the pattern would consume the wrapper, making it
	// impossible to validate via Deref on the original extractor type.
	// The 3-step approach (extract to temp -> validate temp -> destructure) avoids this.
	let (extractor_calls, validation_calls, destructure_calls, extractor_args): (
		Vec<_>,
		proc_macro2::TokenStream,
		proc_macro2::TokenStream,
		Vec<Box<Pat>>,
	) = if options.pre_validate {
		// Step 1: Extract into temporary variables
		let temp_names: Vec<syn::Ident> = extractors
			.iter()
			.enumerate()
			.map(|(i, _)| syn::Ident::new(&format!("__ext_{}", i), Span::call_site()))
			.collect();

		let calls: Vec<_> = extractors
			.iter()
			.zip(temp_names.iter())
			.map(|(ext, temp)| {
				let ty = &ext.ty;
				quote! {
					let #temp = <#ty as #params_crate::FromRequest>::from_request(&req, &ctx)
						.await
						.map_err(|e| #core_crate::exception::Error::Validation(
							format!("Parameter extraction failed: {:?}", e)
						))?;
				}
			})
			.collect();

		// Step 2: Validate using temp variables (Deref on the extractor type)
		let validate_calls: Vec<_> = temp_names
			.iter()
			.map(|temp| {
				quote! {
					#core_crate::validators::Validate::validate(&*#temp)
						.map_err(|e| #core_crate::exception::Error::Validation(
							::serde_json::to_string(&e).unwrap_or_else(|_| format!("{}", e))
						))?;
				}
			})
			.collect();

		// Step 3: Destructure temp variables into original patterns
		let destructure: Vec<_> = extractors
			.iter()
			.zip(temp_names.iter())
			.map(|(ext, temp)| {
				let pat = &ext.pat;
				quote! { let #pat = #temp; }
			})
			.collect();

		let args: Vec<Box<Pat>> = extractors.iter().map(|ext| ext.pat.clone()).collect();

		(
			calls,
			quote! { #(#validate_calls)* },
			quote! { #(#destructure)* },
			args,
		)
	} else {
		// Without pre_validate: extract directly into the original pattern
		let calls: Vec<_> = extractors
			.iter()
			.map(|ext| {
				let pat = &ext.pat;
				let ty = &ext.ty;
				quote! {
					let #pat = <#ty as #params_crate::FromRequest>::from_request(&req, &ctx)
						.await
						.map_err(|e| #core_crate::exception::Error::Validation(
							format!("Parameter extraction failed: {:?}", e)
						))?;
				}
			})
			.collect();

		let args: Vec<Box<Pat>> = extractors.iter().map(|ext| ext.pat.clone()).collect();

		(calls, quote! {}, quote! {}, args)
	};

	// Generate the handler body (injection + extraction + call)
	let handler_body = quote! {
		// Resolve injected dependencies
		#(#injection_calls)*

		// Extract request parameters
		#(#extractor_calls)*

		// Validate extracted parameters (when pre_validate = true)
		#validation_calls

		// Destructure into original patterns (when pre_validate = true)
		#destructure_calls

		// Call the original function
		#original_fn_name(#(#extractor_args,)* #(#inject_args),*).await
	};

	// Wrap handler body in RESOLVE_CTX.scope() when DI is active
	let scoped_handler_body = if !inject_params.is_empty() {
		quote! {
			#di_crate::resolve_context::RESOLVE_CTX.scope(__resolve_ctx, async {
				#handler_body
			}).await
		}
	} else {
		handler_body
	};

	// Generate code
	(
		quote! {
			// Original function (renamed, private)
			#(#fn_attrs)*
			#asyncness fn #original_fn_name(#(#original_inputs),*) #output {
				#fn_block
			}
		},
		quote! {
			// Build ParamContext for extractors
			let ctx = #params_crate::ParamContext::with_path_params(req.path_params.clone());

			// Extract DI context (if needed)
			#di_context_extraction

			// Execute handler within resolve context scope
			#scoped_handler_body
		},
	)
}

/// Generate View type and factory function
fn generate_view_type(
	input: &ItemFn,
	method: &str,
	path: &str,
	route_name: &str,
	extractors: &[ExtractorInfo],
	inject_params: &[InjectInfo],
	options: &RouteOptions,
) -> Result<TokenStream> {
	let reinhardt_crate = crate::crate_paths::get_reinhardt_crate();
	let core_crate = get_reinhardt_core_crate();
	let http_crate = get_reinhardt_http_crate();
	let async_trait_crate = get_async_trait_crate();

	let fn_name = &input.sig.ident;
	let fn_vis = &input.vis;
	let fn_attrs: Vec<_> = input
		.attrs
		.iter()
		.filter(|attr| !attr.path().is_ident("inject"))
		.collect();
	let output = &input.sig.output;
	let asyncness = &input.sig.asyncness;

	let view_type_name =
		syn::Ident::new(&fn_name_to_view_type(&fn_name.to_string()), fn_name.span());
	let method_ident = syn::Ident::new(method, Span::call_site());

	// Generate wrapper parts
	let (original_fn, wrapper_body) =
		generate_wrapper_with_both(input, extractors, inject_params, options);

	let route_doc = format!("Route: {} {}", method, path);

	// Generate inventory submission for endpoint metadata
	let metadata_name = if route_name.is_empty() {
		quote! { None }
	} else {
		quote! { Some(#route_name) }
	};

	// Extract request body information
	let (request_body_type, request_content_type) = extract_request_body_info(&input.sig.inputs)
		.map(|(ty, ct)| (quote!(Some(#ty)), quote!(Some(#ct))))
		.unwrap_or((quote!(None), quote!(None)));

	// Detect auth protection level from parameter types
	let auth_detection = detect_auth_protection(extractors, inject_params);
	let (auth_protection_ts, guard_description_ts) =
		auth_detection_to_tokens(&auth_detection, &core_crate);

	let inventory_crate = crate::crate_paths::get_inventory_crate();
	let metadata_submission = quote! {
		#inventory_crate::submit! {
			#[allow(non_upper_case_globals)]
			#core_crate::endpoint::EndpointMetadata {
				path: #path,
				method: #method,
				name: #metadata_name,
				function_name: stringify!(#fn_name),
				module_path: module_path!(),
				request_body_type: #request_body_type,
				request_content_type: #request_content_type,
				responses: &[],
				headers: &[],
				security: &[],
				auth_protection: #auth_protection_ts,
				guard_description: #guard_description_ts,
			}
		}
	};

	let url_resolver_tokens =
		generate_url_resolver_tokens(&options.name, &fn_name.to_string(), path, &reinhardt_crate);

	Ok(quote! {
		// Submit endpoint metadata to global inventory
		#metadata_submission

		#original_fn

		/// View type for route registration
		#[doc = #route_doc]
		#fn_vis struct #view_type_name;

		impl #core_crate::endpoint::EndpointInfo for #view_type_name {
			fn path() -> &'static str {
				#path
			}

			fn method() -> #reinhardt_crate::Method {
				#reinhardt_crate::Method::#method_ident
			}

			fn name() -> &'static str {
				#route_name
			}
		}

		#[#async_trait_crate::async_trait]
		impl #http_crate::Handler for #view_type_name {
			async fn handle(&self, req: #http_crate::Request) -> #http_crate::Result<#http_crate::Response> {
				#view_type_name::#fn_name(req).await
			}
		}

		impl #view_type_name {
			/// Handler function for this view
			#(#fn_attrs)*
			#fn_vis #asyncness fn #fn_name(req: #http_crate::Request) #output {
				#wrapper_body
			}
		}

		/// Factory function for endpoint registration
		///
		/// Returns the View type for use with `UnifiedRouter::endpoint()`
		#fn_vis fn #fn_name() -> #view_type_name {
			#view_type_name
		}

		#url_resolver_tokens
	})
}

/// Convert a snake_case route name to a PascalCase trait name with "Resolve" prefix.
///
/// `auth_login` → `ResolveAuthLogin`
/// `cluster_retrieve` → `ResolveClusterRetrieve`
fn to_resolver_trait_name(route_name: &str) -> String {
	let mut result = String::from("Resolve");
	for segment in route_name.split('_') {
		let mut chars = segment.chars();
		if let Some(first) = chars.next() {
			result.push(first.to_ascii_uppercase());
			result.extend(chars);
		}
	}
	result
}

/// Extract parameter names from a URL path pattern.
///
/// Handles both simple params `{id}` and typed params `{<int:id>}`.
/// Skips wildcard `{*}` patterns.
fn extract_url_params(path: &str) -> Vec<String> {
	let mut params = Vec::new();
	let mut chars = path.chars().peekable();
	while let Some(ch) = chars.next() {
		if ch == '{' {
			let content: String = chars.by_ref().take_while(|&c| c != '}').collect();
			if content == "*" {
				continue;
			}
			// Handle typed params: `<type:name>` → extract `name`
			let param_name = if content.starts_with('<') {
				content
					.split(':')
					.nth(1)
					.map(|s| s.trim_end_matches('>'))
					.unwrap_or(&content)
			} else {
				&content
			};
			params.push(param_name.to_string());
		}
	}
	params
}

/// Generate URL resolver extension trait and per-endpoint resolver module tokens.
///
/// Each endpoint gets a uniquely named `__url_resolver_<fn_name>` module to avoid
/// name collisions when multiple routes are declared in the same Rust module.
/// The `#[url_patterns]` macro references these modules by deriving the module name
/// from the last segment of the endpoint path.
///
/// Returns empty tokens if:
/// - No route name is set
/// - The path contains a wildcard
fn generate_url_resolver_tokens(
	route_name: &Option<String>,
	fn_name: &str,
	path: &str,
	reinhardt_crate: &TokenStream,
) -> TokenStream {
	let Some(name) = route_name.as_ref() else {
		return quote! {};
	};

	// Skip wildcard routes
	if path.contains('*') {
		return quote! {};
	}

	let trait_name_str = to_resolver_trait_name(name);
	let trait_ident = syn::Ident::new(&trait_name_str, Span::call_site());
	let method_ident = syn::Ident::new(name, Span::call_site());
	let resolver_mod_ident =
		syn::Ident::new(&format!("__url_resolver_{fn_name}"), Span::call_site());
	let params = extract_url_params(path);
	let doc_str = format!("Resolve URL for route `{}` (pattern: `{}`).", name, path);

	// Gate with `feature = "url-resolver"` only (not `native`).
	// The `UrlResolver` trait itself is not `native`-gated, so extension traits
	// that only reference `UrlResolver` don't need the `native` gate.
	// The `native` gate belongs on `ResolvedUrls` (in `routes_registration.rs`)
	// because it depends on `ServerRouter` which is `native`-only.
	if params.is_empty() {
		quote! {
			#[cfg(feature = "url-resolver")]
			#[doc = #doc_str]
			pub trait #trait_ident: #reinhardt_crate::UrlResolver {
				#[doc = #doc_str]
				fn #method_ident(&self) -> String {
					self.resolve_url(#name, &[])
				}
			}
			#[cfg(feature = "url-resolver")]
			impl<T: #reinhardt_crate::UrlResolver> #trait_ident for T {}

			#[cfg(feature = "url-resolver")]
			#[doc(hidden)]
			pub mod #resolver_mod_ident {
				pub use super::#trait_ident;
			}
		}
	} else {
		let param_idents: Vec<syn::Ident> = params
			.iter()
			.map(|p| syn::Ident::new(p, Span::call_site()))
			.collect();
		let param_strs: Vec<&str> = params.iter().map(|s| s.as_str()).collect();

		quote! {
			#[cfg(feature = "url-resolver")]
			#[doc = #doc_str]
			pub trait #trait_ident: #reinhardt_crate::UrlResolver {
				#[doc = #doc_str]
				fn #method_ident(&self, #(#param_idents: &str),*) -> String {
					self.resolve_url(#name, &[#((#param_strs, #param_idents)),*])
				}
			}
			#[cfg(feature = "url-resolver")]
			impl<T: #reinhardt_crate::UrlResolver> #trait_ident for T {}

			#[cfg(feature = "url-resolver")]
			#[doc(hidden)]
			pub mod #resolver_mod_ident {
				pub use super::#trait_ident;
			}
		}
	}
}

fn route_impl(method: &str, args: TokenStream, input: ItemFn) -> Result<TokenStream> {
	let reinhardt_crate = crate::crate_paths::get_reinhardt_crate();
	let core_crate = get_reinhardt_core_crate();
	let http_crate = get_reinhardt_http_crate();
	let async_trait_crate = get_async_trait_crate();

	let mut path: Option<(String, Span)> = None;
	let mut options = RouteOptions::default();

	// Handle the common case: #[get("/users/{id}")]
	// Try to parse as a single string literal first
	if let Ok(lit) = syn::parse2::<LitStr>(args.clone()) {
		let path_str = lit.value();
		validate_route_path(&path_str, lit.span())?;
		path = Some((path_str, lit.span()));
	} else {
		// Parse path and options: #[get("/path", use_inject = true)]
		let parser = Punctuated::<Expr, Token![,]>::parse_terminated;
		if let Ok(exprs) = parser.parse2(args.clone()) {
			for (i, expr) in exprs.iter().enumerate() {
				match expr {
					// First argument: path string literal
					Expr::Lit(ExprLit {
						lit: Lit::Str(lit), ..
					}) if i == 0 => {
						let path_str = lit.value();
						validate_route_path(&path_str, lit.span())?;
						path = Some((path_str, lit.span()));
					}
					// use_inject = true/false or name = "xxx"
					Expr::Assign(assign) => {
						if let Expr::Path(path_expr) = &*assign.left {
							if path_expr.path.is_ident("use_inject") {
								if let Expr::Lit(ExprLit {
									lit: Lit::Bool(bool_lit),
									..
								}) = &*assign.right
								{
									options.use_inject = bool_lit.value;
								} else {
									return Err(Error::new_spanned(
										&assign.right,
										"use_inject must be a boolean (true or false)",
									));
								}
							} else if path_expr.path.is_ident("pre_validate") {
								if let Expr::Lit(ExprLit {
									lit: Lit::Bool(bool_lit),
									..
								}) = &*assign.right
								{
									options.pre_validate = bool_lit.value;
								} else {
									return Err(Error::new_spanned(
										&assign.right,
										"pre_validate must be a boolean (true or false)",
									));
								}
							} else if path_expr.path.is_ident("name") {
								if let Expr::Lit(ExprLit {
									lit: Lit::Str(str_lit),
									..
								}) = &*assign.right
								{
									options.name = Some(str_lit.value());
								} else {
									return Err(Error::new_spanned(
										&assign.right,
										"name must be a string literal",
									));
								}
							} else {
								return Err(Error::new_spanned(
									&path_expr.path,
									format!(
										"unknown route option `{}`, expected `use_inject`, `name`, or `pre_validate`",
										path_expr.path.get_ident().map_or_else(
											|| "unknown".to_string(),
											|id| id.to_string()
										)
									),
								));
							}
						}
					}
					_ => {
						return Err(Error::new_spanned(
							expr,
							"unexpected argument in route macro, expected a path string or key = value option",
						));
					}
				}
			}
		} else {
			// Fallback: try parsing as Meta for backwards compatibility
			let meta_list = Punctuated::<Meta, Token![,]>::parse_terminated.parse2(args)?;

			for meta in meta_list {
				match meta {
					Meta::Path(p) => {
						if let Some(ident) = p.get_ident() {
							let path_str = ident.to_string();
							validate_route_path(&path_str, p.span())?;
							path = Some((path_str, p.span()));
						}
					}
					Meta::NameValue(nv) if nv.path.is_ident("path") => {
						if let Expr::Lit(ExprLit {
							lit: Lit::Str(lit), ..
						}) = &nv.value
						{
							let path_str = lit.value();
							validate_route_path(&path_str, lit.span())?;
							path = Some((path_str, lit.span()));
						}
					}
					_ => {
						return Err(Error::new_spanned(
							&meta,
							"unexpected meta argument in route macro",
						));
					}
				}
			}
		}
	}

	// Detect extractors
	let extractors = detect_extractors(&input.sig.inputs);

	// Detect inject params (always detect for error checking)
	let all_inject_params = detect_inject_params(&input.sig.inputs);

	// Auto-enable injection when #[inject] attributes are present
	if !options.use_inject && !all_inject_params.is_empty() {
		options.use_inject = true;
	}

	// Use inject params only when use_inject = true
	let inject_params = if options.use_inject {
		all_inject_params
	} else {
		Vec::new()
	};

	// Validate extractors
	if !extractors.is_empty() {
		validate_extractors(&extractors)?;
	}

	// If we have extractors or inject params, generate View type
	if !extractors.is_empty() || !inject_params.is_empty() {
		let path_str = path
			.as_ref()
			.map(|(p, _)| p.clone())
			.unwrap_or_else(|| "/".to_string());
		let route_name = options
			.name
			.clone()
			.unwrap_or_else(|| input.sig.ident.to_string());

		return generate_view_type(
			&input,
			method,
			&path_str,
			&route_name,
			&extractors,
			&inject_params,
			&options,
		);
	}

	// Simple case: no extractors, no inject - generate View type with EndpointInfo + Handler
	let fn_name = &input.sig.ident;
	let fn_block = &input.block;
	let fn_inputs = &input.sig.inputs;
	let fn_output = &input.sig.output;
	let fn_vis = &input.vis;
	let fn_attrs = &input.attrs;
	let asyncness = &input.sig.asyncness;
	let generics = &input.sig.generics;
	let where_clause = &input.sig.generics.where_clause;

	let path_str = path
		.as_ref()
		.map(|(p, _)| p.clone())
		.unwrap_or_else(|| "/".to_string());
	let route_name = options.name.clone().unwrap_or_else(|| fn_name.to_string());
	let view_type_name =
		syn::Ident::new(&fn_name_to_view_type(&fn_name.to_string()), fn_name.span());
	let method_ident = syn::Ident::new(method, Span::call_site());
	let original_fn_name = quote::format_ident!("{}_original", fn_name);

	let route_doc = format!("Route: {} {}", method, path_str);

	// Determine if the original function takes a Request parameter
	let has_request_param = !fn_inputs.is_empty();

	// Wrapper function signature and body based on whether original takes request
	let (wrapper_sig, wrapper_body) = if has_request_param {
		(
			quote! { req: #http_crate::Request },
			quote! { #original_fn_name(req).await },
		)
	} else {
		(
			quote! { _req: #http_crate::Request },
			quote! { #original_fn_name().await },
		)
	};

	// Generate inventory submission for endpoint metadata
	let metadata_name = option_to_lit(&options.name);

	// Extract request body information
	let (request_body_type, request_content_type) = extract_request_body_info(&input.sig.inputs)
		.map(|(ty, ct)| (quote!(Some(#ty)), quote!(Some(#ct))))
		.unwrap_or((quote!(None), quote!(None)));

	// Detect auth protection level from all function parameter types
	let auth_detection = detect_auth_protection_from_inputs(&input.sig.inputs);
	let (auth_protection_ts, guard_description_ts) =
		auth_detection_to_tokens(&auth_detection, &core_crate);

	let inventory_crate = crate::crate_paths::get_inventory_crate();
	let metadata_submission = quote! {
		#inventory_crate::submit! {
			#[allow(non_upper_case_globals)]
			#core_crate::endpoint::EndpointMetadata {
				path: #path_str,
				method: #method,
				name: #metadata_name,
				function_name: stringify!(#fn_name),
				module_path: module_path!(),
				request_body_type: #request_body_type,
				request_content_type: #request_content_type,
				responses: &[],
				headers: &[],
				security: &[],
				auth_protection: #auth_protection_ts,
				guard_description: #guard_description_ts,
			}
		}
	};

	let url_resolver_tokens =
		generate_url_resolver_tokens(&options.name, &fn_name.to_string(), &path_str, &reinhardt_crate);

	Ok(quote! {
		// Submit endpoint metadata to global inventory
		#metadata_submission

		// Original function (renamed, private)
		#(#fn_attrs)*
		#asyncness fn #original_fn_name #generics (#fn_inputs) #fn_output #where_clause {
			#fn_block
		}

		/// View type for route registration
		#[doc = #route_doc]
		#fn_vis struct #view_type_name;

		impl #core_crate::endpoint::EndpointInfo for #view_type_name {
			fn path() -> &'static str {
				#path_str
			}

			fn method() -> #reinhardt_crate::Method {
				#reinhardt_crate::Method::#method_ident
			}

			fn name() -> &'static str {
				#route_name
			}
		}

		#[#async_trait_crate::async_trait]
		impl #http_crate::Handler for #view_type_name {
			async fn handle(&self, req: #http_crate::Request) -> #http_crate::Result<#http_crate::Response> {
				#view_type_name::#fn_name(req).await
			}
		}

		impl #view_type_name {
			/// Handler function for this view
			#(#fn_attrs)*
			#fn_vis #asyncness fn #fn_name(#wrapper_sig) #fn_output {
				#wrapper_body
			}
		}

		/// Factory function for endpoint registration
		///
		/// Returns the View type for use with `UnifiedRouter::endpoint()`
		#fn_vis fn #fn_name() -> #view_type_name {
			#view_type_name
		}

		#url_resolver_tokens
	})
}

/// Implementation of GET route macro
pub(crate) fn get_impl(args: TokenStream, input: ItemFn) -> Result<TokenStream> {
	route_impl("GET", args, input)
}

/// Implementation of POST route macro
pub(crate) fn post_impl(args: TokenStream, input: ItemFn) -> Result<TokenStream> {
	route_impl("POST", args, input)
}

/// Implementation of PUT route macro
pub(crate) fn put_impl(args: TokenStream, input: ItemFn) -> Result<TokenStream> {
	route_impl("PUT", args, input)
}

/// Implementation of PATCH route macro
pub(crate) fn patch_impl(args: TokenStream, input: ItemFn) -> Result<TokenStream> {
	route_impl("PATCH", args, input)
}

/// Implementation of DELETE route macro
pub(crate) fn delete_impl(args: TokenStream, input: ItemFn) -> Result<TokenStream> {
	route_impl("DELETE", args, input)
}

#[cfg(test)]
mod url_resolver_tests {
	use super::*;

	#[test]
	fn route_name_to_trait_name() {
		assert_eq!(to_resolver_trait_name("auth_login"), "ResolveAuthLogin");
		assert_eq!(
			to_resolver_trait_name("cluster_retrieve"),
			"ResolveClusterRetrieve"
		);
		assert_eq!(to_resolver_trait_name("home"), "ResolveHome");
		assert_eq!(
			to_resolver_trait_name("deployment_logs"),
			"ResolveDeploymentLogs"
		);
	}

	#[test]
	fn extract_path_params_none() {
		assert_eq!(extract_url_params("/login/"), Vec::<String>::new());
	}

	#[test]
	fn extract_path_params_single() {
		assert_eq!(extract_url_params("/{id}/"), vec!["id"]);
	}

	#[test]
	fn extract_path_params_multiple() {
		assert_eq!(
			extract_url_params("/{user_id}/posts/{post_id}/"),
			vec!["user_id", "post_id"]
		);
	}

	#[test]
	fn extract_path_params_with_type_specifier() {
		assert_eq!(extract_url_params("/{<int:id>}/"), vec!["id"]);
		assert_eq!(extract_url_params("/{<uuid:item_id>}/"), vec!["item_id"]);
	}

	#[test]
	fn extract_path_params_wildcard_skipped() {
		assert_eq!(extract_url_params("/static/{*}"), Vec::<String>::new());
	}
}
