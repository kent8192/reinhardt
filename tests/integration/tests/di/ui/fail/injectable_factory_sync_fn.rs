// Compile-fail test: injectable_factory on a non-async function

use reinhardt_di::injectable_factory;

#[derive(Clone)]
struct MyService;

#[injectable_factory(scope = "singleton")]
fn make_service() -> MyService {
	MyService
}

fn main() {}
