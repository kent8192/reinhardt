use reinhardt_db_macros::document;

#[document(backend = "mongodb")]
struct User {
    #[field(primary_key)]
    id: String,
}

fn main() {}
