[package]
name = "{{ project_name }}"
version = "0.1.0"
edition = "2024"
default-run = "manage"

[workspace]
resolver = "3"
members = [
]

[profile.dev]
codegen-units = 16
debug = 0

[lib]
crate-type = ["cdylib", "rlib"]  # cdylib for WASM, rlib for server

[[bin]]
name = "manage"
path = "src/bin/manage.rs"
required-features = ["with-reinhardt"]

[dependencies]
chrono = { version = "0.4", features = ["serde"] }
uuid = { version = "1.11", features = ["v4", "serde"] }
ctor = "0.6"
linkme = "0.3"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
inventory = "0.3"

[target.'cfg(target_arch = "wasm32")'.dependencies]
reinhardt = { version = "{{ reinhardt_version }}", package = "reinhardt-web", default-features = false, features = ["pages", "client-router"] }
wasm-bindgen = "=0.2.122"
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
# Keep the facade dependency after the native-only alias. proc-macro-crate
# resolves duplicate package dependencies to the last declaration, and
# Reinhardt macros must emit the cross-target `reinhardt` crate name.
reinhardt-shell = { version = "{{ reinhardt_version }}", package = "reinhardt-web", default-features = false, features = ["commands-shell"], optional = true }
reinhardt = { version = "{{ reinhardt_version }}", package = "reinhardt-web", default-features = {{ reinhardt_default_features }}, features = {{ reinhardt_features_toml }} }
clap = { version = "4", features = ["derive"] }
console = "0.16.1"
tokio = { version = "1", features = ["full"] }

[build-dependencies]
cfg_aliases = "0.2"

[features]
default = ["with-reinhardt", "client-router"]
client-router = []
commands-shell = ["reinhardt/commands", "reinhardt/database", "reinhardt/di", "dep:reinhardt-commands"]
with-reinhardt = []
msw = ["reinhardt/msw"]

[dev-dependencies]
rstest = "0.26.1"

[dev-dependencies.reinhardt]
version = "{{ reinhardt_version }}"
package = "reinhardt-web"
features = ["full", "test", "testcontainers"]

[target.'cfg(not(target_arch = "wasm32"))'.dev-dependencies]
reqwest = { version = "0.12", features = ["json"] }
tokio = { version = "1", features = ["full"] }
