//! Generated build-script Cargo replay coverage.

#[cfg(unix)]
#[test]
fn generated_build_scripts_fail_closed_when_process_inspection_fails() {
	use std::env;
	use std::fs;
	use std::os::unix::fs::PermissionsExt;
	use std::path::Path;
	use std::process::Command;
	use tempfile::TempDir;

	for template_name in [
		"templates/project_pages_template/build.rs.tpl",
		"templates/project_restful_template/build.rs.tpl",
	] {
		for failures_before_exit in [0, 1] {
			let project = TempDir::new().expect("create generated-project tempdir");
			let ps_dir = project.path().join("bin");
			fs::create_dir_all(project.path().join("src"))
				.expect("create generated source directory");
			fs::create_dir_all(&ps_dir).expect("create fake ps directory");
			let optional_dependency = project.path().join("vendor#branch");
			fs::create_dir_all(optional_dependency.join("src"))
				.expect("create optional dependency source directory");
			fs::write(
				optional_dependency.join("Cargo.toml"),
				"[package]\nname = \"optional-hyphen\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
			)
			.expect("write optional dependency manifest");
			fs::write(optional_dependency.join("src/lib.rs"), "")
				.expect("write optional dependency library");
			fs::write(
			project.path().join("Cargo.toml"),
			"[package]\nname = \"build-script-replay\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[features]\ndefault = [\"with-reinhardt\", \"client-router\", \"foo_bar\", \"optional_feature\", \"optional-hyphen\"]\nwith-reinhardt = []\nclient-router = []\nfoo_bar = []\noptional_feature = [\"dep:optional-feature\"]\n\n[dependencies]\noptional-hyphen = { path = \"vendor#branch\", optional = true } # trailing comment\n\n[dependencies.optional-feature]\npackage = \"serde\"\nversion = \"1.0\"\noptional = true\n\n[build-dependencies]\ncfg_aliases = \"0.2\"\n",
		)
		.expect("write generated Cargo manifest");
			let template_path = Path::new(env!("CARGO_MANIFEST_DIR")).join(template_name);
			fs::write(
				project.path().join("build.rs"),
				fs::read_to_string(&template_path)
					.expect("read build-script template")
					.replace("{{ project_name }}", "build-script-replay"),
			)
			.expect("write generated build script");
			fs::write(
				project.path().join("src/main.rs"),
				"fn main() { println!(\"{}|{}\", env!(\"REINHARDT_ENABLED_FEATURES\"), env!(\"REINHARDT_CARGO_REPLAY\")); }\n",
			)
			.expect("write generated main source");
			let ps = ps_dir.join("ps");
			let ps_state = project.path().join("ps-state");
			fs::write(&ps_state, "0\n").expect("write fake ps state");
			fs::write(
			&ps,
			format!(
				"#!/bin/sh\ncount=$(cat \"{}\")\nif [ \"$count\" = \"{}\" ]; then\nexit 1\nfi\nprintf '1\\n' > \"{}\"\nprintf '1\\n'\nexit 0\n",
				ps_state.display(),
				failures_before_exit,
				ps_state.display(),
			),
		)
		.expect("write failing ps command");
			fs::set_permissions(&ps, fs::Permissions::from_mode(0o755))
				.expect("make failing ps command executable");

			let mut path_entries = vec![ps_dir.clone()];
			path_entries.extend(env::split_paths(
				env::var_os("PATH").as_deref().expect("PATH is set"),
			));
			let path =
				env::join_paths(path_entries).expect("construct PATH with failing ps command");
			let output = Command::new(env!("CARGO"))
				.current_dir(project.path())
				.args(["run", "--offline", "--quiet"])
				.env("CARGO_TARGET_DIR", project.path().join("target"))
				.env("PATH", path)
				.output()
				.expect("run generated project");
			assert!(
				output.status.success(),
				"generated project failed: {}",
				String::from_utf8_lossy(&output.stderr)
			);
			assert_eq!(
				output.stdout,
				b"client-router,default,foo_bar,optional-hyphen,optional_feature,with-reinhardt|unsupported\n",
				"template: {template_name}, failures before exit: {failures_before_exit}"
			);
		}
	}
}

#[cfg(unix)]
#[test]
fn generated_build_scripts_reject_ambiguous_normalized_feature_names() {
	use std::env;
	use std::fs;
	use std::path::Path;
	use std::process::Command;
	use tempfile::TempDir;

	for template_name in [
		"templates/project_pages_template/build.rs.tpl",
		"templates/project_restful_template/build.rs.tpl",
	] {
		let project = TempDir::new().expect("create generated-project tempdir");
		fs::create_dir_all(project.path().join("src")).expect("create generated source directory");
		fs::write(
			project.path().join("Cargo.toml"),
			"[package]\nname = \"build-script-replay\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[features]\ndefault = []\nfoo-bar = []\nfoo_bar = []\n\n[build-dependencies]\ncfg_aliases = \"0.2\"\n",
		)
		.expect("write generated Cargo manifest");
		let template_path = Path::new(env!("CARGO_MANIFEST_DIR")).join(template_name);
		fs::write(
			project.path().join("build.rs"),
			fs::read_to_string(&template_path)
				.expect("read build-script template")
				.replace("{{ project_name }}", "build-script-replay"),
		)
		.expect("write generated build script");
		fs::write(
			project.path().join("src/main.rs"),
			"fn main() { println!(\"{}\", env!(\"REINHARDT_CARGO_REPLAY\")); }\n",
		)
		.expect("write generated main source");

		let output = Command::new(env!("CARGO"))
			.current_dir(project.path())
			.args(["run", "--quiet", "--features", "foo-bar"])
			.env("CARGO_TARGET_DIR", project.path().join("target"))
			.output()
			.expect("run generated project");

		assert!(
			output.status.success(),
			"generated project failed: {}",
			String::from_utf8_lossy(&output.stderr)
		);
		assert_eq!(output.stdout, b"unsupported\n", "template: {template_name}");
	}
}
