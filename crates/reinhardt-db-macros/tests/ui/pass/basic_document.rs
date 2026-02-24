use reinhardt_db_macros::document;
use bson::oid::ObjectId;
use serde::{Serialize, Deserialize};

#[document(collection = "users", backend = "mongodb")]
#[derive(Serialize, Deserialize)]
struct User {
    #[field(primary_key)]
    id: Option<ObjectId>,
    name: String,
    email: String,
}

fn main() {}
