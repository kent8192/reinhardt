use reinhardt_pages::FromRequest;
use reinhardt_pages::router::request::{OptionalQueryParam, PathParam, QueryParam};

#[derive(FromRequest)]
struct UserRequest {
	id: PathParam<i64>,
	#[from_request(name = "tab")]
	selected_tab: QueryParam<String>,
	logs: OptionalQueryParam<i64>,
}

fn main() {}
