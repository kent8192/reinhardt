//! Tests for auth app
//!
//! Tests for user registration, login, and user queries.

#[cfg(test)]
mod tests {
	use crate::config::schema::get_schema;
	use async_graphql::Request;

	#[tokio::test]
	async fn test_register_user() {
		let schema = get_schema();

		// Register a new user
		let query = r#"
			mutation {
				register(input: {
					username: "testuser",
					email: "test@example.com",
					password: "password123",
					firstName: "Test",
					lastName: "User"
				}) {
					token
					user {
						id
						username
						email
						firstName
						lastName
						isActive
					}
				}
			}
		"#;

		let response = schema.execute(Request::new(query)).await;
		let data = response.data.into_json().unwrap();

		// Verify the response structure
		assert!(data["register"]["token"].is_string());
		assert!(data["register"]["user"]["id"].is_string());
		assert_eq!(data["register"]["user"]["username"], "testuser");
		assert_eq!(data["register"]["user"]["email"], "test@example.com");
		assert_eq!(data["register"]["user"]["firstName"], "Test");
		assert_eq!(data["register"]["user"]["lastName"], "User");
		assert_eq!(data["register"]["user"]["isActive"], true);
	}

	#[tokio::test]
	async fn test_login_user() {
		let schema = get_schema();

		// First register a user
		let register_query = r#"
			mutation {
				register(input: {
					username: "loginuser",
					email: "login@example.com",
					password: "password123"
				}) {
					token
				}
			}
		"#;
		schema.execute(Request::new(register_query)).await;

		// Then login
		let login_query = r#"
			mutation {
				login(input: {
					username: "loginuser",
					password: "password123"
				}) {
					token
					user {
						username
						email
					}
				}
			}
		"#;

		let response = schema.execute(Request::new(login_query)).await;
		let data = response.data.into_json().unwrap();

		assert!(data["login"]["token"].is_string());
		assert_eq!(data["login"]["user"]["username"], "loginuser");
		assert_eq!(data["login"]["user"]["email"], "login@example.com");
	}

	#[tokio::test]
	async fn test_login_invalid_credentials() {
		let schema = get_schema();

		// Try to login with non-existent user
		let login_query = r#"
			mutation {
				login(input: {
					username: "nonexistent",
					password: "wrongpassword"
				}) {
					token
				}
			}
		"#;

		let response = schema.execute(Request::new(login_query)).await;

		// Should have errors
		assert!(!response.errors.is_empty());
	}

	#[tokio::test]
	async fn test_users_query() {
		let schema = get_schema();

		// Register multiple users
		let register1 = r#"
			mutation {
				register(input: {
					username: "user1",
					email: "user1@example.com",
					password: "password123"
				}) { token }
			}
		"#;
		let register2 = r#"
			mutation {
				register(input: {
					username: "user2",
					email: "user2@example.com",
					password: "password123"
				}) { token }
			}
		"#;
		schema.execute(Request::new(register1)).await;
		schema.execute(Request::new(register2)).await;

		// Query all users
		let query = r#"
			query {
				users {
					id
					username
					email
				}
			}
		"#;

		let response = schema.execute(Request::new(query)).await;
		let data = response.data.into_json().unwrap();

		let users = data["users"].as_array().unwrap();
		assert_eq!(users.len(), 2);
	}

	#[tokio::test]
	async fn test_user_query_by_id() {
		let schema = get_schema();

		// Register a user and get their ID
		let register_query = r#"
			mutation {
				register(input: {
					username: "findme",
					email: "findme@example.com",
					password: "password123"
				}) {
					user {
						id
					}
				}
			}
		"#;

		let response = schema.execute(Request::new(register_query)).await;
		let data = response.data.into_json().unwrap();
		let user_id = data["register"]["user"]["id"].as_str().unwrap();

		// Query user by ID
		let query = format!(
			r#"
			query {{
				user(id: "{}") {{
					id
					username
					email
				}}
			}}
		"#,
			user_id
		);

		let response = schema.execute(Request::new(query)).await;
		let data = response.data.into_json().unwrap();

		assert_eq!(data["user"]["username"], "findme");
		assert_eq!(data["user"]["email"], "findme@example.com");
	}
}
