use crate::crate_paths::get_reinhardt_apps_crate;
use proc_macro2::TokenStream;
use quote::quote;
use syn::{DeriveInput, Result, parse_macro_input};

/// One vendor asset declaration from
/// `vendor_assets(asset(url=..., target=..., sha256=?))`.
#[derive(Debug, Clone)]
struct VendorAssetEntry {
	url: String,
	target: String,
	sha256: Option<String>,
}

/// Configuration from `#[app_config(...)]` attribute.
#[derive(Debug, Clone)]
struct AppConfigAttr {
	name: String,
	label: String,
	verbose_name: Option<String>,
	vendor_assets: Vec<VendorAssetEntry>,
}

impl AppConfigAttr {
	/// Parse `#[app_config(...)]` or `#[app_config_internal(...)]` attribute.
	fn from_attrs(attrs: &[syn::Attribute], struct_name: &syn::Ident) -> Result<Self> {
		let mut name = None;
		let mut label = None;
		let mut verbose_name = None;
		let mut vendor_assets: Vec<VendorAssetEntry> = Vec::new();

		for attr in attrs {
			if !attr.path().is_ident("app_config") && !attr.path().is_ident("app_config_internal") {
				continue;
			}

			attr.parse_nested_meta(|meta| {
				if meta.path.is_ident("name") {
					let value: syn::LitStr = meta.value()?.parse()?;
					name = Some(value.value());
					Ok(())
				} else if meta.path.is_ident("label") {
					let value: syn::LitStr = meta.value()?.parse()?;
					label = Some(value.value());
					Ok(())
				} else if meta.path.is_ident("verbose_name") {
					let value: syn::LitStr = meta.value()?.parse()?;
					verbose_name = Some(value.value());
					Ok(())
				} else if meta.path.is_ident("vendor_assets") {
					// vendor_assets( asset(...), asset(...), ... )
					meta.parse_nested_meta(|asset_meta| {
						if !asset_meta.path.is_ident("asset") {
							return Err(asset_meta.error(
								"vendor_assets entries must use \
								 `asset(url = ..., target = ..., sha256 = ?)`",
							));
						}
						let mut url: Option<String> = None;
						let mut target: Option<String> = None;
						let mut sha256: Option<String> = None;
						asset_meta.parse_nested_meta(|field| {
							if field.path.is_ident("url") {
								let v: syn::LitStr = field.value()?.parse()?;
								url = Some(v.value());
								Ok(())
							} else if field.path.is_ident("target") {
								let v: syn::LitStr = field.value()?.parse()?;
								target = Some(v.value());
								Ok(())
							} else if field.path.is_ident("sha256") {
								let v: syn::LitStr = field.value()?.parse()?;
								sha256 = Some(v.value());
								Ok(())
							} else {
								Err(field.error("asset() supports only `url`, `target`, `sha256`"))
							}
						})?;
						let url = url.ok_or_else(|| asset_meta.error("asset() requires `url`"))?;
						let target =
							target.ok_or_else(|| asset_meta.error("asset() requires `target`"))?;
						vendor_assets.push(VendorAssetEntry {
							url,
							target,
							sha256,
						});
						Ok(())
					})
				} else {
					Err(meta.error("unsupported app_config attribute"))
				}
			})?;
		}

		let name = name.ok_or_else(|| {
			syn::Error::new_spanned(
				struct_name,
				"app_config attribute requires 'name' parameter",
			)
		})?;

		let label = label.ok_or_else(|| {
			syn::Error::new_spanned(
				struct_name,
				"app_config attribute requires 'label' parameter",
			)
		})?;

		Ok(Self {
			name,
			label,
			verbose_name,
			vendor_assets,
		})
	}
}

/// Derive AppConfig implementation.
pub(crate) fn derive(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
	let input = parse_macro_input!(input as DeriveInput);
	match derive_impl(input) {
		Ok(tokens) => tokens.into(),
		Err(err) => err.to_compile_error().into(),
	}
}

fn derive_impl(input: DeriveInput) -> Result<TokenStream> {
	let apps_crate = get_reinhardt_apps_crate();
	let struct_name = &input.ident;

	let has_internal_attr = input
		.attrs
		.iter()
		.any(|attr| attr.path().is_ident("app_config_internal"));

	if !has_internal_attr {
		return Err(syn::Error::new_spanned(
			struct_name,
			"Direct use of #[derive(AppConfig)] is not allowed. \
			 Use #[app_config(name = \"...\", label = \"...\")] instead.",
		));
	}

	let config = AppConfigAttr::from_attrs(&input.attrs, struct_name)?;

	let name = &config.name;
	let label = &config.label;

	let config_builder = if let Some(verbose_name) = &config.verbose_name {
		quote! {
			#apps_crate::AppConfig::new(#name, #label)
				.with_verbose_name(#verbose_name)
		}
	} else {
		quote! {
			#apps_crate::AppConfig::new(#name, #label)
		}
	};

	// One inventory::submit! per declared asset; the inventory crate handles
	// merging across crates at link time.
	let vendor_submissions = config.vendor_assets.iter().map(|entry| {
		let url = &entry.url;
		let target = &entry.target;
		let sha256 = entry.sha256.clone().unwrap_or_default();
		quote! {
			#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
			#apps_crate::inventory::submit! {
				#apps_crate::AppVendorAsset {
					app_label: #label,
					url: #url,
					target: #target,
					sha256: #sha256,
				}
			}
		}
	});

	let expanded = quote! {
		impl #struct_name {
			/// Create AppConfig instance.
			pub fn config() -> #apps_crate::AppConfig {
				#config_builder
			}
		}

		// Vendor asset registrations (one inventory::submit! per declared asset).
		#( #vendor_submissions )*

		#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
		#apps_crate::inventory::submit! {
			#apps_crate::AppModuleRegistration::new(#label, module_path!())
		}
	};

	Ok(expanded)
}
