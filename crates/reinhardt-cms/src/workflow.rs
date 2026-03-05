//! Workflow engine for draft/review/publish states
//!
//! State machine for page lifecycle management, inspired by Wagtail's workflow system.

use crate::error::{CmsError, CmsResult};
use crate::pages::PageId;
use crate::permissions::UserId;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Page state in the workflow
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PageState {
	/// Initial draft state
	Draft,

	/// Submitted for review
	InReview,

	/// Approved, ready to publish
	Approved,

	/// Published (live)
	Published,

	/// Rejected (back to draft)
	Rejected,

	/// Archived (no longer visible)
	Archived,
}

/// Workflow transition
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WorkflowTransition {
	/// Submit for review
	SubmitForReview,

	/// Approve
	Approve,

	/// Reject
	Reject,

	/// Publish
	Publish,

	/// Unpublish
	Unpublish,

	/// Archive
	Archive,

	/// Restore from archive
	Restore,
}

/// Page version history entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageVersion {
	/// Version ID
	pub id: Uuid,

	/// Page ID
	pub page_id: PageId,

	/// Version number (incremental)
	pub version_number: u32,

	/// Page state at this version
	pub state: PageState,

	/// Author of this version
	pub author_id: UserId,

	/// Version timestamp
	pub created_at: chrono::DateTime<chrono::Utc>,

	/// Serialized page content (JSON)
	pub content: serde_json::Value,

	/// Change description
	pub description: Option<String>,
}

/// Workflow engine
pub struct WorkflowEngine {
	/// Storage for page states
	page_states: std::collections::HashMap<PageId, PageState>,
	/// Storage for page version history
	versions: std::collections::HashMap<PageId, Vec<PageVersion>>,
}

impl WorkflowEngine {
	/// Create a new workflow engine
	pub fn new() -> Self {
		Self {
			page_states: std::collections::HashMap::new(),
			versions: std::collections::HashMap::new(),
		}
	}

	/// Get current state of a page
	pub async fn get_state(&self, page_id: PageId) -> CmsResult<PageState> {
		Ok(self
			.page_states
			.get(&page_id)
			.copied()
			.unwrap_or(PageState::Draft))
	}

	/// Transition a page to a new state
	pub async fn transition(
		&mut self,
		page_id: PageId,
		transition: WorkflowTransition,
		user_id: UserId,
	) -> CmsResult<PageState> {
		let current_state = self.get_state(page_id).await?;

		// Validate transition
		if !self.is_valid_transition(current_state, transition) {
			return Err(CmsError::InvalidWorkflowTransition(format!(
				"Cannot transition from {:?} using {:?}",
				current_state, transition
			)));
		}

		// Determine new state
		let new_state = match transition {
			WorkflowTransition::SubmitForReview => PageState::InReview,
			WorkflowTransition::Approve => PageState::Approved,
			WorkflowTransition::Reject => PageState::Rejected,
			WorkflowTransition::Publish => PageState::Published,
			WorkflowTransition::Unpublish => PageState::Draft,
			WorkflowTransition::Archive => PageState::Archived,
			WorkflowTransition::Restore => PageState::Draft,
		};

		// Update state
		self.page_states.insert(page_id, new_state);

		// Create a version entry for this transition
		let content = serde_json::json!({
			"transition": format!("{:?}", transition),
			"from_state": format!("{:?}", current_state),
			"to_state": format!("{:?}", new_state),
		});

		self.create_version(
			page_id,
			user_id,
			content,
			Some(format!("State transition: {:?}", transition)),
		)
		.await?;

		Ok(new_state)
	}

	/// Create a new version of a page
	pub async fn create_version(
		&mut self,
		page_id: PageId,
		author_id: UserId,
		content: serde_json::Value,
		description: Option<String>,
	) -> CmsResult<PageVersion> {
		use chrono::Utc;

		// Get state before borrowing versions mutably
		let current_state = self
			.page_states
			.get(&page_id)
			.copied()
			.unwrap_or(PageState::Draft);

		let version_history = self.versions.entry(page_id).or_default();
		let version_number = version_history.len() as u32 + 1;

		let version = PageVersion {
			id: Uuid::new_v4(),
			page_id,
			version_number,
			state: current_state,
			author_id,
			created_at: Utc::now(),
			content,
			description,
		};

		version_history.push(version.clone());

		Ok(version)
	}

	/// Get version history for a page
	pub async fn get_versions(&self, page_id: PageId) -> CmsResult<Vec<PageVersion>> {
		Ok(self.versions.get(&page_id).cloned().unwrap_or_default())
	}

	/// Restore a page to a specific version
	pub async fn restore_version(
		&mut self,
		page_id: PageId,
		version_id: Uuid,
		user_id: UserId,
	) -> CmsResult<PageVersion> {
		// Find the target version
		let version_history = self
			.versions
			.get(&page_id)
			.ok_or_else(|| CmsError::PageNotFound(page_id.to_string()))?;

		let target_version = version_history
			.iter()
			.find(|v| v.id == version_id)
			.ok_or_else(|| CmsError::Generic("Version not found".to_string()))?;

		// Create a new version with the content from the target version
		let new_version = self
			.create_version(
				page_id,
				user_id,
				target_version.content.clone(),
				Some(format!(
					"Restored from version {}",
					target_version.version_number
				)),
			)
			.await?;

		Ok(new_version)
	}

	/// Check if a transition is valid
	pub fn is_valid_transition(&self, from: PageState, transition: WorkflowTransition) -> bool {
		use PageState::*;
		use WorkflowTransition::*;

		matches!(
			(from, transition),
			(Draft, SubmitForReview)
				| (InReview, Approve)
				| (InReview, Reject)
				| (Approved, Publish)
				| (Published, Unpublish)
				| (Published, Archive)
				| (Draft, Archive)
				| (Archived, Restore)
		)
	}
}

impl Default for WorkflowEngine {
	fn default() -> Self {
		Self::new()
	}
}
