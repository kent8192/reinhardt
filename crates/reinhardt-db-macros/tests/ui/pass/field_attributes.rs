use reinhardt_db_macros::document;
use bson::oid::ObjectId;
use serde::{Serialize, Deserialize};

#[document(collection = "users", backend = "mongodb")]
#[derive(Serialize, Deserialize)]
struct User {
    #[field(primary_key)]
    id: Option<ObjectId>,

    #[field(required, unique)]
    email: String,

    #[field(default = "Anonymous")]
    name: String,

    #[field(rename = "user_age")]
    age: i32,

    #[field(min = 0, max = 120)]
    score: i32,
}

fn main() {}
