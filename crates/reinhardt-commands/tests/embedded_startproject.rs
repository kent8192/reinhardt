//! Integration test proving `startproject` produces a usable project tree
//! from embedded templates alone — no CARGO_MANIFEST_DIR dependency.

use reinhardt_commands::start_commands::StartProjectCommand;
use reinhardt_commands::{BaseCommand, CommandContext};
use rstest::*;
use serial_test::serial;
use std::collections::HashMap;
use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

// Assert that Cargo can fully parse the generated manifest.
//
// Uses `cargo metadata --no-deps` so no registry access is required; the
// command still exercises the same manifest-parsing step that rejects
// misconfigurations (e.g. a `default-run` pointing at a nonexistent bin)
// which would break the scaffold for a real user on `cargo run`.
fn assert_manifest_parses(manifest: &Path) {
	let output = Command::new(env!("CARGO"))
		.args(["metadata", "--no-deps", "--format-version", "1"])
		.arg("--manifest-path")
		.arg(manifest)
		.output()
		.expect("cargo metadata command spawns");
	assert!(
		output.status.success(),
		"generated manifest failed to parse: {}\nstderr:\n{}",
		manifest.display(),
		String::from_utf8_lossy(&output.stderr),
	);
}

fn normalize_guidance(content: &str) -> String {
	content
		.replace("AGENTS.md", "GUIDANCE.md")
		.replace("CLAUDE.md", "GUIDANCE.md")
}

const COMMON_INSTRUCTIONS: &[&str] = &[
	"MODULE_SYSTEM.md",
	"ANTI_PATTERNS.md",
	"MACRO_USAGE.md",
	"TESTING_STANDARDS.md",
	"DOCUMENTATION_STANDARDS.md",
];

fn assert_generated_instructions(root: &Path, guidance: &str, with_pages: bool) {
	let instructions = root.join("instructions");
	assert!(
		instructions.is_dir(),
		"generated project must contain instructions/"
	);

	let surface = if with_pages {
		"REINHARDT_PAGES.md"
	} else {
		"REINHARDT_RESTFUL.md"
	};
	for filename in COMMON_INSTRUCTIONS.iter().copied().chain([surface]) {
		let path = instructions.join(filename);
		let content = std::fs::read_to_string(&path).unwrap_or_else(|error| {
			panic!("generated instruction {filename} must be readable: {error}")
		});
		assert!(
			!content.is_empty(),
			"generated instruction {filename} must not be empty"
		);
		assert!(
			!content.contains("{{ ") && !content.contains(" }}"),
			"generated instruction {filename} must not contain an unexpanded template token"
		);
		assert!(
			guidance.contains(&format!("@instructions/{filename}")),
			"root guidance must reference instructions/{filename}"
		);
	}

	for excluded in [
		"DESIGN_PHILOSOPHY.md",
		"GIT_AND_GITHUB.md",
		"MIGRATION_0.3.md",
		"QUICK_REFERENCE.md",
		"RESEARCH_ESCALATION.md",
	] {
		assert!(
			!instructions.join(excluded).exists(),
			"generated project must not include excluded instruction {excluded}"
		);
	}
}

fn assert_generated_agent_guidance(root: &Path, project_name: &str, with_pages: bool) {
	let agents = std::fs::read_to_string(root.join("AGENTS.md"))
		.expect("generated project must contain AGENTS.md");
	let claude = std::fs::read_to_string(root.join("CLAUDE.md"))
		.expect("generated project must contain CLAUDE.md");

	assert_eq!(
		normalize_guidance(&agents),
		normalize_guidance(&claude),
		"generated guidance files must differ only by filename"
	);

	let project_marker = format!("work on `{project_name}`");
	for (filename, content) in [
		("AGENTS.md", agents.as_str()),
		("CLAUDE.md", claude.as_str()),
	] {
		assert!(
			content.contains(&project_marker),
			"{filename} must contain the rendered project name"
		);
		assert!(
			!content.contains(r"{{ project_name }}"),
			"{filename} must not contain an unexpanded project-name token"
		);
		assert_eq!(
			content.contains("## Pages Native/WASM Boundaries"),
			with_pages,
			"{filename} must include Pages guidance only for Pages projects"
		);
		assert_generated_instructions(root, content, with_pages);

		for forbidden in [
			"/Users/",
			"/home/",
			"C:\\",
			"secret_key",
			"insecure-",
			"CHANGE_THIS_IN_PRODUCTION",
		] {
			assert!(
				!content.contains(forbidden),
				"{filename} must not contain forbidden marker `{forbidden}`"
			);
		}
	}
}

fn assert_reinhardt_dependency_features(cargo_toml: &str, expected: &[&str]) {
	let document = cargo_toml
		.parse::<toml_edit::DocumentMut>()
		.expect("generated Cargo.toml must parse as TOML");
	let features = document["target"][r#"cfg(not(target_arch = "wasm32"))"#]["dependencies"]
		["reinhardt"]["features"]
		.as_array()
		.expect("generated native reinhardt dependency must declare features");

	for feature in expected {
		assert!(
			features.iter().any(|value| value.as_str() == Some(feature)),
			"generated reinhardt dependency must include `{feature}`:\n{cargo_toml}"
		);
	}
}

fn assert_restful_runtime_dependencies(cargo_toml: &str) {
	let document = cargo_toml
		.parse::<toml_edit::DocumentMut>()
		.expect("generated Cargo.toml must parse as TOML");
	let serde_features = document["dependencies"]["serde"]["features"]
		.as_array()
		.expect("generated REST project must directly depend on serde with features");
	assert!(
		serde_features
			.iter()
			.any(|value| value.as_str() == Some("derive")),
		"generated REST project must enable serde derive for #[settings]:\n{cargo_toml}"
	);
	let reinhardt_features = document["dependencies"]["reinhardt"]["features"]
		.as_array()
		.expect("generated REST reinhardt dependency must declare features");
	assert!(
		reinhardt_features
			.iter()
			.any(|value| value.as_str() == Some("client-router")),
		"generated REST project must enable UnifiedRouter through client-router:\n{cargo_toml}"
	);
	assert!(
		reinhardt_features
			.iter()
			.any(|value| value.as_str() == Some("commands-contract")),
		"generated REST project must enable application contract export:\n{cargo_toml}"
	);
}

fn assert_generated_common_and_migration_settings(root: &Path) {
	let settings = std::fs::read_to_string(root.join("src/config/settings.rs")).unwrap();
	assert!(
		settings.contains(
			"#[settings(core: CoreSettings | contacts: ContactSettings | migrations: MigrationSettings)]"
		),
		"generated settings must satisfy common and migration settings bounds:\n{settings}"
	);
	for required in [
		"pub fn get_settings() -> Result<PendingSettings<ProjectSettings>, BuildError>",
		".build_pending_composed::<ProjectSettings>()",
		"pub fn get_shell_settings() -> ProjectSettings",
		".resolve()",
		".into_parts()",
		"core.secret_key.is_empty()",
	] {
		assert!(
			settings.contains(required),
			"generated settings must contain `{required}`:\n{settings}"
		);
	}
}

fn assert_generated_settings_use_manifest_dir(root: &Path) {
	let settings = std::fs::read_to_string(root.join("src/config/settings.rs")).unwrap();
	let compact_settings = settings.split_whitespace().collect::<String>();
	assert!(
		settings.contains("let base_dir = std::path::PathBuf::from(env!(\"CARGO_MANIFEST_DIR\"));"),
		"generated settings must resolve the managed project directory independently of caller cwd:\n{settings}"
	);
	assert!(
		!settings.contains("env::current_dir()"),
		"generated settings must not derive the project directory from caller cwd:\n{settings}"
	);
	assert!(
		compact_settings
			.contains(".with_value(\"core\",serde_json::json!({\"base_dir\":base_dir}))"),
		"generated settings must expose the managed project directory through core.base_dir:\n{settings}"
	);
	assert!(
		compact_settings.contains(".with_value(\"migrations\",serde_json::json!({}))"),
		"generated settings must provide a default migrations fragment:\n{settings}"
	);
}

fn assert_generated_rust_sources_do_not_use_tab_indents(root: &Path) {
	for relative in [
		"build.rs",
		"src/bin/manage.rs",
		"src/client/components/nav.rs",
		"src/client/lib.rs",
		"src/config/shell.rs",
		"src/config/settings.rs",
		"src/config/wasm.rs",
		"src/lib.rs",
		"tests/integration.rs",
	] {
		let content = std::fs::read_to_string(root.join(relative)).unwrap();
		assert!(
			!content.contains('\t'),
			"generated Rust source should be rustfmt-clean before cargo make dev: {relative}"
		);
	}
}

fn assert_generated_shell_wiring(root: &Path, crate_name: &str) {
	let shell_path = root.join("src/config/shell.rs");
	assert!(
		shell_path.exists(),
		"generated project must include src/config/shell.rs"
	);

	let cargo_toml = std::fs::read_to_string(root.join("Cargo.toml")).unwrap();
	let document = cargo_toml
		.parse::<toml_edit::DocumentMut>()
		.expect("generated Cargo.toml must parse as TOML");
	let commands_shell = document["features"]["commands-shell"]
		.as_array()
		.expect("generated project must declare a commands-shell feature");
	assert!(
		commands_shell.iter().any(|value| {
			matches!(
				value.as_str(),
				Some("reinhardt/commands-shell" | "dep:reinhardt-commands")
			)
		}),
		"generated project must enable the Reinhardt shell implementation"
	);
	let default_features = document["features"]["default"]
		.as_array()
		.expect("generated project must declare default features");
	assert!(
		default_features
			.iter()
			.all(|value| value.as_str() != Some("commands-shell")),
		"commands-shell must remain opt-in:\n{cargo_toml}"
	);

	let config = std::fs::read_to_string(root.join("src/config.rs")).unwrap();
	assert!(
		config.contains("#[cfg(feature = \"commands-shell\")]\npub mod shell;")
			|| config.contains("#[cfg(all(server, feature = \"commands-shell\"))]\npub mod shell;"),
		"generated config must gate the shell module behind commands-shell:\n{config}"
	);

	let shell = std::fs::read_to_string(shell_path).unwrap();
	for required in [
		"pub use reinhardt as framework;",
		"pub type ShellSettings = ProjectSettings;",
		"pub type ProjectShellEnvironment =",
		"framework::commands::ShellEnvironment<ShellSettings>;",
		"pub type ShellDatabase = framework::db::orm::DatabaseConnection;",
		"pub type ShellDi = std::sync::Arc<framework::di::InjectionContext>;",
		"InstalledApp::all_labels()",
	] {
		assert!(
			shell.contains(required),
			"generated shell config must contain `{required}`:\n{shell}"
		);
	}
	assert!(
		shell.contains(&format!("\"{crate_name}\""))
			&& shell.contains(&format!(
				"\"{crate_name}::config::settings::get_shell_settings\""
			)),
		"generated shell config must use the renderer-normalized crate name:\n{shell}"
	);
	assert!(
		!shell.contains("CARGO_CRATE_NAME") && !shell.contains(".replace("),
		"generated shell config must not derive the crate name at runtime:\n{shell}"
	);

	let manage = std::fs::read_to_string(root.join("src/bin/manage.rs")).unwrap();
	let outer_main = manage
		.find("reinhardt::commands::shell_runtime_hook();")
		.expect("native process entry must call shell_runtime_hook");
	let tokio_main = manage
		.find("#[tokio::main]")
		.expect("generated native module must retain Tokio entry");
	assert!(
		outer_main > tokio_main,
		"shell runtime hook must be in the outer process entry, not the Tokio async entry:\n{manage}"
	);
	assert!(
		manage[outer_main..].contains("native::main();"),
		"shell runtime hook must execute before native::main:\n{manage}"
	);
	for required in [
		"command_error_exit_code",
		"process::exit(exit_code)",
		"#[cfg(feature = \"commands-shell\")]",
		"execute_from_command_line_with_pending_settings_and_cargo_context_and_shell(",
		"get_shell_config()",
		"#[cfg(not(feature = \"commands-shell\"))]",
		"execute_from_command_line_with_pending_settings_and_cargo_context(",
		"CargoCheckContext::from_launcher(",
		"get_settings,",
		"#[cfg(target_arch = \"wasm32\")]\nfn main() {}",
	] {
		assert!(
			manage.contains(required),
			"generated manage binary must contain `{required}`:\n{manage}"
		);
	}
	let readme = std::fs::read_to_string(root.join("README.md")).unwrap();
	assert!(
		readme.contains("cargo run --bin manage contract export --format json"),
		"generated README must document application contract export:\n{readme}"
	);
}

#[rstest]
#[tokio::test]
#[serial(cwd)]
async fn startproject_restful_from_embedded_only() {
	// Arrange
	let tmp = TempDir::new().unwrap();
	let prev = std::env::current_dir().unwrap();
	std::env::set_current_dir(tmp.path()).unwrap();

	let mut ctx = CommandContext::new(vec!["sample-proj".to_string()]);
	let mut opts = HashMap::new();
	opts.insert("restful".to_string(), vec!["true".to_string()]);
	ctx = ctx.with_options(opts);

	// Act
	let res = StartProjectCommand.execute(&ctx).await;

	// Assert
	std::env::set_current_dir(prev).unwrap();
	res.expect("startproject succeeds from embedded templates");
	let generated = tmp.path().join("sample-proj");
	assert!(
		generated.join("Cargo.toml").exists(),
		"Cargo.toml must be generated"
	);
	assert!(
		generated.join("src").is_dir(),
		"src/ directory must be generated"
	);
	let cargo_toml = std::fs::read_to_string(generated.join("Cargo.toml")).unwrap();
	assert_restful_runtime_dependencies(&cargo_toml);
	assert_generated_common_and_migration_settings(&generated);
	assert_generated_settings_use_manifest_dir(&generated);
	assert_generated_shell_wiring(&generated, "sample_proj");
	assert_generated_agent_guidance(&generated, "sample-proj", false);
	assert_manifest_parses(&generated.join("Cargo.toml"));
}

#[rstest]
#[tokio::test]
#[serial(cwd)]
async fn startproject_restful_honors_dependency_selection_flags() {
	let tmp = TempDir::new().unwrap();
	let prev = std::env::current_dir().unwrap();
	std::env::set_current_dir(tmp.path()).unwrap();

	let mut ctx = CommandContext::new(vec!["feature_proj".to_string()]);
	let mut opts = HashMap::new();
	opts.insert("restful".to_string(), vec!["true".to_string()]);
	opts.insert(
		"reinhardt-version".to_string(),
		vec!["0.2.0-rc.4".to_string()],
	);
	opts.insert(
		"features".to_string(),
		vec!["minimal,db-sqlite".to_string()],
	);
	opts.insert("no-interactive".to_string(), vec!["true".to_string()]);
	ctx = ctx.with_options(opts);

	let res = StartProjectCommand.execute(&ctx).await;

	std::env::set_current_dir(prev).unwrap();
	res.expect("startproject succeeds with dependency selection flags");
	let cargo_toml = std::fs::read_to_string(tmp.path().join("feature_proj/Cargo.toml")).unwrap();
	assert!(cargo_toml.contains("version = \"0.2.0-rc.4\""));
	assert!(cargo_toml.contains("default-features = false"));
	assert!(cargo_toml.contains(
		"features = [\"minimal\", \"db-sqlite\", \"conf\", \"commands\", \"client-router\", \"api\", \"commands-contract\"]"
	));
}

#[rstest]
#[tokio::test]
#[serial(cwd)]
async fn startproject_pages_from_embedded_only() {
	// Arrange
	let tmp = TempDir::new().unwrap();
	let prev = std::env::current_dir().unwrap();
	std::env::set_current_dir(tmp.path()).unwrap();

	let mut ctx = CommandContext::new(vec!["sample-pages-proj".to_string()]);
	let mut opts = HashMap::new();
	opts.insert("with-pages".to_string(), vec!["true".to_string()]);
	ctx = ctx.with_options(opts);

	// Act
	let res = StartProjectCommand.execute(&ctx).await;

	// Assert
	std::env::set_current_dir(prev).unwrap();
	res.expect("startproject --with-pages succeeds from embedded templates");
	let generated = tmp.path().join("sample-pages-proj");
	assert!(
		generated.join("Cargo.toml").exists(),
		"Cargo.toml must be generated"
	);
	assert!(
		generated.join("src").is_dir(),
		"src/ directory must be generated"
	);
	let index_html = std::fs::read_to_string(generated.join("index.html")).unwrap();
	assert!(index_html.contains("{{ static_url(\"__reinhardt__/components.css\") }}"));
	let cargo_toml = std::fs::read_to_string(generated.join("Cargo.toml")).unwrap();
	assert!(cargo_toml.contains(
		"package = \"reinhardt-web\", default-features = false, features = [\"pages\", \"client-router\"]"
	));
	assert_reinhardt_dependency_features(
		&cargo_toml,
		&[
			"minimal",
			"pages",
			"client-router",
			"admin",
			"conf",
			"commands",
			"commands-contract",
			"commands-server",
			"commands-autoreload",
			"server",
			"grpc",
			"websockets",
			"db-sqlite",
			"forms",
			"auth-session",
			"middleware",
			"argon2-hasher",
		],
	);
	assert!(
		!cargo_toml.contains("\"standard\"") && !cargo_toml.contains("\"db-postgres\""),
		"generated pages manifest must not require PostgreSQL defaults:\n{cargo_toml}"
	);
	let base_toml = std::fs::read_to_string(generated.join("settings/base.toml")).unwrap();
	assert!(
		base_toml.contains("engine = \"sqlite\"")
			&& base_toml.contains("name = \"db.sqlite3\"")
			&& !base_toml.contains("engine = \"postgresql\""),
		"generated pages settings must match the SQLite feature default:\n{base_toml}"
	);
	assert!(
		cargo_toml.contains("required-features = [\"with-reinhardt\"]"),
		"generated pages manage binary must be native-feature gated:\n{cargo_toml}"
	);
	assert!(
		cargo_toml.contains("default = [\"with-reinhardt\", \"client-router\"]")
			&& cargo_toml.contains("msw = [\"reinhardt/msw\"]"),
		"generated pages Cargo.toml must declare local feature gates used by WASM tests:\n{cargo_toml}"
	);
	let makefile_toml = std::fs::read_to_string(generated.join("Makefile.toml")).unwrap();
	assert!(
		makefile_toml.contains("\"--no-input\""),
		"generated pages Makefile must use collectstatic's non-interactive flag"
	);
	assert!(
		!makefile_toml.contains("\"--noinput\""),
		"generated pages Makefile must not use the createsuperuser-only --noinput spelling"
	);
	assert!(
		makefile_toml.contains("command = \"wasm-pack\"")
			&& makefile_toml.contains("\"--out-dir\", \"dist-wasm\""),
		"generated pages Makefile must build the browser bundle with wasm-pack into dist-wasm:\n{makefile_toml}"
	);
	assert!(
		makefile_toml
			.contains("args = [\"build\", \"--target\", \"wasm32-unknown-unknown\", \"--lib\"]")
			&& makefile_toml.contains(
				"args = [\"build\", \"--target\", \"wasm32-unknown-unknown\", \"--release\", \"--lib\"]"
			),
		"generated pages Makefile must compile only the library for WASM pre-checks:\n{makefile_toml}"
	);
	assert!(
		!makefile_toml.contains("ls target/wasm32-unknown-unknown")
			&& !makefile_toml.contains("head -1"),
		"generated pages Makefile must not pick an arbitrary .wasm file such as manage.wasm:\n{makefile_toml}"
	);
	assert!(
		generated.join("scripts/wasm-build-dev.sh").exists()
			&& generated.join("scripts/wasm-build-release.sh").exists(),
		"generated pages project must include WASM post-build scripts"
	);
	let build_rs = std::fs::read_to_string(generated.join("build.rs")).unwrap();
	for cfg in ["with_reinhardt", "client", "server", "wasm", "native"] {
		assert!(
			build_rs.contains(&format!("cargo::rustc-check-cfg=cfg({cfg})")),
			"generated pages build.rs must declare cfg({cfg}) for Rust 2024 check-cfg:\n{build_rs}"
		);
	}
	assert!(
		build_rs.contains("wasm: { target_arch = \"wasm32\" }")
			&& build_rs.contains("native: { not(target_arch = \"wasm32\") }"),
		"generated pages build.rs must keep wasm/native compatibility aliases:\n{build_rs}"
	);
	assert!(
		cargo_toml.contains("[workspace]") && cargo_toml.contains("members = ["),
		"generated pages Cargo.toml must be a nested-workspace-safe root:\n{cargo_toml}"
	);
	assert!(
		!generated.join("src/shared.rs").exists() && !generated.join("src/shared").exists(),
		"generated pages project must not create a root shared module"
	);
	assert_generated_shell_wiring(&generated, "sample_pages_proj");
	assert_generated_common_and_migration_settings(&generated);
	let document = cargo_toml
		.parse::<toml_edit::DocumentMut>()
		.expect("generated Cargo.toml must parse as TOML");
	assert_eq!(
		document["features"]["commands-shell"]
			.as_array()
			.expect("commands-shell must be an array")
			.iter()
			.filter_map(|value| value.as_str())
			.collect::<Vec<_>>(),
		vec![
			"reinhardt/commands",
			"reinhardt/database",
			"reinhardt/di",
			"dep:reinhardt-commands"
		],
		"Pages projects must activate the shell through native-compatible feature wiring"
	);
	assert!(
		document["target"]["cfg(not(target_arch = \"wasm32\"))"]["dependencies"]
			["reinhardt-commands"]
			.is_inline_table(),
		"Pages projects must declare the shell dependency only for native targets"
	);
	let command_features = document["target"]["cfg(not(target_arch = \"wasm32\"))"]["dependencies"]
		["reinhardt-commands"]["features"]
		.as_array()
		.expect("native commands dependency must declare features");
	for feature in ["shell", "server", "autoreload", "grpc", "websockets"] {
		assert!(
			command_features
				.iter()
				.any(|value| value.as_str() == Some(feature)),
			"native commands dependency must include {feature}:\n{cargo_toml}"
		);
	}
	assert_generated_settings_use_manifest_dir(&generated);
	assert_generated_rust_sources_do_not_use_tab_indents(&generated);
	assert_generated_agent_guidance(&generated, "sample-pages-proj", true);
	assert_manifest_parses(&generated.join("Cargo.toml"));
}

#[rstest]
#[tokio::test]
#[serial(cwd)]
async fn startproject_pages_adds_required_pages_features() {
	let tmp = TempDir::new().unwrap();
	let prev = std::env::current_dir().unwrap();
	std::env::set_current_dir(tmp.path()).unwrap();

	let mut ctx = CommandContext::new(vec!["pages_feature_proj".to_string()]);
	let mut opts = HashMap::new();
	opts.insert("with-pages".to_string(), vec!["true".to_string()]);
	opts.insert(
		"features".to_string(),
		vec!["minimal,pages,client-router,server-fn,db-sqlite".to_string()],
	);
	opts.insert("no-interactive".to_string(), vec!["true".to_string()]);
	ctx = ctx.with_options(opts);

	let res = StartProjectCommand.execute(&ctx).await;

	std::env::set_current_dir(prev).unwrap();
	res.expect("startproject --with-pages succeeds with dependency selection flags");
	let cargo_toml =
		std::fs::read_to_string(tmp.path().join("pages_feature_proj/Cargo.toml")).unwrap();
	assert_reinhardt_dependency_features(
		&cargo_toml,
		&[
			"minimal",
			"pages",
			"client-router",
			"db-sqlite",
			"admin",
			"conf",
			"commands",
			"commands-contract",
			"commands-server",
			"commands-autoreload",
			"server",
			"grpc",
			"websockets",
			"forms",
			"auth-session",
			"middleware",
			"argon2-hasher",
		],
	);
	assert!(
		!cargo_toml.contains("\"server-fn\""),
		"stale server-fn feature alias must not be written to generated Cargo.toml:\n{cargo_toml}"
	);
	assert!(
		!cargo_toml.contains("\"db-postgres\""),
		"explicit db-sqlite selection must not be overwritten by db-postgres:\n{cargo_toml}"
	);
	assert_manifest_parses(&tmp.path().join("pages_feature_proj/Cargo.toml"));
}

#[rstest]
#[tokio::test]
#[serial(cwd)]
async fn startproject_pages_explicit_tutorial_features_get_minimal_runtime() {
	let tmp = TempDir::new().unwrap();
	let prev = std::env::current_dir().unwrap();
	std::env::set_current_dir(tmp.path()).unwrap();

	let mut ctx = CommandContext::new(vec!["pages_tutorial_proj".to_string()]);
	let mut opts = HashMap::new();
	opts.insert("with-pages".to_string(), vec!["true".to_string()]);
	opts.insert(
		"features".to_string(),
		vec![
			"pages,admin,conf,commands-server,commands-autoreload,db-sqlite,forms,auth-session,middleware,argon2-hasher,static-files"
				.to_string(),
		],
	);
	opts.insert("default-features".to_string(), vec!["false".to_string()]);
	opts.insert("no-interactive".to_string(), vec!["true".to_string()]);
	ctx = ctx.with_options(opts);

	let res = StartProjectCommand.execute(&ctx).await;

	std::env::set_current_dir(prev).unwrap();
	res.expect("startproject --with-pages succeeds with tutorial-style explicit features");
	let cargo_toml =
		std::fs::read_to_string(tmp.path().join("pages_tutorial_proj/Cargo.toml")).unwrap();
	assert!(
		cargo_toml.contains("\"minimal\""),
		"explicit Pages feature selections must be augmented with the minimal runtime facade:\n{cargo_toml}"
	);
	assert!(
		cargo_toml.contains("\"server\""),
		"explicit Pages feature selections must be augmented with the HTTP server facade:\n{cargo_toml}"
	);
	assert!(
		cargo_toml.contains("\"db-sqlite\"") && !cargo_toml.contains("\"db-postgres\""),
		"explicit SQLite selection must not be overwritten by db-postgres:\n{cargo_toml}"
	);
	assert_manifest_parses(&tmp.path().join("pages_tutorial_proj/Cargo.toml"));
}
