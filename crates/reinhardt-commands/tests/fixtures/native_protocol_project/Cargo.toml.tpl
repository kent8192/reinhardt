[package]
name = "native-protocol-project"
version = "0.1.0"
edition = "2024"

[workspace]
resolver = "3"

[dependencies]
reinhardt = { path = "{{ workspace_root }}", package = "reinhardt-web", default-features = false, features = ["minimal", "grpc", "websockets", "commands", "commands-server"] }
futures-util = "0.3.32"
prost = "0.14.3"
reqwest = { version = "0.12", features = ["json"] }
tokio = { version = "1.51.0", features = ["full"] }
tokio-tungstenite = "0.28.0"
tonic = "0.14.5"
tonic-prost = "0.14.5"

[build-dependencies]
tonic-prost-build = "0.14.5"
