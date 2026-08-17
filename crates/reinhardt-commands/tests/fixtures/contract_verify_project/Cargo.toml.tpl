[package]
name = "contract-verify-consumer"
version = "0.1.0"
edition = "2024"

[lib]
path = "src/lib.rs"

[[bin]]
name = "manage"
path = "src/bin/manage.rs"

[dependencies]
reinhardt = { path = "__REINHARDT_ROOT__", package = "reinhardt-web", default-features = false, features = ["core", "commands-contract", "db-sqlite"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
tokio = { version = "1.48", features = ["macros", "rt-multi-thread"] }
ctor = "0.6.1"
