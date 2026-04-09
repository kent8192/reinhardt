// Compile-fail test: injectable_factory with invalid scope value

use reinhardt_di::injectable_factory;

#[derive(Clone)]
struct MyService;

#[injectable_factory(scope = "invalid_scope")]
async fn make_service() -> MyService {
	MyService
}

fn main() {}
