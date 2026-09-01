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
use syn::ext::IdentExt;
use syn::punctuated::Punctuated;
use syn::{FnArg, ItemFn, Meta, Token, parse_macro_input};

// Import crate path helpers for dynamic resolution
use crate::crate_paths::{
	CratePathInfo, get_reinhardt_core_crate, get_reinhardt_di_crate, get_reinhardt_http_crate,
	get_reinhardt_pages_crate, get_reinhardt_pages_crate_info,
};

fn generate_inject_resolver_expr(
	di_crate: &proc_macro2::TokenStream,
	ty: &syn::Type,
	ctx: proc_macro2::TokenStream,
	use_cache: bool,
) -> proc_macro2::TokenStream {
	quote! {
		{
			use #di_crate::{
				__InjectFallbackResolver as _,
				__InjectWrapperResolver as _,
			};
			#di_crate::__InjectResolver::<#ty>::new()
				.__resolve_inject_parameter(#ctx, #use_cache)
				.await
		}
	}
}

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
	let pascal_name = ident.unraw().to_string().to_case(Case::Pascal);
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

	/// Enable native HTML model-form payload decoding for this endpoint.
	///
	/// This is deliberately opt-in because a normal JSON server function may
	/// also use a single parameter named `payload`.
	pub model_form: bool,
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
			model_form: false,
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

/// Information about `FromRequest` extractor parameters
///
/// This struct holds metadata about parameters that should be resolved
/// via `FromRequest::from_request(&req, &ctx)` on the server side.
/// These are excluded from the Args struct (like `#[inject]` params).
#[derive(Debug, Clone)]
struct ExtractorInfo {
	/// Parameter pattern (e.g., `form`, `header`)
	pat: Box<syn::Pat>,
	/// Parameter type (e.g., `Validated<Form<LoginRequest>>`, `Header<String>`)
	ty: Box<syn::Type>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum WireParamKind {
	Json,
	File,
	OptionalFile,
}

#[derive(Clone, Debug)]
struct WireParam {
	name: syn::Ident,
	kind: WireParamKind,
}

fn wire_param_name(parameter: &WireParam) -> String {
	parameter.name.unraw().to_string()
}

/// Known `FromRequest` extractor type names.
///
/// When a parameter's outermost type matches one of these names, it is
/// treated as an extractor and resolved via `FromRequest::from_request`.
const KNOWN_EXTRACTOR_TYPES: &[&str] = &[
	"Validated",
	"Json",
	"Form",
	"Header",
	"HeaderNamed",
	"HeaderStruct",
	"Cookie",
	"CookieNamed",
	"CookieStruct",
	"Path",
	"PathStruct",
	"Query",
	"Body",
	"Multipart",
	"Authorization",
	"ContentType",
	"SessionId",
	"CsrfToken",
	"PolicyPrincipal",
];

/// Check if a type is a known `FromRequest` extractor.
///
/// Returns `true` if the outermost type segment matches one of the
/// known extractor names from `reinhardt_di::params`.
pub(crate) fn is_extractor_type(ty: &syn::Type) -> bool {
	if let syn::Type::Path(type_path) = ty
		&& let Some(last_seg) = type_path.path.segments.last()
	{
		let name = last_seg.ident.to_string();
		if name == "PolicyPrincipal" {
			return is_model_policy_principal_type(ty);
		}
		return KNOWN_EXTRACTOR_TYPES.contains(&name.as_str());
	}
	false
}

fn is_body_consuming_extractor(ty: &syn::Type) -> bool {
	let syn::Type::Path(type_path) = ty else {
		return false;
	};
	let Some(segment) = type_path.path.segments.last() else {
		return false;
	};
	match segment.ident.to_string().as_str() {
		"Body" | "Form" | "Json" | "Multipart" => true,
		"Validated" => {
			let syn::PathArguments::AngleBracketed(arguments) = &segment.arguments else {
				return false;
			};
			arguments.args.iter().find_map(|argument| {
				let syn::GenericArgument::Type(inner) = argument else {
					return None;
				};
				Some(is_body_consuming_extractor(inner))
			}) == Some(true)
		}
		_ => false,
	}
}

fn is_model_policy_principal_type(ty: &syn::Type) -> bool {
	let syn::Type::Path(type_path) = ty else {
		return false;
	};
	if type_path.path.leading_colon.is_some() || type_path.path.segments.len() != 1 {
		return false;
	}
	let Some(segment) = type_path.path.segments.last() else {
		return false;
	};
	if segment.ident != "PolicyPrincipal" {
		return false;
	}
	let syn::PathArguments::AngleBracketed(arguments) = &segment.arguments else {
		return false;
	};
	let Some(syn::GenericArgument::Type(syn::Type::Path(resource))) = arguments.args.first() else {
		return false;
	};
	resource.path.segments.last().is_some_and(|segment| {
		let name = segment.ident.to_string();
		name.ends_with("Resource") || name.starts_with("__ReinhardtServerFnSetResource")
	})
}

/// Detect parameters that implement `FromRequest` (extractors).
///
/// Scans function parameters and identifies those whose type matches a known
/// extractor. These parameters are excluded from the Args struct on the client
/// side and resolved via `FromRequest::from_request` on the server side.
///
/// # Examples
///
/// ```ignore
/// async fn handler(
///     name: String,                             // Regular (Args)
///     form: Validated<Form<LoginRequest>>,      // Extractor
///     auth: Header<String>,                     // Extractor
///     #[inject] db: Database,                   // DI (handled separately)
/// ) -> Result<(), ServerFnError>
/// ```
fn detect_extractor_params(inputs: &Punctuated<FnArg, Token![,]>) -> Vec<ExtractorInfo> {
	let mut extractor_params = Vec::new();

	for input in inputs {
		if let FnArg::Typed(pat_type) = input {
			// Skip #[inject] params — those are handled by DI, not FromRequest
			let has_inject_attr = pat_type.attrs.iter().any(is_inject_attr);
			if has_inject_attr {
				continue;
			}

			if is_extractor_type(&pat_type.ty) {
				extractor_params.push(ExtractorInfo {
					pat: pat_type.pat.clone(),
					ty: pat_type.ty.clone(),
				});
			}
		}
	}

	extractor_params
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
	codec_explicit: bool,
	metadata_name: Option<String>,
	endpoint_tokens: Option<proc_macro2::TokenStream>,
	metadata_name_tokens: Option<proc_macro2::TokenStream>,
	detail: bool,
	transactional: bool,
	structured_error: bool,
}

impl ServerFnInfo {
	/// Parse from macro input
	fn parse(args: Vec<Meta>, func: ItemFn) -> Result<Self, darling::Error> {
		let codec_explicit = args.iter().any(
			|argument| matches!(argument, Meta::NameValue(value) if value.path.is_ident("codec")),
		);
		// Convert Meta to NestedMeta for darling compatibility
		let nested: Vec<NestedMeta> = args.into_iter().map(NestedMeta::Meta).collect();
		let options = ServerFnOptions::from_list(&nested)?;

		// Validate endpoint path if explicitly specified
		if let Some(ref endpoint) = options.endpoint {
			validate_endpoint_path(endpoint)?;
		}

		Ok(Self {
			func,
			options,
			codec_explicit,
			metadata_name: None,
			endpoint_tokens: None,
			metadata_name_tokens: None,
			detail: false,
			transactional: false,
			structured_error: false,
		})
	}

	/// Get the function name
	fn name(&self) -> &syn::Ident {
		&self.func.sig.ident
	}

	/// Get the function visibility
	fn vis(&self) -> &syn::Visibility {
		&self.func.vis
	}

	fn allows_private_interfaces(&self) -> bool {
		self.func
			.attrs
			.iter()
			.any(attribute_allows_private_interfaces)
	}

	fn emits_typed_response_metadata(&self) -> bool {
		!self.allows_private_interfaces() && !matches!(self.vis(), syn::Visibility::Restricted(_))
	}

	/// Get the endpoint path
	///
	/// Returns the custom endpoint if specified, otherwise generates default.
	fn endpoint(&self) -> proc_macro2::TokenStream {
		self.endpoint_tokens.clone().unwrap_or_else(|| {
			let endpoint = self
				.options
				.endpoint
				.clone()
				.unwrap_or_else(|| format!("/api/server_fn/{}", self.name()));
			quote!(#endpoint)
		})
	}

	/// Get the codec name
	fn codec(&self) -> &str {
		&self.options.codec
	}

	fn metadata_name(&self) -> proc_macro2::TokenStream {
		self.metadata_name_tokens.clone().unwrap_or_else(|| {
			let name = self
				.metadata_name
				.clone()
				.unwrap_or_else(|| self.name().to_string());
			quote!(#name)
		})
	}

	/// Check if the deprecated `use_inject` option is enabled (for deprecation warning)
	fn use_inject_enabled(&self) -> bool {
		self.options.use_inject
	}
}

/// Generate one server function for a sibling proc-macro expansion.
pub(crate) fn generate_internal_server_fn(
	func: ItemFn,
	endpoint: String,
	metadata_name: String,
	detail: bool,
	transactional: bool,
) -> proc_macro2::TokenStream {
	let info = ServerFnInfo {
		func,
		options: ServerFnOptions {
			endpoint: Some(endpoint),
			..ServerFnOptions::default()
		},
		codec_explicit: false,
		metadata_name: Some(metadata_name),
		endpoint_tokens: None,
		metadata_name_tokens: None,
		detail,
		transactional,
		structured_error: true,
	};
	generate_server_fn(&info)
}

/// Generate one server function whose endpoint metadata is supplied as Rust expressions.
pub(crate) fn generate_internal_server_fn_with_tokens(
	func: ItemFn,
	endpoint: proc_macro2::TokenStream,
	metadata_name: proc_macro2::TokenStream,
	detail: bool,
	transactional: bool,
) -> proc_macro2::TokenStream {
	let info = ServerFnInfo {
		func,
		options: ServerFnOptions::default(),
		codec_explicit: false,
		metadata_name: None,
		endpoint_tokens: Some(endpoint),
		metadata_name_tokens: Some(metadata_name),
		detail,
		transactional,
		structured_error: true,
	};
	generate_server_fn(&info)
}

fn attribute_allows_private_interfaces(attr: &syn::Attribute) -> bool {
	if !attr.path().is_ident("allow") {
		return false;
	}
	let mut found = false;
	let _ = attr.parse_nested_meta(|meta| {
		if meta.path.is_ident("private_interfaces") {
			found = true;
		}
		Ok(())
	});
	found
}

fn marker_struct_visibility(vis: &syn::Visibility) -> proc_macro2::TokenStream {
	match vis {
		syn::Visibility::Public(_) => quote! { pub },
		syn::Visibility::Restricted(restricted) => {
			let path = marker_struct_restricted_visibility_path(&restricted.path);
			if path.is_ident("crate") {
				quote! { pub(crate) }
			} else {
				quote! { pub(in #path) }
			}
		}
		syn::Visibility::Inherited => quote! { pub(super) },
	}
}

fn marker_struct_restricted_visibility_path(path: &syn::Path) -> syn::Path {
	let mut marker_path = path.clone();
	if marker_path.leading_colon.is_none()
		&& let Some(first_segment) = marker_path.segments.first_mut()
	{
		if first_segment.ident == "crate" {
			return marker_path;
		}
		if first_segment.ident == "self" {
			first_segment.ident = quote::format_ident!("super");
			return marker_path;
		}
		if first_segment.ident == "super" {
			return syn::parse_quote!(super::#marker_path);
		}
	}
	marker_path
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

fn regular_server_fn_params(inputs: &Punctuated<FnArg, Token![,]>) -> Vec<&syn::PatType> {
	inputs
		.iter()
		.filter_map(|arg| {
			if let syn::FnArg::Typed(pat_type) = arg {
				let has_inject = pat_type.attrs.iter().any(is_inject_attr);
				if has_inject || is_extractor_type(&pat_type.ty) {
					return None;
				}
				Some(pat_type)
			} else {
				None
			}
		})
		.collect()
}

fn classify_wire_params(
	inputs: &Punctuated<FnArg, Token![,]>,
	codec: &str,
	codec_explicit: bool,
) -> Result<Vec<WireParam>, syn::Error> {
	let mut parameters = Vec::new();

	for parameter in regular_server_fn_params(inputs) {
		let syn::Pat::Ident(pattern) = parameter.pat.as_ref() else {
			return Err(syn::Error::new_spanned(
				&parameter.pat,
				"server_fn client-visible parameters must use identifier patterns",
			));
		};

		let kind = if is_uploaded_file_type(&parameter.ty) {
			WireParamKind::File
		} else if is_optional_uploaded_file_type(&parameter.ty) {
			WireParamKind::OptionalFile
		} else if contains_uploaded_file_type(&parameter.ty) {
			return Err(syn::Error::new_spanned(
				&parameter.ty,
				"server_fn file arguments support only UploadedFile or Option<UploadedFile>",
			));
		} else {
			WireParamKind::Json
		};

		parameters.push(WireParam {
			name: pattern.ident.clone(),
			kind,
		});
	}

	if codec_explicit
		&& matches!(codec, "json" | "url" | "msgpack")
		&& parameters
			.iter()
			.any(|parameter| !matches!(parameter.kind, WireParamKind::Json))
	{
		return Err(syn::Error::new(
			proc_macro2::Span::call_site(),
			"server_fn file arguments infer multipart framing and cannot use an explicit codec",
		));
	}

	Ok(parameters)
}

fn is_uploaded_file_type(ty: &syn::Type) -> bool {
	let syn::Type::Path(type_path) = ty else {
		return false;
	};
	let segments = &type_path.path.segments;
	let Some(segment) = segments.last() else {
		return false;
	};
	segment.ident == "UploadedFile"
		&& matches!(segment.arguments, syn::PathArguments::None)
		&& (segments.len() == 1
			|| segments.iter().rev().nth(1).is_some_and(|segment| {
				matches!(segment.ident.to_string().as_str(), "parsers" | "parser")
			}))
}

fn is_option_type(ty: &syn::Type) -> bool {
	let syn::Type::Path(type_path) = ty else {
		return false;
	};
	type_path
		.path
		.segments
		.last()
		.is_some_and(|segment| segment.ident == "Option")
}

fn is_optional_uploaded_file_type(ty: &syn::Type) -> bool {
	let syn::Type::Path(type_path) = ty else {
		return false;
	};
	let Some(segment) = type_path.path.segments.last() else {
		return false;
	};
	if !is_option_type(ty) {
		return false;
	}
	let syn::PathArguments::AngleBracketed(arguments) = &segment.arguments else {
		return false;
	};
	if arguments.args.len() != 1 {
		return false;
	}
	matches!(arguments.args.first(), Some(syn::GenericArgument::Type(inner)) if is_uploaded_file_type(inner))
}

fn contains_uploaded_file_type(ty: &syn::Type) -> bool {
	if is_uploaded_file_type(ty) {
		return true;
	}

	match ty {
		syn::Type::Array(array) => contains_uploaded_file_type(&array.elem),
		syn::Type::BareFn(function) => {
			function
				.inputs
				.iter()
				.any(|input| contains_uploaded_file_type(&input.ty))
				|| matches!(&function.output, syn::ReturnType::Type(_, ty) if contains_uploaded_file_type(ty))
		}
		syn::Type::Group(group) => contains_uploaded_file_type(&group.elem),
		syn::Type::ImplTrait(impl_trait) => bounds_contain_uploaded_file_type(&impl_trait.bounds),
		syn::Type::Paren(parenthesized) => contains_uploaded_file_type(&parenthesized.elem),
		syn::Type::Path(path) => path
			.path
			.segments
			.iter()
			.any(|segment| path_arguments_contain_uploaded_file_type(&segment.arguments)),
		syn::Type::Ptr(pointer) => contains_uploaded_file_type(&pointer.elem),
		syn::Type::Reference(reference) => contains_uploaded_file_type(&reference.elem),
		syn::Type::Slice(slice) => contains_uploaded_file_type(&slice.elem),
		syn::Type::TraitObject(trait_object) => {
			bounds_contain_uploaded_file_type(&trait_object.bounds)
		}
		syn::Type::Tuple(tuple) => tuple.elems.iter().any(contains_uploaded_file_type),
		_ => false,
	}
}

fn bounds_contain_uploaded_file_type(
	bounds: &Punctuated<syn::TypeParamBound, syn::Token![+]>,
) -> bool {
	bounds.iter().any(|bound| {
		matches!(bound, syn::TypeParamBound::Trait(trait_bound) if trait_bound.path.segments.iter().any(|segment| path_arguments_contain_uploaded_file_type(&segment.arguments)))
	})
}

fn path_arguments_contain_uploaded_file_type(arguments: &syn::PathArguments) -> bool {
	match arguments {
		syn::PathArguments::AngleBracketed(arguments) => arguments
			.args
			.iter()
			.any(generic_argument_contains_uploaded_file),
		syn::PathArguments::Parenthesized(arguments) => {
			arguments.inputs.iter().any(contains_uploaded_file_type)
				|| matches!(&arguments.output, syn::ReturnType::Type(_, ty) if contains_uploaded_file_type(ty))
		}
		syn::PathArguments::None => false,
	}
}

fn generic_argument_contains_uploaded_file(argument: &syn::GenericArgument) -> bool {
	match argument {
		syn::GenericArgument::Type(ty) => contains_uploaded_file_type(ty),
		syn::GenericArgument::AssocType(association) => {
			contains_uploaded_file_type(&association.ty)
		}
		syn::GenericArgument::Constraint(constraint) => {
			bounds_contain_uploaded_file_type(&constraint.bounds)
		}
		_ => false,
	}
}

fn add_native_mock_probe(
	info: &ServerFnInfo,
	clean_func: &ItemFn,
	regular_params: &[&syn::PatType],
	pages_crate_info: &CratePathInfo,
) -> Result<ItemFn, proc_macro2::TokenStream> {
	let mut param_idents = Vec::new();
	for param in regular_params {
		let syn::Pat::Ident(pat_ident) = &*param.pat else {
			return Err(quote! {
				compile_error!("server_fn component-test mocks require identifier parameters");
			});
		};
		param_idents.push(pat_ident.ident.clone());
	}

	let mut func = clean_func.clone();
	let pages_use_statement = &pages_crate_info.use_statement;
	let pages_crate = &pages_crate_info.ident;
	let name = info.name();
	let original_block = func.block;
	let native_mock_probe = if cfg!(feature = "msw") {
		quote! {
			{
				// Generated server functions may expand into consumer crates that do not
				// declare an `msw` feature, even when the dependency feature is active.
				#![allow(unexpected_cfgs)]

				#[cfg(all(native, feature = "msw"))]
				{
					if #pages_crate::server_fn::has_active_server_fn_mock_scope() {
						let __args = #name::Args {
							#(#param_idents: #param_idents),*
						};
						let __mock_result =
							#pages_crate::server_fn::try_call_active_mock::<#name::marker>(__args)
								.expect("active server-function mock scope must return a result");
						match __mock_result {
							Ok(__mock_value) => return Ok(__mock_value),
							Err(__mock_error) => {
								return Err(::std::convert::Into::into(__mock_error));
							}
						}
					}
				}
			}
		}
	} else {
		quote! {}
	};
	func.block = Box::new(syn::parse_quote!({
		#pages_use_statement
		#native_mock_probe
		#original_block
	}));
	Ok(func)
}

/// Generate server function code
///
/// This generates both client and server code with conditional compilation.
/// `#[inject]` parameters are always auto-detected, matching the behavior of
/// route macros (`#[get]`, `#[post]`, etc.).
fn generate_server_fn(info: &ServerFnInfo) -> proc_macro2::TokenStream {
	let func = &info.func;
	let wire_params =
		match classify_wire_params(&func.sig.inputs, info.codec(), info.codec_explicit) {
			Ok(parameters) => parameters,
			Err(error) => return error.to_compile_error(),
		};
	let uses_multipart = wire_params
		.iter()
		.any(|parameter| !matches!(parameter.kind, WireParamKind::Json));

	// Auto-detect #[inject] parameters unconditionally
	let inject_params = detect_inject_params(&func.sig.inputs);

	// Auto-detect FromRequest extractor parameters
	let extractor_params = detect_extractor_params(&func.sig.inputs);
	if uses_multipart
		&& let Some(extractor) = extractor_params
			.iter()
			.find(|extractor| is_body_consuming_extractor(&extractor.ty))
	{
		return syn::Error::new_spanned(
			&extractor.ty,
			"server_fn multipart arguments cannot be combined with body-consuming extractors",
		)
		.to_compile_error();
	}

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
	let regular_params = regular_server_fn_params(&func.sig.inputs);
	let native_clean_func = if uses_multipart {
		quote! { #clean_func }
	} else {
		match add_native_mock_probe(info, &clean_func, &regular_params, &pages_crate_info) {
			Ok(func) => quote! { #func },
			Err(err) => err,
		}
	};

	// Generate client stub (with DI and extractor parameter filtering)
	let client_stub = generate_client_stub(
		info,
		&inject_params,
		&extractor_params,
		&pages_crate_info,
		&wire_params,
	);

	// Generate server handler (with DI and extractor resolution)
	let server_handler =
		generate_server_handler(info, &inject_params, &extractor_params, &wire_params);

	quote! {
		// Deprecation warning for use_inject = true (if specified)
		#deprecation_warning

		// Server-side: Original function (with #[inject] attributes removed)
		#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
		#native_clean_func

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
///     let body = serde_json::to_string(&args)?;
///     let response = reinhardt_pages::__private::fetch::request_with_credentials(
///         "POST",
///         url,
///         Some(&body),
///         vec![("Content-Type".to_string(), "application/json".to_string())],
///         reinhardt_pages::__private::fetch::FetchCredentials::Include,
///     )
///     .await?;
///     response.json()
/// }
/// ```
fn generate_client_stub(
	info: &ServerFnInfo,
	_inject_params: &[InjectInfo],
	_extractor_params: &[ExtractorInfo],
	pages_crate_info: &CratePathInfo,
	wire_params: &[WireParam],
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

	// Extract function parameters, excluding #[inject] and extractor parameters.
	// Client-side doesn't need DI or extractor parameters - they're resolved on the server.
	let params: Vec<_> = sig
		.inputs
		.iter()
		.filter_map(|arg| {
			if let syn::FnArg::Typed(pat_type) = arg {
				// Skip #[inject] parameters
				let has_inject = pat_type.attrs.iter().any(is_inject_attr);
				if has_inject {
					return None;
				}
				// Skip FromRequest extractor parameters
				if is_extractor_type(&pat_type.ty) {
					return None;
				}
				Some(pat_type)
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
		let mut client_params: Vec<syn::PatType> =
			params.iter().map(|param| (*param).clone()).collect();
		for (parameter, wire_param) in client_params.iter_mut().zip(wire_params) {
			parameter.ty = match wire_param.kind {
				WireParamKind::Json => parameter.ty.clone(),
				WireParamKind::File => {
					Box::new(syn::parse_quote!(#pages_crate::__private::web_sys::File))
				}
				WireParamKind::OptionalFile => Box::new(syn::parse_quote!(
					::std::option::Option<#pages_crate::__private::web_sys::File>
				)),
			};
		}
		// Replace inputs with filtered params (without #[inject])
		new_sig.inputs = params
			.iter()
			.zip(client_params)
			.map(|(_, parameter)| syn::FnArg::Typed(parameter))
			.collect();
		new_sig
	};
	let uses_multipart = wire_params
		.iter()
		.any(|parameter| !matches!(parameter.kind, WireParamKind::Json));
	let csrf_enabled = !info.options.no_csrf;

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
				__headers.push((__csrf_header_name.to_string(), __csrf_header_value));
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
			__headers.push((__auth_header_name.to_string(), __auth_header_value));
		}
	};
	let error_decode_code = if info.structured_error {
		quote! {
			return Err(#pages_crate::server_fn::ServerFnSetError::from_http_error(
				__status,
				&__message,
			));
		}
	} else {
		quote! {
			return Err(#pages_crate::server_fn::ServerFnError::from_http_response(
				__status,
				&__message,
			).into());
		}
	};
	if uses_multipart {
		let multipart_append_code = wire_params.iter().map(|parameter| {
			let name = &parameter.name;
			let field_name = wire_param_name(parameter);
			match parameter.kind {
				WireParamKind::Json => quote! {
					let __value = ::serde_json::to_string(&#name)
						.map_err(|error| #pages_crate::server_fn::ServerFnError::serialization(error.to_string()))?;
					__form_data
						.append_with_str(#field_name, &__value)
						.map_err(|error| #pages_crate::server_fn::ServerFnError::network(format!("{error:?}")))?;
				},
				WireParamKind::File => quote! {
					__form_data
						.append_with_blob(#field_name, &#name)
						.map_err(|error| #pages_crate::server_fn::ServerFnError::network(format!("{error:?}")))?;
				},
				WireParamKind::OptionalFile => quote! {
					if let Some(__file) = #name {
						__form_data
							.append_with_blob(#field_name, &__file)
							.map_err(|error| #pages_crate::server_fn::ServerFnError::network(format!("{error:?}")))?;
					}
				},
			}
		});

		return quote! {
			#[cfg(all(target_family = "wasm", target_os = "unknown"))]
			#vis #client_sig {
				#pages_use_statement

				let __form_data = #pages_crate::__private::web_sys::FormData::new()
					.map_err(|error| #pages_crate::server_fn::ServerFnError::network(format!("{error:?}")))?;
				#(#multipart_append_code)*

				let __response = #pages_crate::server_fn::request_multipart(
					#endpoint,
					__form_data,
					#csrf_enabled,
				)
					.await?;

				if !__response.is_success() {
					let __status = __response.status();
					let __message = __response.into_text();
					#error_decode_code
				}

				__response.json().map_err(::std::convert::Into::into)
			}
		};
	}

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
			},
		),
		"url" => (
			"application/x-www-form-urlencoded",
			quote! {
				let __body = ::serde_urlencoded::to_string(&__args)
					.map_err(|e| #pages_crate::server_fn::ServerFnError::serialization(e.to_string()))?;
			},
			quote! {
				let __text = __response.into_text();
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
				let __text = __response.into_text();
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
			use ::serde::Serialize;

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

			let mut __headers: ::std::vec::Vec<(::std::string::String, ::std::string::String)> =
				::std::vec![("Content-Type".to_string(), #content_type.to_string())];

			#csrf_injection_code
			#auth_injection_code

			// Send request with credentials for cookie-backed server function sessions.
			let __response = #pages_crate::__private::fetch::request_with_credentials(
					"POST",
					&__endpoint,
					Some(&__body),
					__headers,
					#pages_crate::__private::fetch::FetchCredentials::Include,
				)
				.await
				?;

			// Check HTTP status
			if !__response.is_success() {
				let __status = __response.status();
				let __message = __response.into_text();
				#error_decode_code
			}

			// Deserialize response based on codec
			{
				#deserialize_code
			}
			.map_err(::std::convert::Into::into)
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
	extractor_params: &[ExtractorInfo],
	wire_params: &[WireParam],
) -> proc_macro2::TokenStream {
	let name = info.name();
	let endpoint = info.endpoint();
	let codec = info.codec();
	let pre_validate = info.options.pre_validate;
	let func = &info.func;
	let sig = &func.sig;

	// Extract function parameters, separating regular, #[inject], and extractor parameters.
	// Regular params go into the Args deserialization struct.
	// #[inject] params are resolved via DI.
	// Extractor params are resolved via FromRequest::from_request.
	let regular_params: Vec<_> = sig
		.inputs
		.iter()
		.filter_map(|arg| {
			if let syn::FnArg::Typed(pat_type) = arg {
				let has_inject = pat_type.attrs.iter().any(is_inject_attr);
				if has_inject {
					return None;
				}
				// Extractor params are excluded from Args struct
				if is_extractor_type(&pat_type.ty) {
					return None;
				}
				Some(pat_type)
			} else {
				None
			}
		})
		.collect();

	let regular_param_names: Vec<_> = regular_params.iter().map(|p| &p.pat).collect();
	let regular_param_types: Vec<_> = regular_params.iter().map(|p| &p.ty).collect();
	let uses_multipart = wire_params
		.iter()
		.any(|parameter| !matches!(parameter.kind, WireParamKind::Json));
	let pages_crate = get_reinhardt_pages_crate();
	let multipart_param_names: Vec<_> = (0..wire_params.len())
		.map(|index| {
			proc_macro2::Ident::new(
				&format!("__server_fn_argument_{}", index),
				proc_macro2::Span::mixed_site(),
			)
		})
		.collect();
	let multipart_argument_extraction = wire_params
		.iter()
		.zip(&regular_param_types)
		.zip(&multipart_param_names)
		.map(|((parameter, ty), ident)| {
			let name = wire_param_name(parameter);
			let take = match parameter.kind {
				WireParamKind::Json if is_option_type(ty) => quote! { take_optional_json },
				WireParamKind::Json => quote! { take_json },
				WireParamKind::File => quote! { take_file },
				WireParamKind::OptionalFile => quote! { take_optional_file },
			};
			quote! {
				let #ident: #ty = arguments
					.#take(#name)
					.map_err(|_| __invalid_request_error())?;
			}
		});

	// Injected source patterns belong to the implementation signature. Generated
	// calls use hygienic identifiers so mutable and destructuring patterns are
	// never emitted in expression position or shadowed by call-site bindings.
	let inject_param_names: Vec<_> = (0..inject_params.len())
		.map(|index| {
			proc_macro2::Ident::new(
				&format!("__server_fn_inject_{}", index),
				proc_macro2::Span::mixed_site(),
			)
		})
		.collect();

	let extractor_param_names: Vec<_> = if uses_multipart {
		(0..extractor_params.len())
			.map(|index| {
				let ident = proc_macro2::Ident::new(
					&format!("__server_fn_extractor_{}", index),
					proc_macro2::Span::mixed_site(),
				);
				quote! { #ident }
			})
			.collect()
	} else {
		extractor_params
			.iter()
			.map(|parameter| {
				let pat = &parameter.pat;
				quote! { #pat }
			})
			.collect()
	};

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

	// Generate DI resolution code. Runtime trait dispatch resolves
	// `InjectableType` wrappers from the registry and falls back to normal
	// `Injectable` values for non-wrapper parameters.
	let di_resolution = if !inject_params.is_empty() {
		// Dynamically resolve crate paths
		let di_crate = get_reinhardt_di_crate();
		let pages_crate_for_di = get_reinhardt_pages_crate();

		let param_resolutions: Vec<_> = inject_params
			.iter()
			.zip(&inject_param_names)
			.map(|(p, resolved_ident)| {
				let ty = &p.ty;
				let resolve_expr =
					generate_inject_resolver_expr(&di_crate, ty, quote! { &__di_ctx }, true);
				quote! {
					let #resolved_ident: #ty =
						#resolve_expr
							.map_err(|e| {
								// Auth errors (401/403) expose framework-provided user-facing
								// messages. Any other DI failure is treated as an internal
								// error: the detailed cause is logged server-side, and the
								// client receives a generic message to avoid leaking internals.
								let (status, msg) = match &e {
									#di_crate::DiError::Authentication(m) => (401u16, m.clone()),
									#di_crate::DiError::Authorization(m) => (403u16, m.clone()),
									other => {
										#pages_crate_for_di::__private::tracing::error!(
											error = ?other,
											param = stringify!(#ty),
											"Dependency injection failed",
										);
										(500u16, "Internal server error".to_string())
									}
								};
								let server_err = #pages_crate_for_di::server_fn::ServerFnError::server(status, msg);
								::serde_json::to_string(&server_err)
									.unwrap_or_else(|_| "Internal server error".to_string())
							})?;
				}
			})
			.collect();

		quote! {
			// Get DI context from request and fork for per-request isolation
			let __di_ctx = {
				let __shared_ctx = __req.get_di_context::<::std::sync::Arc<#di_crate::InjectionContext>>()
					.ok_or_else(|| "DI context not set. Ensure the router is configured with .with_di_context()".to_string())?;
				let __di_request = __req.clone_for_di();
				::std::sync::Arc::new((*__shared_ctx).fork_for_request(__di_request))
			};

			// Resolve each #[inject] parameter
			#(#param_resolutions)*
		}
	} else {
		quote! {}
	};

	// Generate FromRequest extractor resolution code.
	//
	// For server functions that have extractor params, we need access to the
	// request object (__req). The handler signature already uses Request when
	// inject_params is non-empty; when only extractor_params are present, we
	// still need to ensure the handler receives a Request.
	let extractor_resolution = if !extractor_params.is_empty() {
		let di_crate = get_reinhardt_di_crate();
		let pages_crate_for_ext = get_reinhardt_pages_crate();
		let extractor_error = if info.structured_error {
			quote! {
				match e {
					#di_crate::params::ParamError::Authentication(_) =>
						::serde_json::to_string(
							&#pages_crate_for_ext::server_fn::ServerFnSetError::Unauthenticated,
						)
						.unwrap_or_else(|_| "\"Unauthenticated\"".to_string()),
					#di_crate::params::ParamError::Internal(detail) => {
						#pages_crate_for_ext::__private::tracing::error!(
							error = %detail,
							"FromRequest extractor failed internally",
						);
						::serde_json::to_string(
							&#pages_crate_for_ext::server_fn::ServerFnSetError::Internal,
						)
						.unwrap_or_else(|_| "\"Internal\"".to_string())
					}
					other => {
						#pages_crate_for_ext::__private::tracing::error!(
							error = %other,
							"FromRequest extractor failed",
						);
						let server_err = #pages_crate_for_ext::server_fn::ServerFnError::server(
							400u16,
							"Parameter extraction failed",
						);
						::serde_json::to_string(&server_err)
							.unwrap_or_else(|_| "Parameter extraction failed".to_string())
					}
				}
			}
		} else {
			quote! {
					match e {
						#di_crate::params::ParamError::Authentication(_) => {
							let server_err = #pages_crate_for_ext::server_fn::ServerFnError::auth(
								401u16,
								"Authentication required",
						);
						::serde_json::to_string(&server_err)
							.unwrap_or_else(|_| "Authentication required".to_string())
					}
					#di_crate::params::ParamError::Internal(detail) => {
						#pages_crate_for_ext::__private::tracing::error!(
							error = %detail,
							"FromRequest extractor failed internally",
						);
						let server_err = #pages_crate_for_ext::server_fn::ServerFnError::server(
							500u16,
							"Internal server error",
						);
						::serde_json::to_string(&server_err)
							.unwrap_or_else(|_| "Internal server error".to_string())
					}
					other => {
						#pages_crate_for_ext::__private::tracing::error!(
							error = %other,
							"FromRequest extractor failed",
						);
						let server_err = #pages_crate_for_ext::server_fn::ServerFnError::server(
							400u16,
							"Parameter extraction failed",
						);
						::serde_json::to_string(&server_err)
							.unwrap_or_else(|_| "Parameter extraction failed".to_string())
					}
				}
			}
		};

		let ext_resolutions: Vec<_> = extractor_params
			.iter()
			.zip(&extractor_param_names)
			.map(|(p, resolved_ident)| {
				let ty = &p.ty;
				quote! {
					let #resolved_ident: #ty = <#ty as #di_crate::params::FromRequest>::from_request(&__req, &__param_ctx)
						.await
						.map_err(|e| #extractor_error)?;
				}
			})
			.collect();

		quote! {
			// Build an empty ParamContext for extractor resolution
			let __param_ctx = #di_crate::params::ParamContext::new();

			// Resolve each FromRequest extractor parameter
			#(#ext_resolutions)*
		}
	} else {
		quote! {}
	};

	let has_inject_or_extractor = !inject_params.is_empty() || !extractor_params.is_empty();
	let mut inject_names = inject_param_names.iter();
	let mut extractor_names = extractor_param_names.iter();
	let mut multipart_names = multipart_param_names.iter();
	let function_call_params: Vec<_> = sig
		.inputs
		.iter()
		.filter_map(|arg| {
			let syn::FnArg::Typed(pat_type) = arg else {
				return None;
			};
			if pat_type.attrs.iter().any(is_inject_attr) {
				let ident = inject_names
					.next()
					.expect("each injected argument must have detected metadata");
				Some(quote! { #ident })
			} else if is_extractor_type(&pat_type.ty) {
				let pat = extractor_names
					.next()
					.expect("each extractor argument must have detected metadata");
				Some(quote! { #pat })
			} else {
				if uses_multipart {
					let ident = multipart_names
						.next()
						.expect("each multipart argument must have wire metadata");
					Some(quote! { #ident })
				} else {
					let pat = &pat_type.pat;
					Some(quote! { args.#pat })
				}
			}
		})
		.collect();

	// Native form extraction normalizes URL-encoded controls into a flat JSON
	// object before invoking the generated handler. The explicit `model_form`
	// option enables the payload-specific fallback without changing ordinary
	// JSON endpoints that happen to use the same parameter name.
	let pages_crate_for_model_form = get_reinhardt_pages_crate();
	if info.options.model_form && codec != "json" {
		return quote! {
			compile_error!("server_fn(model_form = true) requires codec = \"json\" because native model-form controls use the JSON payload fallback");
		};
	}
	let native_model_form_fallback = if info.options.model_form {
		match regular_params.as_slice() {
			[parameter] => {
				let syn::Pat::Ident(parameter_name) = parameter.pat.as_ref() else {
					return quote! { compile_error!("server_fn(model_form = true) requires one identifier parameter"); };
				};
				let parameter_name = &parameter_name.ident;
				let payload_type = &parameter.ty;
				quote! {{
					let native_value = ::serde_json::from_slice(body)
						.map_err(|_| __invalid_request_error())?;
					let #parameter_name: #payload_type = <#payload_type as #pages_crate_for_model_form::form::NativeModelFormPayload>::from_native_form_value(native_value)
						.map_err(|_| __invalid_request_error())?;
					#args_struct_name { #parameter_name }
				}}
			}
			_ => quote! {{ return Err(__invalid_request_error()); }},
		}
	} else {
		quote! {{ return Err(__invalid_request_error()); }}
	};

	// Generate codec-specific deserialization code for server
	let deserialize_code = match codec {
		"json" => quote! {
			let args: #args_struct_name = match ::serde_json::from_slice(body) {
				::core::result::Result::Ok(args) => args,
				::core::result::Result::Err(_) => #native_model_form_fallback,
			};
		},
		"url" => quote! {
			let args: #args_struct_name = ::serde_urlencoded::from_str(&body)
				.map_err(|_| __invalid_request_error())?;
		},
		"msgpack" => quote! {
			// Decode base64 to bytes
			let bytes = ::base64::Engine::decode(&::base64::engine::general_purpose::STANDARD, &body)
				.map_err(|_| __invalid_request_error())?;
			// Deserialize from msgpack bytes
			let args: #args_struct_name = ::rmp_serde::from_slice(&bytes)
				.map_err(|_| __invalid_request_error())?;
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

	// When there are no regular params (all params are extractors), skip deserialization.
	// The Args struct will still be emitted (empty) but we don't need to deserialize it.
	let deserialize_code = if regular_params.is_empty() {
		quote! {
			// No regular params — skip Args deserialization; all params are extractors or injected.
			let args = #args_struct_name {};
		}
	} else {
		deserialize_code
	};

	// Generate pre_validate validation code
	let validation_code = if pre_validate {
		let core_crate = get_reinhardt_core_crate();
		let validation_statements = regular_param_names.iter().map(|param_name| {
			quote! {
				if let Err(error) = #core_crate::validators::Validate::validate(&args.#param_name) {
					let error = #pages_crate::server_fn::ServerFnError::from(error);
					let error_body = ::serde_json::to_vec(&error)
						.map(#pages_crate::__private::bytes::Bytes::from)
						.unwrap_or_else(|_| #pages_crate::__private::bytes::Bytes::from_static(
							br#"{"version":1,"kind":"server","status":500,"message":"Internal server error","field_errors":[]}"#,
						));
					return Err(error_body);
				}
			}
		});
		quote! {
			#(#validation_statements)*
		}
	} else {
		quote! {}
	};
	let multipart_validation_code = if pre_validate {
		let core_crate = get_reinhardt_core_crate();
		let validation_statements = wire_params.iter().zip(&multipart_param_names).filter_map(
			|(parameter, ident)| {
			if !matches!(parameter.kind, WireParamKind::Json) {
				return None;
			}
			Some(quote! {
				if let Err(error) = #core_crate::validators::Validate::validate(&#ident) {
					let error = #pages_crate::server_fn::ServerFnError::from(error);
					let error_body = ::serde_json::to_vec(&error)
						.map(#pages_crate::__private::bytes::Bytes::from)
						.unwrap_or_else(|_| #pages_crate::__private::bytes::Bytes::from_static(
							br#"{"version":1,"kind":"server","status":500,"message":"Internal server error","field_errors":[]}"#,
						));
					return Err(error_body);
				}
			})
		},
		);
		quote! {
			#(#validation_statements)*
		}
	} else {
		quote! {}
	};

	// Generate codec-specific serialization code for server response
	let serialize_response_code = match codec {
		"json" => quote! {
			::serde_json::to_vec(&value)
				.map(#pages_crate::__private::bytes::Bytes::from)
				.map_err(|e| #pages_crate::__private::bytes::Bytes::from(
					format!("Failed to serialize response: {}", e)
				))
		},
		"url" => quote! {
			// For URL-encoded codec, response is still JSON
			::serde_json::to_vec(&value)
				.map(#pages_crate::__private::bytes::Bytes::from)
				.map_err(|e| #pages_crate::__private::bytes::Bytes::from(
					format!("Failed to serialize response: {}", e)
				))
		},
		"msgpack" => quote! {
			// Serialize to msgpack bytes
			let bytes = ::rmp_serde::to_vec(&value)
				.map_err(|e| #pages_crate::__private::bytes::Bytes::from(
					format!("Failed to serialize response: {}", e)
				))?;
			// Encode as base64 for HTTP transport
			Ok(#pages_crate::__private::bytes::Bytes::from(
				::base64::Engine::encode(&::base64::engine::general_purpose::STANDARD, &bytes)
			))
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

	// Generate handler signature and body extraction.
	// The handler receives Request in every native configuration. This keeps body
	// handling in one place and lets JSON decode directly from Bytes when content
	// negotiation is not needed.
	let http_crate = get_reinhardt_http_crate();
	let handler_signature = quote! {
		pub async fn #handler_name(__req: #http_crate::Request) -> ::std::result::Result<#pages_crate::__private::bytes::Bytes, #pages_crate::__private::bytes::Bytes>
	};
	let handler_body_extraction = if regular_params.is_empty() {
		quote! {}
	} else {
		match codec {
			"json" if !has_inject_or_extractor => {
				quote! {
					let __content_type = __req
						.headers
						.get(#pages_crate::__private::hyper::header::CONTENT_TYPE)
						.and_then(|value| value.to_str().ok())
						.unwrap_or("");
					let __media_type = __content_type
						.split(';')
						.next()
						.unwrap_or("")
						.trim();
					let __converted_body;
					let body: &[u8] = if __media_type.is_empty()
						|| __media_type.eq_ignore_ascii_case("application/json")
					{
						__req.body().as_ref()
					} else {
						let __body_text = ::std::string::String::from_utf8(__req.body().to_vec())
							.map_err(|_| __invalid_request_error())?;
						__converted_body = #pages_crate::server_fn::convert_body_for_codec(
							__body_text,
							&__content_type,
							#codec,
						)
						.map_err(|_| __invalid_request_error())?;
						__converted_body.as_bytes()
					};
				}
			}
			"json" => {
				quote! {
					let __content_type = __req
						.headers
						.get(#pages_crate::__private::hyper::header::CONTENT_TYPE)
						.and_then(|value| value.to_str().ok())
						.unwrap_or("");
					let body = __req.read_body()
						.map_err(|_| __invalid_request_error())?;
					let __media_type = __content_type
						.split(';')
						.next()
						.unwrap_or("")
						.trim();
					let __converted_body;
					let body: &[u8] = if __media_type.is_empty()
						|| __media_type.eq_ignore_ascii_case("application/json")
					{
						body.as_ref()
					} else {
						let __body_text = ::std::string::String::from_utf8(body.to_vec())
							.map_err(|_| __invalid_request_error())?;
						__converted_body = #pages_crate::server_fn::convert_body_for_codec(
							__body_text,
							&__content_type,
							#codec,
						)
						.map_err(|_| __invalid_request_error())?;
						__converted_body.as_bytes()
					};
				}
			}
			_ => quote! {
				let __content_type = __req
					.headers
					.get(#pages_crate::__private::hyper::header::CONTENT_TYPE)
					.and_then(|value| value.to_str().ok())
					.unwrap_or("");
				let body = __req.read_body()
					.map_err(|_| __invalid_request_error())?;
				let body = ::std::string::String::from_utf8(body.to_vec())
					.map_err(|_| __invalid_request_error())?;
				let body = #pages_crate::server_fn::convert_body_for_codec(body, &__content_type, #codec)
					.map_err(|_| __invalid_request_error())?;
			},
		}
	};
	let invalid_request_error = if regular_params.is_empty() {
		quote! {}
	} else {
		quote! {
			let __invalid_request_error = || {
				let error = #pages_crate::server_fn::ServerFnError::server(
					400u16,
					"Invalid server function request",
				);
				#pages_crate::__private::bytes::Bytes::from(
					::serde_json::to_string(&error)
						.expect("ServerFnError must serialize into its versioned error envelope"),
				)
			};
		}
	};
	let wrapper_body_extraction = quote! {};
	let wrapper_call_args = vec![quote! { req }];

	// Generate unique name for the static wrapper function
	let static_wrapper_name = quote::format_ident!("__server_fn_static_wrapper_{}", name);
	let name_str = info.metadata_name();
	let detail = info.detail;
	let transactional = info.transactional;
	let is_json_codec = codec == "json";

	// Note: pages_crate is already resolved above for body extraction.
	// http_crate is resolved above when inject_params is not empty,
	// but we need it for the static wrapper regardless
	let http_crate_for_wrapper = get_reinhardt_http_crate();

	// Get visibility for marker struct (same as original function)
	let vis = info.vis();
	let marker_struct_vis = marker_struct_visibility(vis);

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
	let metadata_alias_prefix = name.unraw().to_string().to_case(Case::Pascal);
	let response_alias = quote::format_ident!("__ServerFn{}Response", metadata_alias_prefix);
	let error_alias = quote::format_ident!("__ServerFn{}Error", metadata_alias_prefix);
	let request_alias = quote::format_ident!("__ServerFn{}Request", metadata_alias_prefix);
	let argument_metadata: Vec<_> = wire_params
		.iter()
		.map(|parameter| {
			let name = wire_param_name(parameter);
			let kind = match parameter.kind {
				WireParamKind::Json => {
					quote! { #pages_crate::server_fn::ServerFnArgumentKind::Json }
				}
				WireParamKind::File => {
					quote! { #pages_crate::server_fn::ServerFnArgumentKind::File }
				}
				WireParamKind::OptionalFile => {
					quote! { #pages_crate::server_fn::ServerFnArgumentKind::OptionalFile }
				}
			};
			quote! {
				#pages_crate::server_fn::ServerFnArgumentMetadata { name: #name, kind: #kind }
			}
		})
		.collect();
	let argument_marker_types: Vec<_> = wire_params
		.iter()
		.map(|parameter| parameter.name.clone())
		.collect();
	let argument_trait_impls = wire_params.iter().enumerate().map(|(index, parameter)| {
		let marker_type = &argument_marker_types[index];
		let name = wire_param_name(parameter);
		let optional = is_option_type(regular_param_types[index]);
		let kind = match parameter.kind {
			WireParamKind::Json => quote! { #pages_crate::server_fn::ServerFnArgumentKind::Json },
			WireParamKind::File => quote! { #pages_crate::server_fn::ServerFnArgumentKind::File },
			WireParamKind::OptionalFile => {
				quote! { #pages_crate::server_fn::ServerFnArgumentKind::OptionalFile }
			}
		};
		quote! {
			impl #pages_crate::server_fn::ServerFnArgument<#index> for marker {
				type Name = __args::#marker_type;
				const OPTIONAL: bool = #optional;
				const METADATA: #pages_crate::server_fn::ServerFnArgumentMetadata =
					#pages_crate::server_fn::ServerFnArgumentMetadata { name: #name, kind: #kind };
			}
		}
	});
	let argument_count = wire_params.len();
	let argument_metadata_tokens = quote! {
		#[doc(hidden)]
		pub mod __args {
			#(
				// Keep marker type names identical to wire argument identifiers for compile-time checks.
				#[allow(non_camel_case_types)]
				pub struct #argument_marker_types;
			)*
		}

		#(#argument_trait_impls)*
		impl #pages_crate::server_fn::ServerFnArgumentCount<#argument_count> for marker {}
	};

	// MSW: Generate MockableServerFn impl when the macro crate was compiled
	// with `msw` feature.
	//
	// (Fixes #4290) Previously this also checked
	// `std::env::var("CARGO_FEATURE_MSW").is_ok()` as a "consuming-crate has msw"
	// guard, but per Cargo's documented behavior `CARGO_FEATURE_*` env vars are
	// only set for build.rs invocations — NOT for proc-macro expansion. The env
	// var check was therefore guaranteed to evaluate to `false` for every
	// consumer in every configuration, so the WASM `marker` module emitted by
	// the conditional block below was never actually produced. Removing the
	// always-false clause restores the intended behavior.
	//
	// Cargo's transitive feature unification already guarantees that when any
	// node in the dependency graph activates `reinhardt-pages-macros/msw`, this
	// proc-macro is compiled with the feature on; the consuming crate must
	// independently enable the matching feature on `reinhardt-pages` (via
	// `reinhardt-web/msw`) so that `MockableServerFn` is in scope.
	let msw_enabled = cfg!(feature = "msw");

	let emits_public_metadata = info.emits_typed_response_metadata();
	let emits_msw_metadata = emits_public_metadata && !uses_multipart;
	let emits_typed_response_metadata = emits_public_metadata;
	let metadata_response_type = quote! {
		<#return_type as #pages_crate::server_fn::ServerFnQueryResult>::Response
	};
	let metadata_error_type = quote! {
		<#return_type as #pages_crate::server_fn::ServerFnQueryResult>::Error
	};
	let response_metadata_type_aliases = if emits_typed_response_metadata {
		quote! {
			#[doc(hidden)]
			#vis type #response_alias = #metadata_response_type;
			#[doc(hidden)]
			#vis type #error_alias = #metadata_error_type;
		}
	} else {
		quote! {}
	};
	let response_metadata_impl = if emits_typed_response_metadata {
		quote! {
			impl #pages_crate::server_fn::ServerFnResponseMetadata for marker {
				type Response = super::#response_alias;
				type Error = super::#error_alias;
			}
		}
	} else {
		quote! {}
	};
	let model_form_payload_error = quote! {
		.map_err(|error| {
			#pages_crate::ServerFnError::validation_with_message(
				error.to_string(),
				::core::iter::empty::<(&str, &str)>(),
			)
		})?
	};
	let model_form_server_fn_impl = if uses_multipart {
		let selection_bounds: Vec<_> = wire_params
			.iter()
			.enumerate()
			.map(|(index, _)| {
				quote! {
					#pages_crate::form::ModelFormSelectionArgument<#index>
				}
			})
			.collect();
		let selection_name_checks = wire_params.iter().enumerate().map(|(index, _)| {
			quote! {
				let () = <#pages_crate::form::ModelFormSelectionArgumentNameCheck<
					__ReinhardtSelection,
					marker,
					#index,
				>>::ASSERT;
			}
		});
		let selection_kind_checks = wire_params.iter().enumerate().map(|(index, _)| {
			quote! {
				#pages_crate::form::assert_model_form_argument_compatibility::<
					__ReinhardtSelection,
					marker,
					#index,
				>();
			}
		});
		let model_form_arguments: Vec<_> = wire_params
			.iter()
			.zip(regular_param_types.iter())
			.map(|(parameter, parameter_type)| {
				let parameter_name = &parameter.name;
				let field_name = parameter.name.to_string();
				match parameter.kind {
					WireParamKind::Json => quote! {
						let #parameter_name: #parameter_type = state
							.json_argument(#field_name)
							#model_form_payload_error;
					},
					WireParamKind::File => quote! {
						let #parameter_name = state
							.required_file_argument(#field_name)
							#model_form_payload_error;
					},
					WireParamKind::OptionalFile => quote! {
						let #parameter_name = state
							.optional_file_argument(#field_name)
							#model_form_payload_error;
					},
				}
			})
			.collect();
		let model_form_argument_validation: Vec<_> = wire_params
			.iter()
			.zip(regular_param_types.iter())
			.map(|(parameter, parameter_type)| {
				let field_name = parameter.name.to_string();
				match parameter.kind {
					WireParamKind::Json => quote! {
						let _: #parameter_type = state.json_argument(#field_name)?;
					},
					WireParamKind::File => quote! {
						let _ = state.required_file_argument(#field_name)?;
					},
					WireParamKind::OptionalFile => quote! {
						let _ = state.optional_file_argument(#field_name)?;
					},
				}
			})
			.collect();
		let model_form_argument_names: Vec<_> = wire_params
			.iter()
			.map(|parameter| &parameter.name)
			.collect();
		let argument_count = wire_params.len();
		quote! {
			impl<__ReinhardtSelection, __ReinhardtSchema, __ReinhardtPolicy>
				#pages_crate::form::ModelFormServerFn<
					__ReinhardtSelection,
					__ReinhardtSchema,
					__ReinhardtPolicy,
				> for marker
			where
				__ReinhardtSchema: #pages_crate::form::ModelFormSchema,
				__ReinhardtPolicy: #pages_crate::form::ModelFormPolicy,
				__ReinhardtSelection:
					#pages_crate::form::ModelFormSelectionCount<#argument_count>
					#(+ #selection_bounds)*,
					#return_type: #pages_crate::server_fn::ServerFnQueryResult,
					#pages_crate::ServerFnError:
						::core::convert::Into<
							<#return_type as #pages_crate::server_fn::ServerFnQueryResult>::Error,
						>,
				{
					const VALIDATE_SELECTION: () = {
						#(#selection_name_checks)*
						#(#selection_kind_checks)*
					};

					type Response = <#return_type as #pages_crate::server_fn::ServerFnQueryResult>::Response;
					type Error = <#return_type as #pages_crate::server_fn::ServerFnQueryResult>::Error;

				fn validate_input(
					state: &#pages_crate::form::ModelFormState<
						__ReinhardtSchema,
						__ReinhardtPolicy,
					>,
				) -> ::core::result::Result<
					(),
					#pages_crate::form::ModelFormPayloadError,
				> {
					#[cfg(all(target_family = "wasm", target_os = "unknown"))]
					{
						#(#model_form_argument_validation)*
						::core::result::Result::Ok(())
					}
					#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
					{
						let _ = state;
						::core::result::Result::Ok(())
					}
				}

				fn submit(
					state: &#pages_crate::form::ModelFormState<
						__ReinhardtSchema,
						__ReinhardtPolicy,
					>,
				) -> impl ::core::future::Future<
					Output = ::core::result::Result<Self::Response, Self::Error>,
				> {
					#[cfg(all(target_family = "wasm", target_os = "unknown"))]
					{
						async move {
							#(#model_form_arguments)*
							super::#name(#(#model_form_argument_names),*)
								.await
						}
					}
					#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
					{
						async move {
							let _ = state;
							::core::unreachable!()
						}
					}
				}
			}
		}
	} else if info.options.model_form {
		match regular_param_types.as_slice() {
			[payload_type] => quote! {
				impl<__ReinhardtSelection, __ReinhardtSchema, __ReinhardtPolicy>
					#pages_crate::form::ModelFormServerFn<
						__ReinhardtSelection,
						__ReinhardtSchema,
						__ReinhardtPolicy,
					> for marker
				where
					__ReinhardtSchema: #pages_crate::form::ModelFormSchema,
					__ReinhardtPolicy: #pages_crate::form::ModelFormPolicy,
					__ReinhardtSelection: #pages_crate::form::ModelFormSelectionPayload<
						__ReinhardtSchema,
						__ReinhardtPolicy,
						Payload = #payload_type,
					>,
					#return_type: #pages_crate::server_fn::ServerFnQueryResult<
						Error = #pages_crate::ServerFnError,
					>,
				{
						type Response = <#return_type as #pages_crate::server_fn::ServerFnQueryResult>::Response;
						type Error = #pages_crate::ServerFnError;

					fn validate_input(
						state: &#pages_crate::form::ModelFormState<
							__ReinhardtSchema,
							__ReinhardtPolicy,
						>,
					) -> ::core::result::Result<
						(),
						#pages_crate::form::ModelFormPayloadError,
					> {
						#[cfg(all(target_family = "wasm", target_os = "unknown"))]
						{
							<__ReinhardtSelection as #pages_crate::form::ModelFormSelectionPayload<
								__ReinhardtSchema,
								__ReinhardtPolicy,
							>>::build_payload(state)
								.map(|_| ())
						}
						#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
						{
							let _ = state;
							::core::result::Result::Ok(())
						}
					}

					fn submit(
						state: &#pages_crate::form::ModelFormState<
							__ReinhardtSchema,
							__ReinhardtPolicy,
						>,
					) -> impl ::core::future::Future<
						Output = ::core::result::Result<Self::Response, #pages_crate::ServerFnError>,
					> {
						#[cfg(all(target_family = "wasm", target_os = "unknown"))]
						{
							async move {
								let payload = <__ReinhardtSelection as #pages_crate::form::ModelFormSelectionPayload<
									__ReinhardtSchema,
									__ReinhardtPolicy,
								>>::build_payload(state)
									#model_form_payload_error;
								super::#name(payload).await
							}
						}
						#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
						{
							async move {
								let _ = state;
								::core::unreachable!()
							}
						}
					}
				}
			},
			_ => quote! {
				compile_error!("server_fn(model_form = true) requires one client-visible parameter");
			},
		}
	} else {
		quote! {}
	};
	let request_metadata_type_aliases = if !uses_multipart && emits_typed_response_metadata {
		let request_type = match regular_param_types.as_slice() {
			[] => quote! { () },
			[request_type] => quote! { #request_type },
			_ => quote! { (#(#regular_param_types),*) },
		};
		quote! {
			#[doc(hidden)]
			#vis type #request_alias = #request_type;
		}
	} else {
		quote! {}
	};
	let request_metadata_impl = if !uses_multipart && emits_typed_response_metadata {
		quote! {
			impl #pages_crate::server_fn::ServerFnRequestMetadata for marker {
				type Request = super::#request_alias;
			}
		}
	} else {
		quote! {}
	};
	let mutation_call = match regular_param_types.as_slice() {
		[] => quote! {
			let () = request;
			super::#name().await
		},
		[_] => quote! {
			super::#name(request).await
		},
		_ => quote! {
			let (#(#regular_param_names),*) = request;
			super::#name(#(#regular_param_names),*).await
		},
	};
	let mutation_helper_tokens = if !uses_multipart && emits_typed_response_metadata {
		quote! {
			/// Returns a target-neutral mutation callable for this server function.
			pub fn mutation() -> impl Fn(
				<marker as #pages_crate::server_fn::ServerFnRequestMetadata>::Request,
			) -> ::std::pin::Pin<::std::boxed::Box<dyn ::std::future::Future<
				Output = ::std::result::Result<
					<marker as #pages_crate::server_fn::ServerFnResponseMetadata>::Response,
					<marker as #pages_crate::server_fn::ServerFnResponseMetadata>::Error,
				>,
			>>> {
				|request| {
					#[cfg(all(target_family = "wasm", target_os = "unknown"))]
					{
						::std::boxed::Box::pin(async move { #mutation_call })
					}
					#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
					{
						let _request = request;
						::std::boxed::Box::pin(async move { ::core::unreachable!() })
					}
				}
			}
		}
	} else {
		quote! {}
	};

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
	let uses_response_cookie_jar = !inject_params.is_empty() || !extractor_params.is_empty();
	let sanitize_error = if info.structured_error {
		quote! {
			let e = e.into_server_wire_error();
		}
	} else {
		quote! {}
	};
	let serialize_error = if info.structured_error {
		quote! {
			let error_json = ::serde_json::to_string(&e)
				.map_err(|e| #pages_crate::__private::bytes::Bytes::from(
					format!("Failed to serialize error: {}", e)
				))?;
		}
	} else {
		quote! {
			let serialized_error = ::serde_json::to_value(&e)
				.map_err(|e| #pages_crate::__private::bytes::Bytes::from(
					format!("Failed to serialize error: {}", e)
				))?;
			let error_message = match &serialized_error {
				::serde_json::Value::String(message) => message.clone(),
				::serde_json::Value::Object(fields) => fields
					.clone()
					.remove("message")
					.and_then(|value| value.as_str().map(::std::string::String::from))
					.unwrap_or_else(|| "Server function failed".to_string()),
				_ => "Server function failed".to_string(),
			};
			let error = ::serde_json::from_value::<#pages_crate::server_fn::ServerFnError>(serialized_error)
				.unwrap_or_else(|_| #pages_crate::server_fn::ServerFnError::application_with_status(500u16, error_message));
			let error_json = ::serde_json::to_string(&error)
				.map_err(|e| #pages_crate::__private::bytes::Bytes::from(
					format!("Failed to serialize error: {}", e)
				))?;
		}
	};
	let normalize_handler_error = if info.structured_error {
		quote! {}
	} else {
		quote! {
			.map_err(|error_body| {
				if ::serde_json::from_slice::<#pages_crate::server_fn::ServerFnError>(&error_body).is_ok() {
					error_body
				} else {
					let error = #pages_crate::server_fn::ServerFnError::server(
						500u16,
						"Internal server error",
					);
					#pages_crate::__private::bytes::Bytes::from(
						::serde_json::to_string(&error)
							.expect("ServerFnError must serialize into its versioned error envelope"),
					)
				}
			})
		}
	};
	let structured_status_override = if info.structured_error {
		quote! {
			fn error_status(error_body: &[u8]) -> u16 {
				#pages_crate::server_fn::ServerFnSetError::http_status_from_body(error_body)
			}
		}
	} else {
		quote! {}
	};
	let handle_call = if info.structured_error {
		quote! {
			async move {
				super::#handler_name(req)
					.await
					.map_err(#pages_crate::server_fn::ServerFnSetError::sanitize_server_error_body)
			}
		}
	} else {
		quote! { super::#handler_name(req) }
	};
	let regular_param_idents: Vec<_> = regular_params
		.iter()
		.filter_map(|p| {
			if let syn::Pat::Ident(pat_ident) = &*p.pat {
				Some(pat_ident.ident.clone())
			} else {
				None
			}
		})
		.collect();
	let query_arg_generics: Vec<_> = (0..regular_param_idents.len())
		.map(|index| quote::format_ident!("QueryArg{index}"))
		.collect();
	let key_generics = if query_arg_generics.is_empty() {
		quote! {}
	} else {
		quote! { <#(#query_arg_generics),*> }
	};
	let query_param_bounds: Vec<_> = query_arg_generics
		.iter()
		.zip(regular_param_types.iter())
		.map(|(generic, ty)| {
			quote! {
				#generic: #pages_crate::server_fn::ServerFnQueryArg<#ty>
			}
		})
		.collect();
	let key_where_clause = quote! {
		where
			#return_type: #pages_crate::server_fn::ServerFnQueryResult,
			#(#query_param_bounds,)*
	};
	let query_result_conversion = |call: proc_macro2::TokenStream| {
		quote! {
			#pages_crate::server_fn::ServerFnQueryResult::into_query_result(#call)
		}
	};
	let regular_query_call = query_result_conversion(quote! {
		super::#name(
			#(#pages_crate::server_fn::ServerFnQueryArg::into_query_arg(#regular_param_idents)),*
		).await
	});
	let clone_query_args = quote! {
		let (#(#regular_param_idents,)*) = (*__query_fetch_args).clone();
	};
	let native_query_call = if has_inject_or_extractor {
		quote! {
			::std::panic!(
				concat!(
					"server function `",
					#name_str,
					"` query cannot run natively because it has injected or extractor parameters",
				),
			)
		}
	} else {
		regular_query_call.clone()
	};
	let query_response_type = quote! {
		<#return_type as #pages_crate::server_fn::ServerFnQueryResult>::Response
	};
	let query_fetcher = if has_inject_or_extractor && emits_msw_metadata {
		quote! {
			{
				let __query_fetch_args = ::std::rc::Rc::new((#(#regular_param_idents,)*));
				move || {
					let __query_fetch_args = ::std::rc::Rc::clone(&__query_fetch_args);
					async move {
					#[cfg(all(target_family = "wasm", target_os = "unknown"))]
					{
						#clone_query_args
						#regular_query_call
					}

					#[cfg(all(not(all(target_family = "wasm", target_os = "unknown")), feature = "msw"))]
					{
						let __args = {
							let (#(#regular_param_idents,)*) =
								(*::std::rc::Rc::clone(&__query_fetch_args)).clone();
							Args {
								#(
									#regular_param_names: #pages_crate::server_fn::ServerFnQueryArg::into_query_arg(#regular_param_idents)
								),*
							}
						};
						if let Some(__mock_result) =
							#pages_crate::server_fn::try_call_active_mock::<marker>(__args)
						{
							match __mock_result {
								Ok(__mock_value) => return Ok(__mock_value),
								Err(__mock_error) => {
									return Err(::std::convert::Into::into(__mock_error));
								}
							}
						}
						#clone_query_args
						#native_query_call
					}

					#[cfg(all(not(all(target_family = "wasm", target_os = "unknown")), not(feature = "msw")))]
					{
						#clone_query_args
						#native_query_call
					}
				}
				}
			}
		}
	} else if has_inject_or_extractor {
		quote! {
			{
				let __query_fetch_args = ::std::rc::Rc::new((#(#regular_param_idents,)*));
				move || {
					let __query_fetch_args = ::std::rc::Rc::clone(&__query_fetch_args);
					async move {
						#[cfg(all(target_family = "wasm", target_os = "unknown"))]
						{
							#clone_query_args
							#regular_query_call
						}

						#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
						{
							#clone_query_args
							#native_query_call
						}
					}
				}
			}
		}
	} else {
		quote! {
			{
				let __query_fetch_args = ::std::rc::Rc::new((#(#regular_param_idents,)*));
				move || {
					let __query_fetch_args = ::std::rc::Rc::clone(&__query_fetch_args);
					async move {
						#clone_query_args
						#regular_query_call
					}
				}
			}
		}
	};
	let query_ssr_policy = if has_inject_or_extractor {
		quote! { __query_descriptor.with_ssr_prefetch(false) }
	} else {
		quote! { __query_descriptor }
	};
	let private_interfaces_allowance = if info.allows_private_interfaces() {
		quote! { #[allow(private_interfaces)] }
	} else {
		quote! {}
	};
	let key_cfg_allowances = if has_inject_or_extractor {
		quote! { #[allow(unused_variables, unexpected_cfgs)] }
	} else {
		quote! {}
	};
	let query_endpoint = info
		.options
		.endpoint
		.clone()
		.unwrap_or_else(|| format!("/api/server_fn/{name}"));
	let query_family_id = syn::LitStr::new(
		&format!("server_fn:{query_endpoint}:{codec}"),
		proc_macro2::Span::call_site(),
	);
	let query_argument_tuple_type = quote! { (#(#regular_param_types,)*) };
	let query_key_argument_tuple_type = quote! { (#(#query_arg_generics,)*) };
	let query_helper_tokens = if uses_multipart {
		quote! {}
	} else {
		quote! {
		/// Returns the typed query family for this server function.
		// The generated signature mirrors endpoints that deliberately allow private
		// request or response types on the source server function.
		#private_interfaces_allowance
		pub fn family() -> #pages_crate::reactive::QueryFamily<
			#query_argument_tuple_type,
			<#return_type as #pages_crate::server_fn::ServerFnQueryResult>::Response,
			<#return_type as #pages_crate::server_fn::ServerFnQueryResult>::Error,
		>
		where
			#return_type: #pages_crate::server_fn::ServerFnQueryResult,
		{
			#pages_crate::reactive::QueryFamily::new(#query_family_id)
		}

		/// Builds the exact typed cache key for this server function and argument set.
		// Injected and extractor-only native paths intentionally cannot consume the
		// client-visible arguments outside MSW or WASM builds, and consumer crates
		// may intentionally omit the optional `msw` feature checked in the body.
		#private_interfaces_allowance
		#key_cfg_allowances
		pub fn key #key_generics(
			#(#regular_param_idents: #query_arg_generics),*
		) -> #pages_crate::reactive::QueryKey<
			<#return_type as #pages_crate::server_fn::ServerFnQueryResult>::Response,
			<#return_type as #pages_crate::server_fn::ServerFnQueryResult>::Error,
		>
			#key_where_clause
		{
			let __query_args = (#(#regular_param_idents.clone(),)*);
			#pages_crate::reactive::QueryFamily::<
				#query_key_argument_tuple_type,
				<#return_type as #pages_crate::server_fn::ServerFnQueryResult>::Response,
				<#return_type as #pages_crate::server_fn::ServerFnQueryResult>::Error,
			>::new(#query_family_id)
			.key(__query_args)
		}

		/// Builds a typed fetch descriptor for this server function and argument set.
		#private_interfaces_allowance
		#key_cfg_allowances
		pub fn query #key_generics(
			#(#regular_param_idents: #query_arg_generics),*
		) -> #pages_crate::reactive::QueryDescriptor<
			<#return_type as #pages_crate::server_fn::ServerFnQueryResult>::Response,
			<#return_type as #pages_crate::server_fn::ServerFnQueryResult>::Error,
		>
		#key_where_clause
		{
			let __query_args = (#(#regular_param_idents.clone(),)*);
			let __query_descriptor = #pages_crate::reactive::QueryFamily::<
				#query_key_argument_tuple_type,
				<#return_type as #pages_crate::server_fn::ServerFnQueryResult>::Response,
				<#return_type as #pages_crate::server_fn::ServerFnQueryResult>::Error,
			>::new(#query_family_id)
			.query(__query_args, #query_fetcher);
			#query_ssr_policy
		}
		}
	};

	// MSW: Generate server-side MockableServerFn tokens only when msw feature is enabled
	let msw_server_tokens = if msw_enabled && emits_msw_metadata {
		quote! {
			mod __msw {
				// Generated MSW support may expand in crates that do not declare every
				// optional cfg name used by this framework.
				#![allow(unexpected_cfgs)]

				// Import signature-local aliases and private types from the server function.
				#[allow(unused_imports)]
				use super::super::*;
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
				type Response = #query_response_type;
			}
		}
	} else {
		quote! {}
	};

	// MSW: optional `Args` struct + `MockableServerFn` impl for type-safe
	// mocking on wasm. Lives inside the always-emitted wasm marker module
	// further below; gated on `feature = "msw"` so consumer crates that
	// never activate `reinhardt-pages/msw` (e.g. mixed-feature workspace
	// builds where another package activates `reinhardt-pages-macros/msw`
	// and Cargo reuses the proc-macro artifact — Issue #4290) do not see
	// the `MockableServerFn` impl whose trait isn't in scope for them.
	// `#[allow(unexpected_cfgs)]` keeps the cfg quiet in consumer crates
	// that don't themselves declare an `msw` feature.
	let msw_wasm_inner_tokens = if msw_enabled && emits_msw_metadata {
		quote! {
			mod __msw {
				// Generated MSW support may expand in crates that do not declare every
				// optional cfg name used by this framework.
				#![allow(unexpected_cfgs)]
				// Import signature-local aliases and private types from the server function.
				#[allow(unused_imports)]
				use super::super::*;

				#[cfg(feature = "msw")]
				mod args {
					// The generated args module reuses caller-local type paths from the
					// original server function signature.
					#[allow(unused_imports)]
					use super::super::super::*;
					use ::serde::{Serialize, Deserialize};

					/// Public Args struct for MSW type-safe mocking.
					#[derive(Serialize, Deserialize)]
					pub struct Args {
						#(pub #regular_param_names: #regular_param_types),*
					}
				}

				#[cfg(feature = "msw")]
				pub use args::Args;

				#[cfg(feature = "msw")]
				impl #pages_crate::server_fn::MockableServerFn for super::marker {
					type Args = Args;
					type Response = #query_response_type;
				}
			}

			pub use __msw::*;
		}
	} else {
		quote! {}
	};

	// WASM marker module — emitted unconditionally on wasm targets so that
	// `s.server_fn(my_fn::marker)` resolves regardless of whether `msw` is
	// active (#4711). The struct + ServerFnMetadata impl are always present;
	// the optional `Args` struct and `MockableServerFn` impl live behind the
	// inner `feature = "msw"` cfg (see `msw_wasm_inner_tokens` above).
	// Parity: the marker module is P1. WASM emits the marker so
	// `.server_fn(function_name::marker)` remains nameable in shared route
	// declarations, but route registration is native-only behavior.
	let wasm_marker_tokens = quote! {
		#[cfg(all(target_family = "wasm", target_os = "unknown"))]
		#vis mod #marker_module_name {
			// (Fixes #4290) Bring in the parent scope's imports so that
			// the `#response_type` (which can be a tuple of user types
			// like `(QuestionInfo, Vec<ChoiceInfo>)`) resolves inside the
			// optional `MockableServerFn` impl. Mirrors the native marker
			// module further below.
			//
			// `#[allow(unused_imports)]` silences the warning when the
			// server_fn signature uses only primitive / fully-qualified
			// types and `use super::*;` ends up importing nothing the
			// generated body references. (Copilot review on PR #4293.)
			#[allow(unused_imports)]
			use super::*;

			#[doc = concat!("Marker struct for server function `", #name_str, "` (use with `.server_fn()`).")]
			#[doc = ""]
			#[doc = "Parity: P1."]
			#[doc = ""]
			#[doc = "The marker is emitted on WASM so shared route declarations can name it, but server route registration is native-only behavior."]
			// The public API intentionally names marker types `function_name::marker`.
			#[allow(non_camel_case_types)]
			#marker_struct_vis struct marker;

			#argument_metadata_tokens

			impl #pages_crate::server_fn::ServerFnMetadata for marker {
				const PATH: &'static str = #endpoint;
				const NAME: &'static str = #name_str;
				const CODEC: &'static str = #codec;
				const IS_JSON_CODEC: bool = #is_json_codec;
				const INJECTED_PARAMS: &'static [&'static str] = &[#(#inject_param_name_strs),*];
				const ARGUMENTS: &'static [#pages_crate::server_fn::ServerFnArgumentMetadata] =
					&[#(#argument_metadata),*];
				const USES_MULTIPART: bool = #uses_multipart;
				const DETAIL: bool = #detail;
				const TRANSACTIONAL: bool = #transactional;
				const USES_RESPONSE_COOKIE_JAR: bool = #uses_response_cookie_jar;
			}

			#response_metadata_impl
			#request_metadata_impl
			#mutation_helper_tokens
			#model_form_server_fn_impl
			#query_helper_tokens

			#msw_wasm_inner_tokens
		}
	};
	let handler_execution = if uses_multipart {
		quote! {
			let __handler_result = async {
				#invalid_request_error

				let mut arguments = #pages_crate::server_fn::MultipartArguments::from_request(&__req)
					.await
					.map_err(|_| __invalid_request_error())?;
				#(#multipart_argument_extraction)*
				arguments.finish().map_err(|_| __invalid_request_error())?;
				#multipart_validation_code
				#di_resolution
				#extractor_resolution

				let result: #return_type = #name(#(#function_call_params),*).await;
				match result {
					Ok(value) => { #serialize_response_code }
					Err(e) => {
						#sanitize_error
						#serialize_error
						Err(#pages_crate::__private::bytes::Bytes::from(error_json))
					}
				}
			};
			__handler_result.await #normalize_handler_error
		}
	} else {
		quote! {
			use ::serde::Deserialize;
			let __handler_result = async {
				#invalid_request_error

				#[derive(Deserialize)]
				struct #args_struct_name {
					#(#regular_param_names: #regular_param_types),*
				}

				#handler_body_extraction
				#deserialize_code
				#validation_code
				#di_resolution
				#extractor_resolution

				let result: #return_type = #name(#(#function_call_params),*).await;
				match result {
					Ok(value) => { #serialize_response_code }
					Err(e) => {
						#sanitize_error
						#serialize_error
						Err(#pages_crate::__private::bytes::Bytes::from(error_json))
					}
				}
			};
			__handler_result.await #normalize_handler_error
		}
	};

	quote! {
		#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
		/// Server-side handler function
		///
		/// This function is called by the router when the endpoint receives a request.
		/// It deserializes the request body, calls the server function, and serializes the response.
		#handler_signature {
			#handler_execution
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

		#response_metadata_type_aliases
		#request_metadata_type_aliases

		// Static wrapper function for explicit registration
		// This is used by ServerFnRegistration::handler() to provide a function pointer.
		#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
		fn #static_wrapper_name(
			req: #http_crate_for_wrapper::Request
		) -> ::std::pin::Pin<::std::boxed::Box<dyn ::std::future::Future<Output = ::std::result::Result<#pages_crate::__private::bytes::Bytes, #pages_crate::__private::bytes::Bytes>> + ::std::marker::Send>> {
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
		// Server-side explicit registration uses marker modules such as
		// `login::marker` and `logout::marker`. WASM callers import and invoke
		// the generated function directly.
		#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
		#vis mod #marker_module_name {
			// User-defined types in generated signatures may require parent-scope imports.
			#[allow(unused_imports)]
			use super::*;

			#[doc = concat!("Marker struct for server function `", #name_str, "` (use with `.server_fn()`).")]
			#[doc = ""]
			#[doc = "Parity: P1."]
			#[doc = ""]
			#[doc = "The marker is emitted on both native and WASM so shared route declarations can name it. Native builds register the server handler; WASM builds keep the marker metadata inert."]
			// The public API intentionally names marker types `function_name::marker`.
			#[allow(non_camel_case_types)]
			#marker_struct_vis struct marker;

			#argument_metadata_tokens

			// Cross-target metadata. ServerFnMetadata lives in reinhardt-pages
			// and is available on both native and wasm — the constants below
			// are inherited by ServerFnRegistration (native) and
			// MockableServerFn (msw) via supertrait, keeping a single source
			// of truth for PATH / NAME / CODEC across targets.
			impl #pages_crate::server_fn::ServerFnMetadata for marker {
				const PATH: &'static str = #endpoint;
				const NAME: &'static str = #name_str;
				const CODEC: &'static str = #codec;
				const IS_JSON_CODEC: bool = #is_json_codec;
				const INJECTED_PARAMS: &'static [&'static str] = &[#(#inject_param_name_strs),*];
				const ARGUMENTS: &'static [#pages_crate::server_fn::ServerFnArgumentMetadata] =
					&[#(#argument_metadata),*];
				const USES_MULTIPART: bool = #uses_multipart;
				const DETAIL: bool = #detail;
				const TRANSACTIONAL: bool = #transactional;
				const USES_RESPONSE_COOKIE_JAR: bool = #uses_response_cookie_jar;
			}

			#response_metadata_impl
			#request_metadata_impl
			#mutation_helper_tokens
			#model_form_server_fn_impl

			// Native-only handler entry point for explicit router registration.
			impl #pages_crate::server_fn::ServerFnRegistration for marker {
				#structured_status_override

				fn handler() -> #pages_crate::server_fn::ServerFnHandler {
					super::#static_wrapper_name
				}

				fn handle(
					req: #http_crate_for_wrapper::Request
				) -> impl ::std::future::Future<Output = ::std::result::Result<#pages_crate::__private::bytes::Bytes, #pages_crate::__private::bytes::Bytes>> + ::std::marker::Send {
					#handle_call
				}
			}

			#query_helper_tokens

			// MSW: server-side MockableServerFn (conditionally generated; Issue #3673)
			#msw_server_tokens
		}

		// WASM-side marker module — always emitted on wasm (#4711); the
		// optional MSW Args / MockableServerFn impl is gated inside.
		#wasm_marker_tokens
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_marker_struct_visibility_rewrites_relative_restrictions_for_marker_module() {
		use syn::parse_quote;

		let crate_vis: syn::Visibility = parse_quote!(pub(crate));
		assert_eq!(
			marker_struct_visibility(&crate_vis).to_string(),
			quote! { pub(crate) }.to_string()
		);

		let super_vis: syn::Visibility = parse_quote!(pub(super));
		assert_eq!(
			marker_struct_visibility(&super_vis).to_string(),
			quote! { pub(in super::super) }.to_string()
		);

		let super_nested_vis: syn::Visibility = parse_quote!(pub(in super::endpoints));
		assert_eq!(
			marker_struct_visibility(&super_nested_vis).to_string(),
			quote! { pub(in super::super::endpoints) }.to_string()
		);

		let self_vis: syn::Visibility = parse_quote!(pub(self));
		assert_eq!(
			marker_struct_visibility(&self_vis).to_string(),
			quote! { pub(in super) }.to_string()
		);

		let self_nested_vis: syn::Visibility = parse_quote!(pub(in self::endpoints));
		assert_eq!(
			marker_struct_visibility(&self_nested_vis).to_string(),
			quote! { pub(in super::endpoints) }.to_string()
		);
	}

	#[test]
	fn test_server_fn_options_default() {
		let options = ServerFnOptions::default();
		assert!(!options.use_inject);
		assert!(!options.model_form);
		assert_eq!(options.endpoint, None);
		assert_eq!(options.codec, "json");
	}

	#[test]
	fn test_server_fn_options_parse() {
		use darling::FromMeta;
		use darling::ast::NestedMeta;
		use syn::parse_quote;

		// Test with endpoint only (use_inject is no longer needed)
		let attr: syn::Attribute =
			parse_quote!(#[server_fn(endpoint = "/custom", model_form = true)]);
		let meta_list = attr.meta.require_list().unwrap();
		let nested: Vec<NestedMeta> = NestedMeta::parse_meta_list(meta_list.tokens.clone())
			.unwrap()
			.into_iter()
			.collect();
		let options = ServerFnOptions::from_list(&nested).unwrap();

		assert!(!options.use_inject);
		assert_eq!(options.endpoint, Some("/custom".to_string()));
		assert_eq!(options.codec, "json");
		assert!(options.model_form);
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
	fn classifies_direct_and_optional_uploaded_files() {
		use syn::parse_quote;

		let function: syn::ItemFn = parse_quote! {
			async fn save(
				name: String,
				avatar: Option<reinhardt_core::parsers::UploadedFile>,
				attachment: UploadedFile,
			) -> Result<(), ServerFnError> {
				Ok(())
			}
		};

		let parameters = classify_wire_params(&function.sig.inputs, "json", false).unwrap();

		assert_eq!(parameters.len(), 3);
		assert_eq!(parameters[0].kind, WireParamKind::Json);
		assert_eq!(parameters[1].kind, WireParamKind::OptionalFile);
		assert_eq!(parameters[2].kind, WireParamKind::File);
	}

	#[test]
	fn qualified_custom_uploaded_file_remains_json() {
		use syn::parse_quote;

		let function: syn::ItemFn = parse_quote! {
			async fn save(file: my_types::UploadedFile) -> Result<(), ServerFnError> {
				Ok(())
			}
		};

		let parameters = classify_wire_params(&function.sig.inputs, "json", false).unwrap();

		assert_eq!(parameters[0].kind, WireParamKind::Json);
	}

	#[test]
	fn rejects_explicit_scalar_codecs_for_file_arguments() {
		use syn::parse_quote;

		let function: syn::ItemFn = parse_quote! {
			async fn save(avatar: UploadedFile) -> Result<(), ServerFnError> {
				Ok(())
			}
		};

		for codec in ["json", "url", "msgpack"] {
			let error = classify_wire_params(&function.sig.inputs, codec, true).unwrap_err();
			assert_eq!(
				error.to_string(),
				"server_fn file arguments infer multipart framing and cannot use an explicit codec"
			);
		}
	}

	#[test]
	fn detects_uploaded_file_inside_trait_bounds() {
		use syn::parse_quote;

		let impl_trait: syn::Type = parse_quote!(impl Into<reinhardt_core::parsers::UploadedFile>);
		let trait_object: syn::Type =
			parse_quote!(&dyn AsRef<reinhardt_core::parsers::UploadedFile>);
		let impl_callback: syn::Type = parse_quote!(impl Fn(reinhardt_core::parsers::UploadedFile));
		let callback_object: syn::Type =
			parse_quote!(&dyn Fn() -> reinhardt_core::parsers::UploadedFile);

		assert!(contains_uploaded_file_type(&impl_trait));
		assert!(contains_uploaded_file_type(&trait_object));
		assert!(contains_uploaded_file_type(&impl_callback));
		assert!(contains_uploaded_file_type(&callback_object));
	}

	#[test]
	fn identifies_body_consuming_extractors_for_multipart_validation() {
		use syn::parse_quote;

		assert!(is_body_consuming_extractor(&parse_quote!(Body)));
		assert!(is_body_consuming_extractor(&parse_quote!(Multipart)));
		assert!(is_body_consuming_extractor(&parse_quote!(Json<Payload>)));
		assert!(is_body_consuming_extractor(&parse_quote!(Form<Payload>)));
		assert!(is_body_consuming_extractor(&parse_quote!(
			Validated<Json<Payload>>
		)));
		assert!(!is_body_consuming_extractor(&parse_quote!(Query<Payload>)));
		assert!(!is_body_consuming_extractor(&parse_quote!(
			Validated<Query<Payload>>
		)));
	}

	#[test]
	fn multipart_client_stub_uses_browser_file_types() {
		use syn::parse_quote;

		let func: ItemFn = parse_quote! {
			async fn save(
				name: String,
				avatar: Option<reinhardt_core::parsers::UploadedFile>,
				attachment: UploadedFile,
			) -> Result<(), ServerFnError> {
				Ok(())
			}
		};
		let info = ServerFnInfo {
			func,
			options: ServerFnOptions::default(),
			codec_explicit: false,
			metadata_name: None,
			endpoint_tokens: None,
			metadata_name_tokens: None,
			detail: false,
			transactional: false,
			structured_error: false,
		};
		let wire_params = classify_wire_params(&info.func.sig.inputs, "json", false).unwrap();
		let pages_crate_info = CratePathInfo {
			needs_conditional: false,
			use_statement: quote::quote! {},
			ident: quote::quote! { ::reinhardt_pages },
		};

		let generated =
			generate_client_stub(&info, &[], &[], &pages_crate_info, &wire_params).to_string();

		assert!(generated.contains(
			"avatar : :: std :: option :: Option < :: reinhardt_pages :: __private :: web_sys :: File >"
		));
		assert!(
			generated.contains("attachment : :: reinhardt_pages :: __private :: web_sys :: File")
		);
		assert!(!generated.contains("Option < :: web_sys :: File >"));
		assert!(!generated.contains("UploadedFile"));
	}

	#[test]
	fn multipart_handler_uses_optional_json_extractor() {
		use syn::parse_quote;

		let func: ItemFn = parse_quote! {
			async fn save(
				note: Option<String>,
				attachment: UploadedFile,
			) -> Result<(), ServerFnError> {
				Ok(())
			}
		};
		let info = ServerFnInfo {
			func,
			options: ServerFnOptions::default(),
			codec_explicit: false,
			metadata_name: None,
			endpoint_tokens: None,
			metadata_name_tokens: None,
			detail: false,
			transactional: false,
			structured_error: false,
		};

		let generated = generate_server_fn(&info).to_string();

		assert!(generated.contains("take_optional_json"));
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

	/// Tests that generated extractor failure logs use Display formatting so raw
	/// request data stored for debugging is not emitted through Debug output.
	#[test]
	fn test_extractor_error_logging_uses_display_formatting() {
		use syn::parse_quote;

		let func: syn::ItemFn = parse_quote! {
			pub async fn login(form: Form<LoginRequest>) -> Result<(), ServerFnError> {
				Ok(())
			}
		};
		let info = ServerFnInfo {
			func,
			options: ServerFnOptions::default(),
			codec_explicit: false,
			metadata_name: None,
			endpoint_tokens: None,
			metadata_name_tokens: None,
			detail: false,
			transactional: false,
			structured_error: false,
		};

		let generated = generate_server_fn(&info).to_string();

		assert!(
			generated.contains("error = %"),
			"extractor errors should be logged with Display formatting: {generated}"
		);
		assert!(
			!generated.contains("error = ? e"),
			"extractor errors must not be logged with Debug formatting: {generated}"
		);
	}

	#[test]
	fn generated_extractor_errors_do_not_serialize_details() {
		use syn::parse_quote;

		let func: syn::ItemFn = parse_quote! {
			pub async fn read_header(header: Header) -> Result<(), ServerFnError> {
				Ok(())
			}
		};
		let standard = ServerFnInfo {
			func: func.clone(),
			options: ServerFnOptions::default(),
			codec_explicit: false,
			metadata_name: None,
			endpoint_tokens: None,
			metadata_name_tokens: None,
			detail: false,
			transactional: false,
			structured_error: false,
		};
		let structured = ServerFnInfo {
			func,
			options: ServerFnOptions::default(),
			codec_explicit: false,
			metadata_name: None,
			endpoint_tokens: None,
			metadata_name_tokens: None,
			detail: false,
			transactional: false,
			structured_error: true,
		};

		let standard_generated = generate_server_fn(&standard).to_string();
		let structured_generated = generate_server_fn(&structured).to_string();

		assert!(
			!standard_generated.contains("Parameter extraction failed: {}"),
			"standard extractor errors must not serialize details: {standard_generated}"
		);
		assert!(
			!structured_generated.contains("Parameter extraction failed: {}"),
			"structured extractor errors must not serialize details: {structured_generated}"
		);
	}

	#[test]
	fn generated_client_stub_decodes_generic_error_envelopes() {
		use syn::parse_quote;

		let func: syn::ItemFn = parse_quote! {
			pub async fn select_choice(choice_id: String) -> Result<(), ServerFnError> {
				Ok(())
			}
		};
		let info = ServerFnInfo {
			func,
			options: ServerFnOptions::default(),
			codec_explicit: false,
			metadata_name: None,
			endpoint_tokens: None,
			metadata_name_tokens: None,
			detail: false,
			transactional: false,
			structured_error: false,
		};

		let generated = generate_server_fn(&info).to_string();

		assert!(
			generated.contains("from_http_response"),
			"generic client stubs must decode structured error envelopes: {generated}"
		);
	}

	#[test]
	fn generated_pre_validation_failures_use_the_structured_error_envelope() {
		use syn::parse_quote;

		let func: syn::ItemFn = parse_quote! {
			async fn create_user(request: CreateUserRequest) -> Result<(), ServerFnError> {
				Ok(())
			}
		};
		let info = ServerFnInfo {
			func,
			options: ServerFnOptions {
				pre_validate: true,
				..ServerFnOptions::default()
			},
			codec_explicit: false,
			metadata_name: None,
			endpoint_tokens: None,
			metadata_name_tokens: None,
			detail: false,
			transactional: false,
			structured_error: false,
		};

		let generated = generate_server_fn(&info).to_string();

		assert!(
			generated.contains("ServerFnError :: from"),
			"pre-validation failures must convert ValidationErrors into ServerFnError: {generated}"
		);
		assert!(
			generated.contains("Validate :: validate (& args . request)"),
			"pre-validation must validate the deserialized argument instead of the generated wrapper: {generated}"
		);
		assert!(
			generated.contains("serde_json :: to_vec"),
			"pre-validation failures must serialize the versioned error envelope: {generated}"
		);
		assert!(
			generated.contains("Bytes :: from_static"),
			"pre-validation failures must retain a valid envelope fallback: {generated}"
		);
	}

	/// Tests for `is_extractor_type` — verifies known extractor type detection.
	#[test]
	fn test_is_extractor_type_known_types() {
		use syn::parse_quote;

		// Known extractor types should return true
		let ty: syn::Type = parse_quote!(Validated<Form<LoginRequest>>);
		assert!(is_extractor_type(&ty), "Validated should be an extractor");

		let ty: syn::Type = parse_quote!(Header<String>);
		assert!(is_extractor_type(&ty), "Header should be an extractor");

		let ty: syn::Type = parse_quote!(Json<UserRequest>);
		assert!(is_extractor_type(&ty), "Json should be an extractor");

		let ty: syn::Type = parse_quote!(Form<CreateUser>);
		assert!(is_extractor_type(&ty), "Form should be an extractor");

		let ty: syn::Type = parse_quote!(Query<PaginationParams>);
		assert!(is_extractor_type(&ty), "Query should be an extractor");

		let ty: syn::Type = parse_quote!(Path<u32>);
		assert!(is_extractor_type(&ty), "Path should be an extractor");

		let ty: syn::Type = parse_quote!(Cookie<String>);
		assert!(is_extractor_type(&ty), "Cookie should be an extractor");

		let ty: syn::Type = parse_quote!(CookieNamed<SessionId, String>);
		assert!(is_extractor_type(&ty), "CookieNamed should be an extractor");

		let ty: syn::Type = parse_quote!(CookieStruct<MyCookies>);
		assert!(
			is_extractor_type(&ty),
			"CookieStruct should be an extractor"
		);

		let ty: syn::Type = parse_quote!(Body);
		assert!(is_extractor_type(&ty), "Body should be an extractor");

		let ty: syn::Type = parse_quote!(PolicyPrincipal<ArticleResource>);
		assert!(
			is_extractor_type(&ty),
			"model PolicyPrincipal should be an extractor"
		);
	}

	/// Tests for `is_extractor_type` — verifies that non-extractor types return false.
	#[test]
	fn test_is_extractor_type_non_extractors() {
		use syn::parse_quote;

		let ty: syn::Type = parse_quote!(u32);
		assert!(!is_extractor_type(&ty), "u32 should not be an extractor");

		let ty: syn::Type = parse_quote!(String);
		assert!(!is_extractor_type(&ty), "String should not be an extractor");

		let ty: syn::Type = parse_quote!(Database);
		assert!(
			!is_extractor_type(&ty),
			"Database should not be an extractor"
		);

		let ty: syn::Type = parse_quote!(Arc<Database>);
		assert!(
			!is_extractor_type(&ty),
			"Arc<Database> should not be an extractor"
		);

		let ty: syn::Type = parse_quote!(Vec<String>);
		assert!(
			!is_extractor_type(&ty),
			"Vec<String> should not be an extractor"
		);

		let ty: syn::Type = parse_quote!(PolicyPrincipal<String>);
		assert!(
			!is_extractor_type(&ty),
			"unrelated PolicyPrincipal types should not be extractors"
		);
	}

	/// Tests that `detect_extractor_params` correctly identifies extractor parameters
	/// and skips #[inject] params.
	#[test]
	fn test_detect_extractor_params_basic() {
		use syn::parse_quote;

		// Parse a function with mixed param types
		let func: syn::ItemFn = parse_quote! {
			async fn login(
				form: Validated<Form<LoginRequest>>,
				auth_header: Header<String>,
				name: String,
				#[reinhardt::inject] db: Database,
			) -> Result<(), ServerFnError> {}
		};

		let extractor_params = detect_extractor_params(&func.sig.inputs);

		// Should detect 2 extractor params (form + auth_header), not name or db
		assert_eq!(
			extractor_params.len(),
			2,
			"Expected 2 extractor params, got: {:#?}",
			extractor_params
				.iter()
				.map(|p| format!("{:?}", p.pat))
				.collect::<Vec<_>>()
		);
	}

	/// Tests that extractor params are not included in regular params
	#[test]
	fn test_extractor_params_excluded_from_regular() {
		use syn::parse_quote;

		// All params are extractors
		let func: syn::ItemFn = parse_quote! {
			async fn handler(
				form: Form<CreateUser>,
				hdr: Header<String>,
			) -> Result<(), ServerFnError> {}
		};

		let extractor_params = detect_extractor_params(&func.sig.inputs);
		assert_eq!(extractor_params.len(), 2);

		// No regular params should remain (simulating generate_server_handler logic)
		let regular_count = func
			.sig
			.inputs
			.iter()
			.filter(|arg| {
				if let syn::FnArg::Typed(pt) = arg {
					!is_extractor_type(&pt.ty)
				} else {
					false
				}
			})
			.count();
		assert_eq!(regular_count, 0, "All params should be extractors");
	}

	#[test]
	fn server_handler_preserves_interleaved_argument_order() {
		use syn::parse_quote;
		let func: ItemFn = parse_quote! {
			async fn handler(first: String, #[inject] mut left: Service, Json(last): Json<usize>, #[inject] mut right: Service) -> Result<(), ServerFnError> { Ok(()) }
		};
		let info = ServerFnInfo {
			func,
			options: ServerFnOptions::default(),
			codec_explicit: false,
			metadata_name: None,
			endpoint_tokens: None,
			metadata_name_tokens: None,
			detail: false,
			transactional: false,
			structured_error: false,
		};
		let inject = detect_inject_params(&info.func.sig.inputs);
		let extractors = detect_extractor_params(&info.func.sig.inputs);
		let generated = generate_server_handler(&info, &inject, &extractors, &[]).to_string();
		assert!(
			generated.contains(
				"handler (args . first , __server_fn_inject_0 , Json (last) , __server_fn_inject_1)"
			),
			"{generated}"
		);
	}

	#[test]
	fn server_handler_accepts_url_encoded_model_payload_fallback() {
		use syn::parse_quote;

		let func: ItemFn = parse_quote! {
		async fn save(data: QuestionModelFormData<AllEditableModelFields>) -> Result<(), ServerFnError> { Ok(()) }
		};
		let info = ServerFnInfo {
			func,
			options: ServerFnOptions {
				model_form: true,
				..ServerFnOptions::default()
			},
			codec_explicit: false,
			metadata_name: None,
			endpoint_tokens: None,
			metadata_name_tokens: None,
			detail: false,
			transactional: false,
			structured_error: false,
		};

		let generated = generate_server_handler(&info, &[], &[], &[]).to_string();

		assert!(
			generated.contains("NativeModelFormPayload")
				&& generated.contains("from_native_form_value"),
			"single payload handlers must accept progressive-enhancement form posts: {generated}"
		);
		assert!(
			generated.contains("let data : QuestionModelFormData")
				&& generated.contains("{ data }"),
			"the native fallback must bind and forward the declared parameter name: {generated}"
		);
	}

	#[test]
	fn server_handler_without_model_form_fallback_is_valid_rust() {
		use syn::parse_quote;

		let func: ItemFn = parse_quote! {
			async fn save(data: String) -> Result<(), ServerFnError> { Ok(()) }
		};
		let info = ServerFnInfo {
			func,
			options: ServerFnOptions::default(),
			codec_explicit: false,
			metadata_name: None,
			endpoint_tokens: None,
			metadata_name_tokens: None,
			detail: false,
			transactional: false,
			structured_error: false,
		};

		let generated = generate_server_handler(&info, &[], &[], &[]);

		assert!(
			syn::parse2::<syn::File>(generated).is_ok(),
			"a default JSON fallback must remain a valid match-arm expression"
		);
	}
}
