//! Build script for reinhardt-pages.
//!
//! Sets up cfg aliases for simplified conditional compilation.

use cfg_aliases::cfg_aliases;

fn main() {
	// Rust 2024 edition requires explicit check-cfg declarations
	println!("cargo::rustc-check-cfg=cfg(wasm)");
	println!("cargo::rustc-check-cfg=cfg(native)");
	println!("cargo::rustc-check-cfg=cfg(ssr)");
	println!("cargo::rustc-check-cfg=cfg(csr)");
	println!("cargo::rustc-check-cfg=cfg(hydrate)");
	println!("cargo::rustc-check-cfg=cfg(hmr)");

	cfg_aliases! {
		// Platform aliases for simpler conditional compilation
		// Use `#[cfg(wasm)]` instead of `#[cfg(all(target_family = "wasm", target_os = "unknown"))]`
		wasm: { all(target_family = "wasm", target_os = "unknown") },
		// Use `#[cfg(native)]` instead of `#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]`
		native: { not(all(target_family = "wasm", target_os = "unknown")) },

		// Rendering mode aliases (for future use)
		ssr: { all(not(all(target_family = "wasm", target_os = "unknown")), feature = "ssr") },
		csr: { all(all(target_family = "wasm", target_os = "unknown"), not(feature = "hydrate")) },
		hydrate: { all(all(target_family = "wasm", target_os = "unknown"), feature = "hydrate") },
	}
}
