include!("../support.rs");

struct User;

fn main() {
	let _ = crate::db::orm::expressions::FieldRef::<User, String>::new("forged");
}
