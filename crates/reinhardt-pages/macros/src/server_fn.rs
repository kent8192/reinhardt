//! Server Function Macro Implementation
//!
//! This module implements the `#[server_fn]` procedural macro for generating
//! client-side stubs (WASM) and server-side handlers (non-WASM).
//!
//! ## Architecture
//!
//! The macro performs conditional compilation:
//! - **WASM target**: Generates HTTP client stub
//! - **Non-WASM target**: Generates route handler
//!
use convert_case::{Case, Casing};
use darling::FromMeta;
use darling::ast::NestedMeta;
use proc_macro::TokenStream;
use quote::quote;
use syn::punctuated::Punctuated;
use syn::{FnArg, ItemFn, Meta, Token, parse_macro_input};

// Import crate path helpers for dynamic resolution
use crate::crate_paths::{
	CratePathInfo, get_reinhardt_core_crate, get_reinhardt_di_crate, get_reinhardt_http_crate,
	get_reinhardt_pages_crate, get_reinhardt_pages_crate_info,
};

/// Convert snake_case identifier to UpperCamelCase for struct naming
///
/// # Examples
///
/// ```ignore
/// use proc_macro2::Ident;
/// use quote::format_ident;
///
/// let ident = format_ident!("create_user");
/// let pascal = to_pascal_case_ident(&ident);
/// assert_eq!(pascal.to_string(), "CreateUser");
///
/// let ident = format_ident!("get_user_list");
/// let pascal = to_pascal_case_ident(&ident);
/// assert_eq!(pascal.to_string(), "GetUserList");
/// ```
fn to_pascal_case_ident(ident: &proc_macro2::Ident) -> proc_macro2::Ident {
	let pascal_name = ident.to_string().to_case(Case::Pascal);
	quote::format_ident!("{}", pascal_name)
}

/// Options for `#[server_fn]` macro
///
/// These options are parsed from the attribute arguments.
#[derive(Debug, Clone, FromMeta)]
#[darling(default)]
pub(crate) struct ServerFnOptions {
	/// **Deprecated**: `#[inject]` parameters are now auto-detected.
	///
	/// Previously required `use_inject = true` to enable DI parameter detection.
	/// Now `#[inject]` attributes are detected unconditionally, matching the
	/// behavior of route macros (`#[get]`, `#[post]`, etc.).
	///
	/// # Migration
	///
	/// ```ignore
	/// // Before (deprecated):
	/// #[server_fn(use_inject = true)]
	/// async fn get_user(
	///     id: u32,
	///     #[inject] db: Database,
	/// ) -> Result<User, ServerFnError> { /* ... */ }
	///
	/// // After (recommended):
	/// #[server_fn]
	/// async fn get_user(
	///     id: u32,
	///     #[inject] db: Database,
	/// ) -> Result<User, ServerFnError> { /* ... */ }
	/// ```
	pub use_inject: bool,

	/// Optional custom endpoint path
	///
	/// If not specified, defaults to `/api/server_fn/{function_name}`
	///
	/// # Example
	///
	/// ```ignore
	/// #[server_fn(endpoint = "/api/users/get")]
	/// async fn get_user(id: u32) -> Result<User, ServerFnError> {
	///     // ...
	/// }
	/// ```
	pub endpoint: Option<String>,

	/// Codec: "json" (default), "url", "msgpack"
	///
	/// Determines the serialization format for arguments and return values.
	///
	/// # Example
	///
	/// ```ignore
	/// #[server_fn(codec = "msgpack")]
	/// async fn upload_data(data: Vec<u8>) -> Result<(), ServerFnError> {
	///     // ...
	/// }
	/// ```
	#[darling(default = "default_codec")]
	pub codec: String,

	/// Disable automatic CSRF token injection
	///
	/// By default, server function client stubs automatically include the
	/// X-CSRFToken header in requests. Set this to `true` to disable this
	/// behavior for endpoints that don't require CSRF protection.
	///
	/// # Example
	///
	/// ```ignore
	/// // Public API endpoint without CSRF protection
	/// #[server_fn(no_csrf = true)]
	/// async fn public_health_check() -> Result<String, ServerFnError> {
	///     Ok("OK".to_string())
	/// }
	/// ```
	pub no_csrf: bool,

	/// Enable automatic validation with `pre_validate = true`
	///
	/// When enabled, deserialized arguments implementing `validator::Validate`
	/// are automatically validated before the server function is called.
	/// Returns an error with validation details on failure.
	///
	/// # Example
	///
	/// ```ignore
	/// #[server_fn(pre_validate = true)]
	/// async fn create_user(req: CreateUserRequest) -> Result<User, ServerFnError> {
	///     // req is already validated
	/// }
	/// ```
	pub pre_validate: bool,
}

fn default_codec() -> String {
	"json".to_string()
}

impl Default for ServerFnOptions {
	fn default() -> Self {
		Self {
			use_inject: false,
			endpoint: None,
			codec: default_codec(),
			no_csrf: false,
			pre_validate: false,
		}
	}
}

/// Information about `#[inject]` parameters
///
/// This struct holds metadata about parameters that should be resolved
/// via dependency injection on the server side.
#[derive(Debug, Clone)]
struct InjectInfo {
	/// Parameter pattern (e.g., `db`, `auth`)
	pat: Box<syn::Pat>,
	/// Parameter type (e.g., `Database`, `AuthContext`)
	ty: Box<syn::Type>,
}

/// Check if an attribute is `#[inject]` or #[reinhardt::inject]
///
/// # Examples
///
/// ```ignore
/// #[inject] db: Database              // Legacy (causes compiler errors on params)
/// #[reinhardt::inject] db: Database   // Recommended tool attribute
/// ```
fn is_inject_attr(attr: &syn::Attribute) -> bool {
	// Check for bare #[inject] (legacy, causes compiler errors on function params)
	if attr.path().is_ident("inject") {
		return true;
	}

	// Check for #[reinhardt::inject] (recommended tool attribute)
	if let Some(seg0) = attr.path().segments.first()
		&& seg0.ident == "reinhardt"
		&& let Some(seg1) = attr.path().segments.iter().nth(1)
	{
		return seg1.ident == "inject";
	}

	false
}

/// Detect parameters for dependency injection
///
/// This function scans function parameters and identifies those that should be
/// injected by the DI system. Detection is based on:
/// - Parameters with `#[inject]` attribute
///
/// # Examples
///
/// ```ignore
/// async fn handler(
///     id: u32,                    // Regular parameter
///     #[inject] db: Arc<Database>, // DI parameter (explicit)
///     #[inject] site: Arc<AdminSite>, // DI parameter (explicit)
/// ) -> Result<User, Error>
/// ```
fn detect_inject_params(inputs: &Punctuated<FnArg, Token![,]>) -> Vec<InjectInfo> {
	let mut inject_params = Vec::new();

	for input in inputs {
		if let FnArg::Typed(pat_type) = input {
			// Check for explicit #[inject] attribute
			let has_inject_attr = pat_type.attrs.iter().any(is_inject_attr);

			if has_inject_attr {
				inject_params.push(InjectInfo {
					pat: pat_type.pat.clone(),
					ty: pat_type.ty.clone(),
				});
			}
		}
	}

	inject_params
}

/// Remove `#[inject]` attributes from function parameters
///
/// This creates a clean version of the function for server-side compilation.
/// Pattern copied from reinhardt-core/crates/macros/src/routes.rs.
///
/// # Example
///
/// Input:
/// ```ignore
/// async fn handler(id: u32, #[inject] db: Database) -> Result<User, Error>
/// ```
///
/// Output:
/// ```ignore
/// async fn handler(id: u32, db: Database) -> Result<User, Error>
/// ```
fn remove_inject_attrs(func: &ItemFn) -> ItemFn {
	let mut func = func.clone();

	// Remove #[inject] attributes from parameters
	func.sig.inputs = func
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

	func
}

/// Server function metadata
///
/// This struct holds all the information extracted from the function signature.
struct ServerFnInfo {
	/// Original function
	func: ItemFn,
	/// Parsed options
	options: ServerFnOptions,
}

impl ServerFnInfo {
	/// Parse from macro input
	fn parse(args: Vec<Meta>, func: ItemFn) -> Result<Self, darling::Error> {
		// Convert Meta to NestedMeta for darling compatibility
		let nested: Vec<NestedMeta> = args.into_iter().map(NestedMeta::Meta).collect();
		let options = ServerFnOptions::from_list(&nested)?;

		// Validate endpoint path if explicitly specified
		if let Some(ref endpoint) = options.endpoint {
			validate_endpoint_path(endpoint)?;
		}

		Ok(Self { func, options })
	}

	/// Get the function name
	fn name(&self) -> &syn::Ident {
		&self.func.sig.ident
	}

	/// Get the function visibility
	fn vis(&self) -> &syn::Visibility {
		&self.func.vis
	}

	/// Get the endpoint path
	///
	/// Returns the custom endpoint if specified, otherwise generates default.
	fn endpoint(&self) -> String {
		self.options
			.endpoint
			.clone()
			.unwrap_or_else(|| format!("/api/server_fn/{}", self.name()))
	}

	/// Get the codec name
	fn codec(&self) -> &str {
		&self.options.codec
	}

	/// Check if the deprecated `use_inject` option is enabled (for deprecation warning)
	fn use_inject_enabled(&self) -> bool {
		self.options.use_inject
	}
}

/// Validate server_fn endpoint path.
///
/// The endpoint path must:
/// - Start with `/`
/// - Not contain path traversal sequences (`..`)
/// - Not contain query strings (`?`)
/// - Not contain fragment identifiers (`#`)
/// - Not be a full URL (e.g. `http://` or `https://`)
fn validate_endpoint_path(path: &str) -> Result<(), darling::Error> {
	if path.contains(char::is_whitespace) {
		return Err(darling::Error::custom(
			"endpoint path must not contain whitespace",
		));
	}

	if path.starts_with("http://") || path.starts_with("https://") {
		return Err(darling::Error::custom(
			"endpoint must be a relative path starting with '/', not a full URL",
		));
	}

	if !path.starts_with('/') {
		return Err(darling::Error::custom("endpoint path must start with '/'"));
	}

	if path.contains("..") {
		return Err(darling::Error::custom(
			"endpoint path must not contain path traversal sequences ('..')",
		));
	}

	if path.contains('?') {
		return Err(darling::Error::custom(
			"endpoint path must not contain query strings ('?')",
		));
	}

	if path.contains('#') {
		return Err(darling::Error::custom(
			"endpoint path must not contain fragment identifiers ('#')",
		));
	}

	Ok(())
}

/// Main entry point for `#[server_fn]` macro
pub(crate) fn server_fn_impl(args: TokenStream, input: TokenStream) -> TokenStream {
	// Parse attribute arguments
	let attr_args = match syn::parse::Parser::parse(
		syn::punctuated::Punctuated::<Meta, syn::Token![,]>::parse_terminated,
		args.clone(),
	) {
		Ok(args) => args.into_iter().collect(),
		Err(e) => return e.to_compile_error().into(),
	};

	// Parse function
	let func = parse_macro_input!(input as ItemFn);

	// Parse metadata
	let info = match ServerFnInfo::parse(attr_args, func) {
		Ok(info) => info,
		Err(e) => return e.write_errors().into(),
	};

	generate_server_fn(&info).into()
}

/// Generate server function code
///
/// This generates both client and server code with conditional compilation.
/// `#[inject]` parameters are always auto-detected, matching the behavior of
/// route macros (`#[get]`, `#[post]`, etc.).
fn generate_server_fn(info: &ServerFnInfo) -> proc_macro2::TokenStream {
	let func = &info.func;

	// Auto-detect #[inject] parameters unconditionally
	let inject_params = detect_inject_params(&func.sig.inputs);

	// Remove #[inject] attributes from original function
	// This ensures the server-side code compiles without unknown attributes
	let clean_func = if !inject_params.is_empty() {
		remove_inject_attrs(func)
	} else {
		func.clone()
	};

	// Emit deprecation warning if use_inject = true is enabled
	let deprecation_warning = if info.use_inject_enabled() {
		quote! {
			#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
			const _: () = {
				#[deprecated(
					note = "use_inject = true is deprecated. #[inject] parameters are now auto-detected. Remove `use_inject = true` from #[server_fn] attribute."
				)]
				#[allow(non_upper_case_globals, dead_code)]
				const __use_inject_deprecated: () = ();

				#[allow(dead_code)]
				const _trigger: () = __use_inject_deprecated;
			};
		}
	} else {
		quote! {}
	};

	// Dynamically resolve reinhardt_pages crate path for client stub
	let pages_crate_info = get_reinhardt_pages_crate_info();

	// Generate client stub (with DI parameter filtering)
	let client_stub = generate_client_stub(info, &inject_params, &pages_crate_info);

	// Generate server handler (with DI resolution)
	let server_handler = generate_server_handler(info, &inject_params);

	quote! {
		// Deprecation warning for use_inject = true (if specified)
		#deprecation_warning

		// Server-side: Original function (with #[inject] attributes removed)
		#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
		#clean_func

		// Client-side: HTTP request stub
		#client_stub

		// Server-side: Route handler and registration
		#server_handler
	}
}

/// Generate client-side HTTP request stub
///
/// This generates an async function that:
/// 1. Serializes function arguments to JSON
/// 2. Sends HTTP POST request to the endpoint
/// 3. Deserializes the response
///
/// Example expansion:
/// ```ignore
/// // Input:
/// #[server_fn]
/// async fn get_user(id: u32) -> Result<User, ServerFnError> { ... }
///
/// // Expands to (on WASM):
/// pub async fn get_user(id: u32) -> Result<User, ServerFnError> {
///     #[derive(serde::Serialize)]
///     struct Args { id: u32 }
///
///     let url = "/api/server_fn/get_user";
///     let args = Args { id };
///     let response = reqwest::Client::new().post(url)
///         .json(&args)
///         .send()
///         .await?;
///     response.json().await
/// }
/// ```
fn generate_client_stub(
	info: &ServerFnInfo,
	_inject_params: &[InjectInfo],
	pages_crate_info: &CratePathInfo,
) -> proc_macro2::TokenStream {
	// Extract crate path info components
	let pages_use_statement = &pages_crate_info.use_statement;
	let pages_crate = &pages_crate_info.ident;
	let name = info.name();
	let vis = info.vis();
	let endpoint = info.endpoint();
	let codec = info.codec();
	let func = &info.func;
	let sig = &func.sig;

	// Extract function parameters, excluding #[inject] parameters
	// Client-side doesn't need DI parameters - they're resolved on the server
	let params: Vec<_> = sig
		.inputs
		.iter()
		.filter_map(|arg| {
			if let syn::FnArg::Typed(pat_type) = arg {
				// Skip #[inject] parameters
				let has_inject = pat_type.attrs.iter().any(is_inject_attr);
				if !has_inject { Some(pat_type) } else { None }
			} else {
				None
			}
		})
		.collect();

	// Extract parameter names and types for Args struct
	let param_names: Vec<_> = params.iter().map(|p| &p.pat).collect();

	let param_types: Vec<_> = params.iter().map(|p| &p.ty).collect();

	// Generate unique struct name to avoid conflicts
	let args_struct_name = {
		let pascal_name = to_pascal_case_ident(name);
		quote::format_ident!("{}Args", pascal_name)
	};

	// Create a new signature without #[inject] parameters for the client stub
	// This ensures the WASM-side function signature matches what the client code expects
	let client_sig = {
		let mut new_sig = sig.clone();
		// Replace inputs with filtered params (without #[inject])
		new_sig.inputs = params
			.iter()
			.map(|p| syn::FnArg::Typed((*p).clone()))
			.collect();
		new_sig
	};

	// Generate CSRF injection code conditionally based on no_csrf option
	let csrf_injection_code = if info.options.no_csrf {
		// no_csrf = true: Skip CSRF header injection
		quote! {}
	} else {
		// no_csrf = false (default): Inject CSRF header
		quote! {
			// Inject CSRF header if available (automatic CSRF protection)
			use #pages_crate::csrf::csrf_headers;
			if let Some((__csrf_header_name, __csrf_header_value)) = csrf_headers() {
				__request_builder = __request_builder.header(__csrf_header_name, &__csrf_header_value);
			}
		}
	};

	// Generate JWT auth header injection code (always enabled).
	// If a JWT token exists in sessionStorage, it is attached as
	// an Authorization: Bearer header. When no token is stored, this
	// is a no-op — backward compatible with unauthenticated calls.
	let auth_injection_code = quote! {
		// Inject Authorization header if JWT token is available
		use #pages_crate::auth::auth_headers;
		if let Some((__auth_header_name, __auth_header_value)) = auth_headers() {
			__request_builder = __request_builder.header(__auth_header_name, &__auth_header_value);
		}
	};

	// Generate codec-specific serialization and deserialization code
	let (content_type, serialize_code, deserialize_code) = match codec {
		"json" => (
			"application/json",
			quote! {
				let __body = ::serde_json::to_string(&__args)
					.map_err(|e| #pages_crate::server_fn::ServerFnError::serialization(e.to_string()))?;
			},
			quote! {
				__response
					.json()
					.await
					.map_err(|e| #pages_crate::server_fn::ServerFnError::deserialization(e.to_string()))
			},
		),
		"url" => (
			"application/x-www-form-urlencoded",
			quote! {
				let __body = ::serde_urlencoded::to_string(&__args)
					.map_err(|e| #pages_crate::server_fn::ServerFnError::serialization(e.to_string()))?;
			},
			quote! {
				let __text = __response.text().await
					.map_err(|e| #pages_crate::server_fn::ServerFnError::deserialization(e.to_string()))?;
				::serde_json::from_str(&__text)
					.map_err(|e| #pages_crate::server_fn::ServerFnError::deserialization(e.to_string()))
			},
		),
		"msgpack" => (
			"application/msgpack",
			quote! {
				let __body_bytes = ::rmp_serde::to_vec(&__args)
					.map_err(|e| #pages_crate::server_fn::ServerFnError::serialization(e.to_string()))?;
				// Convert to base64 for transport over HTTP text body
				let __body = ::base64::Engine::encode(&::base64::engine::general_purpose::STANDARD, &__body_bytes);
			},
			quote! {
				let __text = __response.text().await
					.map_err(|e| #pages_crate::server_fn::ServerFnError::deserialization(e.to_string()))?;
				let __bytes = ::base64::Engine::decode(&::base64::engine::general_purpose::STANDARD, &__text)
					.map_err(|e| #pages_crate::server_fn::ServerFnError::deserialization(e.to_string()))?;
				::rmp_serde::from_slice(&__bytes)
					.map_err(|e| #pages_crate::server_fn::ServerFnError::deserialization(e.to_string()))
			},
		),
		// Fixes #843: emit compile error for unknown codec instead of silent fallback
		unknown => {
			let msg = format!(
				"unknown codec '{}'. Valid options: \"json\", \"url\", \"msgpack\"",
				unknown,
			);
			return quote! { compile_error!(#msg); };
		}
	};

	quote! {
		#[cfg(all(target_family = "wasm", target_os = "unknown"))]
		#vis #client_sig {
			use ::serde::{Serialize, Deserialize};

			// Conditional crate path resolution for WASM/server compatibility
			#pages_use_statement

			// Argument struct for serialization
			#[derive(Serialize)]
			struct #args_struct_name {
				#(#param_names: #param_types),*
			}

			let __endpoint = #pages_crate::server_fn::resolve_endpoint(#endpoint);
			let __args = #args_struct_name {
				#(#param_names),*
			};

			// Serialize arguments based on codec
			#serialize_code

			// Build HTTP client and POST request.
			// WASM: fetch_credentials_include() sends browser cookies via
			// the Fetch API's credentials: "include" mode, which is
			// required for CSRF double-submit cookie validation.
			let __client = #pages_crate::__private::reqwest::Client::builder()
				.build()
				.expect("Failed to build reqwest client");

			let mut __request_builder = __client.post(&__endpoint)
				.header("Content-Type", #content_type);

			// WASM: include browser cookies (CSRF, auth session) via Fetch API
			#[cfg(target_arch = "wasm32")]
			{
				__request_builder = __request_builder.fetch_credentials_include();
			}

			#csrf_injection_code
			#auth_injection_code

			// Send request
			let __response = __request_builder
				.body(__body)
				.send()
				.await
				.map_err(|e| #pages_crate::server_fn::ServerFnError::network(e.to_string()))?;

			// Check HTTP status
			if !__response.status().is_success() {
				let __status = __response.status().as_u16();
				let __message = __response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
				return Err(#pages_crate::server_fn::ServerFnError::server(__status, __message));
			}

			// Deserialize response based on codec
			#deserialize_code
		}
	}
}

/// Generate server handler and registration function
///
/// This generates a route handler that:
/// 1. Deserializes JSON request body to function arguments
/// 2. Calls the original server function
/// 3. Serializes the response to JSON
/// 4. Handles errors appropriately
///
/// Example expansion:
/// ```ignore
/// // Input:
/// #[server_fn]
/// async fn get_user(id: u32) -> Result<User, ServerFnError> { ... }
///
/// // Expands to (on server):
/// pub async fn __server_fn_handler_get_user(
///     body: String,
/// ) -> Result<String, String> {
///     #[derive(serde::Deserialize)]
///     struct Args { id: u32 }
///
///     let args: Args = serde_json::from_str(&body)?;
///     let result = get_user(args.id).await?;
///     Ok(serde_json::to_string(&result)?)
/// }
///
/// pub fn register_server_fn_get_user<R>(router: R) -> R
/// where
///     R: ServerFnRouter,
/// {
///     router.post("/api/server_fn/get_user", __server_fn_handler_get_user)
/// }
/// ```
fn generate_server_handler(
	info: &ServerFnInfo,
	inject_params: &[InjectInfo],
) -> proc_macro2::TokenStream {
	let name = info.name();
	let endpoint = info.endpoint();
	let codec = info.codec();
	let pre_validate = info.options.pre_validate;
	let func = &info.func;
	let sig = &func.sig;

	// Extract function parameters, separating regular and #[inject] parameters
	let regular_params: Vec<_> = sig
		.inputs
		.iter()
		.filter_map(|arg| {
			if let syn::FnArg::Typed(pat_type) = arg {
				let has_inject = pat_type.attrs.iter().any(is_inject_attr);
				if !has_inject { Some(pat_type) } else { None }
			} else {
				None
			}
		})
		.collect();

	let regular_param_names: Vec<_> = regular_params.iter().map(|p| &p.pat).collect();
	let regular_param_types: Vec<_> = regular_params.iter().map(|p| &p.ty).collect();

	// Extract inject parameter names and types
	let inject_param_names: Vec<_> = inject_params.iter().map(|p| &p.pat).collect();
	let inject_param_types: Vec<_> = inject_params.iter().map(|p| &p.ty).collect();

	// Generate unique names to avoid conflicts
	let handler_name = quote::format_ident!("__server_fn_handler_{}", name);
	let register_fn_name = quote::format_ident!("register_server_fn_{}", name);
	let args_struct_name = {
		let pascal_name = to_pascal_case_ident(name);
		quote::format_ident!("{}Args", pascal_name)
	};

	// Extract return type inner (T from Result<T, E>)
	// We assume the return type is Result<T, ServerFnError>
	let return_type = match &sig.output {
		syn::ReturnType::Type(_, ty) => ty,
		syn::ReturnType::Default => {
			return quote! {
				compile_error!("Server functions must return Result<T, ServerFnError>");
			};
		}
	};

	// Generate DI resolution code
	// Pattern copied from reinhardt-core/crates/macros/src/use_inject.rs
	let di_resolution = if !inject_params.is_empty() {
		// Dynamically resolve crate paths
		let di_crate = get_reinhardt_di_crate();
		let pages_crate_for_di = get_reinhardt_pages_crate();

		quote! {
			// Get DI context from request and fork for per-request isolation
			let __di_ctx = {
				let __shared_ctx = __req.get_di_context::<::std::sync::Arc<#di_crate::InjectionContext>>()
					.ok_or_else(|| "DI context not set. Ensure the router is configured with .with_di_context()".to_string())?;
				let __di_request = __req.clone_for_di();
				::std::sync::Arc::new((*__shared_ctx).fork_for_request(__di_request))
			};

			// Resolve each #[inject] parameter using reinhardt_di::Depends<T>
			#(
				let #inject_param_names: #inject_param_types =
					#di_crate::Depends::<#inject_param_types>::resolve(&__di_ctx, true)
						.await
						.map_err(|e| {
							// Preserve HTTP status codes for auth-related DI errors
							let (status, msg) = match &e {
								#di_crate::DiError::Authentication(m) => (401u16, m.clone()),
								#di_crate::DiError::Authorization(m) => (403u16, m.clone()),
								other => (500u16, format!("Dependency injection failed for {}: {:?}", stringify!(#inject_param_types), other)),
							};
							let server_err = #pages_crate_for_di::server_fn::ServerFnError::server(status, msg);
							::serde_json::to_string(&server_err)
								.unwrap_or_else(|_| format!("Dependency injection failed for {}: {:?}", stringify!(#inject_param_types), e))
						})?
						.into_inner();
			)*
		}
	} else {
		quote! {}
	};

	// Build function call with both regular and inject parameters
	let function_call_params = if inject_params.is_empty() {
		quote! {
			#(args.#regular_param_names),*
		}
	} else {
		quote! {
			#(args.#regular_param_names,)*
			#(#inject_param_names),*
		}
	};

	// Generate codec-specific deserialization code for server
	let deserialize_code = match codec {
		"json" => quote! {
			let args: #args_struct_name = ::serde_json::from_str(&body)
				.map_err(|e| format!("Failed to deserialize arguments: {}", e))?;
		},
		"url" => quote! {
			let args: #args_struct_name = ::serde_urlencoded::from_str(&body)
				.map_err(|e| format!("Failed to deserialize arguments: {}", e))?;
		},
		"msgpack" => quote! {
			// Decode base64 to bytes
			let bytes = ::base64::Engine::decode(&::base64::engine::general_purpose::STANDARD, &body)
				.map_err(|e| format!("Failed to decode base64: {}", e))?;
			// Deserialize from msgpack bytes
			let args: #args_struct_name = ::rmp_serde::from_slice(&bytes)
				.map_err(|e| format!("Failed to deserialize arguments: {}", e))?;
		},
		// Fixes #843: emit compile error for unknown codec instead of silent fallback
		unknown => {
			let msg = format!(
				"unknown codec '{}'. Valid options: \"json\", \"url\", \"msgpack\"",
				unknown,
			);
			return quote! { compile_error!(#msg); };
		}
	};

	// Generate pre_validate validation code
	let validation_code = if pre_validate {
		let core_crate = get_reinhardt_core_crate();
		quote! {
			#core_crate::validators::Validate::validate(&args)
				.map_err(|e| ::serde_json::to_string(&e).unwrap_or_else(|_| format!("{}", e)))?;
		}
	} else {
		quote! {}
	};

	// Generate codec-specific serialization code for server response
	let serialize_response_code = match codec {
		"json" => quote! {
			::serde_json::to_string(&value)
				.map_err(|e| format!("Failed to serialize response: {}", e))
		},
		"url" => quote! {
			// For URL-encoded codec, response is still JSON
			::serde_json::to_string(&value)
				.map_err(|e| format!("Failed to serialize response: {}", e))
		},
		"msgpack" => quote! {
			// Serialize to msgpack bytes
			let bytes = ::rmp_serde::to_vec(&value)
				.map_err(|e| format!("Failed to serialize response: {}", e))?;
			// Encode as base64 for HTTP transport
			Ok(::base64::Engine::encode(&::base64::engine::general_purpose::STANDARD, &bytes))
		},
		// Fixes #843: emit compile error for unknown codec instead of silent fallback
		unknown => {
			let msg = format!(
				"unknown codec '{}'. Valid options: \"json\", \"url\", \"msgpack\"",
				unknown,
			);
			return quote! { compile_error!(#msg); };
		}
	};

	// Dynamically resolve crate paths for body extraction and registration
	let pages_crate = get_reinhardt_pages_crate();

	// Generate handler signature based on whether DI is needed
	let (handler_signature, handler_body_extraction, wrapper_body_extraction, wrapper_call_args) =
		if !inject_params.is_empty() {
			// Dynamically resolve reinhardt_http crate path
			let http_crate = get_reinhardt_http_crate();

			// When we have inject params, handler receives Request to extract DI context
			(
				quote! {
					pub async fn #handler_name(__req: #http_crate::Request) -> ::std::result::Result<::std::string::String, ::std::string::String>
				},
				// Handler body extraction (from __req parameter) with Content-Type negotiation
				quote! {
					let __content_type = __req.get_header("content-type").unwrap_or_default();
					let body = __req.read_body()
						.map_err(|e| format!("Failed to read body: {}", e))?;
					let body = ::std::string::String::from_utf8(body.to_vec())
						.map_err(|e| format!("Body is not valid UTF-8: {}", e))?;
					let body = #pages_crate::server_fn::convert_body_for_codec(body, &__content_type, #codec)?;
				},
				// Wrapper doesn't extract body when DI is enabled; passes Request directly
				quote! {
					// Pass Request directly to handler (which will read the body)
				},
				vec![quote! { req }],
			)
		} else {
			// No DI needed, handler receives body directly
			(
				quote! {
					pub async fn #handler_name(body: ::std::string::String) -> ::std::result::Result<::std::string::String, ::std::string::String>
				},
				// Handler doesn't need body extraction (body is already a parameter)
				quote! {},
				// Wrapper needs to extract body from req with Content-Type negotiation
				quote! {
					let __content_type = req.get_header("content-type").unwrap_or_default();
					let body = req.read_body()
						.map_err(|e| format!("Failed to read body: {}", e))?;
					let body = ::std::string::String::from_utf8(body.to_vec())
						.map_err(|e| format!("Body is not valid UTF-8: {}", e))?;
					let body = #pages_crate::server_fn::convert_body_for_codec(body, &__content_type, #codec)?;
				},
				vec![quote! { body }],
			)
		};

	// Generate unique name for the static wrapper function
	let static_wrapper_name = quote::format_ident!("__server_fn_static_wrapper_{}", name);
	let name_str = name.to_string();

	// Note: pages_crate is already resolved above for body extraction.
	// http_crate is resolved above when inject_params is not empty,
	// but we need it for the static wrapper regardless
	let http_crate_for_wrapper = get_reinhardt_http_crate();

	// Get visibility for marker struct (same as original function)
	let vis = info.vis();

	// Generate marker module name for `.server_fn(login::marker)` pattern
	// Example: login -> pub mod login { pub struct marker; }
	//
	// This pattern enables `.server_fn(login::marker)` usage with the snake_case
	// function name. The marker struct is defined in a public module with the same
	// name as the function, containing a `marker` struct for registration.
	//
	// Note: We cannot use `pub use` to export the marker with the same name as
	// the function because Rust's value namespace doesn't allow both a function
	// and a `use` item with the same name in the same module.
	let marker_module_name = name.clone();

	// MSW: Generate MockableServerFn impl only when BOTH conditions are met:
	// 1. The macro crate was compiled with `msw` feature (compile-time guard)
	// 2. The consuming crate has `msw` feature enabled (proc-macro expansion-time env var check)
	//
	// The compile-time guard (`cfg!`) avoids the MSW branch during proc-macro expansion
	// when the macro crate is built without `msw`, preventing any possibility of
	// env var leakage from the dependency graph. The env var check handles the
	// case where the macro crate has `msw` but the consuming crate does not.
	// This avoids emitting `#[cfg(feature = "msw")]` in generated code, which
	// would trigger unexpected_cfgs warnings in consuming crates. (Issue #3673, #3700)
	let msw_enabled = cfg!(feature = "msw") && std::env::var("CARGO_FEATURE_MSW").is_ok();

	// MSW: Extract the Ok type from Result<T, ServerFnError> for MockableServerFn::Response
	let response_type = extract_result_ok_type(return_type);

	// Convert inject param names to string literals for INJECTED_PARAMS const
	let inject_param_name_strs: Vec<String> = inject_params
		.iter()
		.map(|p| {
			if let syn::Pat::Ident(pat_ident) = &*p.pat {
				pat_ident.ident.to_string()
			} else {
				"_".to_string()
			}
		})
		.collect();

	// MSW: Generate server-side MockableServerFn tokens only when msw feature is enabled
	let msw_server_tokens = if msw_enabled {
		quote! {
			mod __msw {
				use ::serde::{Serialize, Deserialize};

				/// Public Args struct for MSW type-safe mocking.
				#[derive(Serialize, Deserialize)]
				pub struct Args {
					#(pub #regular_param_names: #regular_param_types),*
				}
			}

			pub use __msw::Args;

			impl #pages_crate::server_fn::MockableServerFn for marker {
				type Args = Args;
				type Response = #response_type;
				const INJECTED_PARAMS: &'static [&'static str] = &[#(#inject_param_name_strs),*];
			}
		}
	} else {
		quote! {}
	};

	// MSW: Generate WASM-side marker module only when msw feature is enabled
	let msw_wasm_tokens = if msw_enabled {
		quote! {
			#[cfg(all(target_family = "wasm", target_os = "unknown"))]
			#vis mod #marker_module_name {
				use ::serde::{Serialize, Deserialize};

				#[doc = concat!("Marker struct for server function `", #name_str, "` (WASM MSW mock target)")]
				pub struct marker;

				/// Public Args struct for MSW type-safe mocking.
				#[derive(Serialize, Deserialize)]
				pub struct Args {
					#(pub #regular_param_names: #regular_param_types),*
				}

				impl #pages_crate::server_fn::MockableServerFn for marker {
					type Args = Args;
					type Response = #response_type;
					const PATH: &'static str = #endpoint;
					const NAME: &'static str = #name_str;
					const CODEC: &'static str = #codec;
					const INJECTED_PARAMS: &'static [&'static str] = &[];
				}
			}
		}
	} else {
		quote! {}
	};

	quote! {
		#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
		/// Server-side handler function
		///
		/// This function is called by the router when the endpoint receives a request.
		/// It deserializes the request body, calls the server function, and serializes the response.
		#handler_signature {
			use ::serde::{Deserialize, Serialize};

			// Argument struct for deserialization (only regular parameters)
			#[derive(Deserialize)]
			struct #args_struct_name {
				#(#regular_param_names: #regular_param_types),*
			}

			// Extract body if needed (when using DI)
			#handler_body_extraction

			// Deserialize request body based on codec
			#deserialize_code

			// Validate deserialized arguments (when pre_validate = true)
			#validation_code

			// Resolve #[inject] parameters via DI
			#di_resolution

			// Call the original server function with both regular and injected parameters
			let result: #return_type = #name(#function_call_params).await;

			// Handle Result and serialize
			match result {
				Ok(value) => {
					#serialize_response_code
				}
				Err(e) => {
					// Serialize the error as ServerFnError
					let error_json = ::serde_json::to_string(&e)
						.map_err(|e| format!("Failed to serialize error: {}", e))?;
					Err(error_json)
				}
			}
		}

		#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
		/// Register this server function with a router
		///
		/// This function should be called during application startup to register
		/// the server function handler with the HTTP router.
		///
		/// # Example
		///
		/// ```text
		/// use reinhardt_pages::server_fn::ServerFnRouterExt;
		/// use reinhardt_urls::routers::ServerRouter;
		///
		/// let router = ServerRouter::new()
		///     .server_fn(get_user);
		/// ```
		pub fn #register_fn_name() -> &'static str {
			#endpoint
		}

		// Static wrapper function for explicit registration
		// This is used by ServerFnRegistration::handler() to provide a function pointer.
		#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
		fn #static_wrapper_name(
			req: #http_crate_for_wrapper::Request
		) -> ::std::pin::Pin<::std::boxed::Box<dyn ::std::future::Future<Output = ::std::result::Result<::std::string::String, ::std::string::String>> + ::std::marker::Send>> {
			::std::boxed::Box::pin(async move {
				// When DI is enabled, pass Request directly
				// When DI is disabled, extract body from Request
				#wrapper_body_extraction
				#handler_name(#(#wrapper_call_args),*).await
			})
		}

		// Public marker module containing `marker` struct for explicit registration
		//
		// This pattern enables `.server_fn(login::marker)` usage with the snake_case
		// function name. The module has the same name as the function and contains
		// a `marker` struct that implements `ServerFnRegistration`.
		//
		// Example:
		// ```ignore
		// use reinhardt::pages::server_fn::ServerFnRouterExt;
		// use crate::server_fn::auth::{login, logout};  // Import marker modules
		//
		// let router = UnifiedRouter::new()
		//     .server_fn(login::marker)   // Use snake_case name + ::marker
		//     .server_fn(logout::marker);
		// ```
		//
		// Note: On WASM (client side), import and call the function directly:
		// ```ignore
		// use crate::server_fn::auth::login;  // Function (snake_case)
		// login(email, password).await;
		// ```
		#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
		#vis mod #marker_module_name {
			use super::*;

			#[doc = concat!("Marker struct for server function `", #name_str, "` (use with `.server_fn()`)")]
			pub struct marker;

			// Implement ServerFnRegistration for explicit router registration
			impl #pages_crate::server_fn::ServerFnRegistration for marker {
				const PATH: &'static str = #endpoint;
				const NAME: &'static str = #name_str;
				const CODEC: &'static str = #codec;

				fn handler() -> #pages_crate::server_fn::ServerFnHandler {
					super::#static_wrapper_name
				}
			}

			// MSW: server-side MockableServerFn (conditionally generated; Issue #3673)
			#msw_server_tokens
		}

		// MSW: WASM-side marker module (conditionally generated; Issue #3673)
		#msw_wasm_tokens
	}
}

/// Extracts the first generic argument `T` from `Result<T, E>`.
///
/// Given `Result<User, ServerFnError>`, returns the token stream for `User`.
/// Falls back to the full return type if it cannot be parsed as `Result<T, E>`.
fn extract_result_ok_type(return_type: &syn::Type) -> proc_macro2::TokenStream {
	if let syn::Type::Path(type_path) = return_type
		&& let Some(segment) = type_path.path.segments.last()
		&& segment.ident == "Result"
		&& let syn::PathArguments::AngleBracketed(args) = &segment.arguments
		&& let Some(syn::GenericArgument::Type(ok_type)) = args.args.first()
	{
		return quote! { #ok_type };
	}
	// Fallback: use the full type
	quote! { #return_type }
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_server_fn_options_default() {
		let options = ServerFnOptions::default();
		assert!(!options.use_inject);
		assert_eq!(options.endpoint, None);
		assert_eq!(options.codec, "json");
	}

	#[test]
	fn test_server_fn_options_parse() {
		use darling::FromMeta;
		use darling::ast::NestedMeta;
		use syn::parse_quote;

		// Test with endpoint only (use_inject is no longer needed)
		let attr: syn::Attribute = parse_quote!(#[server_fn(endpoint = "/custom")]);
		let meta_list = attr.meta.require_list().unwrap();
		let nested: Vec<NestedMeta> = NestedMeta::parse_meta_list(meta_list.tokens.clone())
			.unwrap()
			.into_iter()
			.collect();
		let options = ServerFnOptions::from_list(&nested).unwrap();

		assert!(!options.use_inject);
		assert_eq!(options.endpoint, Some("/custom".to_string()));
		assert_eq!(options.codec, "json");
	}

	#[test]
	fn test_server_fn_options_parse_deprecated_use_inject() {
		use darling::FromMeta;
		use darling::ast::NestedMeta;
		use syn::parse_quote;

		// use_inject = true is still accepted (deprecated but functional)
		let attr: syn::Attribute =
			parse_quote!(#[server_fn(use_inject = true, endpoint = "/custom")]);
		let meta_list = attr.meta.require_list().unwrap();
		let nested: Vec<NestedMeta> = NestedMeta::parse_meta_list(meta_list.tokens.clone())
			.unwrap()
			.into_iter()
			.collect();
		let options = ServerFnOptions::from_list(&nested).unwrap();

		assert!(options.use_inject);
		assert_eq!(options.endpoint, Some("/custom".to_string()));
		assert_eq!(options.codec, "json");
	}

	#[test]
	fn test_validate_endpoint_valid_path() {
		assert!(validate_endpoint_path("/api/users").is_ok());
		assert!(validate_endpoint_path("/api/server_fn/create_user").is_ok());
		assert!(validate_endpoint_path("/").is_ok());
	}

	#[test]
	fn test_validate_endpoint_rejects_no_leading_slash() {
		let result = validate_endpoint_path("api/users");
		assert!(result.is_err());
		let err = result.unwrap_err().to_string();
		assert!(err.contains("must start with '/'"));
	}

	#[test]
	fn test_validate_endpoint_rejects_full_url() {
		let result = validate_endpoint_path("http://example.com/api");
		assert!(result.is_err());
		let err = result.unwrap_err().to_string();
		assert!(err.contains("not a full URL"));

		let result = validate_endpoint_path("https://example.com/api");
		assert!(result.is_err());
	}

	#[test]
	fn test_validate_endpoint_rejects_traversal() {
		let result = validate_endpoint_path("/api/../secret");
		assert!(result.is_err());
		let err = result.unwrap_err().to_string();
		assert!(err.contains("path traversal"));
	}

	#[test]
	fn test_validate_endpoint_rejects_query_string() {
		let result = validate_endpoint_path("/api/users?admin=true");
		assert!(result.is_err());
		let err = result.unwrap_err().to_string();
		assert!(err.contains("query strings"));
	}

	#[test]
	fn test_validate_endpoint_rejects_fragment() {
		let result = validate_endpoint_path("/api/users#section");
		assert!(result.is_err());
		let err = result.unwrap_err().to_string();
		assert!(err.contains("fragment identifiers"));
	}
}
