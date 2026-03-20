use reinhardt_db_macros::document;

#[document(collection = "users", backend = "mongodb")]
struct User {
    #[field(primary_key)]
    id: Option<String>,

    #[field(requried)]
    name: String,
}

fn main() {}
