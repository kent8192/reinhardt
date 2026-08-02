[package]
name = "{{ project_name }}"
version = "0.1.0"
edition = "2024"

[profile.dev]
codegen-units = 16
debug = 0

[[bin]]
name = "manage"
path = "src/bin/manage.rs"

[dependencies]
reinhardt = { version = "{{ reinhardt_version }}", package = "reinhardt-web", default-features = {{ reinhardt_default_features }}, features = {{ reinhardt_features_toml }} }
ctor = "0.6"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
tokio = { version = "1", features = ["full"] }
chrono = { version = "0.4", features = ["serde"] }
rust_decimal = "1"
uuid = { version = "1", features = ["serde"] }

[features]
default = []
commands-shell = ["reinhardt/commands-shell"]
