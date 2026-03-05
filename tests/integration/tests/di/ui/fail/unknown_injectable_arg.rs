use reinhardt_di::injectable;

// This should fail: `unknown_arg` is not a valid argument for #[injectable]
#[injectable(unknown_arg = "value")]
#[derive(Clone, Default)]
struct MyService;

fn main() {}
