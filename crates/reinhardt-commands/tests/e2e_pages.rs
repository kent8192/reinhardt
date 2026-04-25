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

/// Helper: build a `CommandContext` whose only option is `--with-pages`.
fn pages_context(args: Vec<String>) -> CommandContext {
	let mut ctx = CommandContext::new(args);
	let mut opts = HashMap::new();
	opts.insert("with-pages".to_string(), vec!["true".to_string()]);
	ctx = ctx.with_options(opts);
	ctx
}

#[rstest]
#[tokio::test]
#[serial(cwd)]
async fn project_pages_layout_matches_tutorial() {
	// Arrange
	let tmp = TempDir::new().unwrap();
	let prev = std::env::current_dir().unwrap();
	std::env::set_current_dir(tmp.path()).unwrap();

	// Act
	let res = StartProjectCommand
		.execute(&pages_context(vec!["polls_project".to_string()]))
		.await;

	// Assert
	std::env::set_current_dir(prev).unwrap();
	res.expect("startproject --with-pages must succeed");

	let project = tmp.path().join("polls_project");
	let src = project.join("src");

	// 1. server_fn lives at the crate root, NOT under server/.
	assert!(
		src.join("server_fn.rs").exists(),
		"src/server_fn.rs must exist at the crate root"
	);
	assert!(
		!src.join("server").exists(),
		"src/server/ must not be generated (server_fn moved to crate root)"
	);

	// 2. config/wasm.rs is required for collectstatic to find dist-wasm.
	assert!(
		src.join("config").join("wasm.rs").exists(),
		"src/config/wasm.rs must exist (collectstatic registration)"
	);

	// 3. client/lib.rs (WASM entry) and client/pages.rs replace
	//    bootstrap/state from the previous scaffold.
	assert!(
		src.join("client").join("lib.rs").exists(),
		"src/client/lib.rs must exist (WASM entry point)"
	);
	assert!(
		src.join("client").join("pages.rs").exists(),
		"src/client/pages.rs must exist (page components)"
	);
	assert!(
		src.join("client").join("router.rs").exists(),
		"src/client/router.rs must exist"
	);
	assert!(
		src.join("client").join("components.rs").exists(),
		"src/client/components.rs must exist"
	);
	assert!(
		!src.join("client").join("bootstrap.rs").exists(),
		"src/client/bootstrap.rs must not be generated"
	);
	assert!(
		!src.join("client").join("state.rs").exists(),
		"src/client/state.rs must not be generated"
	);

	// 4. lib.rs declares the server-only re-export shim.
	let lib_rs = fs::read_to_string(src.join("lib.rs")).expect("read lib.rs");
	assert!(
		lib_rs.contains("mod server_only"),
		"src/lib.rs must declare the `mod server_only` re-export shim:\n{lib_rs}"
	);
	assert!(
		lib_rs.contains("pub mod server_fn;"),
		"src/lib.rs must declare `pub mod server_fn;` (top-level)"
	);

	// 5. shared/ now exposes types and forms (no more shared/errors.rs).
	assert!(
		src.join("shared").join("types.rs").exists(),
		"src/shared/types.rs must exist"
	);
	assert!(
		src.join("shared").join("forms.rs").exists(),
		"src/shared/forms.rs must exist (Part 4 forms scaffold)"
	);
	assert!(
		!src.join("shared").join("errors.rs").exists(),
		"src/shared/errors.rs must not be generated"
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
	let prev = std::env::current_dir().unwrap();
	std::env::set_current_dir(tmp).unwrap();
	let cmd = StartProjectCommand;
	let ctx = pages_context(vec![name.to_string()]);
	let result = cmd.execute(&ctx).await;
	std::env::set_current_dir(prev).unwrap();
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
	let prev = std::env::current_dir().unwrap();
	std::env::set_current_dir(&project_dir).unwrap();

	// Act
	let res = StartAppCommand
		.execute(&pages_context(vec!["polls".to_string()]))
		.await;

	// Assert
	std::env::set_current_dir(prev).unwrap();
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

	// 2. Sub-modules sit directly under apps/<app>/, mirroring the tutorial.
	let polls_dir = apps.join("polls");
	assert!(
		polls_dir.join("models.rs").exists(),
		"apps/polls/models.rs must exist"
	);
	assert!(
		polls_dir.join("serializers.rs").exists(),
		"apps/polls/serializers.rs must exist"
	);
	assert!(
		polls_dir.join("urls.rs").exists(),
		"apps/polls/urls.rs must exist"
	);
	assert!(
		polls_dir.join("views.rs").exists(),
		"apps/polls/views.rs must exist"
	);

	// 3. None of the previous over-generated subdirectories are produced.
	for unwanted in [
		"client",
		"server",
		"shared",
		"urls/client_urls.rs",
		"urls/server_urls.rs",
		"urls/ws_urls.rs",
	] {
		let path = polls_dir.join(unwanted);
		assert!(
			!path.exists(),
			"apps/polls/{unwanted} must not be generated (was: {})",
			path.display()
		);
	}

	// 4. urls.rs is a plain `routes()` mounted from config/urls.rs — not a
	//    `unified_url_patterns` with #[url_patterns(... mode = unified)].
	let urls_rs = fs::read_to_string(polls_dir.join("urls.rs")).expect("read apps/polls/urls.rs");
	assert!(
		urls_rs.contains("pub fn routes() -> ServerRouter"),
		"apps/polls/urls.rs must define `pub fn routes() -> ServerRouter`:\n{urls_rs}"
	);
	assert!(
		!urls_rs.contains("unified_url_patterns"),
		"apps/polls/urls.rs must not use the unified_url_patterns scaffold:\n{urls_rs}"
	);
}
