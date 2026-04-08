// Compile-fail test: injectable_factory with non-inject parameters

use reinhardt_di::injectable_factory;

#[derive(Clone)]
struct MyService {
	name: String,
}

#[injectable_factory(scope = "singleton")]
async fn make_service(name: String) -> MyService {
	MyService { name }
}

fn main() {}
