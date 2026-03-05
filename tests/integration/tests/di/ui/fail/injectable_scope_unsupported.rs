use reinhardt_di::injectable;

// This should fail: `scope` is not yet supported on #[injectable] structs
#[injectable(scope = "singleton")]
#[derive(Clone, Default)]
struct MyService;

fn main() {}
