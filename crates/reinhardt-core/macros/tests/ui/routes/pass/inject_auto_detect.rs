//! Test: `#[inject]` auto-enables injection without `use_inject = true`

use reinhardt_macros::get;

struct MyService;

#[get("/test")]
async fn handler(#[inject] service: MyService) -> String {
	format!("Hello")
}

fn main() {}
