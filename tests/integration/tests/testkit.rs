// Cross-crate coverage for the reinhardt-testkit integration surfaces.

#[path = "testkit/auth_integration.rs"]
mod auth_integration;

#[path = "testkit/client_integration.rs"]
mod client_integration;

#[path = "testkit/messages_integration.rs"]
mod messages_integration;

#[path = "testkit/static_files_integration.rs"]
mod static_files_integration;

#[path = "testkit/server_fn_integration.rs"]
mod server_fn_integration;

#[path = "testkit/mock_integration.rs"]
mod mock_integration;
