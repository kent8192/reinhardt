use reinhardt_db::orm::{FieldRef, GeneratedModelField};

struct User;

fn main() {
	let _ = FieldRef::<User, String, GeneratedModelField>::new("forged");
}
