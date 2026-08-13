//! Build script for {{ project_name }}.
//!
//! Sets up cfg aliases for simplified conditional compilation.

use cfg_aliases::cfg_aliases;
use std::env;

fn main() {
	let mut features: Vec<_> = env::vars()
		.filter_map(|(key, _)| key.strip_prefix("CARGO_FEATURE_").map(str::to_owned))
		.map(|feature| feature.to_ascii_lowercase().replace('_', "-"))
		.collect();
	features.sort();
	features.dedup();
	println!("cargo:rustc-env=REINHARDT_ENABLED_FEATURES={}", features.join(","));
	if let Ok(target) = env::var("TARGET") {
		println!("cargo:rustc-env=REINHARDT_TARGET={target}");
	}
	if let Ok(profile) = env::var("PROFILE") {
		println!("cargo:rustc-env=REINHARDT_PROFILE={profile}");
	}
	if let Ok(flags) = env::var("CARGO_ENCODED_RUSTFLAGS") {
		println!("cargo:rustc-env=REINHARDT_ENCODED_RUSTFLAGS={flags}");
	}
    println!("cargo:rustc-cfg=with_reinhardt");
    println!("cargo:rerun-if-changed=build.rs");

    // Rust 2024 edition requires explicit check-cfg declarations
    println!("cargo::rustc-check-cfg=cfg(with_reinhardt)");
    println!("cargo::rustc-check-cfg=cfg(client)");
    println!("cargo::rustc-check-cfg=cfg(server)");
    println!("cargo::rustc-check-cfg=cfg(wasm)");
    println!("cargo::rustc-check-cfg=cfg(native)");

    cfg_aliases! {
        // Platform aliases for simpler conditional compilation
        // Use `#[cfg(client)]` instead of `#[cfg(target_arch = "wasm32")]`
        client: { target_arch = "wasm32" },
        // Use `#[cfg(server)]` instead of `#[cfg(not(target_arch = "wasm32"))]`
        server: { not(target_arch = "wasm32") },
        // Compatibility aliases used by framework macro expansions.
        wasm: { target_arch = "wasm32" },
        native: { not(target_arch = "wasm32") },
    }
}
