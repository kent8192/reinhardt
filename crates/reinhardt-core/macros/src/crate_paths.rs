//! Helper functions for dynamic crate path resolution using proc_macro_crate

use proc_macro2::TokenStream;
use quote::quote;

/// Resolves the path to the `serde` crate dynamically.
pub(crate) fn get_serde_crate() -> TokenStream {
	use proc_macro_crate::{FoundCrate, crate_name};

	match crate_name("serde") {
		Ok(FoundCrate::Itself) => quote!(crate),
		Ok(FoundCrate::Name(name)) => {
			let ident = syn::Ident::new(&name, proc_macro2::Span::call_site());
			quote!(::#ident)
		}
		Err(_) => quote!(::serde),
	}
}

/// Resolves the path to the `serde_json` crate dynamically.
pub(crate) fn get_serde_json_crate() -> TokenStream {
	use proc_macro_crate::{FoundCrate, crate_name};

	match crate_name("serde_json") {
		Ok(FoundCrate::Itself) => quote!(crate),
		Ok(FoundCrate::Name(name)) => {
			let ident = syn::Ident::new(&name, proc_macro2::Span::call_site());
			quote!(::#ident)
		}
		Err(_) => quote!(::serde_json),
	}
}

/// Resolves the path to the Reinhardt crate dynamically.
/// This supports different crate naming scenarios (reinhardt, reinhardt-web, reinhardt-core, etc.)
pub(crate) fn get_reinhardt_crate() -> TokenStream {
	use proc_macro_crate::{FoundCrate, crate_name};

	// Try reinhardt crate first (when used with `package = "reinhardt-web"`)
	match crate_name("reinhardt") {
		Ok(FoundCrate::Itself) => return quote!(crate),
		Ok(FoundCrate::Name(name)) => {
			let ident = syn::Ident::new(&name, proc_macro2::Span::call_site());
			return quote!(::#ident);
		}
		Err(_) => {}
	}

	// Try reinhardt-web (published package name)
	match crate_name("reinhardt-web") {
		Ok(FoundCrate::Itself) => return quote!(crate),
		Ok(FoundCrate::Name(name)) => {
			let ident = syn::Ident::new(&name, proc_macro2::Span::call_site());
			return quote!(::#ident);
		}
		Err(_) => {}
	}

	// Try via reinhardt-core crate
	match crate_name("reinhardt-core") {
		Ok(FoundCrate::Itself) => return quote!(crate),
		Ok(FoundCrate::Name(name)) => {
			let ident = syn::Ident::new(&name, proc_macro2::Span::call_site());
			return quote!(::#ident);
		}
		Err(_) => {}
	}

	// Final fallback
	quote!(::reinhardt_core)
}

/// Resolves the path to the reinhardt_di crate dynamically.
pub(crate) fn get_reinhardt_di_crate() -> TokenStream {
	use proc_macro_crate::{FoundCrate, crate_name};

	// Try direct crate first
	match crate_name("reinhardt-di") {
		Ok(FoundCrate::Itself) => return quote!(crate),
		Ok(FoundCrate::Name(name)) => {
			let ident = syn::Ident::new(&name, proc_macro2::Span::call_site());
			return quote!(::#ident);
		}
		Err(_) => {
			// Try via reinhardt crate
			match crate_name("reinhardt") {
				Ok(FoundCrate::Itself) => return quote!(crate::reinhardt_di),
				Ok(FoundCrate::Name(name)) => {
					let ident = syn::Ident::new(&name, proc_macro2::Span::call_site());
					return quote!(::#ident::reinhardt_di);
				}
				Err(_) => {}
			}

			// Try via reinhardt-web (published package name)
			match crate_name("reinhardt-web") {
				Ok(FoundCrate::Itself) => return quote!(crate::reinhardt_di),
				Ok(FoundCrate::Name(name)) => {
					let ident = syn::Ident::new(&name, proc_macro2::Span::call_site());
					return quote!(::#ident::reinhardt_di);
				}
				Err(_) => {}
			}
		}
	}

	// Final fallback
	quote!(::reinhardt_di)
}

/// Resolves the path to the reinhardt_conf crate dynamically.
pub(crate) fn get_reinhardt_conf_crate() -> TokenStream {
	use proc_macro_crate::{FoundCrate, crate_name};

	// Try direct crate first
	match crate_name("reinhardt-conf") {
		Ok(FoundCrate::Itself) => return quote!(crate),
		Ok(FoundCrate::Name(name)) => {
			let ident = syn::Ident::new(&name, proc_macro2::Span::call_site());
			return quote!(::#ident);
		}
		Err(_) => {}
	}

	// Try via reinhardt crate (when used with `package = "reinhardt-web"`)
	match crate_name("reinhardt") {
		Ok(FoundCrate::Itself) => return quote!(crate::conf),
		Ok(FoundCrate::Name(name)) => {
			let ident = syn::Ident::new(&name, proc_macro2::Span::call_site());
			return quote!(::#ident::conf);
		}
		Err(_) => {}
	}

	// Try via reinhardt-web (published package name)
	match crate_name("reinhardt-web") {
		Ok(FoundCrate::Itself) => return quote!(crate::conf),
		Ok(FoundCrate::Name(name)) => {
			let ident = syn::Ident::new(&name, proc_macro2::Span::call_site());
			return quote!(::#ident::conf);
		}
		Err(_) => {}
	}

	// Final fallback
	quote!(::reinhardt_conf)
}

/// Resolves the path to the reinhardt_core crate dynamically.
pub(crate) fn get_reinhardt_core_crate() -> TokenStream {
	use proc_macro_crate::{FoundCrate, crate_name};

	// Try direct crate first
	match crate_name("reinhardt-core") {
		Ok(FoundCrate::Itself) => return quote!(crate),
		Ok(FoundCrate::Name(name)) => {
			let ident = syn::Ident::new(&name, proc_macro2::Span::call_site());
			return quote!(::#ident);
		}
		Err(_) => {}
	}

	// Try via reinhardt crate (when used with `package = "reinhardt-web"`)
	match crate_name("reinhardt") {
		Ok(FoundCrate::Itself) => return quote!(crate::reinhardt_core),
		Ok(FoundCrate::Name(name)) => {
			let ident = syn::Ident::new(&name, proc_macro2::Span::call_site());
			return quote!(::#ident::reinhardt_core);
		}
		Err(_) => {}
	}

	// Try via reinhardt-web (published package name)
	match crate_name("reinhardt-web") {
		Ok(FoundCrate::Itself) => return quote!(crate::reinhardt_core),
		Ok(FoundCrate::Name(name)) => {
			let ident = syn::Ident::new(&name, proc_macro2::Span::call_site());
			return quote!(::#ident::reinhardt_core);
		}
		Err(_) => {}
	}

	// Final fallback
	quote!(::reinhardt_core)
}

/// Resolves the path to the reinhardt_auth crate dynamically.
pub(crate) fn get_reinhardt_auth_crate() -> TokenStream {
	use proc_macro_crate::{FoundCrate, crate_name};

	// Try direct crate first
	match crate_name("reinhardt-auth") {
		Ok(FoundCrate::Itself) => return quote!(crate),
		Ok(FoundCrate::Name(name)) => {
			let ident = syn::Ident::new(&name, proc_macro2::Span::call_site());
			return quote!(::#ident);
		}
		Err(_) => {}
	}

	// Try via reinhardt crate (when used with `package = "reinhardt-web"`)
	match crate_name("reinhardt") {
		Ok(FoundCrate::Itself) => return quote!(crate::reinhardt_auth),
		Ok(FoundCrate::Name(name)) => {
			let ident = syn::Ident::new(&name, proc_macro2::Span::call_site());
			return quote!(::#ident::reinhardt_auth);
		}
		Err(_) => {}
	}

	// Try via reinhardt-web (published package name)
	match crate_name("reinhardt-web") {
		Ok(FoundCrate::Itself) => return quote!(crate::reinhardt_auth),
		Ok(FoundCrate::Name(name)) => {
			let ident = syn::Ident::new(&name, proc_macro2::Span::call_site());
			return quote!(::#ident::reinhardt_auth);
		}
		Err(_) => {}
	}

	// Final fallback
	quote!(::reinhardt_auth)
}

/// Resolves the path to the reinhardt_openapi crate dynamically.
pub(crate) fn get_reinhardt_openapi_crate() -> TokenStream {
	use proc_macro_crate::{FoundCrate, crate_name};

	// Try direct crate first
	match crate_name("reinhardt-openapi") {
		Ok(FoundCrate::Itself) => return quote!(crate),
		Ok(FoundCrate::Name(name)) => {
			let ident = syn::Ident::new(&name, proc_macro2::Span::call_site());
			return quote!(::#ident);
		}
		Err(_) => {
			// Try via reinhardt crate
			match crate_name("reinhardt") {
				Ok(FoundCrate::Itself) => return quote!(crate::reinhardt_openapi),
				Ok(FoundCrate::Name(name)) => {
					let ident = syn::Ident::new(&name, proc_macro2::Span::call_site());
					return quote!(::#ident::reinhardt_openapi);
				}
				Err(_) => {}
			}
		}
	}

	// Final fallback
	quote!(::reinhardt_openapi)
}

/// Resolves the path to the reinhardt_db crate dynamically.
pub(crate) fn get_reinhardt_db_crate() -> TokenStream {
	use proc_macro_crate::{FoundCrate, crate_name};

	// Try direct crate first
	match crate_name("reinhardt-db") {
		Ok(FoundCrate::Itself) => return quote!(crate),
		Ok(FoundCrate::Name(name)) => {
			let ident = syn::Ident::new(&name, proc_macro2::Span::call_site());
			return quote!(::#ident);
		}
		Err(_) => {}
	}

	// Try via reinhardt crate
	match crate_name("reinhardt") {
		Ok(FoundCrate::Itself) => return quote!(crate::db),
		Ok(FoundCrate::Name(name)) => {
			let ident = syn::Ident::new(&name, proc_macro2::Span::call_site());
			return quote!(::#ident::db);
		}
		Err(_) => {}
	}

	// Try via reinhardt-web (published package name)
	match crate_name("reinhardt-web") {
		Ok(FoundCrate::Itself) => return quote!(crate::db),
		Ok(FoundCrate::Name(name)) => {
			let ident = syn::Ident::new(&name, proc_macro2::Span::call_site());
			return quote!(::#ident::db);
		}
		Err(_) => {}
	}

	// Final fallback
	quote!(::reinhardt_db)
}

/// Resolves the path to the database ORM API used by generated field codecs.
pub(crate) fn get_reinhardt_db_orm_crate() -> TokenStream {
	let db_crate = get_reinhardt_db_crate();
	quote!(#db_crate::orm)
}

/// Resolves the path to the reinhardt_orm module dynamically.
/// Note: reinhardt-orm has been merged into reinhardt-db as a submodule.
pub(crate) fn get_reinhardt_orm_crate() -> TokenStream {
	use proc_macro_crate::{FoundCrate, crate_name};

	// Try reinhardt-db first (orm is now a submodule of reinhardt-db)
	match crate_name("reinhardt-db") {
		Ok(FoundCrate::Itself) => return quote!(crate::orm),
		Ok(FoundCrate::Name(name)) => {
			let ident = syn::Ident::new(&name, proc_macro2::Span::call_site());
			return quote!(::#ident::orm);
		}
		Err(_) => {}
	}

	// Try via reinhardt crate
	match crate_name("reinhardt") {
		Ok(FoundCrate::Itself) => return quote!(crate::db::orm),
		Ok(FoundCrate::Name(name)) => {
			let ident = syn::Ident::new(&name, proc_macro2::Span::call_site());
			return quote!(::#ident::db::orm);
		}
		Err(_) => {}
	}

	// Try via reinhardt-web (published package name)
	match crate_name("reinhardt-web") {
		Ok(FoundCrate::Itself) => return quote!(crate::db::orm),
		Ok(FoundCrate::Name(name)) => {
			let ident = syn::Ident::new(&name, proc_macro2::Span::call_site());
			return quote!(::#ident::db::orm);
		}
		Err(_) => {}
	}

	// Final fallback
	quote!(::reinhardt::db::orm)
}

/// Resolves the path to the reinhardt_signals module dynamically.
/// Note: reinhardt-signals functionality has been merged into reinhardt-core as signals submodule.
pub(crate) fn get_reinhardt_signals_crate() -> TokenStream {
	use proc_macro_crate::{FoundCrate, crate_name};

	// Try reinhardt-core first (signals is now a submodule of reinhardt-core)
	match crate_name("reinhardt-core") {
		Ok(FoundCrate::Itself) => return quote!(crate::signals),
		Ok(FoundCrate::Name(name)) => {
			let ident = syn::Ident::new(&name, proc_macro2::Span::call_site());
			return quote!(::#ident::signals);
		}
		Err(_) => {}
	}

	// Try via reinhardt crate
	match crate_name("reinhardt") {
		Ok(FoundCrate::Itself) => return quote!(crate::core::signals),
		Ok(FoundCrate::Name(name)) => {
			let ident = syn::Ident::new(&name, proc_macro2::Span::call_site());
			return quote!(::#ident::core::signals);
		}
		Err(_) => {}
	}

	// Try via reinhardt-web (published package name)
	match crate_name("reinhardt-web") {
		Ok(FoundCrate::Itself) => return quote!(crate::core::signals),
		Ok(FoundCrate::Name(name)) => {
			let ident = syn::Ident::new(&name, proc_macro2::Span::call_site());
			return quote!(::#ident::core::signals);
		}
		Err(_) => {}
	}

	// Final fallback
	quote!(::reinhardt::core::signals)
}

/// Resolves the path to the reinhardt_params crate dynamically.
pub(crate) fn get_reinhardt_params_crate() -> TokenStream {
	use proc_macro_crate::{FoundCrate, crate_name};

	// Try direct crate first
	match crate_name("reinhardt-params") {
		Ok(FoundCrate::Itself) => return quote!(crate),
		Ok(FoundCrate::Name(name)) => {
			let ident = syn::Ident::new(&name, proc_macro2::Span::call_site());
			return quote!(::#ident);
		}
		Err(_) => {
			// Try via reinhardt crate
			match crate_name("reinhardt") {
				Ok(FoundCrate::Itself) => return quote!(crate::reinhardt_params),
				Ok(FoundCrate::Name(name)) => {
					let ident = syn::Ident::new(&name, proc_macro2::Span::call_site());
					return quote!(::#ident::reinhardt_params);
				}
				Err(_) => {}
			}

			// Try via reinhardt-web (published package name)
			match crate_name("reinhardt-web") {
				Ok(FoundCrate::Itself) => return quote!(crate::reinhardt_params),
				Ok(FoundCrate::Name(name)) => {
					let ident = syn::Ident::new(&name, proc_macro2::Span::call_site());
					return quote!(::#ident::reinhardt_params);
				}
				Err(_) => {}
			}
		}
	}

	// Final fallback
	quote!(::reinhardt_params)
}

/// Resolves the path to the reinhardt_exception crate dynamically.
#[allow(dead_code)]
pub(crate) fn get_reinhardt_exception_crate() -> TokenStream {
	use proc_macro_crate::{FoundCrate, crate_name};

	match crate_name("reinhardt-exception") {
		Ok(FoundCrate::Itself) => quote!(crate),
		Ok(FoundCrate::Name(name)) => {
			let ident = syn::Ident::new(&name, proc_macro2::Span::call_site());
			quote!(::#ident)
		}
		Err(_) => quote!(::reinhardt_exception), // Fallback
	}
}

/// Resolves the path to the reinhardt_urls crate dynamically.
///
/// Used to reference `UrlReverser` and its global accessor from the
/// typed-URL-helper module.
pub(crate) fn get_reinhardt_urls_crate() -> TokenStream {
	use proc_macro_crate::{FoundCrate, crate_name};

	// Try direct crate first.
	match crate_name("reinhardt-urls") {
		Ok(FoundCrate::Itself) => return quote!(crate),
		Ok(FoundCrate::Name(name)) => {
			let ident = syn::Ident::new(&name, proc_macro2::Span::call_site());
			return quote!(::#ident);
		}
		Err(_) => {}
	}

	// Try via the `reinhardt` facade (when used with `package = "reinhardt-web"`).
	match crate_name("reinhardt") {
		Ok(FoundCrate::Itself) => return quote!(crate::reinhardt_urls),
		Ok(FoundCrate::Name(name)) => {
			let ident = syn::Ident::new(&name, proc_macro2::Span::call_site());
			return quote!(::#ident::reinhardt_urls);
		}
		Err(_) => {}
	}

	// Try via the published `reinhardt-web` facade.
	match crate_name("reinhardt-web") {
		Ok(FoundCrate::Itself) => return quote!(crate::reinhardt_urls),
		Ok(FoundCrate::Name(name)) => {
			let ident = syn::Ident::new(&name, proc_macro2::Span::call_site());
			return quote!(::#ident::reinhardt_urls);
		}
		Err(_) => {}
	}

	// Final fallback — assume the published `reinhardt-urls` crate is present.
	quote!(::reinhardt_urls)
}

/// Resolves the path to the reinhardt_apps crate dynamically.
pub(crate) fn get_reinhardt_apps_crate() -> TokenStream {
	use proc_macro_crate::{FoundCrate, crate_name};

	// Try direct crate first
	match crate_name("reinhardt-apps") {
		Ok(FoundCrate::Itself) => return quote!(crate),
		Ok(FoundCrate::Name(name)) => {
			let ident = syn::Ident::new(&name, proc_macro2::Span::call_site());
			return quote!(::#ident);
		}
		Err(_) => {}
	}

	// Try via reinhardt crate (when used with `package = "reinhardt-web"`)
	match crate_name("reinhardt") {
		Ok(FoundCrate::Itself) => return quote!(crate::reinhardt_apps),
		Ok(FoundCrate::Name(name)) => {
			let ident = syn::Ident::new(&name, proc_macro2::Span::call_site());
			return quote!(::#ident::reinhardt_apps);
		}
		Err(_) => {}
	}

	// Try via reinhardt-web (published package name)
	match crate_name("reinhardt-web") {
		Ok(FoundCrate::Itself) => return quote!(crate::reinhardt_apps),
		Ok(FoundCrate::Name(name)) => {
			let ident = syn::Ident::new(&name, proc_macro2::Span::call_site());
			return quote!(::#ident::reinhardt_apps);
		}
		Err(_) => {}
	}

	// Final fallback - use reinhardt facade crate
	quote!(::reinhardt::reinhardt_apps)
}

/// Resolves the path to the reinhardt_migrations module dynamically.
/// Note: reinhardt-migrations has been merged into reinhardt-db as a submodule.
pub(crate) fn get_reinhardt_migrations_crate() -> TokenStream {
	use proc_macro_crate::{FoundCrate, crate_name};

	// Try reinhardt-db first (migrations is now a submodule of reinhardt-db)
	match crate_name("reinhardt-db") {
		Ok(FoundCrate::Itself) => return quote!(crate::migrations),
		Ok(FoundCrate::Name(name)) => {
			let ident = syn::Ident::new(&name, proc_macro2::Span::call_site());
			return quote!(::#ident::migrations);
		}
		Err(_) => {}
	}

	// Try via reinhardt crate (when used with `package = "reinhardt-web"`)
	match crate_name("reinhardt") {
		Ok(FoundCrate::Itself) => return quote!(crate::db::migrations),
		Ok(FoundCrate::Name(name)) => {
			let ident = syn::Ident::new(&name, proc_macro2::Span::call_site());
			return quote!(::#ident::db::migrations);
		}
		Err(_) => {}
	}

	// Try via reinhardt-web (published package name)
	match crate_name("reinhardt-web") {
		Ok(FoundCrate::Itself) => return quote!(crate::db::migrations),
		Ok(FoundCrate::Name(name)) => {
			let ident = syn::Ident::new(&name, proc_macro2::Span::call_site());
			return quote!(::#ident::db::migrations);
		}
		Err(_) => {}
	}

	// Final fallback
	quote!(::reinhardt::db::migrations)
}

/// Resolves the path to the reinhardt_proxy module dynamically.
/// Note: proxy functionality is part of reinhardt-urls crate.
pub(crate) fn get_reinhardt_proxy_crate() -> TokenStream {
	use proc_macro_crate::{FoundCrate, crate_name};

	// Try reinhardt-urls first (proxy is a submodule of reinhardt-urls)
	match crate_name("reinhardt-urls") {
		Ok(FoundCrate::Itself) => return quote!(crate::proxy),
		Ok(FoundCrate::Name(name)) => {
			let ident = syn::Ident::new(&name, proc_macro2::Span::call_site());
			return quote!(::#ident::proxy);
		}
		Err(_) => {}
	}

	// Try via reinhardt crate
	match crate_name("reinhardt") {
		Ok(FoundCrate::Itself) => return quote!(crate::urls::proxy),
		Ok(FoundCrate::Name(name)) => {
			let ident = syn::Ident::new(&name, proc_macro2::Span::call_site());
			return quote!(::#ident::urls::proxy);
		}
		Err(_) => {}
	}

	// Try via reinhardt-web (published package name)
	match crate_name("reinhardt-web") {
		Ok(FoundCrate::Itself) => return quote!(crate::urls::proxy),
		Ok(FoundCrate::Name(name)) => {
			let ident = syn::Ident::new(&name, proc_macro2::Span::call_site());
			return quote!(::#ident::urls::proxy);
		}
		Err(_) => {}
	}

	// Final fallback
	quote!(::reinhardt::urls::proxy)
}

/// Resolves the path to the reinhardt_http crate dynamically.
pub(crate) fn get_reinhardt_http_crate() -> TokenStream {
	use proc_macro_crate::{FoundCrate, crate_name};

	// Try direct crate first
	match crate_name("reinhardt-http") {
		Ok(FoundCrate::Itself) => return quote!(crate),
		Ok(FoundCrate::Name(name)) => {
			let ident = syn::Ident::new(&name, proc_macro2::Span::call_site());
			return quote!(::#ident);
		}
		Err(_) => {}
	}

	// Try via reinhardt crate (when used with `package = "reinhardt-web"`)
	match crate_name("reinhardt") {
		Ok(FoundCrate::Itself) => return quote!(crate::reinhardt_http),
		Ok(FoundCrate::Name(name)) => {
			let ident = syn::Ident::new(&name, proc_macro2::Span::call_site());
			return quote!(::#ident::reinhardt_http);
		}
		Err(_) => {}
	}

	// Try via reinhardt-web (published package name)
	match crate_name("reinhardt-web") {
		Ok(FoundCrate::Itself) => return quote!(crate::reinhardt_http),
		Ok(FoundCrate::Name(name)) => {
			let ident = syn::Ident::new(&name, proc_macro2::Span::call_site());
			return quote!(::#ident::reinhardt_http);
		}
		Err(_) => {}
	}

	// Final fallback
	quote!(::reinhardt_http)
}

/// Resolves the path to the async_trait crate dynamically.
pub(crate) fn get_async_trait_crate() -> TokenStream {
	use proc_macro_crate::{FoundCrate, crate_name};

	// Try direct crate first
	match crate_name("async-trait") {
		Ok(FoundCrate::Itself) => return quote!(crate),
		Ok(FoundCrate::Name(name)) => {
			let ident = syn::Ident::new(&name, proc_macro2::Span::call_site());
			return quote!(::#ident);
		}
		Err(_) => {}
	}

	// Try via reinhardt crate (when used with `package = "reinhardt-web"`)
	match crate_name("reinhardt") {
		Ok(FoundCrate::Itself) => return quote!(crate::async_trait),
		Ok(FoundCrate::Name(name)) => {
			let ident = syn::Ident::new(&name, proc_macro2::Span::call_site());
			return quote!(::#ident::async_trait);
		}
		Err(_) => {}
	}

	// Try via reinhardt-web (published package name)
	match crate_name("reinhardt-web") {
		Ok(FoundCrate::Itself) => return quote!(crate::async_trait),
		Ok(FoundCrate::Name(name)) => {
			let ident = syn::Ident::new(&name, proc_macro2::Span::call_site());
			return quote!(::#ident::async_trait);
		}
		Err(_) => {}
	}

	// Final fallback
	quote!(::async_trait)
}

/// Resolves the path to the reinhardt_admin_adapters crate dynamically.
pub(crate) fn get_reinhardt_admin_adapters_crate() -> TokenStream {
	use proc_macro_crate::{FoundCrate, crate_name};

	// Try direct crate first
	match crate_name("reinhardt-admin-adapters") {
		Ok(FoundCrate::Itself) => return quote!(crate),
		Ok(FoundCrate::Name(name)) => {
			let ident = syn::Ident::new(&name, proc_macro2::Span::call_site());
			return quote!(::#ident);
		}
		Err(_) => {}
	}

	// Try via reinhardt crate (when used with `package = "reinhardt-web"`)
	match crate_name("reinhardt") {
		Ok(FoundCrate::Itself) => return quote!(crate::admin),
		Ok(FoundCrate::Name(name)) => {
			let ident = syn::Ident::new(&name, proc_macro2::Span::call_site());
			return quote!(::#ident::admin);
		}
		Err(_) => {}
	}

	// Try via reinhardt-web (published package name)
	match crate_name("reinhardt-web") {
		Ok(FoundCrate::Itself) => return quote!(crate::admin),
		Ok(FoundCrate::Name(name)) => {
			let ident = syn::Ident::new(&name, proc_macro2::Span::call_site());
			return quote!(::#ident::admin);
		}
		Err(_) => {}
	}

	// Final fallback
	quote!(::reinhardt_admin_adapters)
}

/// Resolves the path to the linkme crate dynamically.
///
/// Fixes #793: Use dynamic resolution instead of hardcoded `::linkme` path.
pub(crate) fn get_linkme_crate() -> TokenStream {
	use proc_macro_crate::{FoundCrate, crate_name};

	// Try direct crate first
	match crate_name("linkme") {
		Ok(FoundCrate::Itself) => return quote!(crate),
		Ok(FoundCrate::Name(name)) => {
			let ident = syn::Ident::new(&name, proc_macro2::Span::call_site());
			return quote!(::#ident);
		}
		Err(_) => {}
	}

	// Try via reinhardt crate (when used with `package = "reinhardt-web"`)
	match crate_name("reinhardt") {
		Ok(FoundCrate::Itself) => return quote!(crate::linkme),
		Ok(FoundCrate::Name(name)) => {
			let ident = syn::Ident::new(&name, proc_macro2::Span::call_site());
			return quote!(::#ident::linkme);
		}
		Err(_) => {}
	}

	// Try via reinhardt-web (published package name)
	match crate_name("reinhardt-web") {
		Ok(FoundCrate::Itself) => return quote!(crate::linkme),
		Ok(FoundCrate::Name(name)) => {
			let ident = syn::Ident::new(&name, proc_macro2::Span::call_site());
			return quote!(::#ident::linkme);
		}
		Err(_) => {}
	}

	// Final fallback
	quote!(::linkme)
}

/// Resolves the path to the inventory crate dynamically.
pub(crate) fn get_inventory_crate() -> TokenStream {
	use proc_macro_crate::{FoundCrate, crate_name};

	// Try direct crate first
	match crate_name("inventory") {
		Ok(FoundCrate::Itself) => return quote!(crate),
		Ok(FoundCrate::Name(name)) => {
			let ident = syn::Ident::new(&name, proc_macro2::Span::call_site());
			return quote!(::#ident);
		}
		Err(_) => {}
	}

	// Try via reinhardt crate (when used with `package = "reinhardt-web"`)
	match crate_name("reinhardt") {
		Ok(FoundCrate::Itself) => return quote!(crate::inventory),
		Ok(FoundCrate::Name(name)) => {
			let ident = syn::Ident::new(&name, proc_macro2::Span::call_site());
			return quote!(::#ident::inventory);
		}
		Err(_) => {}
	}

	// Try via reinhardt-web (published package name)
	match crate_name("reinhardt-web") {
		Ok(FoundCrate::Itself) => return quote!(crate::inventory),
		Ok(FoundCrate::Name(name)) => {
			let ident = syn::Ident::new(&name, proc_macro2::Span::call_site());
			return quote!(::#ident::inventory);
		}
		Err(_) => {}
	}

	// Final fallback
	quote!(::inventory)
}

/// Resolves the path to the `reinhardt_websockets` crate dynamically.
pub(crate) fn get_reinhardt_websockets_crate() -> TokenStream {
	use proc_macro_crate::{FoundCrate, crate_name};

	match crate_name("reinhardt-websockets") {
		Ok(FoundCrate::Itself) => return quote!(crate),
		Ok(FoundCrate::Name(name)) => {
			let ident = syn::Ident::new(&name, proc_macro2::Span::call_site());
			return quote!(::#ident);
		}
		Err(_) => {}
	}

	match crate_name("reinhardt") {
		Ok(FoundCrate::Itself) => return quote!(crate::reinhardt_websockets),
		Ok(FoundCrate::Name(name)) => {
			let ident = syn::Ident::new(&name, proc_macro2::Span::call_site());
			return quote!(::#ident::reinhardt_websockets);
		}
		Err(_) => {}
	}

	match crate_name("reinhardt-web") {
		Ok(FoundCrate::Itself) => return quote!(crate::reinhardt_websockets),
		Ok(FoundCrate::Name(name)) => {
			let ident = syn::Ident::new(&name, proc_macro2::Span::call_site());
			return quote!(::#ident::reinhardt_websockets);
		}
		Err(_) => {}
	}

	quote!(::reinhardt_websockets)
}

/// Resolves the path to the `reinhardt_commands` crate dynamically.
///
/// Used by the `#[hook]` macro to reference `RunserverHookRegistration`.
pub(crate) fn get_reinhardt_commands_crate() -> TokenStream {
	use proc_macro_crate::{FoundCrate, crate_name};

	// Try direct crate first
	match crate_name("reinhardt-commands") {
		Ok(FoundCrate::Itself) => return quote!(crate),
		Ok(FoundCrate::Name(name)) => {
			let ident = syn::Ident::new(&name, proc_macro2::Span::call_site());
			return quote!(::#ident);
		}
		Err(_) => {}
	}

	// Try via reinhardt crate (when used with `package = "reinhardt-web"`)
	match crate_name("reinhardt") {
		Ok(FoundCrate::Itself) => return quote!(crate::reinhardt_commands),
		Ok(FoundCrate::Name(name)) => {
			let ident = syn::Ident::new(&name, proc_macro2::Span::call_site());
			return quote!(::#ident::reinhardt_commands);
		}
		Err(_) => {}
	}

	// Try via reinhardt-web (published package name)
	match crate_name("reinhardt-web") {
		Ok(FoundCrate::Itself) => return quote!(crate::reinhardt_commands),
		Ok(FoundCrate::Name(name)) => {
			let ident = syn::Ident::new(&name, proc_macro2::Span::call_site());
			return quote!(::#ident::reinhardt_commands);
		}
		Err(_) => {}
	}

	// Final fallback
	quote!(::reinhardt_commands)
}

/// Resolves the path to the `reinhardt_views` crate dynamically.
///
/// Used by the impl-form `#[viewset(basename = ...)]` macro (Issue #4507,
/// Phase 5.1) to emit `inventory::submit!` blocks that reference
/// `ActionRegistryEntry` / `ActionMetadata` under the runtime registry.
pub(crate) fn get_reinhardt_views_crate() -> TokenStream {
	use proc_macro_crate::{FoundCrate, crate_name};

	// Try direct crate first
	match crate_name("reinhardt-views") {
		Ok(FoundCrate::Itself) => return quote!(crate),
		Ok(FoundCrate::Name(name)) => {
			let ident = syn::Ident::new(&name, proc_macro2::Span::call_site());
			return quote!(::#ident);
		}
		Err(_) => {}
	}

	// Try via reinhardt crate (when used with `package = "reinhardt-web"`)
	match crate_name("reinhardt") {
		Ok(FoundCrate::Itself) => return quote!(crate::views),
		Ok(FoundCrate::Name(name)) => {
			let ident = syn::Ident::new(&name, proc_macro2::Span::call_site());
			return quote!(::#ident::views);
		}
		Err(_) => {}
	}

	// Try via reinhardt-web (published package name)
	match crate_name("reinhardt-web") {
		Ok(FoundCrate::Itself) => return quote!(crate::views),
		Ok(FoundCrate::Name(name)) => {
			let ident = syn::Ident::new(&name, proc_macro2::Span::call_site());
			return quote!(::#ident::views);
		}
		Err(_) => {}
	}

	// Final fallback
	quote!(::reinhardt_views)
}

/// Resolves the path to the `hyper` crate (or the equivalent facade re-export)
/// dynamically.
///
/// Used by the impl-form `#[viewset(basename = ...)]` macro (Issue #4507,
/// Phase 5.1) to construct `hyper::Method` values in emitted runtime
/// action-registration closures. Mirrors the facade-probing strategy in
/// [`get_reinhardt_views_crate`] so consumers that depend only on
/// `reinhardt` / `reinhardt-web` (which re-export `hyper::Method` and
/// `hyper::StatusCode` from their crate root — see `src/lib.rs`) compile
/// without having to add `hyper` as a direct dependency.
pub(crate) fn get_hyper_crate() -> TokenStream {
	use proc_macro_crate::{FoundCrate, crate_name};

	// Try direct hyper crate first
	match crate_name("hyper") {
		Ok(FoundCrate::Itself) => return quote!(crate),
		Ok(FoundCrate::Name(name)) => {
			let ident = syn::Ident::new(&name, proc_macro2::Span::call_site());
			return quote!(::#ident);
		}
		Err(_) => {}
	}

	// Try via reinhardt crate (when used with `package = "reinhardt-web"`).
	// `reinhardt-web/src/lib.rs` does `pub use hyper::{Method, StatusCode};`
	// at the crate root, so `::reinhardt::Method::from_bytes` resolves the
	// same inherent method as `::hyper::Method::from_bytes`.
	match crate_name("reinhardt") {
		Ok(FoundCrate::Itself) => return quote!(crate),
		Ok(FoundCrate::Name(name)) => {
			let ident = syn::Ident::new(&name, proc_macro2::Span::call_site());
			return quote!(::#ident);
		}
		Err(_) => {}
	}

	// Try via reinhardt-web (published package name) for the same reason.
	match crate_name("reinhardt-web") {
		Ok(FoundCrate::Itself) => return quote!(crate),
		Ok(FoundCrate::Name(name)) => {
			let ident = syn::Ident::new(&name, proc_macro2::Span::call_site());
			return quote!(::#ident);
		}
		Err(_) => {}
	}

	// Final fallback
	quote!(::hyper)
}
