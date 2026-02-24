//! GraphQL resolvers for issues
//!
//! This module contains Query, Mutation, and Subscription resolvers for issue operations.

use async_graphql::{Context, ErrorExtensions, ID, Object, Result as GqlResult, Subscription};
use futures_util::Stream;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{RwLock, broadcast};
use uuid::Uuid;
use validator::Validate;

use crate::apps::issues::errors::ApiError;
use crate::apps::issues::models::Issue;
use crate::apps::issues::serializers::{
	CreateIssueInput, IssueConnection, IssueType, PageInfo, PaginationInput, UpdateIssueInput,
};

/// Issue event types for subscriptions
#[derive(Debug, Clone)]
pub enum IssueEvent {
	/// New issue created
	Created(Issue),
	/// Existing issue updated
	Updated(Issue),
	/// Issue was closed
	Closed(Issue),
}

/// Event broadcaster for issue subscriptions
#[derive(Clone)]
pub struct IssueEventBroadcaster {
	tx: Arc<RwLock<broadcast::Sender<IssueEvent>>>,
}

impl IssueEventBroadcaster {
	/// Create a new event broadcaster
	pub fn new() -> Self {
		let (tx, _) = broadcast::channel(100);
		Self {
			tx: Arc::new(RwLock::new(tx)),
		}
	}

	/// Broadcast an event to all subscribers
	pub async fn broadcast(&self, event: IssueEvent) {
		let tx = self.tx.read().await;
		let _ = tx.send(event);
	}

	/// Subscribe to events
	pub async fn subscribe(&self) -> broadcast::Receiver<IssueEvent> {
		self.tx.read().await.subscribe()
	}
}

impl Default for IssueEventBroadcaster {
	fn default() -> Self {
		Self::new()
	}
}

/// In-memory issue storage
#[derive(Clone, Default)]
pub struct IssueStorage {
	issues: Arc<RwLock<HashMap<String, Issue>>>,
	next_number: Arc<RwLock<HashMap<String, i32>>>,
}

impl IssueStorage {
	/// Create a new empty issue storage
	pub fn new() -> Self {
		Self {
			issues: Arc::new(RwLock::new(HashMap::new())),
			next_number: Arc::new(RwLock::new(HashMap::new())),
		}
	}

	/// Add or update an issue
	pub async fn add_issue(&self, issue: Issue) {
		self.issues
			.write()
			.await
			.insert(issue.id().to_string(), issue);
	}

	/// Get an issue by ID
	pub async fn get_issue(&self, id: &str) -> Option<Issue> {
		self.issues.read().await.get(id).cloned()
	}

	/// Get issues by project ID
	pub async fn get_issues_by_project(&self, project_id: &str) -> Vec<Issue> {
		self.issues
			.read()
			.await
			.values()
			.filter(|i| i.project_id().to_string() == project_id)
			.cloned()
			.collect()
	}

	/// List all issues
	pub async fn list_issues(&self) -> Vec<Issue> {
		self.issues.read().await.values().cloned().collect()
	}

	/// Get next issue number for a project
	pub async fn get_next_number(&self, project_id: &str) -> i32 {
		let mut numbers = self.next_number.write().await;
		let num = numbers.entry(project_id.to_string()).or_insert(0);
		*num += 1;
		*num
	}
}

/// Issue Query resolvers
#[derive(Default)]
pub struct IssueQuery;

#[Object]
impl IssueQuery {
	/// Get an issue by ID
	async fn issue(&self, ctx: &Context<'_>, id: ID) -> GqlResult<Option<IssueType>> {
		let storage = ctx.data::<IssueStorage>()?;
		let issue = storage.get_issue(id.as_str()).await;
		Ok(issue.map(IssueType))
	}

	/// List issues, optionally filtered by project, with pagination support
	async fn issues(
		&self,
		ctx: &Context<'_>,
		project_id: Option<ID>,
		pagination: Option<PaginationInput>,
	) -> GqlResult<IssueConnection> {
		let storage = ctx.data::<IssueStorage>()?;
		let all_issues = if let Some(pid) = project_id {
			storage.get_issues_by_project(pid.as_str()).await
		} else {
			storage.list_issues().await
		};

		// Apply pagination
		let pagination = pagination.unwrap_or_default();
		let page = pagination.page.unwrap_or(1).max(1);
		let page_size = pagination.page_size.unwrap_or(10).max(1);

		let total_count = all_issues.len() as i32;
		let start = ((page - 1) * page_size) as usize;

		let edges: Vec<IssueType> = all_issues
			.into_iter()
			.skip(start)
			.take(page_size as usize)
			.map(IssueType)
			.collect();

		let total_pages = (total_count + page_size - 1) / page_size;

		Ok(IssueConnection {
			edges,
			page_info: PageInfo {
				has_next_page: page < total_pages,
				has_previous_page: page > 1,
				total_count,
				page,
				page_size,
			},
		})
	}
}

/// Issue Mutation resolvers
#[derive(Default)]
pub struct IssueMutation;

#[Object]
impl IssueMutation {
	/// Create a new issue
	async fn create_issue(
		&self,
		ctx: &Context<'_>,
		input: CreateIssueInput,
	) -> GqlResult<IssueType> {
		// Validate input
		input
			.validate()
			.map_err(|e| ApiError::InvalidInput(format!("Validation error: {}", e)).extend())?;

		use reinhardt::Claims;
		let claims = ctx
			.data::<Claims>()
			.map_err(|_| ApiError::Unauthorized("Authentication required".to_string()).extend())?;
		let storage = ctx.data::<IssueStorage>()?;
		let broadcaster = ctx.data::<IssueEventBroadcaster>()?;

		let project_id = Uuid::parse_str(input.project_id.as_str())
			.map_err(|_| ApiError::InvalidInput("Invalid project ID".to_string()).extend())?;
		let author_id = Uuid::parse_str(&claims.sub)
			.map_err(|_| ApiError::InvalidInput("Invalid user ID".to_string()).extend())?;

		let number = storage.get_next_number(&project_id.to_string()).await;

		let issue = Issue::new(
			project_id,
			number,
			input.title,
			input.body,
			"open".to_string(),
			author_id,
		);

		storage.add_issue(issue.clone()).await;
		broadcaster
			.broadcast(IssueEvent::Created(issue.clone()))
			.await;

		Ok(IssueType(issue))
	}

	/// Update an existing issue
	async fn update_issue(
		&self,
		ctx: &Context<'_>,
		id: ID,
		input: UpdateIssueInput,
	) -> GqlResult<IssueType> {
		// Validate input
		input
			.validate()
			.map_err(|e| ApiError::InvalidInput(format!("Validation error: {}", e)).extend())?;

		let storage = ctx.data::<IssueStorage>()?;
		let broadcaster = ctx.data::<IssueEventBroadcaster>()?;

		let issue = storage
			.get_issue(id.as_str())
			.await
			.ok_or_else(|| ApiError::NotFound("Issue not found".to_string()).extend())?;

		// Update issue using setters
		let mut updated_issue = issue.clone();
		if let Some(title) = input.title {
			updated_issue.set_title(title);
		}
		if let Some(body) = input.body {
			updated_issue.set_body(body);
		}

		storage.add_issue(updated_issue.clone()).await;
		broadcaster
			.broadcast(IssueEvent::Updated(updated_issue.clone()))
			.await;

		Ok(IssueType(updated_issue))
	}

	/// Close an issue
	async fn close_issue(&self, ctx: &Context<'_>, id: ID) -> GqlResult<IssueType> {
		let storage = ctx.data::<IssueStorage>()?;
		let broadcaster = ctx.data::<IssueEventBroadcaster>()?;

		let issue = storage
			.get_issue(id.as_str())
			.await
			.ok_or_else(|| ApiError::NotFound("Issue not found".to_string()).extend())?;

		let mut closed_issue = issue.clone();
		closed_issue.set_state("closed".to_string());

		storage.add_issue(closed_issue.clone()).await;
		broadcaster
			.broadcast(IssueEvent::Closed(closed_issue.clone()))
			.await;

		Ok(IssueType(closed_issue))
	}

	/// Reopen a closed issue
	async fn reopen_issue(&self, ctx: &Context<'_>, id: ID) -> GqlResult<IssueType> {
		let storage = ctx.data::<IssueStorage>()?;
		let broadcaster = ctx.data::<IssueEventBroadcaster>()?;

		let issue = storage
			.get_issue(id.as_str())
			.await
			.ok_or_else(|| ApiError::NotFound("Issue not found".to_string()).extend())?;

		let mut reopened_issue = issue.clone();
		reopened_issue.set_state("open".to_string());

		storage.add_issue(reopened_issue.clone()).await;
		broadcaster
			.broadcast(IssueEvent::Updated(reopened_issue.clone()))
			.await;

		Ok(IssueType(reopened_issue))
	}
}

/// Issue Subscription resolvers
#[derive(Default)]
pub struct IssueSubscription;

#[Subscription]
impl IssueSubscription {
	/// Subscribe to new issues
	async fn issue_created<'ctx>(
		&self,
		ctx: &Context<'ctx>,
		project_id: Option<ID>,
	) -> impl Stream<Item = IssueType> + 'ctx {
		let broadcaster = ctx.data::<IssueEventBroadcaster>().unwrap();
		let mut rx = broadcaster.subscribe().await;
		let project_filter = project_id.map(|id| id.to_string());

		async_stream::stream! {
			while let Ok(event) = rx.recv().await {
				if let IssueEvent::Created(issue) = event
					&& project_filter.as_ref().is_none_or(|p| p == &issue.project_id().to_string()) {
						yield IssueType(issue);
					}
			}
		}
	}

	/// Subscribe to issue updates
	async fn issue_updated<'ctx>(
		&self,
		ctx: &Context<'ctx>,
		project_id: Option<ID>,
	) -> impl Stream<Item = IssueType> + 'ctx {
		let broadcaster = ctx.data::<IssueEventBroadcaster>().unwrap();
		let mut rx = broadcaster.subscribe().await;
		let project_filter = project_id.map(|id| id.to_string());

		async_stream::stream! {
			while let Ok(event) = rx.recv().await {
				if let IssueEvent::Updated(issue) = event
					&& project_filter.as_ref().is_none_or(|p| p == &issue.project_id().to_string()) {
						yield IssueType(issue);
					}
			}
		}
	}

	/// Subscribe to closed issues
	async fn issue_closed<'ctx>(
		&self,
		ctx: &Context<'ctx>,
		project_id: Option<ID>,
	) -> impl Stream<Item = IssueType> + 'ctx {
		let broadcaster = ctx.data::<IssueEventBroadcaster>().unwrap();
		let mut rx = broadcaster.subscribe().await;
		let project_filter = project_id.map(|id| id.to_string());

		async_stream::stream! {
			while let Ok(event) = rx.recv().await {
				if let IssueEvent::Closed(issue) = event
					&& project_filter.as_ref().is_none_or(|p| p == &issue.project_id().to_string()) {
						yield IssueType(issue);
					}
			}
		}
	}
}
