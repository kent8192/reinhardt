//! Tests for projects app
//!
//! Tests for project creation, member management, and queries.

#[cfg(test)]
mod tests {
	use crate::config::schema::get_schema;
	use crate::config::urls::AppSchema;
	use async_graphql::Request;
	use reinhardt::Claims;
	use std::sync::Arc;

	/// Helper to get authenticated claims
	async fn get_auth_claims(schema: &Arc<AppSchema>) -> (Claims, String) {
		let register_query = r#"
			mutation {
				register(input: {
					username: "projectowner",
					email: "owner@example.com",
					password: "password123"
				}) {
					token
					user { id }
				}
			}
		"#;
		let response = schema.execute(Request::new(register_query)).await;
		let data = response.data.into_json().unwrap();
		let _token = data["register"]["token"].as_str().unwrap().to_string();
		let user_id = data["register"]["user"]["id"].as_str().unwrap().to_string();

		let now = chrono::Utc::now().timestamp();
		let claims = Claims {
			sub: user_id.clone(),
			exp: now + 86400, // 24 hours
			iat: now,
			username: "projectowner".to_string(),
		};

		(claims, user_id)
	}

	#[tokio::test]
	async fn test_create_project() {
		let schema = get_schema();
		let (claims, _) = get_auth_claims(&schema).await;

		let query = r#"
			mutation {
				createProject(input: {
					name: "My Project",
					description: "A great project"
				}) {
					id
					name
					description
					visibility
					ownerId
				}
			}
		"#;

		let response = schema.execute(Request::new(query).data(claims)).await;
		let data = response.data.into_json().unwrap();

		assert!(data["createProject"]["id"].is_string());
		assert_eq!(data["createProject"]["name"], "My Project");
		assert_eq!(data["createProject"]["description"], "A great project");
		assert_eq!(data["createProject"]["visibility"], "PUBLIC");
	}

	#[tokio::test]
	async fn test_create_private_project() {
		let schema = get_schema();
		let (claims, _) = get_auth_claims(&schema).await;

		let query = r#"
			mutation {
				createProject(input: {
					name: "Private Project",
					description: "A secret project",
					visibility: PRIVATE
				}) {
					id
					name
					visibility
				}
			}
		"#;

		let response = schema.execute(Request::new(query).data(claims)).await;
		let data = response.data.into_json().unwrap();

		assert_eq!(data["createProject"]["name"], "Private Project");
		assert_eq!(data["createProject"]["visibility"], "PRIVATE");
	}

	#[tokio::test]
	async fn test_projects_query() {
		let schema = get_schema();
		let (claims, _) = get_auth_claims(&schema).await;

		// Create multiple projects
		for i in 1..=3 {
			let query = format!(
				r#"
				mutation {{
					createProject(input: {{
						name: "Project {}",
						description: "Description {}"
					}}) {{
						id
					}}
				}}
			"#,
				i, i
			);
			schema
				.execute(Request::new(query).data(claims.clone()))
				.await;
		}

		// Query all projects
		let query = r#"
			query {
				projects {
					id
					name
					description
				}
			}
		"#;

		let response = schema.execute(Request::new(query)).await;
		let data = response.data.into_json().unwrap();

		let projects = data["projects"].as_array().unwrap();
		assert_eq!(projects.len(), 3);
	}

	#[tokio::test]
	async fn test_projects_query_with_visibility_filter() {
		let schema = get_schema();
		let (claims, _) = get_auth_claims(&schema).await;

		// Create public and private projects
		let public_query = r#"
			mutation {
				createProject(input: {
					name: "Public Project",
					description: "Visible to all",
					visibility: PUBLIC
				}) { id }
			}
		"#;
		let private_query = r#"
			mutation {
				createProject(input: {
					name: "Private Project",
					description: "Hidden",
					visibility: PRIVATE
				}) { id }
			}
		"#;
		schema
			.execute(Request::new(public_query).data(claims.clone()))
			.await;
		schema
			.execute(Request::new(private_query).data(claims))
			.await;

		// Query only public projects
		let query = r#"
			query {
				projects(visibility: PUBLIC) {
					id
					name
					visibility
				}
			}
		"#;

		let response = schema.execute(Request::new(query)).await;
		let data = response.data.into_json().unwrap();

		let projects = data["projects"].as_array().unwrap();
		assert_eq!(projects.len(), 1);
		assert_eq!(projects[0]["visibility"], "PUBLIC");
	}

	#[tokio::test]
	async fn test_project_query_by_id() {
		let schema = get_schema();
		let (claims, _) = get_auth_claims(&schema).await;

		// Create a project
		let create_query = r#"
			mutation {
				createProject(input: {
					name: "Findable Project",
					description: "Can be found by ID"
				}) {
					id
				}
			}
		"#;

		let response = schema
			.execute(Request::new(create_query).data(claims))
			.await;
		let data = response.data.into_json().unwrap();
		let project_id = data["createProject"]["id"].as_str().unwrap();

		// Query project by ID
		let query = format!(
			r#"
			query {{
				project(id: "{}") {{
					id
					name
					description
				}}
			}}
		"#,
			project_id
		);

		let response = schema.execute(Request::new(query)).await;
		let data = response.data.into_json().unwrap();

		assert_eq!(data["project"]["name"], "Findable Project");
		assert_eq!(data["project"]["description"], "Can be found by ID");
	}

	#[tokio::test]
	async fn test_add_member_to_project() {
		let schema = get_schema();
		let (owner_claims, _) = get_auth_claims(&schema).await;

		// Register another user to add as member
		let register_member = r#"
			mutation {
				register(input: {
					username: "newmember",
					email: "member@example.com",
					password: "password123"
				}) {
					user { id }
				}
			}
		"#;
		let response = schema.execute(Request::new(register_member)).await;
		let data = response.data.into_json().unwrap();
		let member_id = data["register"]["user"]["id"].as_str().unwrap();

		// Create a project
		let create_query = r#"
			mutation {
				createProject(input: {
					name: "Team Project",
					description: "A project with members"
				}) {
					id
				}
			}
		"#;
		let response = schema
			.execute(Request::new(create_query).data(owner_claims))
			.await;
		let data = response.data.into_json().unwrap();
		let project_id = data["createProject"]["id"].as_str().unwrap();

		// Add member to project
		let add_member_query = format!(
			r#"
			mutation {{
				addMember(input: {{
					projectId: "{}",
					userId: "{}",
					role: MAINTAINER
				}}) {{
					id
					projectId
					userId
					role
				}}
			}}
		"#,
			project_id, member_id
		);

		let response = schema.execute(Request::new(add_member_query)).await;
		let data = response.data.into_json().unwrap();

		assert!(data["addMember"]["id"].is_string());
		assert_eq!(data["addMember"]["projectId"], project_id);
		assert_eq!(data["addMember"]["userId"], member_id);
		assert_eq!(data["addMember"]["role"], "MAINTAINER");
	}

	#[tokio::test]
	async fn test_remove_member_from_project() {
		let schema = get_schema();
		let (owner_claims, _) = get_auth_claims(&schema).await;

		// Register a member
		let register_member = r#"
			mutation {
				register(input: {
					username: "removablemember",
					email: "removable@example.com",
					password: "password123"
				}) {
					user { id }
				}
			}
		"#;
		let response = schema.execute(Request::new(register_member)).await;
		let data = response.data.into_json().unwrap();
		let member_id = data["register"]["user"]["id"].as_str().unwrap();

		// Create a project
		let create_query = r#"
			mutation {
				createProject(input: {
					name: "Team Project",
					description: "A project with members"
				}) {
					id
				}
			}
		"#;
		let response = schema
			.execute(Request::new(create_query).data(owner_claims))
			.await;
		let data = response.data.into_json().unwrap();
		let project_id = data["createProject"]["id"].as_str().unwrap();

		// Add member
		let add_member_query = format!(
			r#"
			mutation {{
				addMember(input: {{
					projectId: "{}",
					userId: "{}"
				}}) {{
					id
				}}
			}}
		"#,
			project_id, member_id
		);
		schema.execute(Request::new(add_member_query)).await;

		// Remove member
		let remove_member_query = format!(
			r#"
			mutation {{
				removeMember(projectId: "{}", userId: "{}")
			}}
		"#,
			project_id, member_id
		);

		let response = schema.execute(Request::new(remove_member_query)).await;
		let data = response.data.into_json().unwrap();

		assert_eq!(data["removeMember"], true);
	}

	#[tokio::test]
	async fn test_project_with_members_relation() {
		let schema = get_schema();
		let (owner_claims, owner_id) = get_auth_claims(&schema).await;

		// Create a project (owner is automatically added as member)
		let create_query = r#"
			mutation {
				createProject(input: {
					name: "Project with Members",
					description: "Testing member relations"
				}) {
					id
					members {
						id
						role
						userId
					}
				}
			}
		"#;

		let response = schema
			.execute(Request::new(create_query).data(owner_claims))
			.await;
		let data = response.data.into_json().unwrap();

		let members = data["createProject"]["members"].as_array().unwrap();
		// Owner should be added as a member with OWNER role
		assert_eq!(members.len(), 1);
		assert_eq!(members[0]["role"], "OWNER");
		assert_eq!(members[0]["userId"], owner_id);
	}
}
