//! Test: pre_validate must be a boolean

use reinhardt_macros::get;

#[get("/test", pre_validate = "yes")]
async fn handler() -> String {
	"Hello".to_string()
}

fn main() {}
