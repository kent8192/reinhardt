use std::{fs, path::PathBuf, process::Command};

#[test]
fn vector_module_requires_the_pgvector_feature() {
	let temporary_project = tempfile::Builder::new()
		.prefix("reinhardt-pgvector-feature-boundary-")
		.tempdir_in("/tmp")
		.unwrap();
	let manifest_path = temporary_project.path().join("Cargo.toml");
	let source_directory = temporary_project.path().join("src");
	let crate_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
	let crate_path = crate_path.to_str().unwrap();

	fs::create_dir(&source_directory).unwrap();
	fs::write(
		&manifest_path,
		format!(
			"[package]\nname = \"pgvector-feature-boundary\"\nversion = \"0.0.0\"\nedition = \"2024\"\n\n[dependencies]\nreinhardt-db = {{ path = \"{crate_path}\", default-features = false, features = [\"orm\", \"postgres\"] }}\n"
		),
	)
	.unwrap();
	fs::write(
		source_directory.join("main.rs"),
		"use reinhardt_db::orm::vector::Vector;\n\nfn main() {\n    let _ = Vector::<3>::try_from(vec![1.0, 2.0, 3.0]);\n}\n",
	)
	.unwrap();

	let output = Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".into()))
		.arg("check")
		.arg("--manifest-path")
		.arg(manifest_path)
		.env("CARGO_TARGET_DIR", temporary_project.path().join("target"))
		.output()
		.unwrap();

	assert!(!output.status.success());
	let stderr = String::from_utf8(output.stderr).unwrap();
	assert!(stderr.contains("could not find `vector` in `orm`"));
}
