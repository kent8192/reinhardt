//! End-to-end checks that `startproject --with-pages` and
//! `startapp --with-pages` produce the directory layout taught in the
//! Reinhardt basics tutorial (`examples/examples-tutorial-basis/`).
//!
//! These tests guard against regressions of #3970 — every assertion below
//! corresponds to a concrete divergence reported in that issue.

use reinhardt_commands::start_commands::{StartAppCommand, StartProjectCommand};
use reinhardt_commands::{BaseCommand, CommandContext};
use rstest::*;
use serial_test::serial;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use tempfile::TempDir;

/// RAII guard that restores the process-wide current working directory when
/// dropped, including on panic-driven unwind. Tests under `#[serial(cwd)]`
/// mutate global state via `std::env::set_current_dir`, so without this guard
/// a panic inside `execute(...)` would leave the CWD pointing at a `TempDir`
/// that gets deleted at end-of-scope, corrupting subsequent tests.
struct CwdGuard {
	prev: std::path::PathBuf,
}

impl CwdGuard {
	fn enter(new_cwd: &Path) -> Self {
		let prev = std::env::current_dir().unwrap();
		std::env::set_current_dir(new_cwd).unwrap();
		Self { prev }
	}
}

impl Drop for CwdGuard {
	fn drop(&mut self) {
		// Best-effort restore — swallow errors during unwind so we never
		// double-panic. The original directory may itself have been removed
		// in pathological cases, which is acceptable for test cleanup.
		let _ = std::env::set_current_dir(&self.prev);
	}
}

/// Helper: build a `CommandContext` whose only option is `--with-pages`.
fn pages_context(args: Vec<String>) -> CommandContext {
	let mut ctx = CommandContext::new(args);
	let mut opts = HashMap::new();
	opts.insert("with-pages".to_string(), vec!["true".to_string()]);
	ctx = ctx.with_options(opts);
	ctx
}

fn assert_empty_client_style(style_rs: &str, app_name: &str, style_type: &str) {
	let expected = format!(
		concat!(
			"//! Component styles for the {app_name} application.\n\n",
			"use reinhardt::pages::style_def;\n\n",
			"/// Component styles for the {app_name} application.\n",
			"#[style_def]\n",
			"pub static STYLES: {style_type} = style! {{\n",
			"    // Add component styles here.\n",
			"}};\n",
		),
		app_name = app_name,
		style_type = style_type,
	);

	assert_eq!(style_rs, expected);
}

fn assert_models_placeholder_is_tutorial_safe(
	models_rs: &str,
	app_label: &str,
	expected_type: &str,
) {
	assert!(
		models_rs.contains("Replace this placeholder with the models for the app."),
		"models.rs placeholder must be explicit that tutorial users replace it:\n{models_rs}"
	);
	assert!(
		!models_rs.contains("#[user("),
		"models.rs placeholder must not include a generic auth User example; the tutorial owns that code:\n{models_rs}"
	);
	assert!(
		models_rs.contains("use reinhardt::prelude::*;"),
		"models.rs placeholder example must import the prelude so #[model] resolves:\n{models_rs}"
	);
	assert!(
		models_rs.contains("use reinhardt::{Deserialize, Serialize};"),
		"models.rs placeholder example must avoid undeclared direct serde dependency:\n{models_rs}"
	);
	let model_attr =
		format!("#[model(app_label = \"{app_label}\", table_name = \"{app_label}_item\")]");
	assert!(
		models_rs.contains(&model_attr),
		"models.rs placeholder example must include the generated app_label and explicit table name:\n{models_rs}"
	);
	assert!(
		models_rs.contains(&format!("pub struct {expected_type}")),
		"models.rs placeholder example must render the app-specific type name:\n{models_rs}"
	);
	let model_pos = models_rs
		.find(&model_attr)
		.expect("model attribute checked above");
	let derive_pos = models_rs
		.find("#[derive(Serialize, Deserialize)]")
		.expect("derive attribute must be present");
	assert!(
		model_pos < derive_pos,
		"#[model] must be shown before #[derive] so macro helper attributes are in scope:\n{models_rs}"
	);
}

#[rstest]
#[tokio::test]
#[serial(cwd)]
async fn project_pages_layout_matches_tutorial() {
	// Arrange
	let tmp = TempDir::new().unwrap();
	let _cwd_guard = CwdGuard::enter(tmp.path());

	// Act
	let res = StartProjectCommand
		.execute(&pages_context(vec!["polls_project".to_string()]))
		.await;

	// Assert
	res.expect("startproject --with-pages must succeed");

	let project = tmp.path().join("polls_project");
	let src = project.join("src");

	let cargo_toml = fs::read_to_string(project.join("Cargo.toml")).expect("read Cargo.toml");
	assert_eq!(
		cargo_toml
			.lines()
			.filter(|line| line.trim() == "rust_decimal = \"1\"")
			.count(),
		1,
		"Pages project Cargo.toml must declare rust_decimal exactly once:\n{cargo_toml}"
	);
	assert!(
		cargo_toml.contains("[workspace]") && cargo_toml.contains("resolver = \"3\""),
		"Pages projects must be standalone Cargo workspace roots so they build inside another workspace:\n{cargo_toml}"
	);
	assert!(
		cargo_toml.contains("members = ["),
		"Pages project Cargo.toml must include a members array for startapp --workspace:\n{cargo_toml}"
	);
	for feature in [
		"minimal",
		"pages",
		"client-router",
		"admin",
		"conf",
		"commands",
		"commands-server",
		"commands-autoreload",
		"server",
		"db-sqlite",
		"forms",
		"auth-session",
		"middleware",
		"argon2-hasher",
	] {
		assert!(
			cargo_toml.contains(&format!("\"{feature}\"")),
			"Pages project native dependency must include `{feature}` for the generated dev workflow:\n{cargo_toml}"
		);
	}
	assert!(
		!cargo_toml.contains("\"db-postgres\""),
		"Pages project native dependency must keep the SQLite default:\n{cargo_toml}"
	);
	let makefile = fs::read_to_string(project.join("Makefile.toml")).expect("read Makefile.toml");
	assert!(
		makefile.contains("[tasks.install-tools]"),
		"Pages project Makefile.toml must include the install-tools task advertised by startproject:\n{makefile}"
	);
	assert!(
		makefile.contains("command = \"wasm-pack\"")
			&& makefile.contains("\"--out-dir\", \"dist-wasm\""),
		"Pages project Makefile.toml must build browser artifacts through wasm-pack:\n{makefile}"
	);
	assert!(
		makefile.contains("[tasks.wasm-test]")
			&& makefile.contains("\"--features\", \"client-router,msw\""),
		"Pages project Makefile.toml must include the browser test task used by the tutorial:\n{makefile}"
	);
	assert!(
		!makefile.contains("ls target/wasm32-unknown-unknown") && !makefile.contains("head -1"),
		"Pages project Makefile.toml must not select an arbitrary .wasm artifact:\n{makefile}"
	);
	assert!(
		project.join("scripts/wasm-build-dev.sh").exists()
			&& project.join("scripts/wasm-build-release.sh").exists(),
		"Pages project must include WASM post-build helper scripts"
	);

	let settings_rs =
		fs::read_to_string(src.join("config").join("settings.rs")).expect("read settings.rs");
	assert!(
		settings_rs.contains(
			"core: CoreSettings | contacts: ContactSettings | migrations: MigrationSettings"
		),
		"ProjectSettings must compose common settings required by management commands:\n{settings_rs}"
	);
	let base_toml =
		fs::read_to_string(project.join("settings").join("base.toml")).expect("read base.toml");
	assert!(
		base_toml.contains("secret_key = \"insecure-"),
		"generated base.toml must contain the generated development secret:\n{base_toml}"
	);
	assert!(
		base_toml.contains("[contacts]")
			&& base_toml.contains("admins = []")
			&& base_toml.contains("managers = []"),
		"generated base.toml must include the contacts fragment:\n{base_toml}"
	);
	let base_example = fs::read_to_string(project.join("settings").join("base.example.toml"))
		.expect("read base.example.toml");
	assert!(
		base_example.contains("secret_key = \"CHANGE_THIS_IN_PRODUCTION_MUST_BE_KEPT_SECRET\""),
		"base.example.toml must keep a safe placeholder secret:\n{base_example}"
	);

	// 1. Per-app reorg: top-level `src/server_fn.rs` no longer exists; each
	//    app owns its own under `apps/<app>/server_fn.rs`.
	assert!(
		!src.join("server_fn.rs").exists(),
		"src/server_fn.rs must NOT exist (per-app under apps/<app>/server_fn.rs)"
	);
	assert!(
		!src.join("server_fn").exists(),
		"src/server_fn/ directory must NOT exist"
	);
	assert!(
		!src.join("server").exists(),
		"src/server/ must not be generated"
	);

	// 2. config/wasm.rs is required for collectstatic to find dist-wasm.
	assert!(
		src.join("config").join("wasm.rs").exists(),
		"src/config/wasm.rs must exist (collectstatic registration)"
	);

	// 3. Client shell: lib.rs (WASM entry) + components.rs (`pub mod nav;`
	//    only). Per-app components live under `apps/<app>/client/`.
	assert!(
		src.join("client").join("lib.rs").exists(),
		"src/client/lib.rs must exist (WASM entry point)"
	);
	assert!(
		src.join("client").join("components.rs").exists(),
		"src/client/components.rs must exist (shared shell aggregator)"
	);
	assert!(
		src.join("client")
			.join("components")
			.join("nav.rs")
			.exists(),
		"src/client/components/nav.rs must exist (shared with_nav helper)"
	);
	assert!(
		!src.join("client").join("pages.rs").exists(),
		"src/client/pages.rs must NOT exist (route-backed components live under apps/<app>/client/components/)"
	);
	assert!(
		!src.join("client").join("router.rs").exists(),
		"src/client/router.rs must NOT exist (replaced by per-app urls.rs)"
	);
	assert!(
		!src.join("client").join("bootstrap.rs").exists(),
		"src/client/bootstrap.rs must not be generated"
	);
	assert!(
		!src.join("client").join("state.rs").exists(),
		"src/client/state.rs must not be generated"
	);

	// 4. lib.rs declares the server-only re-export shim and un-gates apps.
	let lib_rs = fs::read_to_string(src.join("lib.rs")).expect("read lib.rs");
	assert!(
		lib_rs.contains("mod server_only"),
		"src/lib.rs must declare the `mod server_only` re-export shim:\n{lib_rs}"
	);
	assert!(
		!lib_rs.contains("pub mod server_fn;"),
		"src/lib.rs must NOT declare `pub mod server_fn;` (moved per-app):\n{lib_rs}"
	);
	// `pub mod apps;` must be un-gated (no `#[cfg(server)]` directly above it).
	for cfg_line in [
		"#[cfg(server)]\npub mod apps;",
		"#[cfg(not(client))]\npub mod apps;",
	] {
		assert!(
			!lib_rs.contains(cfg_line),
			"src/lib.rs must declare `pub mod apps;` without a cfg gate (found {cfg_line:?}):\n{lib_rs}"
		);
	}
	assert!(
		lib_rs.contains("pub mod apps;"),
		"src/lib.rs must still declare `pub mod apps;`:\n{lib_rs}"
	);

	// 4b. The shared components shell aggregator declares only `pub mod nav;`.
	let components_rs = fs::read_to_string(src.join("client").join("components.rs"))
		.expect("read client/components.rs");
	assert!(
		components_rs.contains("pub mod nav;"),
		"src/client/components.rs must declare `pub mod nav;`:\n{components_rs}"
	);
	for unwanted in ["pub mod polls", "pub mod users"] {
		assert!(
			!components_rs.contains(unwanted),
			"src/client/components.rs must not preserve per-app submodules ({unwanted}):\n{components_rs}"
		);
	}

	// 5. Root shared modules are not generated. Manual wire DTOs live inside
	//    each app under serializers/, shared model info DTOs come from
	//    app-level models.rs, and server forms live under server/forms/.
	assert!(
		!src.join("shared.rs").exists() && !src.join("shared").exists(),
		"src/shared.rs and src/shared/ must not be generated"
	);

	// 6. Part 5 (Testing) directory is in place.
	assert!(
		project.join("tests").join("integration.rs").exists(),
		"tests/integration.rs must exist (Part 5 testing scaffold)"
	);
	assert!(
		project.join("tests").join("wasm").exists(),
		"tests/wasm/ must exist (Part 5 WASM tests directory)"
	);
}

/// Set up a project so that `startapp --with-pages` can run inside it.
async fn scaffold_pages_project(tmp: &Path, name: &str) {
	let _cwd_guard = CwdGuard::enter(tmp);
	let cmd = StartProjectCommand;
	let ctx = pages_context(vec![name.to_string()]);
	let result = cmd.execute(&ctx).await;
	result.expect("startproject --with-pages must succeed");
}

#[rstest]
#[tokio::test]
#[serial(cwd)]
async fn app_pages_layout_matches_tutorial() {
	// Arrange
	let tmp = TempDir::new().unwrap();
	let project_name = "polls_project";
	scaffold_pages_project(tmp.path(), project_name).await;

	let project_dir = tmp.path().join(project_name);
	let _cwd_guard = CwdGuard::enter(&project_dir);

	// Act
	let res = StartAppCommand
		.execute(&pages_context(vec!["polls".to_string()]))
		.await;

	// Assert
	res.expect("startapp --with-pages must succeed");

	let apps = project_dir.join("src").join("apps");

	// 1. apps/<app>.rs is the module entry point (Rust 2024 edition).
	assert!(
		apps.join("polls.rs").exists(),
		"src/apps/polls.rs must exist as the app's module entry point"
	);
	let polls_rs = fs::read_to_string(apps.join("polls.rs")).expect("read apps/polls.rs");
	assert!(
		polls_rs.contains("#[app_config(name = \"polls\", label = \"polls\")]"),
		"apps/polls.rs must carry the #[app_config] attribute:\n{polls_rs}"
	);

	// 2. Shared models live at the app root; server-only implementation
	//    submodules live under apps/<app>/server/.
	let polls_dir = apps.join("polls");
	assert!(
		polls_dir.join("models.rs").exists(),
		"apps/polls/models.rs must exist so generated info DTOs are shared with WASM"
	);
	let models_rs =
		fs::read_to_string(polls_dir.join("models.rs")).expect("read apps/polls/models.rs");
	assert_models_placeholder_is_tutorial_safe(&models_rs, "polls", "PollsItem");
	assert!(
		polls_dir.join("server.rs").exists(),
		"apps/polls/server.rs must exist as the server facade"
	);
	let server_dir = polls_dir.join("server");
	assert!(
		server_dir.join("forms.rs").exists(),
		"apps/polls/server/forms.rs must exist as the server forms facade"
	);
	assert!(
		server_dir.join("forms").join(".gitkeep").exists(),
		"apps/polls/server/forms/.gitkeep must preserve the empty forms directory"
	);
	assert!(
		server_dir.join("views.rs").exists(),
		"apps/polls/server/views.rs must exist"
	);

	// 3. Server-only implementation files must not be mixed into the app root.
	//    `client/` is REQUIRED at the per-app level (per-app UI lives here).
	for unwanted in [
		"admin.rs",
		"models",
		"forms",
		"forms.rs",
		"views.rs",
		"shared",
		"pages.rs",
		"server/models",
		"server/models.rs",
		"server/urls.rs",
		"client/pages.rs",
	] {
		let path = polls_dir.join(unwanted);
		assert!(
			!path.exists(),
			"apps/polls/{unwanted} must not be generated (was: {})",
			path.display()
		);
	}

	// 4. Per-app common facades, cfg-gated client/server facades, and
	//    implementation directories are generated.
	assert!(
		polls_dir.join("serializers.rs").exists(),
		"apps/polls/serializers.rs must exist"
	);
	assert!(
		polls_dir.join("serializers").join(".gitkeep").exists(),
		"apps/polls/serializers/.gitkeep must preserve the empty serializers directory"
	);
	assert!(
		polls_dir.join("server_fn.rs").exists(),
		"apps/polls/server_fn.rs must exist"
	);
	let server_fn_rs =
		fs::read_to_string(polls_dir.join("server_fn.rs")).expect("read apps/polls/server_fn.rs");
	assert!(
		server_fn_rs.contains("pub mod placeholder;")
			&& !server_fn_rs.contains("pub use placeholder::placeholder;"),
		"apps/polls/server_fn.rs must be a declaration-only facade:\n{server_fn_rs}"
	);
	let server_fn_placeholder =
		fs::read_to_string(polls_dir.join("server_fn").join("placeholder.rs"))
			.expect("read apps/polls/server_fn/placeholder.rs");
	assert!(
		server_fn_placeholder.contains("#[server_fn]")
			&& server_fn_placeholder.contains("pub async fn placeholder"),
		"apps/polls/server_fn/placeholder.rs must contain the #[server_fn]-annotated placeholder:\n{server_fn_placeholder}"
	);
	assert!(
		polls_dir.join("services.rs").exists(),
		"apps/polls/services.rs must exist"
	);
	let services_rs =
		fs::read_to_string(polls_dir.join("services.rs")).expect("read apps/polls/services.rs");
	assert!(
		services_rs.contains("#[cfg(client)]\npub mod client;")
			&& services_rs.contains("#[cfg(server)]\npub mod server;"),
		"apps/polls/services.rs must split client/server services with cfg gates:\n{services_rs}"
	);
	assert!(
		polls_dir.join("services").join("client.rs").exists()
			&& polls_dir
				.join("services")
				.join("client")
				.join(".gitkeep")
				.exists(),
		"apps/polls/services/client.rs and services/client/.gitkeep must exist"
	);
	assert!(
		polls_dir.join("services").join("server.rs").exists()
			&& polls_dir
				.join("services")
				.join("server")
				.join(".gitkeep")
				.exists(),
		"apps/polls/services/server.rs and services/server/.gitkeep must exist"
	);

	assert!(
		polls_dir.join("client.rs").exists(),
		"apps/polls/client.rs must exist"
	);
	let client_rs =
		fs::read_to_string(polls_dir.join("client.rs")).expect("read apps/polls/client.rs");
	assert!(
		client_rs.contains("pub mod components;")
			&& client_rs.contains("pub mod hooks;")
			&& client_rs.contains("pub mod style;")
			&& !client_rs.contains("pub mod pages;"),
		"apps/polls/client.rs must be a client-only facade, not a mixed route-entry facade:\n{client_rs}"
	);
	let style_rs = fs::read_to_string(polls_dir.join("client").join("style.rs"))
		.expect("read apps/polls/client/style.rs");
	assert_empty_client_style(&style_rs, "polls", "PollsStyles");
	assert!(
		!polls_dir.join("style.rs").exists(),
		"apps/polls/style.rs must not exist; component styles are client-only"
	);
	let hooks_rs = fs::read_to_string(polls_dir.join("client").join("hooks.rs"))
		.expect("read apps/polls/client/hooks.rs");
	assert_eq!(
		hooks_rs,
		"//! Reusable client-side custom hooks for the polls application.\n"
	);
	assert!(
		polls_dir
			.join("client")
			.join("hooks")
			.join(".gitkeep")
			.exists(),
		"apps/polls/client/hooks/.gitkeep must preserve the directory for one hook per Rust file"
	);

	let polls_components = polls_dir.join("client").join("components.rs");
	assert!(
		polls_components.exists(),
		"apps/polls/client/components.rs must exist"
	);
	let components_body = fs::read_to_string(&polls_components).expect("read components.rs");
	assert!(
		components_body.contains("pub mod placeholder;")
			&& !components_body.contains("pub use placeholder::placeholder;"),
		"apps/polls/client/components.rs must be a declaration-only facade:\n{components_body}"
	);
	let placeholder_component = fs::read_to_string(
		polls_dir
			.join("client")
			.join("components")
			.join("placeholder.rs"),
	)
	.expect("read client/components/placeholder.rs");
	assert!(
		placeholder_component
			.contains("#[reinhardt::pages::component(\"/polls/\", name = \"placeholder\")]")
			&& placeholder_component.contains("pub fn placeholder")
			&& placeholder_component.contains("use crate::client::components::nav::with_nav;")
			&& !placeholder_component.contains("super::"),
		"apps/polls/client/components/placeholder.rs must define the route-backed component:\n{placeholder_component}"
	);

	// 5. urls.rs is the app router aggregate. It gates the split router modules
	//    at declaration time so target-specific files do not leak across builds.
	let urls_rs = fs::read_to_string(polls_dir.join("urls.rs")).expect("read apps/polls/urls.rs");
	assert!(
		urls_rs.contains("#[cfg(client)]\npub mod client_router;")
			&& urls_rs.contains("#[cfg(server)]\npub mod server_router;"),
		"apps/polls/urls.rs must cfg-gate split router modules:\n{urls_rs}"
	);
	assert!(
		urls_rs.contains("#[cfg(client)]\npub use client_router::{client_url_patterns, reverse};"),
		"apps/polls/urls.rs must expose client routes only on client builds:\n{urls_rs}"
	);
	assert!(
		urls_rs.contains("#[cfg(server)]\npub use server_router::server_url_patterns;"),
		"apps/polls/urls.rs must expose server routes only on server builds:\n{urls_rs}"
	);
	assert!(
		!urls_rs.contains("pub fn client_url_patterns")
			&& !urls_rs.contains("pub fn server_url_patterns")
			&& !urls_rs.contains("ClientRouter")
			&& !urls_rs.contains("ServerRouter"),
		"apps/polls/urls.rs must not define native-safe client/server wrapper functions:\n{urls_rs}"
	);
	assert!(
		!urls_rs.contains("unified_url_patterns"),
		"apps/polls/urls.rs must not use the unified_url_patterns scaffold:\n{urls_rs}"
	);
}

#[rstest]
#[tokio::test]
#[serial(cwd)]
async fn startapp_pages_layout_has_target_gated_route_surface() {
	// Arrange — scaffold a project, then scaffold a pages app inside it.
	let tmp = TempDir::new().unwrap();
	let project_name = "polls_project";
	scaffold_pages_project(tmp.path(), project_name).await;

	let project_dir = tmp.path().join(project_name);
	let _cwd_guard = CwdGuard::enter(&project_dir);

	// Act
	let res = StartAppCommand
		.execute(&pages_context(vec!["foo".to_string()]))
		.await;

	// Assert
	res.expect("startapp --with-pages must succeed");

	let foo_dir = project_dir.join("src").join("apps").join("foo");

	// 1. The target-gated aggregator and split router modules exist.
	let urls_rs = foo_dir.join("urls.rs");
	let client_router = foo_dir.join("urls").join("client_router.rs");
	let server_router = foo_dir.join("urls").join("server_router.rs");
	assert!(urls_rs.exists(), "apps/foo/urls.rs must exist");
	assert!(
		client_router.exists(),
		"apps/foo/urls/client_router.rs must exist"
	);
	assert!(
		server_router.exists(),
		"apps/foo/urls/server_router.rs must exist"
	);

	// 2. ws_urls.rs remains out of scope.
	assert!(
		!foo_dir.join("urls").join("ws_urls.rs").exists(),
		"apps/foo/urls/ws_urls.rs must NOT be generated"
	);

	// 3. The app-level route surface gates target-specific modules and exports
	//    at declaration time.
	let urls_contents = fs::read_to_string(&urls_rs).expect("read apps/foo/urls.rs");
	assert!(
		urls_contents.contains("#[cfg(client)]\npub mod client_router;")
			&& urls_contents.contains("#[cfg(server)]\npub mod server_router;"),
		"apps/foo/urls.rs must cfg-gate split router modules:\n{urls_contents}"
	);
	assert!(
		urls_contents
			.contains("#[cfg(client)]\npub use client_router::{client_url_patterns, reverse};"),
		"apps/foo/urls.rs must expose client routes only on client builds:\n{urls_contents}"
	);
	assert!(
		urls_contents.contains("#[cfg(server)]\npub use server_router::server_url_patterns;"),
		"apps/foo/urls.rs must expose server routes only on server builds:\n{urls_contents}"
	);
	assert!(
		!urls_contents.contains("pub fn client_url_patterns")
			&& !urls_contents.contains("pub fn reverse")
			&& !urls_contents.contains("pub fn server_url_patterns")
			&& !urls_contents.contains("ClientRouter")
			&& !urls_contents.contains("ServerRouter"),
		"apps/foo/urls.rs must not keep native wrapper functions for target-specific routes:\n{urls_contents}"
	);

	// 4. Split router wiring defines url_patterns functions with generated
	//    route-backed component registration on the client side.
	let client_contents = fs::read_to_string(&client_router).expect("read client_router.rs");
	assert_eq!(
		client_contents.matches("#[url_patterns").count(),
		0,
		"client_router.rs must NOT carry the removed #[url_patterns] attribute:\n{client_contents}"
	);
	let client_body_start = client_contents
		.find("pub fn client_url_patterns")
		.expect("client_router.rs must define `pub fn client_url_patterns`");
	let client_body = &client_contents[client_body_start..];
	assert!(
		client_body.contains(".component(components::placeholder::placeholder)"),
		"client_router.rs must register the route-backed placeholder component:\n{client_body}"
	);
	assert!(
		client_contents.contains("use crate::apps::foo::client::components;")
			&& !client_contents.contains("super::super::"),
		"client_router.rs must use the app client component path instead of super::super:::\n{client_contents}"
	);
	assert!(
		!client_contents.contains("#[cfg("),
		"client_router.rs must rely on urls.rs module gates instead of internal cfg gates:\n{client_contents}"
	);
	assert!(
		client_contents.contains("pub fn reverse"),
		"client_router.rs must define `pub fn reverse`:\n{client_contents}"
	);
	assert!(
		client_contents.contains("failed to reverse foo client route"),
		"client_router.rs reverse helper must include the generated app name in panic context:\n{client_contents}"
	);

	// Server-only router wiring defines its url_patterns function with an
	//    empty body. The body is isolated from the module doc-comment (which
	//    may quote an example) by slicing the file at the `pub fn` definition
	//    before searching for example calls.
	let server_contents = fs::read_to_string(&server_router).expect("read server_router.rs");
	assert_eq!(
		server_contents.matches("#[url_patterns").count(),
		0,
		"server_router.rs must NOT carry the removed #[url_patterns] attribute:\n{server_contents}"
	);
	let server_body_start = server_contents
		.find("pub fn server_url_patterns")
		.expect("server_router.rs must define `pub fn server_url_patterns`");
	let server_body = &server_contents[server_body_start..];
	assert_eq!(
		server_body.matches(".endpoint(views::").count(),
		0,
		"server_router.rs function body must not embed example route calls:\n{server_body}"
	);

	// 5. Per-app aggregator `apps/foo.rs` declares cfg-gated facades and
	//    cross-target `serializers` / `server_fn` / `services` / `urls`
	//    surfaces without cfg gates.
	let foo_rs = fs::read_to_string(foo_dir.parent().expect("apps/").join("foo.rs"))
		.expect("read apps/foo.rs");
	assert!(
		foo_rs.contains("#[cfg(client)]\npub mod client;"),
		"apps/foo.rs must declare `#[cfg(client)] pub mod client;`:\n{foo_rs}"
	);
	assert!(
		foo_rs.contains("#[cfg(server)]\npub mod server;"),
		"apps/foo.rs must declare `#[cfg(server)] pub mod server;`:\n{foo_rs}"
	);
	// Bi-target lines: ensure they have no cfg attr immediately preceding.
	for bi_target in [
		"pub mod serializers;",
		"pub mod server_fn;",
		"pub mod services;",
		"pub mod urls;",
	] {
		let pos = foo_rs
			.find(bi_target)
			.unwrap_or_else(|| panic!("`{bi_target}` not found in apps/foo.rs:\n{foo_rs}"));
		let prefix = &foo_rs[..pos];
		let prior_line = prefix.lines().last().unwrap_or("").trim();
		assert!(
			!prior_line.starts_with("#[cfg("),
			"`{bi_target}` must not be cfg-gated in apps/foo.rs (preceding line was: {prior_line:?}):\n{foo_rs}"
		);
	}
}

/// Helper: build a `CommandContext` with `--with-pages` and `--workspace` options.
fn pages_workspace_context(args: Vec<String>) -> CommandContext {
	let mut ctx = CommandContext::new(args);
	let mut opts = HashMap::new();
	opts.insert("with-pages".to_string(), vec!["true".to_string()]);
	opts.insert("workspace".to_string(), vec!["true".to_string()]);
	ctx = ctx.with_options(opts);
	ctx
}

#[rstest]
#[tokio::test]
#[serial(cwd)]
async fn workspace_app_pages_uses_unified_template() {
	// Arrange — scaffold a pages project, then create a workspace app.
	let tmp = TempDir::new().unwrap();
	let project_name = "myproject";
	scaffold_pages_project(tmp.path(), project_name).await;

	let project_dir = tmp.path().join(project_name);
	let _cwd_guard = CwdGuard::enter(&project_dir);
	let cargo_toml = project_dir.join("Cargo.toml");

	// Act
	let res = StartAppCommand
		.execute(&pages_workspace_context(vec!["bar".to_string()]))
		.await;

	// Assert
	res.expect("startapp --with-pages --workspace must succeed");

	let app_dir = project_dir.join("apps").join("bar");

	// 1. Workspace infrastructure files exist at apps/<name>/
	assert!(
		app_dir.join("Cargo.toml").exists(),
		"apps/bar/Cargo.toml must exist for workspace crate"
	);
	assert!(
		app_dir.join("build.rs").exists(),
		"apps/bar/build.rs must exist for pages workspace crate"
	);
	let build_rs = fs::read_to_string(app_dir.join("build.rs")).expect("read workspace build.rs");
	for cfg in ["client", "server", "wasm", "native"] {
		assert!(
			build_rs.contains(&format!("cargo::rustc-check-cfg=cfg({cfg})")),
			"workspace app build.rs must declare cfg({cfg}) for Rust 2024 check-cfg:\n{build_rs}"
		);
	}
	assert!(
		build_rs.contains("wasm: { target_arch = \"wasm32\" }")
			&& build_rs.contains("native: { not(target_arch = \"wasm32\") }"),
		"workspace app build.rs must keep wasm/native compatibility aliases:\n{build_rs}"
	);

	// 2. Source files live under apps/<name>/src/
	let src = app_dir.join("src");
	assert!(
		src.join("lib.rs").exists(),
		"apps/bar/src/lib.rs must exist"
	);
	assert!(
		src.join("urls.rs").exists(),
		"apps/bar/src/urls.rs must exist"
	);
	assert!(
		src.join("urls").join("client_router.rs").exists(),
		"apps/bar/src/urls/client_router.rs must exist"
	);
	assert!(
		src.join("urls").join("server_router.rs").exists(),
		"apps/bar/src/urls/server_router.rs must exist"
	);

	// 3. The unified template now provides client/, server_fn/, serializers/,
	//    and services/ modules
	//    to workspace apps as well (previously absent from the workspace
	//    template, closing the layout drift noted in #4363).
	assert!(
		src.join("client.rs").exists(),
		"apps/bar/src/client.rs must exist (workspace apps now get full module structure)"
	);
	assert!(
		src.join("client").join("components.rs").exists(),
		"apps/bar/src/client/components.rs must exist"
	);
	let workspace_client_rs =
		fs::read_to_string(src.join("client.rs")).expect("read apps/bar/src/client.rs");
	assert!(
		workspace_client_rs.contains("pub mod hooks;")
			&& workspace_client_rs.contains("pub mod style;"),
		"apps/bar/src/client.rs must declare the custom hooks and style modules:\n{workspace_client_rs}"
	);
	let workspace_style_rs = fs::read_to_string(src.join("client").join("style.rs"))
		.expect("read apps/bar/src/client/style.rs");
	assert_empty_client_style(&workspace_style_rs, "bar", "BarStyles");
	assert!(
		!src.join("style.rs").exists(),
		"apps/bar/src/style.rs must not exist; component styles are client-only"
	);
	let workspace_hooks_rs = fs::read_to_string(src.join("client").join("hooks.rs"))
		.expect("read apps/bar/src/client/hooks.rs");
	assert_eq!(
		workspace_hooks_rs,
		"//! Reusable client-side custom hooks for the bar application.\n"
	);
	assert!(
		src.join("client").join("hooks").join(".gitkeep").exists(),
		"apps/bar/src/client/hooks/.gitkeep must preserve the directory for one hook per Rust file"
	);
	assert!(
		src.join("client")
			.join("components")
			.join("placeholder.rs")
			.exists(),
		"apps/bar/src/client/components/placeholder.rs must exist"
	);
	assert!(
		!src.join("client").join("pages.rs").exists(),
		"apps/bar/src/client/pages.rs must not exist; route-backed components live under client/components/"
	);
	assert!(
		!src.join("pages.rs").exists(),
		"apps/bar/src/pages.rs must not exist"
	);
	assert!(
		src.join("server.rs").exists(),
		"apps/bar/src/server.rs must exist"
	);
	assert!(
		src.join("server_fn.rs").exists(),
		"apps/bar/src/server_fn.rs must exist"
	);
	assert!(
		src.join("server_fn").join("placeholder.rs").exists(),
		"apps/bar/src/server_fn/placeholder.rs must exist"
	);
	assert!(
		src.join("serializers.rs").exists() && src.join("serializers").join(".gitkeep").exists(),
		"apps/bar/src/serializers.rs and serializers/.gitkeep must exist"
	);
	assert!(
		src.join("services.rs").exists()
			&& src.join("services").join("client.rs").exists()
			&& src
				.join("services")
				.join("client")
				.join(".gitkeep")
				.exists()
			&& src.join("services").join("server.rs").exists()
			&& src
				.join("services")
				.join("server")
				.join(".gitkeep")
				.exists(),
		"apps/bar/src/services split client/server surface must exist"
	);
	assert!(
		src.join("server").join("forms.rs").exists()
			&& src.join("server").join("forms").join(".gitkeep").exists(),
		"apps/bar/src/server/forms.rs and server/forms/.gitkeep must exist"
	);
	let workspace_models =
		fs::read_to_string(src.join("models.rs")).expect("read apps/bar/src/models.rs");
	assert_models_placeholder_is_tutorial_safe(&workspace_models, "bar", "BarItem");
	assert!(
		!src.join("server").join("models.rs").exists()
			&& !src.join("server").join("models").exists(),
		"apps/bar/src/server/models* must not exist because shared models live at src/models.rs"
	);

	// 4. lib.rs has cfg gates (shared template, not the old workspace-only version)
	let lib_rs = fs::read_to_string(src.join("lib.rs")).expect("read lib.rs");
	assert!(
		lib_rs.contains("#[cfg(server)]"),
		"workspace lib.rs must have #[cfg(server)] gates:\n{lib_rs}"
	);
	assert!(
		lib_rs.contains("#[cfg(client)]"),
		"workspace lib.rs must have #[cfg(client)] gate:\n{lib_rs}"
	);
	assert!(
		lib_rs.contains("crate"),
		"workspace lib.rs doc comment must say 'crate':\n{lib_rs}"
	);

	// 5. No `InstalledApp` import is generated. Since the `#[url_patterns]`
	//    attribute macro was removed (feat!: remove #[url_patterns]), the
	//    generated routers no longer reference `InstalledApp`, so neither the
	//    `crate::` form nor the project-crate form is emitted (an unused import
	//    would otherwise fail to compile under `-D warnings`).
	let crate_import = "use crate::config::apps::InstalledApp;";
	let project_import = format!("use {}::config::apps::InstalledApp;", project_name);
	let server_urls = fs::read_to_string(src.join("urls").join("server_router.rs"))
		.expect("read server_router.rs");
	assert!(
		!server_urls
			.lines()
			.any(|l| l.trim() == crate_import || l.trim() == project_import),
		"workspace server_router.rs must NOT import InstalledApp:\n{server_urls}"
	);
	let client_router = fs::read_to_string(src.join("urls").join("client_router.rs"))
		.expect("read client_router.rs");
	assert!(
		!client_router
			.lines()
			.any(|l| l.trim() == crate_import || l.trim() == project_import),
		"workspace client_router.rs must NOT import InstalledApp:\n{client_router}"
	);
	assert!(
		client_router.contains("use crate::client::components;")
			&& !client_router.contains("super::super::"),
		"workspace client_router.rs must import client components through crate:: without internal cfg gates:\n{client_router}"
	);
	assert!(
		!client_router.contains("#[cfg("),
		"workspace client_router.rs must rely on urls.rs module gates instead of internal cfg gates:\n{client_router}"
	);

	// 6. placeholder component imports with_nav from project crate, not crate::
	let expected_workspace_with_nav =
		format!("use {}::client::components::nav::with_nav;", project_name);
	let placeholder_rs =
		fs::read_to_string(src.join("client").join("components").join("placeholder.rs"))
			.expect("read client/components/placeholder.rs");
	assert!(
		!placeholder_rs
			.lines()
			.any(|l| l.trim() == "use crate::client::components::nav::with_nav;"),
		"workspace placeholder component must NOT use crate:: for with_nav import:\n{placeholder_rs}"
	);
	assert!(
		placeholder_rs
			.lines()
			.any(|l| l.trim() == expected_workspace_with_nav),
		"workspace placeholder component must import with_nav from project crate:\n{placeholder_rs}"
	);
	assert!(
		placeholder_rs
			.contains("#[reinhardt::pages::component(\"/bar/\", name = \"placeholder\")]")
			&& !placeholder_rs.contains("super::"),
		"workspace placeholder component must be route-backed without super:: paths:\n{placeholder_rs}"
	);

	// 8. Cargo.toml is valid, references src/lib.rs, and depends on parent crate
	let cargo_content =
		fs::read_to_string(app_dir.join("Cargo.toml")).expect("read app Cargo.toml");
	assert!(
		cargo_content.contains("name = \"bar\""),
		"app Cargo.toml must name the crate:\n{cargo_content}"
	);
	assert!(
		cargo_content.contains("path = \"src/lib.rs\""),
		"app Cargo.toml must reference src/lib.rs:\n{cargo_content}"
	);
	assert!(
		cargo_content.contains(&format!("{project_name} = {{ path = \"../..\" }}")),
		"app Cargo.toml must depend on parent project crate:\n{cargo_content}"
	);

	// 9. Workspace Cargo.toml has the new member registered
	let root_cargo = fs::read_to_string(&cargo_toml).expect("read root Cargo.toml");
	assert!(
		root_cargo.contains("apps/bar"),
		"workspace Cargo.toml must list apps/bar as member:\n{root_cargo}"
	);
}

#[rstest]
#[tokio::test]
#[serial(cwd)]
async fn module_app_pages_does_not_generate_workspace_files() {
	// Arrange
	let tmp = TempDir::new().unwrap();
	let project_name = "polls_project";
	scaffold_pages_project(tmp.path(), project_name).await;

	let project_dir = tmp.path().join(project_name);
	let _cwd_guard = CwdGuard::enter(&project_dir);

	// Act
	let res = StartAppCommand
		.execute(&pages_context(vec!["baz".to_string()]))
		.await;

	// Assert
	res.expect("startapp --with-pages must succeed");

	let apps = project_dir.join("src").join("apps");

	// Module apps must NOT have workspace infrastructure files
	assert!(
		!apps.join("baz").join("Cargo.toml").exists(),
		"module app must NOT have its own Cargo.toml"
	);
	assert!(
		!apps.join("baz").join("build.rs").exists(),
		"module app must NOT have its own build.rs"
	);

	// No `InstalledApp` import is generated. The `#[url_patterns]` attribute
	// macro that previously consumed `InstalledApp` was removed, so the module
	// app routers no longer reference it (an unused import would otherwise fail
	// to compile under `-D warnings`).
	let unwanted_module_import = "use crate::config::apps::InstalledApp;";
	let server_urls = fs::read_to_string(apps.join("baz").join("urls").join("server_router.rs"))
		.expect("read server_router.rs");
	assert!(
		!server_urls
			.lines()
			.any(|l| l.trim() == unwanted_module_import),
		"module server_router.rs must NOT import InstalledApp:\n{server_urls}"
	);
	let client_router = fs::read_to_string(apps.join("baz").join("urls").join("client_router.rs"))
		.expect("read client_router.rs");
	assert!(
		!client_router
			.lines()
			.any(|l| l.trim() == unwanted_module_import),
		"module client_router.rs must NOT import InstalledApp:\n{client_router}"
	);
	assert!(
		client_router.contains("use crate::apps::baz::client::components;")
			&& !client_router.contains("super::super::"),
		"module client_router.rs must import client components through the app's absolute crate path without internal cfg gates:\n{client_router}"
	);
	assert!(
		!client_router.contains("#[cfg("),
		"module client_router.rs must rely on urls.rs module gates instead of internal cfg gates:\n{client_router}"
	);

	// placeholder component with_nav import uses crate::, not project_crate_name::
	let expected_module_with_nav = "use crate::client::components::nav::with_nav;";
	let placeholder_rs = fs::read_to_string(
		apps.join("baz")
			.join("client")
			.join("components")
			.join("placeholder.rs"),
	)
	.expect("read client/components/placeholder.rs");
	assert!(
		placeholder_rs
			.lines()
			.any(|l| l.trim() == expected_module_with_nav),
		"module placeholder component must use crate:: for with_nav import:\n{placeholder_rs}"
	);
	assert!(
		placeholder_rs
			.contains("#[reinhardt::pages::component(\"/baz/\", name = \"placeholder\")]")
			&& !placeholder_rs.contains("super::"),
		"module placeholder component must be route-backed without super:: paths:\n{placeholder_rs}"
	);
}

#[rstest]
#[tokio::test]
#[serial(cwd)]
async fn startapp_pages_template_dir_overrides_client_style() {
	// Arrange
	let tmp = TempDir::new().unwrap();
	let project_name = "styled_project";
	scaffold_pages_project(tmp.path(), project_name).await;

	let override_client = tmp
		.path()
		.join("overrides")
		.join("app_pages_template")
		.join("client");
	fs::create_dir_all(&override_client).expect("create client template override directory");
	fs::write(
		override_client.join("style.rs.tpl"),
		concat!(
			"//! External style template for {{ app_name }}.\n\n",
			"use reinhardt::pages::style_def;\n\n",
			"#[style_def]\n",
			"pub static STYLES: {{ camel_case_app_name }}OverrideStyles = style! {};\n",
		),
	)
	.expect("write client style template override");

	let project_dir = tmp.path().join(project_name);
	let _cwd_guard = CwdGuard::enter(&project_dir);
	let mut options = HashMap::new();
	options.insert("with-pages".to_string(), vec!["true".to_string()]);
	options.insert(
		"template-dir".to_string(),
		vec![tmp.path().join("overrides").display().to_string()],
	);
	let context = CommandContext::new(vec!["themed".to_string()]).with_options(options);

	// Act
	let result = StartAppCommand.execute(&context).await;

	// Assert
	result.expect("startapp --with-pages with a partial template override must succeed");
	let app_dir = project_dir.join("src").join("apps").join("themed");
	let client_rs =
		fs::read_to_string(app_dir.join("client.rs")).expect("read apps/themed/client.rs");
	assert_eq!(
		client_rs
			.lines()
			.filter(|line| *line == "pub mod style;")
			.count(),
		1
	);
	let style_rs = fs::read_to_string(app_dir.join("client").join("style.rs"))
		.expect("read overridden apps/themed/client/style.rs");
	assert_eq!(
		style_rs,
		concat!(
			"//! External style template for themed.\n\n",
			"use reinhardt::pages::style_def;\n\n",
			"#[style_def]\n",
			"pub static STYLES: ThemedOverrideStyles = style! {};\n",
		)
	);
}
