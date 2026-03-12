[package]
name = "{{ project_name }}"
version = "0.1.0"
edition = "2024"

[[bin]]
name = "manage"
path = "src/bin/manage.rs"

[dependencies]
reinhardt = { version = "{{ reinhardt_version }}", package = "reinhardt-web", features = ["full"] }
serde_json = "1.0"
tokio = { version = "1", features = ["full"] }

[features]
client-router = ["reinhardt/client-router"]
