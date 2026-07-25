//! Default server functions emit a native inventory registration.

use reinhardt_pages::server_fn::{ServerFnError, ServerFnMetadata};
use reinhardt_pages_macros::server_fn;

#[server_fn]
async fn automatically_mounted() -> Result<(), ServerFnError> {
	Ok(())
}

fn main() {
	assert_eq!(
		<automatically_mounted::marker as ServerFnMetadata>::MODULE_PATH,
		format!("{}::automatically_mounted", module_path!())
	);
}
