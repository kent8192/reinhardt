// Tests that #[injectable] cannot be applied to tuple structs
use reinhardt_di_macros::injectable;

#[injectable]
struct TupleService(String, u32);

fn main() {}
