//! Common views for the project
//!
//! Root endpoints and demo endpoints

use reinhardt::core::serde::json;
use reinhardt::get;
use reinhardt::http::ViewResult;
use reinhardt::{Response, StatusCode};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct User {
	pub id: u64,
	pub name: String,
	pub email: String,
}

/// List all users (demo endpoint)
#[get("/api/users/", name = "users_list")]
pub async fn list_users() -> ViewResult<Response> {
	let users = vec![
		User {
			id: 1,
			name: "Alice".to_string(),
			email: "alice@example.com".to_string(),
		},
		User {
			id: 2,
			name: "Bob".to_string(),
			email: "bob@example.com".to_string(),
		},
	];

	let json = json::to_string(&users)?;
	Ok(Response::new(StatusCode::OK)
		.with_header("Content-Type", "application/json")
		.with_body(json.into_bytes()))
}
