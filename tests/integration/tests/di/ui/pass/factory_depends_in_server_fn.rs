//! Compile-pass test: `Depends<FactoryType>` in `#[server_fn]`.
//!
//! Regression guard for issue #3723 (fixed in PR #3725); test coverage
//! added via issue #3727. The `#[server_fn]` macro routes `Depends<T>`
//! parameters through `resolve_from_registry()`, which has no
//! `T: Injectable` bound. This allows factory-produced types registered
//! via `#[injectable_factory]` to be injected without a manual
//! `Injectable` implementation. A regression that reintroduces the
//! `Injectable` bound on the `Depends<T>` codegen path would cause this
//! test to fail to compile.

use reinhardt_di::{Depends, injectable_factory};
use reinhardt_pages::server_fn::{ServerFnError, server_fn};

#[derive(Clone, Debug)]
struct AppConfig {
	host: String,
}

// Factory-registered type: deliberately no `impl Injectable`.
#[injectable_factory(scope = "transient")]
async fn make_app_config() -> AppConfig {
	AppConfig {
		host: "localhost".to_string(),
	}
}

#[server_fn]
async fn get_host(#[inject] cfg: Depends<AppConfig>) -> Result<String, ServerFnError> {
	Ok(cfg.host.clone())
}

fn main() {}
