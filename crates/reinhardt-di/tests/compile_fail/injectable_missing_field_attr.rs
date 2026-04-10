// Tests that #[injectable] requires #[inject] or #[no_inject] on every field
use reinhardt_di_macros::injectable;

#[injectable]
struct MissingAttrService {
    name: String,
}

fn main() {}
