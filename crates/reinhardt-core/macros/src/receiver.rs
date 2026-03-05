use crate::crate_paths::{get_inventory_crate, get_reinhardt_signals_crate};
use crate::injectable_common::{
	detect_inject_params, generate_di_context_extraction_from_option,
	generate_injection_calls_with_error, strip_inject_attrs,
};
use proc_macro2::TokenStream;
use quote::quote;
use syn::{FnArg, ItemFn, Result, Token, punctuated::Punctuated};

/// Parse receiver macro arguments
#[derive(Default)]
struct ReceiverArgs {
	signal_name: String,
	sender_type: Option<syn::Type>,
	dispatch_uid: Option<String>,
	priority: i32,
	use_inject: bool,
}

fn parse_receiver_args(args: TokenStream) -> Result<ReceiverArgs> {
	let mut result = ReceiverArgs::default();

	if args.is_empty() {
		return Ok(result);
	}

	// Parse arguments using ParseNestedMeta
	let parser = syn::meta::parser(|meta| {
		if meta.path.is_ident("signal") {
			result.signal_name = meta.value()?.parse::<syn::LitStr>()?.value();
			Ok(())
		} else if meta.path.is_ident("sender") {
			result.sender_type = Some(meta.value()?.parse()?);
			Ok(())
		} else if meta.path.is_ident("dispatch_uid") {
			result.dispatch_uid = Some(meta.value()?.parse::<syn::LitStr>()?.value());
			Ok(())
		} else if meta.path.is_ident("priority") {
			result.priority = meta.value()?.parse::<syn::LitInt>()?.base10_parse()?;
			Ok(())
		} else if meta.path.is_ident("use_inject") {
			result.use_inject = meta.value()?.parse::<syn::LitBool>()?.value();
			Ok(())
		} else {
			Err(meta.error("unsupported receiver attribute"))
		}
	});

	syn::parse::Parser::parse2(parser, args)?;
	Ok(result)
}

/// Implementation of the `receiver` procedural macro
///
/// This macro automatically registers receiver functions with the signal system
/// using the `inventory` crate for static collection.
///
/// # Arguments
///
/// - `signal`: Name of the signal to connect to (required)
/// - `sender`: Optional sender type filter
/// - `dispatch_uid`: Optional unique identifier for this receiver
/// - `priority`: Optional priority (higher values execute first, default: 0)
///
/// # Examples
///
/// See the signals documentation for usage examples.
pub(crate) fn receiver_impl(args: TokenStream, input: ItemFn) -> Result<TokenStream> {
	let args = parse_receiver_args(args)?;
	let signals_crate = get_reinhardt_signals_crate();
	// Fixes #793: Use dynamic crate path resolution instead of hardcoded ::inventory
	let inventory = get_inventory_crate();

	let fn_name = &input.sig.ident;
	let fn_vis = &input.vis;
	let fn_block = &input.block;
	let fn_attrs = &input.attrs;
	let fn_sig = &input.sig;

	// Generate registration code
	let signal_name = &args.signal_name;
	let receiver_name = fn_name.to_string();
	let dispatch_uid = args.dispatch_uid.as_deref();
	let priority = args.priority;
	let use_inject = args.use_inject;

	// Validate that the function has at least one parameter
	if fn_sig.inputs.is_empty() {
		return Err(syn::Error::new_spanned(
			fn_sig,
			"Receiver function must have at least one parameter",
		));
	}

	// Detect #[inject] parameters
	let inject_params = detect_inject_params(&fn_sig.inputs);

	// Validate: error if #[inject] is used when use_inject = false
	if !use_inject && !inject_params.is_empty() {
		return Err(syn::Error::new_spanned(
			&inject_params[0].pat,
			"#[inject] attribute requires use_inject = true option",
		));
	}

	// Build dispatch_uid setter
	let dispatch_uid_setter = if let Some(uid) = dispatch_uid {
		quote! { .with_dispatch_uid(#uid) }
	} else {
		quote! {}
	};

	// Build priority setter
	let priority_setter = if priority != 0 {
		quote! { .with_priority(#priority) }
	} else {
		quote! {}
	};

	// Build sender type setter
	let sender_type_setter = if let Some(sender_type) = &args.sender_type {
		// Generate a function that returns TypeId for the sender type
		quote! { .with_sender_type(|| ::std::any::TypeId::of::<#sender_type>()) }
	} else {
		quote! {}
	};

	// Generate wrapper function for DI support
	if use_inject && !inject_params.is_empty() {
		let original_fn_name = quote::format_ident!("{}_impl", fn_name);

		// Original function (with #[inject] attributes stripped)
		let stripped_inputs = strip_inject_attrs(&fn_sig.inputs);
		let stripped_inputs = Punctuated::<FnArg, Token![,]>::from_iter(stripped_inputs);

		// DI context extraction code (from ReceiverContext)
		let ctx_ident = syn::Ident::new("__receiver_ctx", proc_macro2::Span::call_site());
		let di_extraction = generate_di_context_extraction_from_option(&ctx_ident);

		// Error mapper for signals
		// Clone signals_crate for use in closure
		let signals_crate_for_mapper = signals_crate.clone();
		let error_mapper = move |_ty: &syn::Type| {
			let sc = signals_crate_for_mapper.clone();
			quote! {
				#sc::SignalError::new(
					format!("Dependency injection failed for {}: {:?}", stringify!(#_ty), e)
				)
			}
		};

		let injection_calls = generate_injection_calls_with_error(&inject_params, error_mapper);

		// Argument list
		let inject_args: Vec<_> = inject_params.iter().map(|p| &p.pat).collect();
		let regular_args: Vec<_> = stripped_inputs
			.iter()
			.filter_map(|arg| {
				if let FnArg::Typed(pat_type) = arg {
					Some(&pat_type.pat)
				} else {
					None
				}
			})
			.collect();

		// Build the receiver factory function with DI support
		let factory_fn = quote! {
			|| -> ::std::sync::Arc<
				dyn Fn(
					::std::sync::Arc<dyn ::std::any::Any + Send + Sync>,
					#signals_crate::ReceiverContext
				) -> ::std::pin::Pin<Box<dyn ::std::future::Future<Output = Result<(), #signals_crate::SignalError>> + Send>>
					+ Send + Sync
			> {
				::std::sync::Arc::new(|data, ctx| {
					Box::pin(async move {
						#fn_name(data, ctx).await
					})
				})
			}
		};

		let expanded = quote! {
			// Original function (renamed)
			#(#fn_attrs)*
			async fn #original_fn_name(#stripped_inputs) -> Result<(), #signals_crate::SignalError> {
				#fn_block
			}

			// DI-enabled wrapper (with ReceiverContext)
			#(#fn_attrs)*
			#fn_vis async fn #fn_name(
				instance: ::std::sync::Arc<dyn ::std::any::Any + Send + Sync>,
				__receiver_ctx: #signals_crate::ReceiverContext,
			) -> Result<(), #signals_crate::SignalError> {
				// DI context extraction
				#di_extraction

				// Dependency resolution
				#(#injection_calls)*

				// Call original function
				#original_fn_name(instance, #(#regular_args,)* #(#inject_args),*).await
			}

			// Generate static registration
			#inventory::submit! {
				#signals_crate::ReceiverRegistryEntry::new(
					#signal_name,
					#receiver_name,
					#factory_fn,
				)
				#dispatch_uid_setter
				#priority_setter
				#sender_type_setter
			}
		};

		Ok(expanded)
	} else {
		// Without DI, use conventional approach
		let factory_fn = quote! {
			|| -> ::std::sync::Arc<
				dyn Fn(::std::sync::Arc<dyn ::std::any::Any + Send + Sync>)
					-> ::std::pin::Pin<Box<dyn ::std::future::Future<Output = Result<(), #signals_crate::SignalError>> + Send>>
					+ Send + Sync
			> {
				::std::sync::Arc::new(|data| {
					Box::pin(async move {
						#fn_name(data).await
					})
				})
			}
		};

		let expanded = quote! {
			// Keep the original function
			#(#fn_attrs)*
			#fn_vis #fn_sig {
				#fn_block
			}

			// Generate static registration
			#inventory::submit! {
				#signals_crate::ReceiverRegistryEntry::new(
					#signal_name,
					#receiver_name,
					#factory_fn,
				)
				#dispatch_uid_setter
				#priority_setter
				#sender_type_setter
			}
		};

		Ok(expanded)
	}
}
