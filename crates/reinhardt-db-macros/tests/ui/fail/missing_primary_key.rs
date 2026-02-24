use reinhardt_db_macros::document;

#[document(collection = "users", backend = "mongodb")]
struct User {
    name: String,
}

fn main() {}
