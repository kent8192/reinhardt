//! Build script for {{ project_name }}.
//!
//! Sets up cfg aliases for simplified conditional compilation.

use cfg_aliases::cfg_aliases;

fn main() {
    println!("cargo:rustc-cfg=with_reinhardt");
    println!("cargo:rerun-if-changed=build.rs");

    // Rust 2024 edition requires explicit check-cfg declarations
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
