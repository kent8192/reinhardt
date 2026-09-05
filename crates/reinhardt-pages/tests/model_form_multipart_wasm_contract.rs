#![cfg(not(target_arch = "wasm32"))]

use std::fs;
use std::process::Command;

use tempfile::TempDir;

fn check_wasm_fixture(crate_name: &str, form_expression: &str, diagnostic: &str) {
	let crate_dir = TempDir::new().expect("create mismatch fixture");
	let target_dir = TempDir::new().expect("create mismatch target directory");
	let pages_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
	let core_dir = pages_dir
		.parent()
		.expect("pages crate has a crates directory parent")
		.join("reinhardt-core");
	let support = pages_dir.join("tests/ui/form/model_multipart_support.rs");

	fs::write(
		crate_dir.path().join("Cargo.toml"),
		format!(
			r#"[package]
name = "{crate_name}"
version = "0.0.0"
edition = "2024"

[dependencies]
reinhardt-core = {{ path = {core_dir:?}, default-features = false, features = ["parsers"] }}
reinhardt-pages = {{ path = {pages_dir:?}, default-features = false }}
serde_json = "1"
"#,
		),
	)
	.expect("write mismatch fixture manifest");
	fs::create_dir(crate_dir.path().join("src")).expect("create mismatch fixture source directory");
	fs::write(
		crate_dir.path().join("src/main.rs"),
		format!(
			r#"include!({support:?});

use reinhardt_pages::form;

fn main() {{
	{form_expression}
}}
"#,
		),
	)
	.expect("write mismatch fixture source");

	let output = Command::new(std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into()))
		.arg("check")
		.arg("--manifest-path")
		.arg(crate_dir.path().join("Cargo.toml"))
		.arg("--target")
		.arg("wasm32-unknown-unknown")
		.arg("--target-dir")
		.arg(target_dir.path())
		.env_remove("CARGO_BUILD_BUILD_DIR")
		.env_remove("CARGO_TARGET_DIR")
		.env_remove("RUSTC_WRAPPER")
		.output()
		.expect("run mismatch fixture cargo check");
	let stderr = String::from_utf8_lossy(&output.stderr);

	assert!(
		!output.status.success(),
		"mismatched model form must not compile"
	);
	assert!(
		stderr.contains(diagnostic),
		"unexpected mismatch diagnostic:\n{stderr}"
	);
}

#[test]
fn multipart_model_form_rejects_reversed_fields_on_wasm() {
	check_wasm_fixture(
		"model-form-multipart-order-mismatch",
		r#"let _form = form! {
		name: ReversedUploadForm,
		model: Upload,
		policy: UploadPolicy,
		fields: [document, title, avatar],
		server_fn: upload,
	};"#,
		"model-form field name does not match server-function argument",
	);
}

#[test]
fn multipart_model_form_rejects_field_kind_mismatch_on_wasm() {
	check_wasm_fixture(
		"model-form-multipart-kind-mismatch",
		r#"let _form = form! {
		name: KindMismatchUploadForm,
		model: Upload,
		policy: UploadPolicy,
		fields: [title, document, avatar],
		server_fn: upload_wrong_types,
	};"#,
		"model-form field type or requiredness does not match server-function argument",
	);
}

#[test]
fn multipart_model_form_rejects_requiredness_mismatch_on_wasm() {
	check_wasm_fixture(
		"model-form-multipart-requiredness-mismatch",
		r#"let _form = form! {
		name: RequirednessMismatchUploadForm,
		model: Upload,
		policy: UploadPolicy,
		fields: [title, document, avatar],
		server_fn: upload_wrong_requiredness,
	};"#,
		"model-form field type or requiredness does not match server-function argument",
	);
}
