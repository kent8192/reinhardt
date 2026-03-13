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
		// Use `#[cfg(wasm)]` instead of `#[cfg(target_arch = "wasm32")]`
		wasm: { target_arch = "wasm32" },
		// Use `#[cfg(native)]` instead of `#[cfg(not(target_arch = "wasm32"))]`
		native: { not(target_arch = "wasm32") },

		// Rendering mode aliases (for future use)
		ssr: { all(not(target_arch = "wasm32"), feature = "ssr") },
		csr: { all(target_arch = "wasm32", not(feature = "hydrate")) },
		hydrate: { all(target_arch = "wasm32", feature = "hydrate") },
	}
}
