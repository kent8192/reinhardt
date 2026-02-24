//! Tests for issues app
//!
//! Tests for issue creation, updates, and queries.

#[cfg(test)]
mod tests {
	use crate::config::schema::get_schema;
	use crate::config::urls::AppSchema;
	use async_graphql::Request;
	use reinhardt::Claims;
	use std::sync::Arc;

	/// Helper to create a schema with an authenticated user
	async fn create_test_context() -> (Arc<AppSchema>, String, String) {
		let schema = get_schema();

		// Register a user
		let register_query = r#"
			mutation {
				register(input: {
					username: "issueauthor",
					email: "author@example.com",
					password: "password123"
				}) {
					token
					user { id }
				}
			}
		"#;
		let response = schema.execute(Request::new(register_query)).await;
		let data = response.data.into_json().unwrap();
		let token = data["register"]["token"].as_str().unwrap().to_string();
		let user_id = data["register"]["user"]["id"].as_str().unwrap().to_string();

		// Create a project
		let now = chrono::Utc::now().timestamp();
		let claims = Claims {
			sub: user_id.clone(),
			exp: now + 86400, // 24 hours
			iat: now,
			username: "issueauthor".to_string(),
		};

		let create_project = r#"
			mutation {
				createProject(input: {
					name: "Test Project",
					description: "A test project for issues"
				}) {
					id
				}
			}
		"#;
		let response = schema
			.execute(Request::new(create_project).data(claims.clone()))
			.await;
		let data = response.data.into_json().unwrap();
		let project_id = data["createProject"]["id"].as_str().unwrap().to_string();

		(schema, token, project_id)
	}

	#[tokio::test]
	async fn test_create_issue() {
		let (schema, token, project_id) = create_test_context().await;

		// Parse claims from token (in real app, this would be done by middleware)
		let jwt_auth = reinhardt::JwtAuth::new(b"your-secret-key-change-in-production");
		let claims = jwt_auth.verify_token(&token).unwrap();

		let query = format!(
			r#"
			mutation {{
				createIssue(input: {{
					projectId: "{}",
					title: "Test Issue",
					body: "This is a test issue body"
				}}) {{
					id
					number
					title
					body
					state
					projectId
				}}
			}}
		"#,
			project_id
		);

		let response = schema.execute(Request::new(query).data(claims)).await;
		let data = response.data.into_json().unwrap();

		assert!(data["createIssue"]["id"].is_string());
		assert_eq!(data["createIssue"]["number"], 1);
		assert_eq!(data["createIssue"]["title"], "Test Issue");
		assert_eq!(data["createIssue"]["body"], "This is a test issue body");
		assert_eq!(data["createIssue"]["state"], "OPEN");
		assert_eq!(data["createIssue"]["projectId"], project_id);
	}

	#[tokio::test]
	async fn test_update_issue() {
		let (schema, token, project_id) = create_test_context().await;
		let jwt_auth = reinhardt::JwtAuth::new(b"your-secret-key-change-in-production");
		let claims = jwt_auth.verify_token(&token).unwrap();

		// Create an issue first
		let create_query = format!(
			r#"
			mutation {{
				createIssue(input: {{
					projectId: "{}",
					title: "Original Title",
					body: "Original body"
				}}) {{
					id
				}}
			}}
		"#,
			project_id
		);

		let response = schema
			.execute(Request::new(create_query).data(claims.clone()))
			.await;
		let data = response.data.into_json().unwrap();
		let issue_id = data["createIssue"]["id"].as_str().unwrap();

		// Update the issue
		let update_query = format!(
			r#"
			mutation {{
				updateIssue(id: "{}", input: {{
					title: "Updated Title",
					body: "Updated body content"
				}}) {{
					id
					title
					body
				}}
			}}
		"#,
			issue_id
		);

		let response = schema.execute(Request::new(update_query)).await;
		let data = response.data.into_json().unwrap();

		assert_eq!(data["updateIssue"]["title"], "Updated Title");
		assert_eq!(data["updateIssue"]["body"], "Updated body content");
	}

	#[tokio::test]
	async fn test_close_and_reopen_issue() {
		let (schema, token, project_id) = create_test_context().await;
		let jwt_auth = reinhardt::JwtAuth::new(b"your-secret-key-change-in-production");
		let claims = jwt_auth.verify_token(&token).unwrap();

		// Create an issue
		let create_query = format!(
			r#"
			mutation {{
				createIssue(input: {{
					projectId: "{}",
					title: "Issue to Close",
					body: "This issue will be closed"
				}}) {{
					id
					state
				}}
			}}
		"#,
			project_id
		);

		let response = schema
			.execute(Request::new(create_query).data(claims))
			.await;
		let data = response.data.into_json().unwrap();
		let issue_id = data["createIssue"]["id"].as_str().unwrap();
		assert_eq!(data["createIssue"]["state"], "OPEN");

		// Close the issue
		let close_query = format!(
			r#"
			mutation {{
				closeIssue(id: "{}") {{
					id
					state
				}}
			}}
		"#,
			issue_id
		);

		let response = schema.execute(Request::new(close_query)).await;
		let data = response.data.into_json().unwrap();
		assert_eq!(data["closeIssue"]["state"], "CLOSED");

		// Reopen the issue
		let reopen_query = format!(
			r#"
			mutation {{
				reopenIssue(id: "{}") {{
					id
					state
				}}
			}}
		"#,
			issue_id
		);

		let response = schema.execute(Request::new(reopen_query)).await;
		let data = response.data.into_json().unwrap();
		assert_eq!(data["reopenIssue"]["state"], "OPEN");
	}

	#[tokio::test]
	async fn test_issues_query() {
		let (schema, token, project_id) = create_test_context().await;
		let jwt_auth = reinhardt::JwtAuth::new(b"your-secret-key-change-in-production");
		let claims = jwt_auth.verify_token(&token).unwrap();

		// Create multiple issues
		for i in 1..=3 {
			let create_query = format!(
				r#"
				mutation {{
					createIssue(input: {{
						projectId: "{}",
						title: "Issue {}",
						body: "Body for issue {}"
					}}) {{
						id
					}}
				}}
			"#,
				project_id, i, i
			);
			schema
				.execute(Request::new(create_query).data(claims.clone()))
				.await;
		}

		// Query all issues with pagination
		let query = r#"
			query {
				issues {
					edges {
						id
						number
						title
					}
					pageInfo {
						hasNextPage
						hasPreviousPage
						totalCount
						page
						pageSize
					}
				}
			}
		"#;

		let response = schema.execute(Request::new(query)).await;
		let data = response.data.into_json().unwrap();

		let edges = data["issues"]["edges"].as_array().unwrap();
		assert_eq!(edges.len(), 3);

		let page_info = &data["issues"]["pageInfo"];
		assert_eq!(page_info["totalCount"], 3);
		assert_eq!(page_info["page"], 1);
		assert_eq!(page_info["hasNextPage"], false);
		assert_eq!(page_info["hasPreviousPage"], false);
	}

	#[tokio::test]
	async fn test_issues_query_by_project() {
		let (schema, token, project_id) = create_test_context().await;
		let jwt_auth = reinhardt::JwtAuth::new(b"your-secret-key-change-in-production");
		let claims = jwt_auth.verify_token(&token).unwrap();

		// Create an issue
		let create_query = format!(
			r#"
			mutation {{
				createIssue(input: {{
					projectId: "{}",
					title: "Project Issue",
					body: "This is for the project"
				}}) {{
					id
				}}
			}}
		"#,
			project_id
		);
		schema
			.execute(Request::new(create_query).data(claims))
			.await;

		// Query issues by project with pagination
		let query = format!(
			r#"
			query {{
				issues(projectId: "{}") {{
					edges {{
						id
						title
						projectId
					}}
					pageInfo {{
						totalCount
						page
					}}
				}}
			}}
		"#,
			project_id
		);

		let response = schema.execute(Request::new(query)).await;
		let data = response.data.into_json().unwrap();

		let edges = data["issues"]["edges"].as_array().unwrap();
		assert_eq!(edges.len(), 1);
		assert_eq!(edges[0]["projectId"], project_id);
		assert_eq!(data["issues"]["pageInfo"]["totalCount"], 1);
	}

	#[tokio::test]
	async fn test_issue_query_by_id() {
		let (schema, token, project_id) = create_test_context().await;
		let jwt_auth = reinhardt::JwtAuth::new(b"your-secret-key-change-in-production");
		let claims = jwt_auth.verify_token(&token).unwrap();

		// Create an issue
		let create_query = format!(
			r#"
			mutation {{
				createIssue(input: {{
					projectId: "{}",
					title: "Findable Issue",
					body: "This issue can be found by ID"
				}}) {{
					id
				}}
			}}
		"#,
			project_id
		);

		let response = schema
			.execute(Request::new(create_query).data(claims))
			.await;
		let data = response.data.into_json().unwrap();
		let issue_id = data["createIssue"]["id"].as_str().unwrap();

		// Query issue by ID
		let query = format!(
			r#"
			query {{
				issue(id: "{}") {{
					id
					title
					body
				}}
			}}
		"#,
			issue_id
		);

		let response = schema.execute(Request::new(query)).await;
		let data = response.data.into_json().unwrap();

		assert_eq!(data["issue"]["title"], "Findable Issue");
		assert_eq!(data["issue"]["body"], "This issue can be found by ID");
	}
}
