use reinhardt_db::orm::UniqueFieldRef;

struct Article;

fn main() {
	let _ = UniqueFieldRef::<Article, String>::from_model_field("title", "title");
}
