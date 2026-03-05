//! Permission decorator macro

use crate::crate_paths::{get_reinhardt_auth_crate, get_reinhardt_core_crate};
use crate::permission_macro;
use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::{
	Error, Expr, ExprLit, ItemFn, Lit, LitStr, Meta, Result, Token, parse::Parser,
	punctuated::Punctuated, spanned::Spanned,
};

/// Validate a single permission string at compile time
fn validate_permission(permission: &str, span: Span) -> Result<()> {
	permission_macro::parse_and_validate(permission)
		.map(|_| ())
		.map_err(|e| Error::new(span, format!("Invalid permission string: {}", e)))
}
/// Implementation of the `permission_required` procedural macro
///
/// This function is used internally by the `#[permission_required]` attribute macro.
/// Users should not call this function directly.
///
/// # Implementation Details
///
/// The macro performs two types of validation:
///
/// 1. **Compile-time validation**: Validates permission string format
/// 2. **Runtime validation** (if Request parameter exists): Checks user permissions at runtime
///
/// # Examples
///
/// See the authentication documentation for usage examples.
pub(crate) fn permission_required_impl(args: TokenStream, input: ItemFn) -> Result<TokenStream> {
	let mut permissions = Vec::new();

	// Handle the common case: #[permission_required("auth.view_user")]
	// Try to parse as a single string literal first
	if let Ok(lit) = syn::parse2::<LitStr>(args.clone()) {
		let perm_str = lit.value();
		validate_permission(&perm_str, lit.span())?;
		permissions.push(perm_str);
	} else {
		// Parse permission arguments for other formats
		let meta_list = Punctuated::<Meta, Token![,]>::parse_terminated.parse2(args)?;

		for meta in meta_list {
			match meta {
				Meta::Path(p) => {
					if let Some(ident) = p.get_ident() {
						let perm_str = ident.to_string();
						validate_permission(&perm_str, p.span())?;
						permissions.push(perm_str);
					}
				}
				Meta::NameValue(nv) if nv.path.is_ident("permissions") => {
					if let Expr::Lit(ExprLit {
						lit: Lit::Str(lit), ..
					}) = &nv.value
					{
						// Parse permissions array
						let perms_str = lit.value();
						let perms_str = perms_str.trim_matches(|c| c == '[' || c == ']');

						for perm in perms_str.split(',') {
							let perm = perm.trim().trim_matches('"');
							validate_permission(perm, lit.span())?;
							permissions.push(perm.to_string());
						}
					}
				}
				_ => {
					return Err(Error::new_spanned(
						&meta,
						"unknown attribute in permission_required macro",
					));
				}
			}
		}
	}

	let fn_name = &input.sig.ident;
	let fn_block = &input.block;
	let fn_inputs = &input.sig.inputs;
	let fn_output = &input.sig.output;
	let fn_vis = &input.vis;
	let fn_attrs = &input.attrs;
	let asyncness = &input.sig.asyncness;

	let perm_list = permissions.join(", ");
	let perm_doc = format!("Required permissions: {}", perm_list);

	// Find the Request parameter name (optional for runtime checking)
	let request_param = fn_inputs.iter().find_map(|arg| {
		if let syn::FnArg::Typed(pat_type) = arg
			&& let syn::Pat::Ident(pat_ident) = &*pat_type.pat
		{
			// Check if the type is Request
			if let syn::Type::Path(type_path) = &*pat_type.ty
				&& type_path
					.path
					.segments
					.last()
					.map(|seg| seg.ident == "Request")
					.unwrap_or(false)
			{
				return Some(&pat_ident.ident);
			}
		}
		None
	});

	// Build permission check expressions
	let perm_checks: Vec<_> = permissions
		.iter()
		.map(|perm| {
			quote! { #perm }
		})
		.collect();

	// Resolve crate paths dynamically to support different crate naming scenarios
	let auth_crate = get_reinhardt_auth_crate();
	let core_crate = get_reinhardt_core_crate();

	// Generate runtime permission checking code (requires Request parameter)
	let permission_check = if let Some(request_ident) = request_param {
		quote! {
			// Runtime permission check using Request parameter
			// Extract user from request extensions (stored as Arc<dyn PermissionsMixin> by auth middleware)
			let user = #request_ident.extensions.get::<std::sync::Arc<dyn #auth_crate::PermissionsMixin>>()
				.ok_or_else(|| #core_crate::exception::Error::Authorization(
					"Authentication required. User not found in request context.".to_string()
				))?;

			// Check all required permissions
			let required_permissions = &[#(#perm_checks),*];
			if !user.has_perms(required_permissions) {
				return Err(#core_crate::exception::Error::Authorization(
					format!("Permission denied. Required permissions: {}", required_permissions.join(", "))
				).into());
			}
		}
	} else {
		// No Request parameter: emit compile error to prevent silent permission bypass
		// Security: Functions decorated with #[permission_required] MUST have a Request parameter
		// for runtime permission enforcement
		return Err(syn::Error::new_spanned(
			&input.sig,
			"#[permission_required] requires a Request parameter for runtime permission checking. \
			 Add a `request: Request` parameter to this function, or remove the #[permission_required] attribute \
			 if permission checking is handled elsewhere.",
		));
	};

	// Inject permission check into function body
	Ok(quote! {
		#(#fn_attrs)*
		#[doc = #perm_doc]
		#fn_vis #asyncness fn #fn_name(#fn_inputs) #fn_output {
			#permission_check
			#fn_block
		}
	})
}
