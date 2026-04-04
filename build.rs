use cfg_aliases::cfg_aliases;

fn main() -> Result<(), Box<dyn std::error::Error>> {
	// Rust 2024 edition requires explicit check-cfg declarations
	println!("cargo::rustc-check-cfg=cfg(wasm)");
	println!("cargo::rustc-check-cfg=cfg(native)");

	cfg_aliases! {
		wasm: { all(target_family = "wasm", target_os = "unknown") },
		native: { not(all(target_family = "wasm", target_os = "unknown")) },
	}

	// Skip gRPC proto compilation when building for WASM
	// CARGO_CFG_TARGET_FAMILY and CARGO_CFG_TARGET_OS are set by Cargo during cross-compilation
	let target_family = std::env::var("CARGO_CFG_TARGET_FAMILY").unwrap_or_default();
	let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
	if target_family == "wasm" && target_os == "unknown" {
		return Ok(());
	}

	// Skip proto compilation when proto files are not available
	// (e.g., when building from crates.io where tests/ is not included)
	let proto_dir = std::path::Path::new("tests/proto");
	if !proto_dir.exists() {
		return Ok(());
	}

	// Compile proto files for gRPC integration tests
	tonic_prost_build::configure()
		.build_server(true)
		.build_client(true)
		.compile_protos(
			&[
				"tests/proto/common.proto",
				"tests/proto/user.proto",
				"tests/proto/user_events.proto",
			],
			&["tests/proto"],
		)?;

	Ok(())
}
