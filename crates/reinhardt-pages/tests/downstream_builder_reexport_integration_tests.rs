#![cfg(not(target_arch = "wasm32"))]

use std::fs;
use std::process::Command;

use rstest::rstest;
use tempfile::TempDir;

#[test]
fn generated_builders_do_not_require_downstream_bon_dependency() {
	let crate_dir = TempDir::new().expect("create downstream fixture");
	let target_dir = TempDir::new().expect("create downstream target dir");
	let reinhardt_pages_dir = env!("CARGO_MANIFEST_DIR").replace('\\', "\\\\");

	fs::write(
		crate_dir.path().join("Cargo.toml"),
		format!(
			r#"[package]
name = "downstream-builder-reexport-fixture"
version = "0.0.0"
edition = "2024"

[dependencies]
reinhardt-pages = {{ path = "{reinhardt_pages_dir}" }}
# Workaround for Lokathor/tinyvec#225 (tracked in reinhardt-web#6260)
# Remove this workaround when tinyvec publishes a release that compiles with
# the `alloc` feature without `std`.
#
# Ideal implementation (without workaround):
# omit this direct pin and let isolated fixtures resolve tinyvec from crates.io.
tinyvec = "=1.12.0"
"#
		),
	)
	.expect("write downstream manifest");
	fs::create_dir(crate_dir.path().join("src")).expect("create downstream src dir");
	fs::write(
		crate_dir.path().join("src/main.rs"),
		r#"use reinhardt_pages::router::request::{FromRequest, RouteContext};
use reinhardt_pages::{Page, Path, Query, component, page, page_props};

#[page_props]
struct SearchPageProps {
	#[from_request(path)]
	id: i64,
	#[from_request(query)]
	tab: String,
}

#[component("/users/{id}/", name = "user-detail")]
fn user_page(Path(id): Path<i64>, Query(tab): Query<String>) -> Page {
	page!(|id: i64, tab: String| {
		div { {
			format!("{id}:{tab}")
		} }
	})(id, tab)
}

fn main() {
	let _ = SearchPageProps::builder()
		.id(7)
		.tab("profile".to_string())
		.build();
	let _extractor: fn(&RouteContext) -> Result<SearchPageProps, _> =
		SearchPageProps::from_request;
	let _ = UserPageProps::builder()
		.id(7)
		.tab("profile".to_string())
		.build();
}
"#,
	)
	.expect("write downstream source");

	let output = Command::new(std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into()))
		.arg("check")
		.arg("--manifest-path")
		.arg(crate_dir.path().join("Cargo.toml"))
		.arg("--target-dir")
		.arg(target_dir.path())
		.output()
		.expect("run downstream cargo check");

	assert!(
		output.status.success(),
		"downstream fixture should compile without a direct bon dependency\nstdout:\n{}\nstderr:\n{}",
		String::from_utf8_lossy(&output.stdout),
		String::from_utf8_lossy(&output.stderr)
	);
}

#[rstest]
fn multipart_server_fn_wasm_signature_does_not_require_downstream_web_sys_dependency() {
	let crate_dir = TempDir::new().expect("create downstream fixture");
	let target_dir = TempDir::new().expect("create downstream target dir");
	let reinhardt_pages_dir = env!("CARGO_MANIFEST_DIR").replace('\\', "\\\\");
	let reinhardt_core_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
		.parent()
		.expect("pages crate should have a crates directory parent")
		.join("reinhardt-core")
		.display()
		.to_string()
		.replace('\\', "\\\\");

	fs::write(
		crate_dir.path().join("Cargo.toml"),
		format!(
			r#"[package]
name = "downstream-multipart-reexport-fixture"
version = "0.0.0"
edition = "2024"

[dependencies]
reinhardt-core = {{ path = "{reinhardt_core_dir}", default-features = false, features = ["parsers"] }}
reinhardt-pages = {{ path = "{reinhardt_pages_dir}", default-features = false }}
serde_json = "1"
# Workaround for Lokathor/tinyvec#225 (tracked in reinhardt-web#6260)
# Remove this workaround when tinyvec publishes a release that compiles with
# the `alloc` feature without `std`.
#
# Ideal implementation (without workaround):
# omit this direct pin and let isolated fixtures resolve tinyvec from crates.io.
tinyvec = "=1.12.0"
"#
		),
	)
	.expect("write downstream manifest");
	fs::create_dir(crate_dir.path().join("src")).expect("create downstream src dir");
	fs::write(
		crate_dir.path().join("src/main.rs"),
		r#"use reinhardt_core::parsers::UploadedFile;
use reinhardt_pages::server_fn::{ServerFnError, server_fn};

#[server_fn]
async fn upload(avatar: UploadedFile) -> Result<(), ServerFnError> {
	let _ = avatar;
	Ok(())
}

fn main() {}
"#,
	)
	.expect("write downstream source");

	let output = Command::new(std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into()))
		.arg("check")
		.arg("--manifest-path")
		.arg(crate_dir.path().join("Cargo.toml"))
		.arg("--target")
		.arg("wasm32-unknown-unknown")
		.arg("--target-dir")
		.arg(target_dir.path())
		.output()
		.expect("run downstream wasm cargo check");

	assert!(
		output.status.success(),
		"downstream fixture should compile without a direct web-sys dependency\nstdout:\n{}\nstderr:\n{}",
		String::from_utf8_lossy(&output.stdout),
		String::from_utf8_lossy(&output.stderr)
	);
}
