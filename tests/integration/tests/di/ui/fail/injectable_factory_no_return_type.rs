// Compile-fail test: injectable_factory without explicit return type

use reinhardt_di::injectable_factory;

#[injectable_factory(scope = "singleton")]
async fn make_service() {
	// No return type
}

fn main() {}
