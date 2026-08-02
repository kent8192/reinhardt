use reinhardt_db::orm::FieldRef;
use reinhardt_db::orm::expressions::GeneratedModelField;

struct User;

fn main() {
	let _ = FieldRef::<User, String, GeneratedModelField>::new("forged");
}
