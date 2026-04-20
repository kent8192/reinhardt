//! Server functions
//!
//! Server functions that can be called from the WASM client.
//! These functions are automatically converted to HTTP endpoints.
//!
//! Since rc.14, `#[inject]` parameters are auto-detected — no `use_inject = true` needed.

// Example server function:
//
// use reinhardt::pages::server_fn;
// use std::sync::Arc;
//
// #[server_fn]
// pub async fn get_data(
//     #[inject] db: Arc<DatabaseConnection>,
// ) -> Result<Vec<DataItem>, String> {
//     // Implementation
//     todo!()
// }
