//! End-to-end regression test for SPA navigation via the link interceptor.
//!
//! This is a NATIVE test (not wasm-bindgen-test). It:
//! 1. Builds the `spa_navigation_app` fixture WASM bundle via `wasm-pack build`.
//! 2. Boots an axum HTTP server on an ephemeral port serving the bundle.
//! 3. Uses the `cdp_browser` rstest fixture from `reinhardt-test` to spin up
//!    an isolated Chrome container and drive it through the navigation flow.
//!
//! Skipped (with a clear log line) if `wasm-pack` is not on `PATH`.
//!
//! Refs #4088.

#![cfg(all(feature = "e2e-cdp-test", not(target_arch = "wasm32")))]

use std::path::{Path, PathBuf};
use std::process::Command;

use reinhardt_test::fixtures::wasm::e2e_cdp::{CdpBrowser, cdp_browser};
use rstest::*;

const FIXTURE_DIR_REL: &str = "tests/fixtures/spa_navigation_app";

/// Locates the fixture crate root (relative to the test invocation cwd).
fn fixture_dir() -> PathBuf {
	let manifest = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR set by cargo");
	PathBuf::from(manifest).join(FIXTURE_DIR_REL)
}

/// Builds the fixture WASM bundle via `wasm-pack build --target web`.
/// Returns `Ok(Some(pkg_dir))` on success, `Ok(None)` if `wasm-pack` is missing.
fn build_fixture_bundle() -> Result<Option<PathBuf>, String> {
	if Command::new("wasm-pack").arg("--version").output().is_err() {
		return Ok(None);
	}
	let dir = fixture_dir();
	let status = Command::new("wasm-pack")
		.args(["build", "--target", "web", "--out-dir", "pkg"])
		.current_dir(&dir)
		.status()
		.map_err(|e| format!("wasm-pack failed to spawn: {e}"))?;
	if !status.success() {
		return Err(format!("wasm-pack build failed with status {status}"));
	}
	Ok(Some(dir.join("pkg")))
}

/// RAII guard that aborts the server task on drop, ensuring deterministic
/// cleanup regardless of test outcome (including panics).
struct ServerGuard {
	abort: tokio::task::AbortHandle,
}

impl Drop for ServerGuard {
	fn drop(&mut self) {
		self.abort.abort();
	}
}

/// Boots an axum server on an ephemeral port, serving the fixture index.html
/// and the WASM bundle. Returns the bound URL and a `ServerGuard` whose Drop
/// aborts the spawned task. Without the guard, dropping a bare `JoinHandle`
/// only detaches the task — the server would keep running until the tokio
/// runtime is torn down.
async fn boot_test_server(fixture_dir: &Path) -> (String, ServerGuard) {
	use axum::Router;
	use tower_http::services::ServeDir;

	// `append_index_html_on_directories(true)` makes ServeDir serve
	// `index.html` for directory requests like `/`. This is the documented
	// default in tower-http but is set explicitly so the test does not rely
	// on version-specific behavior.
	let app = Router::new()
		.fallback_service(ServeDir::new(fixture_dir).append_index_html_on_directories(true));

	// Bind to 0.0.0.0 so the chromedp container can reach the listener via
	// `host.docker.internal` on Linux CI. With `--add-host=...:host-gateway`
	// (set by `reinhardt-test`'s `cdp_browser` fixture), the name resolves
	// to the host's bridge interface (e.g. 172.17.0.1), which a 127.0.0.1
	// only listener will not accept. Refs #4111. See also the same pattern
	// used by `reinhardt-test::E2eTestServer` in `e2e_cdp.rs`.
	let listener = tokio::net::TcpListener::bind("0.0.0.0:0")
		.await
		.expect("bind ephemeral port");
	let port = listener.local_addr().expect("local_addr").port();
	let handle = tokio::spawn(async move {
		axum::serve(listener, app).await.expect("axum serve");
	});
	let guard = ServerGuard {
		abort: handle.abort_handle(),
	};
	(format!("http://host.docker.internal:{port}"), guard)
}

#[rstest]
#[tokio::test]
async fn spa_navigation_link_click_re_renders_view(#[future] cdp_browser: CdpBrowser) {
	// Arrange
	let pkg_dir = match build_fixture_bundle() {
		Ok(Some(p)) => p,
		Ok(None) => {
			eprintln!("[skip] wasm-pack not found on PATH; spa_navigation_e2e_test skipped");
			return;
		}
		Err(e) => panic!("fixture build failed: {e}"),
	};
	let fixture_root = pkg_dir
		.parent()
		.expect("pkg dir has parent (the fixture crate root)");
	let (base_url, _server) = boot_test_server(fixture_root).await;
	eprintln!("[e2e] base_url = {base_url}");
	let browser = cdp_browser.await;
	let page = browser
		.new_page("about:blank")
		.await
		.expect("open blank browser page");
	page.navigate(&base_url)
		.await
		.expect("navigate to fixture URL");

	// Boot mount: home page is rendered with the link to /login.
	// On failure, dump the URL Chrome actually navigated to and the page
	// HTML so CI logs reveal whether the WASM bundle loaded at all (e.g.
	// `host.docker.internal` resolution failures vs. mount-time errors).
	if let Err(e) = page.wait_for("#route-home").await {
		let actual_url = page.url().await.ok().flatten().unwrap_or_default();
		let html = page
			.content()
			.await
			.unwrap_or_else(|err| format!("<failed to read page content: {err:?}>"));
		panic!(
			"wait for #route-home failed: {e:?}\n\
			 base_url:   {base_url}\n\
			 actual url: {actual_url}\n\
			 html:       {html}"
		);
	}
	let go_to_login = page
		.find("#go-to-login")
		.await
		.expect("locate #go-to-login");

	// Act
	go_to_login.click().await.expect("click <a href=/login>");
	page.wait_for_url(|u| u.ends_with("/login"))
		.await
		.expect("URL updates to /login within timeout");
	page.wait_for("#route-login")
		.await
		.expect("login view mounts within timeout");

	// Assert
	let html = page.content().await.expect("page content");
	assert!(
		html.contains("LOGIN VIEW"),
		"expected LOGIN VIEW after link click; got: {html}"
	);
	assert!(
		html.contains("prepared route data"),
		"expected the route loader value to be rendered; got: {html}"
	);
	assert!(
		!html.contains("Go to login"),
		"home view should be unmounted after navigation; got: {html}"
	);
}
