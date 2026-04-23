[package]
name = "{{ project_name }}"
version = "0.1.0"
edition = "2024"
default-run = "manage"

[lib]
crate-type = ["cdylib", "rlib"]  # cdylib for WASM, rlib for server

[[bin]]
name = "manage"
path = "src/bin/manage.rs"

[dependencies]
reinhardt = { version = "{{ reinhardt_version }}", package = "reinhardt-web", features = [
	"full",
	"admin",  # Admin panel functionality (includes reinhardt-pages)
] }

chrono = { version = "0.4", features = ["serde"] }
uuid = { version = "1.11", features = ["v4", "serde"] }
ctor = "0.6"
linkme = "0.3"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
inventory = "0.3"

[target.'cfg(target_arch = "wasm32")'.dependencies]
wasm-bindgen = "0.2"
web-sys = { version = "0.3", features = [
	"Window",
	"Document",
	"Element",
	"HtmlInputElement",
	"HtmlFormElement",
	"Event",
	"EventTarget",
	"Location",
	"History",
] }
js-sys = "0.3"
console_error_panic_hook = "0.1"
wasm-bindgen-futures = "0.4"

[target.'cfg(not(target_arch = "wasm32"))'.dependencies]
clap = { version = "4", features = ["derive"] }
console = "0.16.1"
tokio = { version = "1", features = ["full"] }

[build-dependencies]
cfg_aliases = "0.2"

[dev-dependencies]
rstest = "0.26.1"
reqwest = { version = "0.12", features = ["json"] }
tokio = { version = "1", features = ["full"] }

[dev-dependencies.reinhardt]
version = "{{ reinhardt_version }}"
package = "reinhardt-web"
features = ["full", "test", "testcontainers"]
