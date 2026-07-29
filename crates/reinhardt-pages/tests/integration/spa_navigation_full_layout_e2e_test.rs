//! Tier 3 e2e regression test — SPA navigation through real Chrome via
//! CDP. Drives the persistent-layout-shell fixture
//! (`spa_navigation_with_full_layout_app`) with sidebar `<a>` clicks
//! and asserts both DOM swap AND the diagnostic counter increments
//! (Inv-1, Inv-3, Inv-4) via `execute_js`.
//!
//! This is a NATIVE test (not wasm-bindgen-test). It:
//! 1. Builds the Tier 3 fixture WASM bundle via `wasm-pack build`.
//! 2. Boots an axum HTTP server on an ephemeral port serving the bundle
//!    plus the fixture's `index.html`. The HTML wires the
//!    `__diag_*_js` `#[wasm_bindgen]` exports onto `window` so this
//!    test can read them through `execute_js`.
//! 3. Uses the `cdp_browser` rstest fixture from `reinhardt-test` to
//!    spin up an isolated Chrome container and drive it through the
//!    sidebar navigation flow.
//!
//! Skipped (with a clear log line) if `wasm-pack` is not on `PATH`.
//!
//! Tests pass on current main HEAD — they exist as a future-regression
//! net for the listener-loss class (#4075, #4088, #4122) so it cannot
//! silently re-emerge through a real source change or a stale-wasm-
//! bundle deployment hazard (the actual root cause of #4122; see
//! tracking issues #4127 / #4128).
//!
//! Refs #4122.

#![cfg(all(feature = "e2e-cdp-test", not(target_arch = "wasm32")))]

use std::path::{Path, PathBuf};
use std::process::Command;

use reinhardt_test::fixtures::wasm::e2e_cdp::{CdpBrowser, cdp_browser};
use rstest::*;

const FIXTURE_DIR_REL: &str = "tests/fixtures/spa_navigation_with_full_layout_app";

/// Locates the Tier 3 fixture crate root (relative to the test invocation cwd).
fn fixture_dir() -> PathBuf {
	let manifest = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR set by cargo");
	PathBuf::from(manifest).join(FIXTURE_DIR_REL)
}

/// Builds the fixture WASM bundle via `wasm-pack build --target web`.
///
/// Returns `Ok(Some(pkg_dir))` on success and `Ok(None)` when `wasm-pack`
/// is missing on a developer workstation. **In CI** (`CI=true`) a missing
/// `wasm-pack` is escalated to an error so a misconfigured runner cannot
/// silently disable the regression net (Copilot review feedback on
/// PR #4129).
fn build_fixture_bundle() -> Result<Option<PathBuf>, String> {
	if Command::new("wasm-pack").arg("--version").output().is_err() {
		if std::env::var("CI").is_ok() {
			return Err(
				"wasm-pack not on PATH but CI=true; refusing to skip the Tier 3 e2e regression net silently"
					.to_string(),
			);
		}
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
/// aborts the spawned task. Mirrors the layout used by
/// `spa_navigation_e2e_test.rs`.
async fn boot_test_server(fixture_dir: &Path) -> (String, ServerGuard) {
	use axum::Router;
	use tower_http::services::ServeDir;

	let app = Router::new()
		.fallback_service(ServeDir::new(fixture_dir).append_index_html_on_directories(true));

	// Bind to 0.0.0.0 so the chromedp container can reach the listener via
	// `host.docker.internal` on Linux CI. Refs #4111.
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

/// Reads a `u64`-valued diagnostic counter exposed by the fixture
/// (`__diag_dispatch_count_js` / `__diag_render_count_js`). The
/// fixture's `index.html` wires these into `window`, so they are
/// reachable as bare globals.
///
/// wasm-bindgen marshals Rust `u64` as JavaScript `BigInt`, which
/// `serde_json` cannot deserialize directly (it is not a JSON type).
/// We therefore wrap the call site with `String(...)` so the value
/// crosses the CDP boundary as a JSON string, then parse it on the
/// Rust side.
async fn read_u64_counter(
	page: &reinhardt_test::fixtures::wasm::e2e_cdp::CdpPage,
	js_name: &str,
) -> u64 {
	let val = page
		.execute_js(&format!("String({}())", js_name))
		.await
		.unwrap_or_else(|e| panic!("execute_js {js_name} failed: {e:?}"));
	let s = val
		.as_str()
		.unwrap_or_else(|| panic!("expected {js_name} stringified value; got {val:?}"));
	s.parse::<u64>()
		.unwrap_or_else(|e| panic!("expected {js_name} to parse as u64; got {s:?}: {e}"))
}

/// Reads a `usize`-valued diagnostic counter
/// (`__diag_observer_count_js`). wasm-bindgen marshals Rust `usize`
/// (32-bit on wasm32) as a JS `Number`, which is JSON-friendly.
async fn read_usize_counter(
	page: &reinhardt_test::fixtures::wasm::e2e_cdp::CdpPage,
	js_name: &str,
) -> usize {
	let val = page
		.execute_js(&format!("{}()", js_name))
		.await
		.unwrap_or_else(|e| panic!("execute_js {js_name} failed: {e:?}"));
	val.as_u64().unwrap_or_else(|| {
		panic!("expected {js_name} to return usize-compatible value; got {val:?}")
	}) as usize
}

#[rstest]
#[tokio::test]
async fn spa_navigation_full_layout_e2e(#[future] cdp_browser: CdpBrowser) {
	// Arrange: build the fixture bundle and boot the server.
	let pkg_dir = match build_fixture_bundle() {
		Ok(Some(p)) => p,
		Ok(None) => {
			eprintln!(
				"[skip] wasm-pack not found on PATH; spa_navigation_full_layout_e2e_test skipped"
			);
			return;
		}
		Err(e) => panic!("Tier 3 fixture build failed: {e}"),
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

	// Boot mount: home content section is rendered under the layout shell.
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

	// Inv-1 (e2e): launch must register at least one render listener.
	let observer_baseline = read_usize_counter(&page, "__diag_observer_count_js").await;
	assert!(
		observer_baseline >= 1,
		"Inv-1 e2e: launch must register render listener; got {}",
		observer_baseline
	);

	// Capture dispatch / render baselines after boot but before any click.
	let dispatch_baseline = read_u64_counter(&page, "__diag_dispatch_count_js").await;
	let render_baseline = read_u64_counter(&page, "__diag_render_count_js").await;

	// Act 1: click the sidebar link to /clusters.
	page.click("a[href='/clusters']")
		.await
		.expect("click sidebar /clusters link");
	page.wait_for_url(|u| u.ends_with("/clusters"))
		.await
		.expect("URL updates to /clusters within timeout");
	page.wait_for("#route-clusters")
		.await
		.expect("clusters view mounts within timeout");

	// Assert (step 1): Inv-3 / Inv-4 / Inv-2 hold and DOM swapped.
	let dispatch_after_one = read_u64_counter(&page, "__diag_dispatch_count_js").await;
	let render_after_one = read_u64_counter(&page, "__diag_render_count_js").await;
	let observer_after_one = read_usize_counter(&page, "__diag_observer_count_js").await;
	assert_eq!(
		dispatch_after_one,
		dispatch_baseline + 1,
		"Inv-3 e2e (click 1): dispatch_count expected {} got {}",
		dispatch_baseline + 1,
		dispatch_after_one
	);
	assert_eq!(
		render_after_one,
		render_baseline + 1,
		"Inv-4 e2e (click 1): render_count expected {} got {}",
		render_baseline + 1,
		render_after_one
	);
	assert!(
		observer_after_one >= observer_baseline,
		"Inv-2 e2e (click 1): observer count dropped {} -> {}",
		observer_baseline,
		observer_after_one
	);

	// Act 2: click the sidebar link to /login.
	page.click("a[href='/login']")
		.await
		.expect("click sidebar /login link");
	page.wait_for_url(|u| u.ends_with("/login"))
		.await
		.expect("URL updates to /login within timeout");
	page.wait_for("#route-login")
		.await
		.expect("login view mounts within timeout");

	// Assert (step 2): cumulative increments and DOM swap.
	let dispatch_after_two = read_u64_counter(&page, "__diag_dispatch_count_js").await;
	let render_after_two = read_u64_counter(&page, "__diag_render_count_js").await;
	let observer_after_two = read_usize_counter(&page, "__diag_observer_count_js").await;
	assert_eq!(
		dispatch_after_two,
		dispatch_baseline + 2,
		"Inv-3 e2e (click 2): dispatch_count expected {} got {}",
		dispatch_baseline + 2,
		dispatch_after_two
	);
	assert_eq!(
		render_after_two,
		render_baseline + 2,
		"Inv-4 e2e (click 2): render_count expected {} got {}",
		render_baseline + 2,
		render_after_two
	);
	assert!(
		observer_after_two >= observer_after_one,
		"Inv-2 e2e (click 2): observer count dropped {} -> {}",
		observer_after_one,
		observer_after_two
	);

	// Final DOM-swap sanity check via the rendered HTML.
	let html = page.content().await.expect("page content");
	assert!(
		html.contains("LOGIN VIEW"),
		"expected LOGIN VIEW after second navigation; got: {html}"
	);
	assert!(
		!html.contains("CLUSTERS VIEW"),
		"clusters view must be unmounted after navigation to /login; got: {html}"
	);
}
