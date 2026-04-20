//! {{ app_name }} - Server functions
//!
//! Server functions that can be called from the WASM client.
//!
//! Since rc.14, `#[inject]` parameters are auto-detected — no `use_inject = true` needed.

// Example server function:
//
// use reinhardt::pages::server_fn;
// use std::sync::Arc;
//
// #[server_fn]
// pub async fn get_items(
//     #[inject] db: Arc<DatabaseConnection>,
// ) -> Result<Vec<DataItem>, String> {
//     // Implementation
//     todo!()
// }
