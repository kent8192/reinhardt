//! GraphQL resolvers for projects
//!
//! This module contains Query and Mutation resolvers for project operations.

use async_graphql::{Context, ID, Object, Result as GqlResult};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::apps::projects::models::{Project, ProjectMember};
use crate::apps::projects::serializers::{
	AddMemberInput, CreateProjectInput, ProjectMemberType, ProjectType, ProjectVisibilityEnum,
};

/// In-memory project storage
#[derive(Clone, Default)]
pub struct ProjectStorage {
	projects: Arc<RwLock<HashMap<String, Project>>>,
}

impl ProjectStorage {
	/// Create a new empty project storage
	pub fn new() -> Self {
		Self {
			projects: Arc::new(RwLock::new(HashMap::new())),
		}
	}

	/// Add or update a project
	pub async fn add_project(&self, project: Project) {
		self.projects
			.write()
			.await
			.insert(project.id().to_string(), project);
	}

	/// Get a project by ID
	pub async fn get_project(&self, id: &str) -> Option<Project> {
		self.projects.read().await.get(id).cloned()
	}

	/// List projects, optionally filtered by visibility
	pub async fn list_projects(&self, visibility: Option<ProjectVisibilityEnum>) -> Vec<Project> {
		self.projects
			.read()
			.await
			.values()
			.filter(|p| visibility.is_none_or(|v| p.visibility() == String::from(v).as_str()))
			.cloned()
			.collect()
	}
}

/// In-memory project member storage
#[derive(Clone, Default)]
pub struct ProjectMemberStorage {
	members: Arc<RwLock<HashMap<String, ProjectMember>>>,
}

impl ProjectMemberStorage {
	/// Create a new empty project member storage
	pub fn new() -> Self {
		Self {
			members: Arc::new(RwLock::new(HashMap::new())),
		}
	}

	/// Add or update a member
	pub async fn add_member(&self, member: ProjectMember) {
		self.members
			.write()
			.await
			.insert(member.id().to_string(), member);
	}

	/// Get members by project ID
	pub async fn get_members_by_project(&self, project_id: &str) -> Vec<ProjectMember> {
		self.members
			.read()
			.await
			.values()
			.filter(|m| m.project_id().to_string() == project_id)
			.cloned()
			.collect()
	}

	/// Check if a user is a member of a project
	pub async fn is_member(&self, project_id: &str, user_id: &str) -> bool {
		self.members
			.read()
			.await
			.values()
			.any(|m| m.project_id().to_string() == project_id && m.user_id().to_string() == user_id)
	}

	/// Remove a member from a project
	pub async fn remove_member(&self, project_id: &str, user_id: &str) -> bool {
		let mut members = self.members.write().await;
		let key = members
			.iter()
			.find(|(_, m)| {
				m.project_id().to_string() == project_id && m.user_id().to_string() == user_id
			})
			.map(|(k, _)| k.clone());
		if let Some(k) = key {
			members.remove(&k);
			true
		} else {
			false
		}
	}
}

/// Project Query resolvers
#[derive(Default)]
pub struct ProjectQuery;

#[Object]
impl ProjectQuery {
	/// Get a project by ID
	async fn project(&self, ctx: &Context<'_>, id: ID) -> GqlResult<Option<ProjectType>> {
		let storage = ctx.data::<ProjectStorage>()?;
		let project = storage.get_project(id.as_str()).await;
		Ok(project.map(ProjectType))
	}

	/// List projects, optionally filtered by visibility
	async fn projects(
		&self,
		ctx: &Context<'_>,
		visibility: Option<ProjectVisibilityEnum>,
	) -> GqlResult<Vec<ProjectType>> {
		let storage = ctx.data::<ProjectStorage>()?;
		let projects = storage.list_projects(visibility).await;
		Ok(projects.into_iter().map(ProjectType).collect())
	}
}

/// Project Mutation resolvers
#[derive(Default)]
pub struct ProjectMutation;

#[Object]
impl ProjectMutation {
	/// Create a new project
	async fn create_project(
		&self,
		ctx: &Context<'_>,
		input: CreateProjectInput,
	) -> GqlResult<ProjectType> {
		use reinhardt::Claims;
		let claims = ctx
			.data::<Claims>()
			.map_err(|_| async_graphql::Error::new("Authentication required"))?;
		let project_storage = ctx.data::<ProjectStorage>()?;
		let member_storage = ctx.data::<ProjectMemberStorage>()?;

		let owner_id = Uuid::parse_str(&claims.sub)
			.map_err(|_| async_graphql::Error::new("Invalid user ID"))?;

		let project = Project::new(
			input.name,
			input.description,
			input
				.visibility
				.map(String::from)
				.unwrap_or_else(|| "public".to_string()),
			owner_id,
		);

		project_storage.add_project(project.clone()).await;

		// Add owner as a member with Owner role
		let member = ProjectMember::new(project.id(), owner_id, "owner".to_string());
		member_storage.add_member(member).await;

		Ok(ProjectType(project))
	}

	/// Add a member to a project
	async fn add_member(
		&self,
		ctx: &Context<'_>,
		input: AddMemberInput,
	) -> GqlResult<ProjectMemberType> {
		let project_storage = ctx.data::<ProjectStorage>()?;
		let member_storage = ctx.data::<ProjectMemberStorage>()?;

		let project_id = Uuid::parse_str(input.project_id.as_str())
			.map_err(|_| async_graphql::Error::new("Invalid project ID"))?;
		let user_id = Uuid::parse_str(input.user_id.as_str())
			.map_err(|_| async_graphql::Error::new("Invalid user ID"))?;

		// Check if project exists
		if project_storage
			.get_project(&project_id.to_string())
			.await
			.is_none()
		{
			return Err(async_graphql::Error::new("Project not found"));
		}

		// Check if already a member
		if member_storage
			.is_member(&project_id.to_string(), &user_id.to_string())
			.await
		{
			return Err(async_graphql::Error::new("User is already a member"));
		}

		let member = ProjectMember::new(
			project_id,
			user_id,
			input
				.role
				.map(String::from)
				.unwrap_or_else(|| "member".to_string()),
		);

		member_storage.add_member(member.clone()).await;

		Ok(ProjectMemberType(member))
	}

	/// Remove a member from a project
	async fn remove_member(
		&self,
		ctx: &Context<'_>,
		project_id: ID,
		user_id: ID,
	) -> GqlResult<bool> {
		let member_storage = ctx.data::<ProjectMemberStorage>()?;
		let removed = member_storage
			.remove_member(project_id.as_str(), user_id.as_str())
			.await;
		Ok(removed)
	}
}
