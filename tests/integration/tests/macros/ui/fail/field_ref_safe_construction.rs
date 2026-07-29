use reinhardt_db::orm::FieldRef;

struct User;

fn main() {
	let _ = FieldRef::<User, String>::new("forged");
}
