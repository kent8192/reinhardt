//! User items may use the former generated auto-registration factory name.

use reinhardt_pages::server_fn::ServerFnError;
use reinhardt_pages_macros::server_fn;

fn __reinhardt_auto_register_collision_safe() {}

#[server_fn]
async fn collision_safe() -> Result<(), ServerFnError> {
	Ok(())
}

fn main() {
	__reinhardt_auto_register_collision_safe();
}
