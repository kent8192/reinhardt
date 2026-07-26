//! HTTP method route macros

use crate::crate_paths::{
	get_async_trait_crate, get_reinhardt_core_crate, get_reinhardt_di_crate,
	get_reinhardt_http_crate, get_reinhardt_params_crate,
};
use crate::injectable_common::{
	InjectOptions, generate_inject_resolver_expr, is_inject_attr, parse_inject_options,
};
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
	ty: Box<Type>,
	// Allow dead_code: production validation derives body classification from the extractor type;
	// the name remains available for focused extractor-detection regression tests.
	#[allow(dead_code)]
	extractor_name: String,
}

/// Information about `#[inject]` parameters
#[derive(Clone)]
pub(crate) struct InjectInfo {
	pub(crate) pat: Box<Pat>,
	pub(crate) ty: Box<Type>,
	pub(crate) options: InjectOptions,
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
			if is_raw_request_parameter(pat_type) {
				continue;
			}
			// Skip parameters with #[inject] attribute
			if pat_type.attrs.iter().any(is_inject_attr) {
				continue;
			}

			if let Type::Path(type_path) = &*pat_type.ty
				&& let Some(segment) = type_path.path.segments.last()
			{
				let type_name = segment.ident.to_string();
				if is_supported_extractor_type_name(&type_name) {
					extractors.push(ExtractorInfo {
						ty: pat_type.ty.clone(),
						extractor_name: type_name,
					});
				}
			}
		}
	}

	extractors
}

fn is_supported_extractor_type_name(type_name: &str) -> bool {
	matches!(
		type_name,
		"Path"
			| "Json" | "Query"
			| "Header"
			| "HeaderStruct"
			| "Cookie"
			| "Form" | "Body"
			| "Multipart"
			| "HeaderNamed"
			| "CookieNamed"
			| "CookieStruct"
			| "SessionValue"
			| "OptionalSessionValue"
			| "SessionValueNamed"
			| "OptionalSessionValueNamed"
			| "PathStruct"
			| "Validated"
	)
}

/// Check whether a type is a raw HTTP `Request`.
fn is_request_type(ty: &Type) -> bool {
	let Type::Path(type_path) = ty else {
		return false;
	};

	type_path
		.path
		.segments
		.last()
		.is_some_and(|segment| segment.ident == "Request" && segment.arguments.is_none())
}

fn is_raw_request_parameter(pat_type: &syn::PatType) -> bool {
	if pat_type.attrs.iter().any(is_inject_attr) {
		return false;
	}

	if is_request_type(&pat_type.ty) {
		return true;
	}
	let Type::Path(type_path) = &*pat_type.ty else {
		return true;
	};
	let Some(segment) = type_path.path.segments.last() else {
		return true;
	};
	!is_supported_extractor_type_name(&segment.ident.to_string())
}

fn raw_request_parameter_ident(inputs: &Punctuated<FnArg, Token![,]>) -> Option<syn::Ident> {
	inputs.iter().find_map(|input| {
		let FnArg::Typed(pat_type) = input else {
			return None;
		};
		if !is_raw_request_parameter(pat_type) {
			return None;
		}
		let Pat::Ident(pat_ident) = &*pat_type.pat else {
			return None;
		};
		Some(pat_ident.ident.clone())
	})
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

			if let Some(body_info) = extract_body_info_from_type(&pat_type.ty) {
				return Some(body_info);
			}
		}
	}

	None
}

/// Derive body metadata from a body-consuming extractor type.
///
/// `Validated<T>` delegates extraction to `T`, so it preserves the body schema
/// and content type of wrapped `Json<T>`, `Form<T>`, or `Body<T>` extractors.
fn extract_body_info_from_type(ty: &Type) -> Option<(String, String)> {
	let Type::Path(type_path) = ty else {
		return None;
	};
	let segment = type_path.path.segments.last()?;
	if segment.ident == "Multipart" && matches!(segment.arguments, syn::PathArguments::None) {
		return Some(("Multipart".to_string(), "multipart/form-data".to_string()));
	}
	let syn::PathArguments::AngleBracketed(args) = &segment.arguments else {
		return None;
	};
	let inner_type = args.args.iter().find_map(|argument| match argument {
		syn::GenericArgument::Type(ty) => Some(ty),
		_ => None,
	})?;

	if segment.ident == "Validated" {
		return extract_body_info_from_type(inner_type);
	}

	let content_type = match segment.ident.to_string().as_str() {
		"Json" => "application/json",
		"Form" => "application/x-www-form-urlencoded",
		"Body" => "application/octet-stream",
		_ => return None,
	};

	Some((quote!(#inner_type).to_string(), content_type.to_string()))
}

fn body_consuming_extractor_name(ty: &Type) -> Option<String> {
	let Type::Path(type_path) = ty else {
		return None;
	};
	let segment = type_path.path.segments.last()?;
	if segment.ident == "Validated" {
		let syn::PathArguments::AngleBracketed(args) = &segment.arguments else {
			return None;
		};
		let inner = args.args.iter().find_map(|argument| match argument {
			syn::GenericArgument::Type(ty) => Some(ty),
			_ => None,
		})?;
		return body_consuming_extractor_name(inner);
	}
	matches!(
		segment.ident.to_string().as_str(),
		"Json" | "Form" | "Body" | "Multipart"
	)
	.then(|| segment.ident.to_string())
}

/// Detect parameters with `#[inject]` attribute
pub(crate) fn detect_inject_params(inputs: &Punctuated<FnArg, Token![,]>) -> Vec<InjectInfo> {
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
	let body_extractors: Vec<_> = extractors
		.iter()
		.filter_map(|ext| body_consuming_extractor_name(&ext.ty))
		.collect();

	if body_extractors.len() > 1 {
		let names: Vec<_> = body_extractors.iter().map(String::as_str).collect();
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
/// - Contains `"CurrentUser"` → `Protected`
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

		if ty_str.contains("CurrentUser") {
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
	let request_binding = syn::Ident::new("__reinhardt_request", Span::mixed_site());
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
			let __shared_ctx = #request_binding.get_di_context::<::std::sync::Arc<#di_crate::InjectionContext>>()
				.ok_or_else(|| #core_crate::exception::Error::Internal(
					"DI context not set. Ensure the router is configured with .with_di_context()".to_string()
				))?;
			let __di_request = #request_binding.clone_for_di();
			let __di_ctx = ::std::sync::Arc::new((*__shared_ctx).fork_for_request(__di_request));
			let __resolve_ctx = #di_crate::resolve_context::ResolveContext {
				root: ::std::sync::Arc::clone(&__shared_ctx),
				current: ::std::sync::Arc::clone(&__di_ctx),
			};
		}
	} else {
		quote! {}
	};

	// Generate injection calls. Runtime trait dispatch resolves `InjectableType`
	// wrappers from the registry and falls back to normal `Injectable` values.
	let inject_temps: Vec<syn::Ident> = inject_params
		.iter()
		.enumerate()
		.map(|(index, _)| {
			syn::Ident::new(&format!("__reinhardt_inject_{index}"), Span::mixed_site())
		})
		.collect();
	let injection_calls: Vec<_> = inject_params
		.iter()
		.zip(inject_temps.iter())
		.map(|(param, temp)| {
			let ty = &param.ty;
			let use_cache = param.options.use_cache;
			let resolve_expr =
				generate_inject_resolver_expr(&di_crate, ty, quote! { &__di_ctx }, use_cache);

			quote! {
				let #temp: #ty = #resolve_expr
					.map_err(#core_crate::exception::Error::from)?;
			}
		})
		.collect();

	// Extract into hygienic temporaries so handler parameter patterns are evaluated
	// only by the renamed original function. This also permits mutable and
	// destructuring extractor patterns without interpolating patterns as expressions.
	let extractor_temps: Vec<syn::Ident> = extractors
		.iter()
		.enumerate()
		.map(|(index, _)| {
			syn::Ident::new(
				&format!("__reinhardt_extractor_{index}"),
				Span::mixed_site(),
			)
		})
		.collect();
	let (extractor_calls, validation_calls, extractor_args): (
		Vec<_>,
		proc_macro2::TokenStream,
		Vec<syn::Ident>,
	) = if options.pre_validate {
		let calls: Vec<_> = extractors
			.iter()
			.zip(extractor_temps.iter())
			.map(|(ext, temp)| {
				let ty = &ext.ty;
				// Route `ParamError` through `From<ParamError> for Error` so
				// variant-specific status mappings (e.g. `Authentication` -> 401)
				// reach the response, instead of being flattened into 400 via
				// `Error::Validation`. See #4446.
				quote! {
					let #temp = <#ty as #params_crate::FromRequest>::from_request(
						&#request_binding,
						&ctx,
					)
						.await
						.map_err(#core_crate::exception::Error::from)?;
				}
			})
			.collect();

		// Step 2: Validate using temp variables (Deref on the extractor type)
		let validate_calls: Vec<_> = extractors
			.iter()
			.zip(extractor_temps.iter())
			.filter(|(ext, _)| ext.extractor_name != "Validated")
			.map(|(_, temp)| {
				quote! {
					#core_crate::validators::Validate::validate(&*#temp)
						.map_err(|e| #core_crate::exception::Error::Validation(
							::serde_json::to_string(&e).unwrap_or_else(|_| format!("{}", e))
						))?;
				}
			})
			.collect();

		(calls, quote! { #(#validate_calls)* }, extractor_temps)
	} else {
		// Without pre_validate: extract directly into the hygienic temporary.
		// Route ParamError through `From<ParamError> for Error` so variant-specific
		// status mappings (e.g. `Authentication` -> 401) reach the response. #4446
		let calls: Vec<_> = extractors
			.iter()
			.zip(extractor_temps.iter())
			.map(|(ext, temp)| {
				let ty = &ext.ty;
				quote! {
					let #temp = <#ty as #params_crate::FromRequest>::from_request(
						&#request_binding,
						&ctx,
					)
						.await
						.map_err(#core_crate::exception::Error::from)?;
				}
			})
			.collect();

		(calls, quote! {}, extractor_temps)
	};

	// A raw Request is forwarded without extraction and must retain its position
	// relative to typed extractors and injected parameters.
	let has_request_param = original_fn
		.sig
		.inputs
		.iter()
		.any(|arg| matches!(arg, FnArg::Typed(pat_type) if is_raw_request_parameter(pat_type)));
	let ordered_call_args = if has_request_param {
		let mut inject_args = inject_temps.iter();
		let mut extractor_args = extractor_args.iter();
		Some(
			original_fn
				.sig
				.inputs
				.iter()
				.filter_map(|arg| {
					let FnArg::Typed(pat_type) = arg else {
						return None;
					};
					if pat_type.attrs.iter().any(is_inject_attr) {
						let temp = inject_args
							.next()
							.expect("each injected argument must have a generated binding");
						Some(quote! { #temp })
					} else if is_raw_request_parameter(pat_type) {
						Some(quote! { #request_binding })
					} else {
						let pat = extractor_args
							.next()
							.expect("each route argument must have extractor metadata");
						Some(quote! { #pat })
					}
				})
				.collect::<Vec<_>>(),
		)
	} else {
		None
	};

	// Preserve the established extractor-then-injection order when no raw
	// Request is present.
	let inject_args = &inject_temps;
	let handler_call = if let Some(call_args) = ordered_call_args {
		quote! { #original_fn_name(#(#call_args),*).await }
	} else {
		quote! { #original_fn_name(#(#extractor_args,)* #(#inject_args),*).await }
	};

	// Generate the handler body (injection + extraction + call)
	let handler_body = quote! {
		// Resolve injected dependencies
		#(#injection_calls)*

		// Extract request parameters
		#(#extractor_calls)*

		// Validate extracted parameters (when pre_validate = true)
		#validation_calls

		// Call the original function
		#handler_call
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
		let ctx = #params_crate::ParamContext::with_path_params(#request_binding.path_params.clone());

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
	let request_binding = syn::Ident::new("__reinhardt_request", Span::mixed_site());
	let wrapper_request_param = raw_request_parameter_ident(&input.sig.inputs);
	let wrapper_request_binding = wrapper_request_param.as_ref().unwrap_or(&request_binding);
	let request_binding_initialization = wrapper_request_param.as_ref().map(|wrapper_param| {
		quote! {
			let #request_binding = #wrapper_param;
		}
	});
	let method_ident = syn::Ident::new(method, Span::call_site());

	// Generate wrapper parts
	let (original_fn, wrapper_body) =
		generate_wrapper_with_both(input, extractors, inject_params, options);
	let wrapper_body = quote! {
		#request_binding_initialization
		#wrapper_body
	};

	let route_doc = format!("Route: {} {}", method, path);

	// Resolve the reverse name (carries the `!` exemption sigil) and the clean
	// metadata name, and emit a compile-time kebab-case warning for an explicit
	// non-kebab name (Issue #4901).
	let (name_method_value, metadata_clean) = resolve_route_names(&options.name, fn_name);
	let kebab_name_warning = match &options.name {
		Some(name) => emit_non_kebab_name_warning(fn_name, name),
		None => quote! {},
	};

	// Generate inventory submission for endpoint metadata. Strip the `!`
	// exemption sigil from the metadata name (Issue #4901); unnamed handlers
	// keep their `None` metadata name (mirrors the simple-path logic so both
	// codegen paths produce identical EndpointMetadata.name behavior).
	let metadata_name = if options.name.is_some() {
		quote! { Some(#metadata_clean) }
	} else {
		quote! { None }
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

	Ok(quote! {
		// Submit endpoint metadata to global inventory
		#metadata_submission

		// Compile-time kebab-case warning marker (empty unless triggered).
		#kebab_name_warning

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
				#name_method_value
			}
		}

		#[#async_trait_crate::async_trait]
		impl #http_crate::Handler for #view_type_name {
			async fn handle(&self, #request_binding: #http_crate::Request) -> #http_crate::Result<#http_crate::Response> {
				#view_type_name::#fn_name(#request_binding).await
			}
		}

		impl #view_type_name {
			/// Handler function for this view
			#(#fn_attrs)*
			#fn_vis #asyncness fn #fn_name(#wrapper_request_binding: #http_crate::Request) #output {
				#wrapper_body
			}
		}

		/// Factory function for endpoint registration
		///
		/// Returns the View type for use with `UnifiedRouter::endpoint()`
		#fn_vis fn #fn_name() -> #view_type_name {
			#view_type_name
		}
	})
}

/// Convert a snake_case route name to a PascalCase trait name with "Resolve" prefix.
///
/// `auth_login` → `ResolveAuthLogin`
/// `cluster_retrieve` → `ResolveClusterRetrieve`
///
/// Used by `#[websocket]` to generate per-route WebSocket URL resolver
/// traits. The HTTP-route counterpart was removed alongside the
/// deprecated flat per-route URL resolver codegen (refs #4520).
pub(crate) fn to_resolver_trait_name(route_name: &str) -> String {
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
pub(crate) fn extract_url_params(path: &str) -> Vec<String> {
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

/// Returns `true` when `name` follows the kebab-case convention used by reverse
/// URL names: no underscores and no ASCII-uppercase letters. Mirrors the runtime
/// `is_kebab_case` helper in `reinhardt-urls` (a proc-macro crate cannot depend
/// on it, so the small check is duplicated). Refs Issue #4901.
fn is_kebab_route_name(name: &str) -> bool {
	!name.chars().any(|c| c == '_' || c.is_ascii_uppercase())
}

/// Convert a snake_case / camelCase / PascalCase `name` to kebab-case for the
/// non-kebab-case warning suggestion. Mirrors the runtime `to_kebab_case` helper
/// in `reinhardt-urls`. Refs Issue #4901.
fn suggest_kebab_route_name(name: &str) -> String {
	let mut out = String::with_capacity(name.len() + 4);
	// Treat the start as a boundary so we never emit a leading '-'.
	let mut prev_is_boundary = true;
	for c in name.chars() {
		if c == '_' || c == '-' {
			if !prev_is_boundary {
				out.push('-');
				prev_is_boundary = true;
			}
		} else if c.is_ascii_uppercase() {
			if !prev_is_boundary {
				out.push('-');
			}
			out.push(c.to_ascii_lowercase());
			prev_is_boundary = false;
		} else {
			out.push(c);
			prev_is_boundary = false;
		}
	}
	out
}

/// Whether the kebab-case URL-name warning is enabled at macro-expansion time.
///
/// Honors the same `REINHARDT_URL_NAME_WARNINGS` global toggle as the runtime
/// reverser (`0`/`false`/`off`/`no` disables it). Note: cargo may cache
/// proc-macro output, so toggling this env var might not re-trigger expansion —
/// the per-route `!` opt-out sigil is the robust, build-cache-independent
/// control. Refs Issue #4901.
fn url_name_warnings_enabled() -> bool {
	match std::env::var("REINHARDT_URL_NAME_WARNINGS") {
		Ok(v) => !matches!(
			v.trim().to_ascii_lowercase().as_str(),
			"0" | "false" | "off" | "no"
		),
		Err(_) => true,
	}
}

/// Resolve the route-name strings for a generated endpoint (Issue #4901).
///
/// Returns `(reverse_name, metadata_name)`:
/// - `reverse_name` is what `EndpointInfo::name()` returns and what the URL
///   reverser registers. It carries the `!` exemption sigil so the runtime
///   kebab-case warning is suppressed for auto-derived (fn-name) defaults — the
///   user did not choose those names — and for explicit opt-outs. The reverser
///   strips the sigil before storage, so reverse lookups use the clean name.
/// - `metadata_name` is the clean name (sigil stripped) used for endpoint
///   metadata such as OpenAPI.
fn resolve_route_names(explicit: &Option<String>, fn_name: &syn::Ident) -> (String, String) {
	match explicit {
		Some(name) => {
			let clean = name.strip_prefix('!').unwrap_or(name).to_string();
			(name.clone(), clean)
		}
		None => {
			let derived = fn_name.to_string();
			(format!("!{derived}"), derived)
		}
	}
}

/// Emit a compile-time `deprecated`-lint warning when an explicit route `name`
/// is not kebab-case (Issue #4901).
///
/// The marker reuses the const-read-of-`#[deprecated]` pattern from
/// `viewset_macro::emit_basename_fallback_deprecation` (Issue #4549) — the only
/// stable way to surface a non-error warning from a proc-macro. A leading `!`
/// opts out (and is consumed by the runtime reverser). Returns empty tokens when
/// the name is exempt, already kebab-case, or warnings are globally disabled.
fn emit_non_kebab_name_warning(fn_name: &syn::Ident, name: &str) -> TokenStream {
	if name.starts_with('!') || is_kebab_route_name(name) || !url_name_warnings_enabled() {
		return quote! {};
	}
	let suggestion = suggest_kebab_route_name(name);
	let note = format!(
		"URL name \"{name}\" is not kebab-case; prefer \"{suggestion}\" to match \
		 ViewSet-generated names (e.g. \"users-list\"). Prefix the name with '!' \
		 (name = \"!{name}\") to opt out, or set REINHARDT_URL_NAME_WARNINGS=0."
	);
	let module_ident = syn::Ident::new(
		&format!("__non_kebab_url_name_{fn_name}"),
		Span::call_site(),
	);
	quote! {
		#[doc(hidden)]
		#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
		#[allow(non_snake_case)]
		mod #module_ident {
			#[deprecated(note = #note)]
			pub const REASON: () = ();
			// Reading the `#[deprecated]` const is what fires the lint at the
			// route macro's call site.
			#[allow(deprecated_in_future, clippy::no_effect)]
			const _: () = REASON;
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
		return generate_view_type(
			&input,
			method,
			&path_str,
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
	// Resolve the reverse name (carries the `!` exemption sigil) and the clean
	// metadata name, and emit a compile-time kebab-case warning for an explicit
	// non-kebab name (Issue #4901).
	let (name_method_value, metadata_clean) = resolve_route_names(&options.name, fn_name);
	let kebab_name_warning = match &options.name {
		Some(name) => emit_non_kebab_name_warning(fn_name, name),
		None => quote! {},
	};
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

	// Generate inventory submission for endpoint metadata. Strip the `!`
	// exemption sigil from the metadata name (Issue #4901); unnamed handlers
	// keep their `None` metadata name.
	let metadata_name = if options.name.is_some() {
		quote! { Some(#metadata_clean) }
	} else {
		quote! { None }
	};

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

	Ok(quote! {
		// Submit endpoint metadata to global inventory
		#metadata_submission

		// Compile-time kebab-case warning marker (empty unless triggered).
		#kebab_name_warning

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
				#name_method_value
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
	use rstest::rstest;

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

	#[test]
	fn resolver_trait_name_format() {
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

	// --- Kebab-case URL name convention (Issue #4901) ---

	fn ident(name: &str) -> syn::Ident {
		syn::Ident::new(name, Span::call_site())
	}

	#[test]
	fn is_kebab_route_name_classifies_names() {
		assert!(is_kebab_route_name("users-list"));
		assert!(is_kebab_route_name("detail"));
		assert!(is_kebab_route_name("v2"));
		assert!(!is_kebab_route_name("user_detail"));
		assert!(!is_kebab_route_name("userDetail"));
		assert!(!is_kebab_route_name("UserDetail"));
	}

	#[test]
	fn suggest_kebab_route_name_converts_names() {
		assert_eq!(suggest_kebab_route_name("user_detail"), "user-detail");
		assert_eq!(suggest_kebab_route_name("userDetail"), "user-detail");
		assert_eq!(suggest_kebab_route_name("UserDetail"), "user-detail");
		assert_eq!(suggest_kebab_route_name("users-list"), "users-list");
	}

	#[test]
	fn resolve_route_names_marks_fallback_and_strips_optout() {
		// Explicit name: reverse name kept verbatim, metadata identical.
		assert_eq!(
			resolve_route_names(&Some("users-list".to_string()), &ident("list_users")),
			("users-list".to_string(), "users-list".to_string())
		);
		// Explicit opt-out: `!` kept on the reverse name, stripped for metadata.
		assert_eq!(
			resolve_route_names(&Some("!user_detail".to_string()), &ident("get_user")),
			("!user_detail".to_string(), "user_detail".to_string())
		);
		// Fallback to fn name: reverse name is exempt (`!`-prefixed), metadata clean.
		assert_eq!(
			resolve_route_names(&None, &ident("get_user")),
			("!get_user".to_string(), "get_user".to_string())
		);
	}

	#[test]
	fn emit_non_kebab_name_warning_is_empty_for_exempt_names() {
		// Already kebab-case: no marker regardless of the global toggle.
		assert!(
			emit_non_kebab_name_warning(&ident("list_users"), "users-list")
				.to_string()
				.is_empty()
		);
		// Explicit opt-out: no marker.
		assert!(
			emit_non_kebab_name_warning(&ident("get_user"), "!user_detail")
				.to_string()
				.is_empty()
		);
	}

	#[test]
	fn emit_non_kebab_name_warning_emits_marker_for_snake_case() {
		// The marker only fires when warnings are enabled (default unless the
		// REINHARDT_URL_NAME_WARNINGS toggle disables them in this environment).
		if url_name_warnings_enabled() {
			let marker = emit_non_kebab_name_warning(&ident("get_user"), "user_detail").to_string();
			assert!(marker.contains("deprecated"));
			assert!(marker.contains("user-detail"));
		}
	}

	#[test]
	fn detect_extractors_includes_cookie_struct() {
		use syn::parse_quote;

		let inputs: syn::punctuated::Punctuated<FnArg, Token![,]> = parse_quote! {
			CookieStruct(cookies): CookieStruct<MyCookies>,
			Query(query): Query<SearchParams>
		};

		let extractors = detect_extractors(&inputs);
		let names: Vec<_> = extractors
			.iter()
			.map(|extractor| extractor.extractor_name.as_str())
			.collect();

		assert_eq!(names, vec!["CookieStruct", "Query"]);
	}

	#[rstest]
	fn detect_extractors_includes_supported_structured_extractors() {
		use syn::parse_quote;

		let inputs: syn::punctuated::Punctuated<FnArg, Token![,]> = parse_quote! {
			Validated(value): Validated<Payload>,
			PathStruct(path): PathStruct<PathData>,
			HeaderStruct(headers): HeaderStruct<HeaderData>,
			multipart: Multipart
		};

		let extractors = detect_extractors(&inputs);
		let names: Vec<_> = extractors
			.iter()
			.map(|extractor| extractor.extractor_name.as_str())
			.collect();

		assert_eq!(
			names,
			vec!["Validated", "PathStruct", "HeaderStruct", "Multipart"]
		);
	}

	#[rstest]
	fn detect_extractors_includes_optional_named_session_value() {
		let inputs: syn::punctuated::Punctuated<FnArg, Token![,]> = syn::parse_quote! {
			session: OptionalSessionValueNamed<SessionKey, UserId>
		};

		let extractors = detect_extractors(&inputs);
		assert_eq!(extractors.len(), 1);
		assert_eq!(extractors[0].extractor_name, "OptionalSessionValueNamed");
	}

	#[rstest]
	fn route_call_passes_raw_request_with_extractors_in_original_order() {
		let input: ItemFn = syn::parse_quote! {
			async fn handler(request: reinhardt_http::Request, Path(req): Path<String>, Json(body): Json<Payload>) -> String { String::new() }
		};
		let extractors = detect_extractors(&input.sig.inputs);
		let inject_params = detect_inject_params(&input.sig.inputs);
		let (_, wrapper) = generate_wrapper_with_both(
			&input,
			&extractors,
			&inject_params,
			&RouteOptions::default(),
		);
		let generated = wrapper.to_string();
		let call_start = generated
			.find("handler_original")
			.expect("generated wrapper should call the renamed handler");
		let call = &generated[call_start..];
		let call_end = call
			.find(") . await")
			.expect("generated handler call should be awaited")
			+ ") . await".len();
		assert_eq!(
			&call[..call_end],
			"handler_original (__reinhardt_request , __reinhardt_extractor_0 , __reinhardt_extractor_1) . await"
		);
	}

	#[rstest]
	fn route_call_extracts_body_when_the_binding_name_contains_request() {
		let input: ItemFn = syn::parse_quote! {
			async fn handler(request_body: Body, Json(payload): Json<Payload>) -> String { String::new() }
		};
		let extractors = detect_extractors(&input.sig.inputs);
		let inject_params = detect_inject_params(&input.sig.inputs);

		assert_eq!(extractors.len(), 2);
		let (_, wrapper) = generate_wrapper_with_both(
			&input,
			&extractors,
			&inject_params,
			&RouteOptions::default(),
		);
		let generated = wrapper.to_string();
		let call_start = generated
			.find("handler_original")
			.expect("generated wrapper should call the renamed handler");
		let call = &generated[call_start..];
		let call_end = call
			.find(") . await")
			.expect("generated handler call should be awaited")
			+ ") . await".len();
		assert_eq!(
			&call[..call_end],
			"handler_original (__reinhardt_extractor_0 , __reinhardt_extractor_1 ,) . await"
		);
	}

	#[rstest]
	fn route_call_extracts_body_when_its_binding_is_request() {
		let input: ItemFn = syn::parse_quote! {
			async fn handler(request: Body, Json(payload): Json<Payload>) -> String { String::new() }
		};
		let extractors = detect_extractors(&input.sig.inputs);
		let inject_params = detect_inject_params(&input.sig.inputs);

		assert_eq!(extractors.len(), 2);
		let (_, wrapper) = generate_wrapper_with_both(
			&input,
			&extractors,
			&inject_params,
			&RouteOptions::default(),
		);
		let generated = wrapper.to_string();
		let call_start = generated
			.find("handler_original")
			.expect("generated wrapper should call the renamed handler");
		let call = &generated[call_start..];
		let call_end = call
			.find(") . await")
			.expect("generated handler call should be awaited")
			+ ") . await".len();
		assert_eq!(
			&call[..call_end],
			"handler_original (__reinhardt_extractor_0 , __reinhardt_extractor_1 ,) . await"
		);
	}

	#[rstest]
	fn route_call_forwards_mutable_injected_parameter_as_a_value() {
		let input: ItemFn = syn::parse_quote! {
			async fn handler(request: reinhardt_http::Request, #[inject] mut service: Service) -> String { String::new() }
		};
		let extractors = detect_extractors(&input.sig.inputs);
		let inject_params = detect_inject_params(&input.sig.inputs);
		let (_, wrapper) = generate_wrapper_with_both(
			&input,
			&extractors,
			&inject_params,
			&RouteOptions::default(),
		);
		let generated = wrapper.to_string();
		let call_start = generated
			.find("handler_original")
			.expect("generated wrapper should call the renamed handler");
		let call = &generated[call_start..];
		let call_end = call
			.find(") . await")
			.expect("generated handler call should be awaited")
			+ ") . await".len();
		assert_eq!(
			&call[..call_end],
			"handler_original (__reinhardt_request , __reinhardt_inject_0) . await"
		);
	}

	#[rstest]
	fn validate_extractors_rejects_multipart_with_another_body_extractor() {
		let inputs: syn::punctuated::Punctuated<FnArg, Token![,]> = syn::parse_quote! {
			multipart: Multipart,
			body: Body
		};

		let error = validate_extractors(&detect_extractors(&inputs))
			.expect_err("Multipart and Body must not be extracted from one request");

		assert_eq!(
			error.to_string(),
			"Cannot use multiple body-consuming extractors: Multipart, Body. Request body can only be read once."
		);
	}

	#[rstest]
	fn validate_extractors_rejects_validated_body_extractor_with_body() {
		let inputs: syn::punctuated::Punctuated<FnArg, Token![,]> = syn::parse_quote! {
			payload: Validated<Json<LoginRequest>>,
			body: Body
		};

		let error = validate_extractors(&detect_extractors(&inputs))
			.expect_err("Validated<Json<_>> and Body must not share a request body");

		assert_eq!(
			error.to_string(),
			"Cannot use multiple body-consuming extractors: Json, Body. Request body can only be read once."
		);
	}

	#[rstest]
	fn extract_request_body_info_preserves_validated_form_metadata() {
		let inputs: syn::punctuated::Punctuated<FnArg, Token![,]> = syn::parse_quote! {
			Validated(payload): Validated<Form<LoginRequest>>
		};

		assert_eq!(
			extract_request_body_info(&inputs),
			Some((
				"LoginRequest".to_string(),
				"application/x-www-form-urlencoded".to_string(),
			))
		);
	}

	#[rstest]
	fn extract_request_body_info_records_multipart_metadata() {
		let inputs: syn::punctuated::Punctuated<FnArg, Token![,]> = syn::parse_quote! {
			multipart: Multipart
		};

		assert_eq!(
			extract_request_body_info(&inputs),
			Some(("Multipart".to_string(), "multipart/form-data".to_string()))
		);
	}

	#[rstest]
	fn route_call_forwards_mutable_extractor_as_a_value() {
		let input: ItemFn = syn::parse_quote! {
			async fn handler(request: reinhardt_http::Request, mut multipart: Multipart) -> String { String::new() }
		};
		let extractors = detect_extractors(&input.sig.inputs);
		let inject_params = detect_inject_params(&input.sig.inputs);
		let (_, wrapper) = generate_wrapper_with_both(
			&input,
			&extractors,
			&inject_params,
			&RouteOptions::default(),
		);
		let generated = wrapper.to_string();
		let call_start = generated
			.find("handler_original")
			.expect("generated wrapper should call the renamed handler");
		let call = &generated[call_start..];
		let call_end = call
			.find(") . await")
			.expect("generated handler call should be awaited")
			+ ") . await".len();
		assert_eq!(
			&call[..call_end],
			"handler_original (__reinhardt_request , __reinhardt_extractor_0) . await"
		);
	}

	#[test]
	fn detect_auth_marks_current_user_as_protected() {
		let detection = detect_auth_from_type_strings(&["CurrentUser < User >".to_string()]);

		assert!(matches!(
			detection.protection,
			AuthProtectionKind::Protected
		));
		assert!(detection.guard_description.is_none());
	}
}
