//! Explicit opt-out preserves the marker API without submitting inventory.

use reinhardt_pages::server_fn::{ServerFnError, ServerFnMetadata};
use reinhardt_pages_macros::server_fn;

#[server_fn(auto_register = false)]
async fn manually_mounted() -> Result<(), ServerFnError> {
	Ok(())
}

fn main() {
	assert_eq!(
		<manually_mounted::marker as ServerFnMetadata>::MODULE_PATH,
		format!("{}::manually_mounted", module_path!())
	);
}
