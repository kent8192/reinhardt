use reinhardt_db_macros::document;

#[document(collection = "users", backend = "postgres")]
struct User {
    #[field(primary_key)]
    id: String,
}

fn main() {}
