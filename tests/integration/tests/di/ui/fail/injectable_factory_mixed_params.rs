// Compile-fail test: injectable_factory with mix of inject and non-inject params

use reinhardt_di::injectable_factory;

#[derive(Clone)]
struct MyService {
	name: String,
}

// This should fail: cannot mix inject and non-inject params
#[injectable_factory(scope = "transient")]
async fn make_service(#[inject] _config: reinhardt_di::Depends<String>, extra: String) -> MyService {
	MyService { name: extra }
}

fn main() {}
