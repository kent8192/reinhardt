use reinhardt_macros::Validate;

trait Validate {
	fn validate(&self) -> Result<(), String>;
}

#[derive(Validate)]
struct Foo {
	#[validate(range())]
	value: i32,
}

fn main() {}
