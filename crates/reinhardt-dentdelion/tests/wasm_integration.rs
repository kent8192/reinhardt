//! WASM Plugin System Integration Tests
//!
//! These tests build a minimal WASM Component from source with `cargo component`
//! and exercise the dentdelion plugin lifecycle contract.

#[cfg(feature = "wasm")]
mod wasm_tests {
	use reinhardt_dentdelion::{
		context::PluginContext,
		error::{PluginError, PluginState},
		plugin::{Plugin, PluginLifecycle},
		wasm::{WasmPluginInstance, WasmPluginLoader, WasmRuntime, WasmRuntimeConfig},
	};
	use semver::Version;
	use std::ffi::OsStr;
	use std::fs;
	use std::io;
	use std::path::{Path, PathBuf};
	use std::process::Command;
	use std::sync::Arc;
	use tempfile::TempDir;

	#[tokio::test]
	async fn source_built_minimal_component_enforces_lifecycle_contract() {
		let fixture = build_minimal_fixture();
		let runtime = Arc::new(
			WasmRuntime::new(WasmRuntimeConfig::default()).expect("WASM runtime should initialize"),
		);
		let loader = WasmPluginLoader::new(fixture.component_dir(), runtime);
		let context = PluginContext::new(PathBuf::from(env!("CARGO_MANIFEST_DIR")));
		let instance = load_instance(&loader, fixture.component_path()).await;

		assert_eq!(instance.state(), PluginState::Registered);
		assert!(instance.is_dynamic());
		assert_eq!(instance.wasm_path(), fixture.component_path());
		assert_eq!(instance.name(), "minimal_plugin");
		assert_eq!(instance.version(), &Version::new(0, 1, 0));
		assert_eq!(instance.capabilities(), &[]);
		assert!(instance.wasm_config().is_none());

		instance
			.on_load(&context)
			.await
			.expect("load should succeed");
		assert_eq!(instance.state(), PluginState::Loaded);
		instance
			.on_enable(&context)
			.await
			.expect("enable should succeed");
		assert_eq!(instance.state(), PluginState::Enabled);
		instance
			.on_disable(&context)
			.await
			.expect("disable should succeed");
		assert_eq!(instance.state(), PluginState::Disabled);
		instance
			.on_enable(&context)
			.await
			.expect("re-enable should succeed");
		assert_eq!(instance.state(), PluginState::Enabled);
		instance
			.on_disable(&context)
			.await
			.expect("second disable should succeed");
		assert_eq!(instance.state(), PluginState::Disabled);
		instance
			.on_unload(&context)
			.await
			.expect("unload should succeed");
		assert_eq!(instance.state(), PluginState::Registered);

		assert_invalid_transition(
			load_instance(&loader, fixture.component_path())
				.await
				.on_enable(&context)
				.await
				.expect_err("enable before load should be rejected"),
			"minimal_plugin",
			PluginState::Registered,
			PluginState::Enabled,
		);

		let disable_before_enable = load_instance(&loader, fixture.component_path()).await;
		disable_before_enable
			.on_load(&context)
			.await
			.expect("load should succeed");
		assert_invalid_transition(
			disable_before_enable
				.on_disable(&context)
				.await
				.expect_err("disable before enable should be rejected"),
			"minimal_plugin",
			PluginState::Loaded,
			PluginState::Disabled,
		);

		let load_twice = load_instance(&loader, fixture.component_path()).await;
		load_twice
			.on_load(&context)
			.await
			.expect("load should succeed");
		assert_invalid_transition(
			load_twice
				.on_load(&context)
				.await
				.expect_err("second load should be rejected"),
			"minimal_plugin",
			PluginState::Loaded,
			PluginState::Loaded,
		);
	}

	#[tokio::test]
	async fn loading_missing_wasm_returns_io_error() {
		let runtime = Arc::new(
			WasmRuntime::new(WasmRuntimeConfig::default()).expect("WASM runtime should initialize"),
		);
		let fixture_dir = TempDir::new().expect("temporary fixture directory should be created");
		let fixture_root = fixture_dir
			.path()
			.canonicalize()
			.expect("temporary fixture directory should have an absolute path");
		let missing_path = fixture_root.join("nonexistent_plugin.wasm");
		assert!(!missing_path.exists());
		let loader = WasmPluginLoader::new(fixture_root, runtime);

		let error = loader
			.load_from_path(&missing_path)
			.await
			.expect_err("loading a missing plugin should fail");
		match error {
			PluginError::Io(error) => assert_eq!(error.kind(), std::io::ErrorKind::NotFound),
			other => panic!("expected Io error, got {other:?}"),
		}
	}

	async fn load_instance(loader: &WasmPluginLoader, path: &Path) -> WasmPluginInstance {
		loader
			.load_from_path(path)
			.await
			.expect("source-built minimal Component should load")
	}

	struct BuiltMinimalFixture {
		_root_dir: TempDir,
		component_path: PathBuf,
	}

	impl BuiltMinimalFixture {
		fn component_path(&self) -> &Path {
			&self.component_path
		}

		fn component_dir(&self) -> &Path {
			self.component_path
				.parent()
				.expect("component output should have a parent directory")
		}
	}

	fn build_minimal_fixture() -> BuiltMinimalFixture {
		let version = Command::new("cargo-component")
			.arg("--version")
			.output()
			.unwrap_or_else(|error| {
				panic!(
					"cargo-component is required; install it before running WASM integration tests: {error}"
				)
			});
		if !version.status.success() {
			panic!(
				"cargo-component --version failed with {}\nstdout:\n{}\nstderr:\n{}",
				version.status,
				String::from_utf8_lossy(&version.stdout),
				String::from_utf8_lossy(&version.stderr),
			);
		}

		let source_dir =
			PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/plugins/minimal");
		let root_dir = TempDir::new().expect("temporary Component root should be created");
		let fixture_dir = root_dir.path().join("minimal");
		copy_fixture_tree(&source_dir, &fixture_dir)
			.expect("minimal Component fixture should be copied to the temporary root");
		let manifest_path = fixture_dir.join("Cargo.toml");
		let target_dir = root_dir.path().join("target");
		let output = Command::new("cargo")
			.args([
				"component",
				"build",
				"--release",
				"--locked",
				"--manifest-path",
			])
			.arg(&manifest_path)
			.env("CARGO_TARGET_DIR", &target_dir)
			.output()
			.expect("cargo component build should start");
		if !output.status.success() {
			panic!(
				"minimal Component build failed with {}\nstdout:\n{}\nstderr:\n{}",
				output.status,
				String::from_utf8_lossy(&output.stdout),
				String::from_utf8_lossy(&output.stderr),
			);
		}

		let component_path = target_dir.join("wasm32-wasip1/release/minimal_plugin.wasm");
		if !component_path.is_file() {
			panic!(
				"minimal Component build succeeded without expected output: wasm32-wasip1/release/minimal_plugin.wasm"
			);
		}

		BuiltMinimalFixture {
			_root_dir: root_dir,
			component_path,
		}
	}

	fn copy_fixture_tree(source: &Path, destination: &Path) -> io::Result<()> {
		fs::create_dir_all(destination)?;
		for entry in fs::read_dir(source)? {
			let entry = entry?;
			let file_type = entry.file_type()?;
			if file_type.is_dir() && entry.file_name() == OsStr::new("target") {
				continue;
			}

			let destination_path = destination.join(entry.file_name());
			if file_type.is_dir() {
				copy_fixture_tree(&entry.path(), &destination_path)?;
			} else {
				fs::copy(entry.path(), destination_path)?;
			}
		}

		Ok(())
	}

	fn assert_invalid_transition(
		error: PluginError,
		expected_plugin: &str,
		expected_from: PluginState,
		expected_to: PluginState,
	) {
		match error {
			PluginError::InvalidStateTransition { plugin, from, to } => {
				assert_eq!(plugin, expected_plugin);
				assert_eq!(from, expected_from);
				assert_eq!(to, expected_to);
			}
			other => panic!("expected InvalidStateTransition, got {other:?}"),
		}
	}
}
