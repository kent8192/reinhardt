// Tests that #[injectable] cannot be applied to enums
use reinhardt_di_macros::injectable;

#[injectable]
enum ServiceKind {
    A,
    B,
}

fn main() {}
