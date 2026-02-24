//! GraphQL types and input objects for issues

use async_graphql::{Context, Enum, ID, InputObject, Object, Result as GqlResult, SimpleObject};
use chrono::{DateTime, Utc};
use validator::Validate;

use crate::apps::issues::models::Issue;

/// Issue state enum for GraphQL
#[derive(Enum, Copy, Clone, Eq, PartialEq)]
pub enum IssueStateEnum {
	/// Issue is open
	Open,
	/// Issue is closed
	Closed,
}

impl From<&str> for IssueStateEnum {
	fn from(s: &str) -> Self {
		match s {
			"closed" => IssueStateEnum::Closed,
			_ => IssueStateEnum::Open,
		}
	}
}

impl From<IssueStateEnum> for String {
	fn from(state: IssueStateEnum) -> Self {
		match state {
			IssueStateEnum::Open => "open".to_string(),
			IssueStateEnum::Closed => "closed".to_string(),
		}
	}
}

/// GraphQL representation of Issue
#[derive(Clone)]
pub struct IssueType(pub Issue);

#[Object]
impl IssueType {
	/// Issue ID
	async fn id(&self) -> ID {
		ID(self.0.id().to_string())
	}

	/// Project ID this issue belongs to
	async fn project_id(&self) -> ID {
		ID(self.0.project_id().to_string())
	}

	/// Project-scoped sequential number (e.g., #1, #2, #3)
	async fn number(&self) -> i32 {
		self.0.number()
	}

	/// Issue title
	async fn title(&self) -> &str {
		self.0.title()
	}

	/// Issue body (supports Markdown)
	async fn body(&self) -> &str {
		self.0.body()
	}

	/// Issue state (OPEN or CLOSED)
	async fn state(&self) -> IssueStateEnum {
		IssueStateEnum::from(self.0.state().as_str())
	}

	/// Author user ID
	async fn author_id(&self) -> ID {
		ID(self.0.author_id().to_string())
	}

	/// Creation timestamp
	async fn created_at(&self) -> DateTime<Utc> {
		self.0.created_at()
	}

	/// Last update timestamp
	async fn updated_at(&self) -> DateTime<Utc> {
		self.0.updated_at()
	}

	/// Resolve related project
	async fn project(
		&self,
		ctx: &Context<'_>,
	) -> GqlResult<Option<crate::apps::projects::serializers::ProjectType>> {
		use crate::apps::projects::views::ProjectStorage;
		let storage = ctx.data::<ProjectStorage>()?;
		let project = storage.get_project(&self.0.project_id().to_string()).await;
		Ok(project.map(crate::apps::projects::serializers::ProjectType))
	}

	/// Resolve author
	async fn author(
		&self,
		ctx: &Context<'_>,
	) -> GqlResult<Option<crate::apps::auth::serializers::UserType>> {
		use crate::apps::auth::views::UserStorage;
		let storage = ctx.data::<UserStorage>()?;
		let user = storage.get_user(&self.0.author_id().to_string()).await;
		Ok(user.map(crate::apps::auth::serializers::UserType))
	}
}

/// Input for creating an issue
#[derive(InputObject, Validate)]
pub struct CreateIssueInput {
	/// Project ID to create the issue in
	pub project_id: ID,
	/// Issue title (1-200 characters)
	#[validate(length(min = 1, max = 200))]
	pub title: String,
	/// Issue body (supports Markdown, max 10000 characters)
	#[validate(length(max = 10000))]
	pub body: String,
}

/// Input for updating an issue
#[derive(InputObject, Validate)]
pub struct UpdateIssueInput {
	/// New title (optional, 1-200 characters if provided)
	#[validate(length(min = 1, max = 200))]
	pub title: Option<String>,
	/// New body (optional, max 10000 characters if provided)
	#[validate(length(max = 10000))]
	pub body: Option<String>,
}

/// Pagination input for list queries
#[derive(InputObject)]
pub struct PaginationInput {
	/// Page number (1-based, defaults to 1)
	pub page: Option<i32>,
	/// Number of items per page (defaults to 10)
	pub page_size: Option<i32>,
}

impl Default for PaginationInput {
	fn default() -> Self {
		Self {
			page: Some(1),
			page_size: Some(10),
		}
	}
}

/// Pagination metadata for list responses
#[derive(SimpleObject)]
pub struct PageInfo {
	/// Whether there is a next page
	pub has_next_page: bool,
	/// Whether there is a previous page
	pub has_previous_page: bool,
	/// Total number of items across all pages
	pub total_count: i32,
	/// Current page number (1-based)
	pub page: i32,
	/// Number of items per page
	pub page_size: i32,
}

/// Paginated issue connection
#[derive(SimpleObject)]
pub struct IssueConnection {
	/// List of issues on the current page
	pub edges: Vec<IssueType>,
	/// Pagination metadata
	pub page_info: PageInfo,
}
