[package]
name = "{{ app_name }}"
version = "0.1.0"
edition = "2024"

[lib]
name = "{{ app_name }}"
path = "src/lib.rs"
crate-type = ["cdylib", "rlib"]  # cdylib for WASM, rlib for server

[dependencies]
reinhardt-core = { workspace = true }
reinhardt-orm = { workspace = true }
reinhardt-routers = { workspace = true }
tokio = { workspace = true }
async-trait = { workspace = true }
serde = { workspace = true }
chrono = { workspace = true }
uuid = { workspace = true }

[target.'cfg(target_arch = "wasm32")'.dependencies]
wasm-bindgen = { workspace = true }
web-sys = { workspace = true }
js-sys = { workspace = true }
console_error_panic_hook = { workspace = true }
wasm-bindgen-futures = { workspace = true }

[build-dependencies]
cfg_aliases = "0.2"
