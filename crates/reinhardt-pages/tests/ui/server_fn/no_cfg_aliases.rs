use reinhardt_pages::server_fn::{ServerFnError, server_fn};

#[server_fn(auto_register = false)]
async fn health() -> Result<&'static str, ServerFnError> {
	Ok("ok")
}

fn main() {}
