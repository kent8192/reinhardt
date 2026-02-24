// build.rs
use cfg_aliases::cfg_aliases;

fn main() {
	// Auto-detect: check if reinhardt workspace exists in parent
	let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
	let examples_dir = std::path::Path::new(&manifest_dir).parent().unwrap();
	let parent_dir = examples_dir.parent().unwrap();
	let parent_cargo = parent_dir.join("Cargo.toml");

	let is_local_dev = parent_cargo.exists()
		&& std::fs::read_to_string(&parent_cargo)
			.map(|c| c.contains("name = \"reinhardt-web\""))
			.unwrap_or(false);

	if is_local_dev {
		// In subtree context - enable integration tests
		println!("cargo:rustc-cfg=with_reinhardt");

		// Warn if .cargo/config.toml is not set up for local override
		let config_path = examples_dir.join(".cargo/config.toml");
		if !config_path.exists() {
			println!(
				"cargo:warning=Local reinhardt workspace detected but .cargo/config.toml is missing. \
				 Copy the template: cp .cargo/config.local.toml .cargo/config.toml"
			);
		}
	} else {
		// Standalone mode - enable tests if crates.io versions are available
		println!("cargo:rustc-cfg=with_reinhardt");
	}

	println!("cargo:rerun-if-changed=build.rs");
	println!("cargo:rerun-if-changed=../.cargo/config.toml");

	// Declare custom cfg to avoid warnings in Rust 2024 edition
	println!("cargo::rustc-check-cfg=cfg(with_reinhardt)");
	println!("cargo::rustc-check-cfg=cfg(client)");
	println!("cargo::rustc-check-cfg=cfg(server)");

	cfg_aliases! {
		// Platform aliases for simpler conditional compilation
		// Use `#[cfg(client)]` instead of `#[cfg(target_arch = "wasm32")]`
		client: { target_arch = "wasm32" },
		// Use `#[cfg(server)]` instead of `#[cfg(not(target_arch = "wasm32"))]`
		server: { not(target_arch = "wasm32") },
	}
}
