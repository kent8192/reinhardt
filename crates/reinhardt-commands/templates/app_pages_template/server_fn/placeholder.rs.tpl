//! Placeholder server function for the {{ app_name }} application.
//!
//! Delete this module once the app has real server functions.

use reinhardt::pages::server_fn::{ServerFnError, server_fn};

#[server_fn(auto_register = false)]
pub async fn placeholder() -> std::result::Result<(), ServerFnError> {
    Ok(())
}
