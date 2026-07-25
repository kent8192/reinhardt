//! Integration tests for the project-static auto-mount behavior introduced
//! by Issue #4484.
//!
//! These tests exercise the *cascade* of two `StaticFilesMiddleware`
//! instances that `runserver --with-pages` registers when a
//! `<project-root>/static/` directory is present:
//!
//! 1. A project-static middleware mounted at `/static/` with
//!    `spa_mode = false` and `passthrough_prefixes = ["/static/admin/"]`.
//! 2. A collected-dist middleware mounted at `/static/` without SPA fallback.
//! 3. The existing dist middleware mounted at `/` (covers WASM bundle and
//!    SPA fallback) and excluding `/static/` from SPA fallback.
//!
//! Anything that neither middleware serves falls through to the application
//! router. The tests fake the application router with a small handler so the
//! cascade can be exercised end-to-end through a real `HttpServer`.

use super::server_test_helpers::{shutdown_test_server, spawn_test_server};
use async_trait::async_trait;
use reinhardt_core::exception::Result;
use reinhardt_http::{Handler, MiddlewareChain, Request, Response};
use reinhardt_utils::staticfiles::{StaticFilesMiddleware, StaticMiddlewareConfig};
use std::sync::Arc;
use tempfile::TempDir;

/// Handler that simulates the application router's `/static/admin/...` mount
/// (provided by `admin_static_routes()` in production). It returns a sentinel
/// body so the test can confirm passthrough reached the router.
struct AdminAssetRouter;

#[async_trait]
impl Handler for AdminAssetRouter {
	async fn handle(&self, request: Request) -> Result<Response> {
		let path = request.uri.path();
		if let Some(rest) = path.strip_prefix("/static/admin/") {
			Ok(Response::ok()
				.with_header("Content-Type", "text/css")
				.with_body(format!("/* admin-bundle:{} */", rest)))
		} else {
			Ok(Response::not_found().with_body("not found"))
		}
	}
}

/// Build the cascade as `runserver --with-pages` does: project-static added
/// first (outermost in the `MiddlewareChain`), dist-static second (inner),
/// final handler = mock application router.
fn build_runserver_cascade(
	project_static_dir: std::path::PathBuf,
	dist_dir: std::path::PathBuf,
) -> Arc<dyn Handler> {
	let manifest_aliases = std::fs::read_to_string(dist_dir.join("manifest.json"))
		.ok()
		.and_then(|content| serde_json::from_str::<serde_json::Value>(&content).ok())
		.and_then(|manifest| manifest.get("paths").cloned())
		.and_then(|paths| serde_json::from_value(paths).ok())
		.unwrap_or_default();
	let project_static = Arc::new(StaticFilesMiddleware::new(
		StaticMiddlewareConfig::new(project_static_dir)
			.url_prefix("/static/")
			.spa_mode(false)
			.auto_inject_wasm(false)
			.passthrough_prefixes(vec!["/static/admin/".to_string()]),
	));
	let dist_static = Arc::new(StaticFilesMiddleware::new(
		StaticMiddlewareConfig::new(dist_dir.clone())
			.url_prefix("/static/")
			.spa_mode(false)
			.auto_inject_wasm(false)
			.manifest_aliases(manifest_aliases),
	));
	let dist_spa = Arc::new(StaticFilesMiddleware::new(
		StaticMiddlewareConfig::new(dist_dir)
			.url_prefix("/")
			.spa_mode(true)
			.excluded_prefixes(vec![
				"/api/".to_string(),
				"/admin/".to_string(),
				"/static/".to_string(),
			]),
	));
	// MiddlewareChain::handle iterates `.iter().rev()` at request time, so
	// the first `.with_middleware` becomes the outermost layer and executes
	// first (project_static), then falls through to dist_static, then to the
	// base handler. The reversal happens during request handling, not chain
	// construction.
	let chain = MiddlewareChain::new(Arc::new(AdminAssetRouter))
		.with_middleware(project_static)
		.with_middleware(dist_static)
		.with_middleware(dist_spa);
	Arc::new(chain)
}

#[tokio::test]
async fn test_project_static_serves_project_css() {
	// Arrange — project-root layout with hand-written CSS plus a dist/
	// containing only the SPA shell.
	let project = TempDir::new().unwrap();
	let project_static = project.path().join("static");
	std::fs::create_dir_all(project_static.join("css")).unwrap();
	std::fs::write(
		project_static.join("css/style.css"),
		"body { color: rebeccapurple; }",
	)
	.unwrap();
	let dist = project.path().join("dist");
	std::fs::create_dir_all(&dist).unwrap();
	std::fs::write(dist.join("index.html"), "<html><body>spa</body></html>").unwrap();

	let handler = build_runserver_cascade(project_static, dist);
	let (url, server_handle) = spawn_test_server(handler).await;

	// Act
	let response = reqwest::get(format!("{}/static/css/style.css", url))
		.await
		.expect("request failed");

	// Assert
	assert_eq!(response.status(), 200);
	let content_type = response
		.headers()
		.get("content-type")
		.expect("missing Content-Type")
		.to_str()
		.unwrap()
		.to_owned();
	assert!(
		content_type.contains("text/css"),
		"expected text/css, got {content_type}"
	);
	let body = response.text().await.unwrap();
	assert_eq!(body, "body { color: rebeccapurple; }");

	shutdown_test_server(server_handle).await;
}

#[tokio::test]
async fn test_admin_static_wins_over_project_collision() {
	// Arrange — project mistakenly contains static/admin/foo.css; the
	// project-static middleware must skip the path so the admin router wins.
	let project = TempDir::new().unwrap();
	let project_static = project.path().join("static");
	std::fs::create_dir_all(project_static.join("admin")).unwrap();
	std::fs::write(
		project_static.join("admin/foo.css"),
		"/* user-supplied collision */",
	)
	.unwrap();
	let dist = project.path().join("dist");
	std::fs::create_dir_all(&dist).unwrap();
	std::fs::write(dist.join("index.html"), "<html></html>").unwrap();

	let handler = build_runserver_cascade(project_static, dist);
	let (url, server_handle) = spawn_test_server(handler).await;

	// Act
	let response = reqwest::get(format!("{}/static/admin/foo.css", url))
		.await
		.expect("request failed");

	// Assert
	assert_eq!(response.status(), 200);
	let body = response.text().await.unwrap();
	assert_eq!(
		body, "/* admin-bundle:foo.css */",
		"admin router must win over a colliding file in <project>/static/admin/"
	);

	shutdown_test_server(server_handle).await;
}

#[tokio::test]
async fn test_dist_assets_still_served_alongside_project_static() {
	// Arrange — dist/ has WASM bundle outputs; project-static must not
	// shadow them since they live at `/` not `/static/`.
	let project = TempDir::new().unwrap();
	let project_static = project.path().join("static");
	std::fs::create_dir_all(&project_static).unwrap();
	let dist = project.path().join("dist");
	std::fs::create_dir_all(&dist).unwrap();
	std::fs::write(dist.join("app.js"), "// bundled js").unwrap();
	std::fs::write(dist.join("app_bg.wasm"), b"\0asm\x01\x00\x00\x00").unwrap();
	std::fs::write(
		dist.join("index.html"),
		"<html><body>spa shell</body></html>",
	)
	.unwrap();

	let handler = build_runserver_cascade(project_static, dist);
	let (url, server_handle) = spawn_test_server(handler).await;

	// Act
	let js = reqwest::get(format!("{}/app.js", url))
		.await
		.expect("request failed");
	let spa = reqwest::get(format!("{}/some/spa/route", url))
		.await
		.expect("request failed");

	// Assert
	assert_eq!(js.status(), 200);
	assert_eq!(js.text().await.unwrap(), "// bundled js");
	assert_eq!(spa.status(), 200);
	assert!(
		spa.text().await.unwrap().contains("spa shell"),
		"unknown SPA route must fall back to dist/index.html"
	);

	shutdown_test_server(server_handle).await;
}

#[tokio::test]
async fn test_collected_dist_assets_are_served_under_static_url() {
	let project = TempDir::new().unwrap();
	let project_static = project.path().join("static");
	std::fs::create_dir_all(&project_static).unwrap();
	let dist = project.path().join("dist");
	std::fs::create_dir_all(dist.join("vendor")).unwrap();
	std::fs::write(
		dist.join("vendor/unocss-runtime.1234.js"),
		"export const runtime = true;",
	)
	.unwrap();
	std::fs::write(
		dist.join("manifest.json"),
		r#"{"version":"1.0","paths":{"vendor/unocss-runtime.js":"vendor/unocss-runtime.1234.js"}}"#,
	)
	.unwrap();
	std::fs::write(dist.join("index.html"), "<html><body>spa</body></html>").unwrap();

	let handler = build_runserver_cascade(project_static, dist);
	let (url, server_handle) = spawn_test_server(handler).await;
	let response = reqwest::get(format!("{}/static/vendor/unocss-runtime.js", url))
		.await
		.expect("request failed");

	assert_eq!(response.status(), 200);
	assert_eq!(
		response
			.headers()
			.get("content-type")
			.expect("missing Content-Type")
			.to_str()
			.unwrap(),
		"text/javascript"
	);
	assert_eq!(
		response.text().await.unwrap(),
		"export const runtime = true;"
	);

	shutdown_test_server(server_handle).await;
}

#[tokio::test]
async fn test_project_static_missing_does_not_use_dist_spa_fallback() {
	// Arrange — request for a file that exists in neither project-static
	// nor dist. SPA mode is OFF on both static-prefix mounts, and the root
	// dist middleware excludes the configured static prefix from fallback.
	let project = TempDir::new().unwrap();
	let project_static = project.path().join("static");
	std::fs::create_dir_all(&project_static).unwrap();
	let dist = project.path().join("dist");
	std::fs::create_dir_all(&dist).unwrap();
	std::fs::write(dist.join("index.html"), "<html><body>spa</body></html>").unwrap();

	let handler = build_runserver_cascade(project_static, dist);
	let (url, server_handle) = spawn_test_server(handler).await;

	// Act
	let response = reqwest::get(format!("{}/static/missing.css", url))
		.await
		.expect("request failed");

	// Assert — static asset misses must reach the application router instead
	// of being converted into a successful SPA HTML response.
	assert_eq!(response.status(), 404);
	let body = response.text().await.unwrap();
	assert_eq!(body, "not found");

	shutdown_test_server(server_handle).await;
}
