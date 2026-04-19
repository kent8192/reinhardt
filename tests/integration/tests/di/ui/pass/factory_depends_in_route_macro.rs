//! Compile-pass test: `Depends<FactoryType>` in `#[get]` handler.
//!
//! Regression guard for issue #3723 (fixed in PR #3725). The `#[get]`
//! route macro routes `Depends<T>` parameters through
//! `resolve_from_registry()`, which has no `T: Injectable` bound. This
//! allows factory-produced types registered via `#[injectable_factory]`
//! to be injected without a manual `Injectable` implementation. A
//! regression that reintroduces the `Injectable` bound on the
//! `Depends<T>` codegen path would cause this test to fail to compile.

use reinhardt_di::{Depends, injectable_factory};
use reinhardt_http::{Response, ViewResult};
use reinhardt_macros::get;

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

#[get("/hello", name = "hello_factory_depends")]
async fn hello(#[inject] cfg: Depends<AppConfig>) -> ViewResult<Response> {
	Ok(Response::ok().with_body(cfg.host.clone()))
}

fn main() {}
