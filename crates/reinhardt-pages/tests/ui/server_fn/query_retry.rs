use reinhardt_pages::reactive::{QueryOptions, RetryPolicy, use_query};
use reinhardt_pages::server_fn::ServerFnError;
use reinhardt_pages_macros::server_fn;

#[server_fn]
async fn load_status() -> Result<String, ServerFnError> {
	Ok("ready".to_owned())
}

fn main() {
	if false {
		let _ = use_query(
			load_status::query(),
			QueryOptions::new().retry(
				RetryPolicy::exponential().when(|_: &ServerFnError| true),
			),
		);
	}
}
