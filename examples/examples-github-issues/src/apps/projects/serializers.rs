//! GraphQL types and input objects for projects

use async_graphql::{Context, Enum, ID, InputObject, Object, Result as GqlResult};
use chrono::{DateTime, Utc};

use crate::apps::projects::models::{Project, ProjectMember};

/// Project visibility enum for GraphQL
#[derive(Enum, Copy, Clone, Eq, PartialEq)]
pub enum ProjectVisibilityEnum {
	/// Public project (visible to everyone)
	Public,
	/// Private project (visible only to members)
	Private,
}

impl From<&str> for ProjectVisibilityEnum {
	fn from(s: &str) -> Self {
		match s {
			"private" => ProjectVisibilityEnum::Private,
			_ => ProjectVisibilityEnum::Public,
		}
	}
}

impl From<ProjectVisibilityEnum> for String {
	fn from(v: ProjectVisibilityEnum) -> Self {
		match v {
			ProjectVisibilityEnum::Public => "public".to_string(),
			ProjectVisibilityEnum::Private => "private".to_string(),
		}
	}
}

/// Member role enum for GraphQL
#[derive(Enum, Copy, Clone, Eq, PartialEq)]
pub enum MemberRoleEnum {
	/// Project owner (full permissions)
	Owner,
	/// Project maintainer (can manage issues and members)
	Maintainer,
	/// Project member (can create and edit issues)
	Member,
	/// Project viewer (read-only access)
	Viewer,
}

impl From<&str> for MemberRoleEnum {
	fn from(s: &str) -> Self {
		match s {
			"owner" => MemberRoleEnum::Owner,
			"maintainer" => MemberRoleEnum::Maintainer,
			"viewer" => MemberRoleEnum::Viewer,
			_ => MemberRoleEnum::Member,
		}
	}
}

impl From<MemberRoleEnum> for String {
	fn from(r: MemberRoleEnum) -> Self {
		match r {
			MemberRoleEnum::Owner => "owner".to_string(),
			MemberRoleEnum::Maintainer => "maintainer".to_string(),
			MemberRoleEnum::Member => "member".to_string(),
			MemberRoleEnum::Viewer => "viewer".to_string(),
		}
	}
}

/// GraphQL representation of Project
#[derive(Clone)]
pub struct ProjectType(pub Project);

#[Object]
impl ProjectType {
	/// Project ID
	async fn id(&self) -> ID {
		ID(self.0.id().to_string())
	}

	/// Project name
	async fn name(&self) -> &str {
		self.0.name()
	}

	/// Project description
	async fn description(&self) -> &str {
		self.0.description()
	}

	/// Project visibility (PUBLIC or PRIVATE)
	async fn visibility(&self) -> ProjectVisibilityEnum {
		ProjectVisibilityEnum::from(self.0.visibility().as_str())
	}

	/// Owner user ID
	async fn owner_id(&self) -> ID {
		ID(self.0.owner_id().to_string())
	}

	/// Creation timestamp
	async fn created_at(&self) -> DateTime<Utc> {
		self.0.created_at()
	}

	/// Resolve project owner
	async fn owner(
		&self,
		ctx: &Context<'_>,
	) -> GqlResult<Option<crate::apps::auth::serializers::UserType>> {
		use crate::apps::auth::views::UserStorage;
		let storage = ctx.data::<UserStorage>()?;
		let user = storage.get_user(&self.0.owner_id().to_string()).await;
		Ok(user.map(crate::apps::auth::serializers::UserType))
	}

	/// Resolve project members
	async fn members(&self, ctx: &Context<'_>) -> GqlResult<Vec<ProjectMemberType>> {
		use crate::apps::projects::views::ProjectMemberStorage;
		let storage = ctx.data::<ProjectMemberStorage>()?;
		let members = storage
			.get_members_by_project(&self.0.id().to_string())
			.await;
		Ok(members.into_iter().map(ProjectMemberType).collect())
	}

	/// Resolve project issues
	async fn issues(
		&self,
		ctx: &Context<'_>,
	) -> GqlResult<Vec<crate::apps::issues::serializers::IssueType>> {
		use crate::apps::issues::views::IssueStorage;
		let storage = ctx.data::<IssueStorage>()?;
		let issues = storage
			.get_issues_by_project(&self.0.id().to_string())
			.await;
		Ok(issues
			.into_iter()
			.map(crate::apps::issues::serializers::IssueType)
			.collect())
	}
}

/// GraphQL representation of ProjectMember
#[derive(Clone)]
pub struct ProjectMemberType(pub ProjectMember);

#[Object]
impl ProjectMemberType {
	/// Member ID
	async fn id(&self) -> ID {
		ID(self.0.id().to_string())
	}

	/// Project ID
	async fn project_id(&self) -> ID {
		ID(self.0.project_id().to_string())
	}

	/// User ID
	async fn user_id(&self) -> ID {
		ID(self.0.user_id().to_string())
	}

	/// Member role in the project
	async fn role(&self) -> MemberRoleEnum {
		MemberRoleEnum::from(self.0.role().as_str())
	}

	/// Join timestamp
	async fn joined_at(&self) -> DateTime<Utc> {
		self.0.joined_at()
	}

	/// Resolve user
	async fn user(
		&self,
		ctx: &Context<'_>,
	) -> GqlResult<Option<crate::apps::auth::serializers::UserType>> {
		use crate::apps::auth::views::UserStorage;
		let storage = ctx.data::<UserStorage>()?;
		let user = storage.get_user(&self.0.user_id().to_string()).await;
		Ok(user.map(crate::apps::auth::serializers::UserType))
	}

	/// Resolve project
	async fn project(&self, ctx: &Context<'_>) -> GqlResult<Option<ProjectType>> {
		use crate::apps::projects::views::ProjectStorage;
		let storage = ctx.data::<ProjectStorage>()?;
		let project = storage.get_project(&self.0.project_id().to_string()).await;
		Ok(project.map(ProjectType))
	}
}

/// Input for creating a project
#[derive(InputObject)]
pub struct CreateProjectInput {
	/// Project name (must be unique)
	pub name: String,
	/// Project description
	pub description: String,
	/// Project visibility (defaults to PUBLIC)
	pub visibility: Option<ProjectVisibilityEnum>,
}

/// Input for adding a member to a project
#[derive(InputObject)]
pub struct AddMemberInput {
	/// Project ID
	pub project_id: ID,
	/// User ID to add
	pub user_id: ID,
	/// Member role (defaults to MEMBER)
	pub role: Option<MemberRoleEnum>,
}
