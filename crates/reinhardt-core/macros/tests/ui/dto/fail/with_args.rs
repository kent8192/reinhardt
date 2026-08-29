//! Verifies that `#[dto(...)]` rejects options other than `schema`.

use reinhardt_macros::dto;

#[dto(no_schema)]
pub struct Foo {
	pub x: i32,
}

fn main() {}
