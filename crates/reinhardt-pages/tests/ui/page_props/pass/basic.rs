use reinhardt_pages::page_props;
use reinhardt_pages::router::request::{FromRequest, RouteContext};

#[page_props]
struct UserPageProps {
	#[from_request(path)]
	id: i64,
	#[from_request(query)]
	tab: Option<String>,
}

fn main() {
	let _ = UserPageProps::builder().id(7).build();
	let _extractor: fn(&RouteContext) -> Result<UserPageProps, _> = UserPageProps::from_request;
}
