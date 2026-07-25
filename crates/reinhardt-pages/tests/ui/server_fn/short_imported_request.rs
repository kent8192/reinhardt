mod dto {
	#[derive(serde::Serialize, serde::Deserialize)]
	pub(crate) struct VoteRequest {
		pub choice_id: i64,
	}
}

use dto::VoteRequest;
use reinhardt_pages::server_fn::{ServerFnError, server_fn};

#[server_fn(auto_register = false)]
async fn vote(request: VoteRequest) -> Result<i64, ServerFnError> {
	Ok(request.choice_id)
}

fn main() {}
