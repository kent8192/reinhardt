// HTTP integration tests module
// Each test file in http/ subdirectory is explicitly included with #[path] attribute

#[path = "http/extensions_integration.rs"]
mod extensions_integration;

#[path = "http/request_response_integration.rs"]
mod request_response_integration;
