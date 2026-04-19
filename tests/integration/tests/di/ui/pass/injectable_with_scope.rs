use reinhardt_di::injectable;

#[injectable(scope = "singleton")]
#[derive(Clone, Default)]
struct SingletonService;

#[injectable(scope = "request")]
#[derive(Clone, Default)]
struct RequestService;

#[injectable(scope = "transient")]
#[derive(Clone, Default)]
struct TransientService;

fn main() {}
