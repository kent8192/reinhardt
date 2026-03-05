//! Tests for workflow engine

use reinhardt_cms::error::CmsError;
use reinhardt_cms::workflow::{PageState, PageVersion, WorkflowEngine, WorkflowTransition};
use rstest::rstest;
use uuid::Uuid;

#[rstest]
#[tokio::test]
async fn test_workflow_state_retrieval() {
	let engine = WorkflowEngine::new();

	let page_id = Uuid::new_v4();

	// New pages should be in Draft state
	let state = engine.get_state(page_id).await.unwrap();
	assert_eq!(state, PageState::Draft);
}

#[rstest]
#[tokio::test]
async fn test_workflow_valid_transition() {
	let mut engine = WorkflowEngine::new();

	let page_id = Uuid::new_v4();
	let user_id = Uuid::new_v4();

	// Draft -> InReview
	let new_state = engine
		.transition(page_id, WorkflowTransition::SubmitForReview, user_id)
		.await
		.unwrap();
	assert_eq!(new_state, PageState::InReview);

	// InReview -> Approved
	let new_state = engine
		.transition(page_id, WorkflowTransition::Approve, user_id)
		.await
		.unwrap();
	assert_eq!(new_state, PageState::Approved);

	// Approved -> Published
	let new_state = engine
		.transition(page_id, WorkflowTransition::Publish, user_id)
		.await
		.unwrap();
	assert_eq!(new_state, PageState::Published);
}

#[rstest]
#[tokio::test]
async fn test_workflow_invalid_transition() {
	let mut engine = WorkflowEngine::new();

	let page_id = Uuid::new_v4();
	let user_id = Uuid::new_v4();

	// Draft -> Publish (invalid, should go through review first)
	let result = engine
		.transition(page_id, WorkflowTransition::Publish, user_id)
		.await;
	assert!(result.is_err());
}

#[rstest]
#[tokio::test]
async fn test_version_creation() {
	let mut engine = WorkflowEngine::new();

	let page_id = Uuid::new_v4();
	let author_id = Uuid::new_v4();

	// Create first version
	let content = serde_json::json!({"title": "Test Page", "content": "Hello"});
	let version1 = engine
		.create_version(
			page_id,
			author_id,
			content,
			Some("Initial version".to_string()),
		)
		.await
		.unwrap();

	assert_eq!(version1.page_id, page_id);
	assert_eq!(version1.version_number, 1);
	assert_eq!(version1.author_id, author_id);

	// Create second version
	let content = serde_json::json!({"title": "Test Page", "content": "Updated"});
	let version2 = engine
		.create_version(
			page_id,
			author_id,
			content,
			Some("Updated content".to_string()),
		)
		.await
		.unwrap();

	assert_eq!(version2.version_number, 2);
}

#[rstest]
#[tokio::test]
async fn test_version_history() {
	let mut engine = WorkflowEngine::new();

	let page_id = Uuid::new_v4();
	let author_id = Uuid::new_v4();

	// Create multiple versions
	for i in 1..=3 {
		let content = serde_json::json!({"version": i});
		engine
			.create_version(page_id, author_id, content, Some(format!("Version {}", i)))
			.await
			.unwrap();
	}

	// Get version history
	let versions = engine.get_versions(page_id).await.unwrap();
	assert_eq!(versions.len(), 3);
	assert_eq!(versions[0].version_number, 1);
	assert_eq!(versions[1].version_number, 2);
	assert_eq!(versions[2].version_number, 3);
}

#[rstest]
#[tokio::test]
async fn test_version_restoration() {
	let mut engine = WorkflowEngine::new();

	let page_id = Uuid::new_v4();
	let author_id = Uuid::new_v4();

	// Create versions
	let content1 = serde_json::json!({"version": 1});
	let version1 = engine
		.create_version(page_id, author_id, content1, Some("Version 1".to_string()))
		.await
		.unwrap();

	let content2 = serde_json::json!({"version": 2});
	engine
		.create_version(page_id, author_id, content2, Some("Version 2".to_string()))
		.await
		.unwrap();

	// Restore to version 1
	let restored = engine
		.restore_version(page_id, version1.id, author_id)
		.await
		.unwrap();

	assert_eq!(restored.content["version"], 1);
	assert_eq!(restored.version_number, 3); // New version number
}

// === State Transition Tests ===

#[rstest]
#[tokio::test]
async fn test_reject_puts_page_in_rejected() {
	// Arrange
	let mut engine = WorkflowEngine::new();
	let page_id = Uuid::new_v4();
	let user_id = Uuid::new_v4();

	engine
		.transition(page_id, WorkflowTransition::SubmitForReview, user_id)
		.await
		.unwrap();

	// Act
	let state = engine
		.transition(page_id, WorkflowTransition::Reject, user_id)
		.await
		.unwrap();

	// Assert
	assert_eq!(state, PageState::Rejected);
}

#[rstest]
#[tokio::test]
async fn test_unpublish_returns_to_draft() {
	// Arrange
	let mut engine = WorkflowEngine::new();
	let page_id = Uuid::new_v4();
	let user_id = Uuid::new_v4();

	engine
		.transition(page_id, WorkflowTransition::SubmitForReview, user_id)
		.await
		.unwrap();
	engine
		.transition(page_id, WorkflowTransition::Approve, user_id)
		.await
		.unwrap();
	engine
		.transition(page_id, WorkflowTransition::Publish, user_id)
		.await
		.unwrap();

	// Act
	let state = engine
		.transition(page_id, WorkflowTransition::Unpublish, user_id)
		.await
		.unwrap();

	// Assert
	assert_eq!(state, PageState::Draft);
}

#[rstest]
#[tokio::test]
async fn test_archive_from_draft() {
	// Arrange
	let mut engine = WorkflowEngine::new();
	let page_id = Uuid::new_v4();
	let user_id = Uuid::new_v4();

	// Act
	let state = engine
		.transition(page_id, WorkflowTransition::Archive, user_id)
		.await
		.unwrap();

	// Assert
	assert_eq!(state, PageState::Archived);
}

#[rstest]
#[tokio::test]
async fn test_archive_from_published() {
	// Arrange
	let mut engine = WorkflowEngine::new();
	let page_id = Uuid::new_v4();
	let user_id = Uuid::new_v4();

	engine
		.transition(page_id, WorkflowTransition::SubmitForReview, user_id)
		.await
		.unwrap();
	engine
		.transition(page_id, WorkflowTransition::Approve, user_id)
		.await
		.unwrap();
	engine
		.transition(page_id, WorkflowTransition::Publish, user_id)
		.await
		.unwrap();

	// Act
	let state = engine
		.transition(page_id, WorkflowTransition::Archive, user_id)
		.await
		.unwrap();

	// Assert
	assert_eq!(state, PageState::Archived);
}

#[rstest]
#[tokio::test]
async fn test_restore_from_archived_to_draft() {
	// Arrange
	let mut engine = WorkflowEngine::new();
	let page_id = Uuid::new_v4();
	let user_id = Uuid::new_v4();

	engine
		.transition(page_id, WorkflowTransition::Archive, user_id)
		.await
		.unwrap();

	// Act
	let state = engine
		.transition(page_id, WorkflowTransition::Restore, user_id)
		.await
		.unwrap();

	// Assert
	assert_eq!(state, PageState::Draft);
}

#[rstest]
#[tokio::test]
async fn test_rejected_is_terminal_state() {
	// Arrange
	let mut engine = WorkflowEngine::new();
	let page_id = Uuid::new_v4();
	let user_id = Uuid::new_v4();

	engine
		.transition(page_id, WorkflowTransition::SubmitForReview, user_id)
		.await
		.unwrap();
	engine
		.transition(page_id, WorkflowTransition::Reject, user_id)
		.await
		.unwrap();

	let all_transitions = [
		WorkflowTransition::SubmitForReview,
		WorkflowTransition::Approve,
		WorkflowTransition::Reject,
		WorkflowTransition::Publish,
		WorkflowTransition::Unpublish,
		WorkflowTransition::Archive,
		WorkflowTransition::Restore,
	];

	// Act & Assert
	for transition in all_transitions {
		let result = engine.transition(page_id, transition, user_id).await;
		let err = result.unwrap_err();
		assert!(matches!(err, CmsError::InvalidWorkflowTransition(_)));
	}
}

#[rstest]
#[tokio::test]
async fn test_full_lifecycle_round_trip() {
	// Arrange
	let mut engine = WorkflowEngine::new();
	let page_id = Uuid::new_v4();
	let user_id = Uuid::new_v4();

	// Act - Full lifecycle: Draft -> InReview -> Approved -> Published -> Draft -> InReview
	let state = engine
		.transition(page_id, WorkflowTransition::SubmitForReview, user_id)
		.await
		.unwrap();
	assert_eq!(state, PageState::InReview);

	let state = engine
		.transition(page_id, WorkflowTransition::Approve, user_id)
		.await
		.unwrap();
	assert_eq!(state, PageState::Approved);

	let state = engine
		.transition(page_id, WorkflowTransition::Publish, user_id)
		.await
		.unwrap();
	assert_eq!(state, PageState::Published);

	let state = engine
		.transition(page_id, WorkflowTransition::Unpublish, user_id)
		.await
		.unwrap();
	assert_eq!(state, PageState::Draft);

	// Assert - Can re-enter the workflow
	let state = engine
		.transition(page_id, WorkflowTransition::SubmitForReview, user_id)
		.await
		.unwrap();
	assert_eq!(state, PageState::InReview);
}

#[rstest]
#[tokio::test]
async fn test_archive_restore_cycle() {
	// Arrange
	let mut engine = WorkflowEngine::new();
	let page_id = Uuid::new_v4();
	let user_id = Uuid::new_v4();

	// Act & Assert - Repeat archive/restore 3 times
	for _ in 0..3 {
		let state = engine
			.transition(page_id, WorkflowTransition::Archive, user_id)
			.await
			.unwrap();
		assert_eq!(state, PageState::Archived);

		let state = engine
			.transition(page_id, WorkflowTransition::Restore, user_id)
			.await
			.unwrap();
		assert_eq!(state, PageState::Draft);
	}
}

#[rstest]
#[tokio::test]
async fn test_approved_only_allows_publish() {
	// Arrange
	let mut engine = WorkflowEngine::new();
	let page_id = Uuid::new_v4();
	let user_id = Uuid::new_v4();

	engine
		.transition(page_id, WorkflowTransition::SubmitForReview, user_id)
		.await
		.unwrap();
	engine
		.transition(page_id, WorkflowTransition::Approve, user_id)
		.await
		.unwrap();

	let invalid_transitions = [
		WorkflowTransition::SubmitForReview,
		WorkflowTransition::Approve,
		WorkflowTransition::Reject,
		WorkflowTransition::Unpublish,
		WorkflowTransition::Archive,
		WorkflowTransition::Restore,
	];

	// Act & Assert - All invalid transitions should fail
	for transition in invalid_transitions {
		let result = engine.transition(page_id, transition, user_id).await;
		let err = result.unwrap_err();
		assert!(matches!(err, CmsError::InvalidWorkflowTransition(_)));
	}

	// Only Publish should succeed
	let state = engine
		.transition(page_id, WorkflowTransition::Publish, user_id)
		.await
		.unwrap();
	assert_eq!(state, PageState::Published);
}

#[rstest]
#[tokio::test]
async fn test_in_review_allows_approve_or_reject_only() {
	// Arrange
	let mut engine = WorkflowEngine::new();
	let page_id = Uuid::new_v4();
	let user_id = Uuid::new_v4();

	engine
		.transition(page_id, WorkflowTransition::SubmitForReview, user_id)
		.await
		.unwrap();

	let invalid_transitions = [
		WorkflowTransition::SubmitForReview,
		WorkflowTransition::Publish,
		WorkflowTransition::Unpublish,
		WorkflowTransition::Archive,
		WorkflowTransition::Restore,
	];

	// Act & Assert - Invalid transitions should fail
	for transition in invalid_transitions {
		let result = engine.transition(page_id, transition, user_id).await;
		let err = result.unwrap_err();
		assert!(matches!(err, CmsError::InvalidWorkflowTransition(_)));
	}

	// Approve should succeed
	let state = engine
		.transition(page_id, WorkflowTransition::Approve, user_id)
		.await
		.unwrap();
	assert_eq!(state, PageState::Approved);

	// Reset to InReview for Reject test
	let mut engine2 = WorkflowEngine::new();
	let page_id2 = Uuid::new_v4();
	engine2
		.transition(page_id2, WorkflowTransition::SubmitForReview, user_id)
		.await
		.unwrap();

	let state = engine2
		.transition(page_id2, WorkflowTransition::Reject, user_id)
		.await
		.unwrap();
	assert_eq!(state, PageState::Rejected);
}

// === Happy Path Tests ===

#[rstest]
#[tokio::test]
async fn test_transition_creates_version_entry() {
	// Arrange
	let mut engine = WorkflowEngine::new();
	let page_id = Uuid::new_v4();
	let user_id = Uuid::new_v4();

	// Act
	engine
		.transition(page_id, WorkflowTransition::SubmitForReview, user_id)
		.await
		.unwrap();

	// Assert
	let versions = engine.get_versions(page_id).await.unwrap();
	assert_eq!(versions.len(), 1);

	let version = &versions[0];
	assert_eq!(version.page_id, page_id);
	assert_eq!(version.author_id, user_id);
	assert_eq!(version.state, PageState::InReview);
	assert_eq!(version.content["transition"], "SubmitForReview");
	assert_eq!(version.content["from_state"], "Draft");
	assert_eq!(version.content["to_state"], "InReview");
}

// === Error Path Tests ===

#[rstest]
#[tokio::test]
async fn test_restore_version_nonexistent_page() {
	// Arrange
	let mut engine = WorkflowEngine::new();
	let page_id = Uuid::new_v4();
	let user_id = Uuid::new_v4();
	let version_id = Uuid::new_v4();

	// Act
	let result = engine.restore_version(page_id, version_id, user_id).await;

	// Assert
	let err = result.unwrap_err();
	assert!(matches!(err, CmsError::PageNotFound(_)));
}

#[rstest]
#[tokio::test]
async fn test_restore_version_nonexistent_version_id() {
	// Arrange
	let mut engine = WorkflowEngine::new();
	let page_id = Uuid::new_v4();
	let author_id = Uuid::new_v4();
	let nonexistent_version_id = Uuid::new_v4();

	// Create a version so the page exists in version history
	let content = serde_json::json!({"title": "Test"});
	engine
		.create_version(page_id, author_id, content, Some("v1".to_string()))
		.await
		.unwrap();

	// Act
	let result = engine
		.restore_version(page_id, nonexistent_version_id, author_id)
		.await;

	// Assert
	let err = result.unwrap_err();
	assert!(matches!(err, CmsError::Generic(ref msg) if msg == "Version not found"));
}

// === Edge Case Tests ===

#[rstest]
#[tokio::test]
async fn test_create_version_before_any_transition() {
	// Arrange
	let mut engine = WorkflowEngine::new();
	let page_id = Uuid::new_v4();
	let author_id = Uuid::new_v4();
	let content = serde_json::json!({"title": "First version"});

	// Act
	let version = engine
		.create_version(page_id, author_id, content, Some("Initial".to_string()))
		.await
		.unwrap();

	// Assert - State defaults to Draft when no transition has occurred
	assert_eq!(version.state, PageState::Draft);
	assert_eq!(version.version_number, 1);
}

// === Sanity Tests ===

#[rstest]
#[tokio::test]
async fn test_workflow_engine_default_trait() {
	// Arrange & Act
	let engine = WorkflowEngine::default();
	let page_id = Uuid::new_v4();

	// Assert
	let state = engine.get_state(page_id).await.unwrap();
	assert_eq!(state, PageState::Draft);
}

#[rstest]
#[tokio::test]
async fn test_page_version_serialization_roundtrip() {
	// Arrange
	let mut engine = WorkflowEngine::new();
	let page_id = Uuid::new_v4();
	let author_id = Uuid::new_v4();
	let content = serde_json::json!({"title": "Test Page", "body": "Content"});

	let version = engine
		.create_version(
			page_id,
			author_id,
			content,
			Some("Test version".to_string()),
		)
		.await
		.unwrap();

	// Act
	let json = serde_json::to_string(&version).unwrap();
	let deserialized: PageVersion = serde_json::from_str(&json).unwrap();

	// Assert
	assert_eq!(deserialized.id, version.id);
	assert_eq!(deserialized.page_id, version.page_id);
	assert_eq!(deserialized.version_number, version.version_number);
	assert_eq!(deserialized.state, version.state);
	assert_eq!(deserialized.author_id, version.author_id);
	assert_eq!(deserialized.content, version.content);
	assert_eq!(deserialized.description, version.description);
}

// === Equivalence Partitioning Tests ===

#[rstest]
#[case(PageState::Draft)]
#[case(PageState::InReview)]
#[case(PageState::Approved)]
#[case(PageState::Published)]
#[case(PageState::Rejected)]
#[case(PageState::Archived)]
#[tokio::test]
async fn test_workflow_state_serialization_variants(#[case] state: PageState) {
	// Arrange
	let json = serde_json::to_string(&state).unwrap();

	// Act
	let deserialized: PageState = serde_json::from_str(&json).unwrap();

	// Assert
	assert_eq!(deserialized, state);
}

#[rstest]
#[case(WorkflowTransition::SubmitForReview)]
#[case(WorkflowTransition::Approve)]
#[case(WorkflowTransition::Reject)]
#[case(WorkflowTransition::Publish)]
#[case(WorkflowTransition::Unpublish)]
#[case(WorkflowTransition::Archive)]
#[case(WorkflowTransition::Restore)]
#[tokio::test]
async fn test_workflow_transition_serialization_variants(#[case] transition: WorkflowTransition) {
	// Arrange
	let json = serde_json::to_string(&transition).unwrap();

	// Act
	let deserialized: WorkflowTransition = serde_json::from_str(&json).unwrap();

	// Assert
	assert_eq!(deserialized, transition);
}

// === Boundary Value Tests ===

#[rstest]
#[case(1)]
#[case(2)]
#[case(10)]
#[case(100)]
#[tokio::test]
async fn test_version_count_boundaries(#[case] count: u32) {
	// Arrange
	let mut engine = WorkflowEngine::new();
	let page_id = Uuid::new_v4();
	let author_id = Uuid::new_v4();

	// Act
	for i in 1..=count {
		let content = serde_json::json!({"version": i});
		engine
			.create_version(page_id, author_id, content, Some(format!("Version {}", i)))
			.await
			.unwrap();
	}

	// Assert
	let versions = engine.get_versions(page_id).await.unwrap();
	assert_eq!(versions.len() as u32, count);
	for (i, version) in versions.iter().enumerate() {
		assert_eq!(version.version_number, (i as u32) + 1);
	}
}

// === Combination Tests ===

#[rstest]
#[tokio::test]
async fn test_workflow_versions_with_transitions() {
	// Arrange
	let mut engine = WorkflowEngine::new();
	let page_id = Uuid::new_v4();
	let user_id = Uuid::new_v4();
	let content = serde_json::json!({"title": "Manual version"});

	// Act - Create manual version, then a transition (which auto-creates a version)
	engine
		.create_version(page_id, user_id, content, Some("Manual".to_string()))
		.await
		.unwrap();
	engine
		.transition(page_id, WorkflowTransition::SubmitForReview, user_id)
		.await
		.unwrap();

	// Assert
	let versions = engine.get_versions(page_id).await.unwrap();
	assert_eq!(versions.len(), 2);
	assert_eq!(versions[0].version_number, 1);
	assert_eq!(versions[1].version_number, 2);
}

// === Decision Table Tests ===

#[rstest]
// Draft
#[case(PageState::Draft, WorkflowTransition::SubmitForReview, true)]
#[case(PageState::Draft, WorkflowTransition::Approve, false)]
#[case(PageState::Draft, WorkflowTransition::Reject, false)]
#[case(PageState::Draft, WorkflowTransition::Publish, false)]
#[case(PageState::Draft, WorkflowTransition::Unpublish, false)]
#[case(PageState::Draft, WorkflowTransition::Archive, true)]
#[case(PageState::Draft, WorkflowTransition::Restore, false)]
// InReview
#[case(PageState::InReview, WorkflowTransition::SubmitForReview, false)]
#[case(PageState::InReview, WorkflowTransition::Approve, true)]
#[case(PageState::InReview, WorkflowTransition::Reject, true)]
#[case(PageState::InReview, WorkflowTransition::Publish, false)]
#[case(PageState::InReview, WorkflowTransition::Unpublish, false)]
#[case(PageState::InReview, WorkflowTransition::Archive, false)]
#[case(PageState::InReview, WorkflowTransition::Restore, false)]
// Approved
#[case(PageState::Approved, WorkflowTransition::SubmitForReview, false)]
#[case(PageState::Approved, WorkflowTransition::Approve, false)]
#[case(PageState::Approved, WorkflowTransition::Reject, false)]
#[case(PageState::Approved, WorkflowTransition::Publish, true)]
#[case(PageState::Approved, WorkflowTransition::Unpublish, false)]
#[case(PageState::Approved, WorkflowTransition::Archive, false)]
#[case(PageState::Approved, WorkflowTransition::Restore, false)]
// Published
#[case(PageState::Published, WorkflowTransition::SubmitForReview, false)]
#[case(PageState::Published, WorkflowTransition::Approve, false)]
#[case(PageState::Published, WorkflowTransition::Reject, false)]
#[case(PageState::Published, WorkflowTransition::Publish, false)]
#[case(PageState::Published, WorkflowTransition::Unpublish, true)]
#[case(PageState::Published, WorkflowTransition::Archive, true)]
#[case(PageState::Published, WorkflowTransition::Restore, false)]
// Rejected
#[case(PageState::Rejected, WorkflowTransition::SubmitForReview, false)]
#[case(PageState::Rejected, WorkflowTransition::Approve, false)]
#[case(PageState::Rejected, WorkflowTransition::Reject, false)]
#[case(PageState::Rejected, WorkflowTransition::Publish, false)]
#[case(PageState::Rejected, WorkflowTransition::Unpublish, false)]
#[case(PageState::Rejected, WorkflowTransition::Archive, false)]
#[case(PageState::Rejected, WorkflowTransition::Restore, false)]
// Archived
#[case(PageState::Archived, WorkflowTransition::SubmitForReview, false)]
#[case(PageState::Archived, WorkflowTransition::Approve, false)]
#[case(PageState::Archived, WorkflowTransition::Reject, false)]
#[case(PageState::Archived, WorkflowTransition::Publish, false)]
#[case(PageState::Archived, WorkflowTransition::Unpublish, false)]
#[case(PageState::Archived, WorkflowTransition::Archive, false)]
#[case(PageState::Archived, WorkflowTransition::Restore, true)]
#[tokio::test]
async fn test_is_valid_transition_decision_table(
	#[case] state: PageState,
	#[case] transition: WorkflowTransition,
	#[case] expected: bool,
) {
	// Arrange
	let engine = WorkflowEngine::new();

	// Act
	let result = engine.is_valid_transition(state, transition);

	// Assert
	assert_eq!(result, expected);
}

#[rstest]
#[tokio::test]
async fn test_transition_preserves_independent_page_states() {
	// Arrange
	let mut engine = WorkflowEngine::new();
	let page_a = Uuid::new_v4();
	let page_b = Uuid::new_v4();
	let user_id = Uuid::new_v4();

	// Act
	engine
		.transition(page_a, WorkflowTransition::SubmitForReview, user_id)
		.await
		.unwrap();
	engine
		.transition(page_b, WorkflowTransition::Archive, user_id)
		.await
		.unwrap();

	// Assert
	let state_a = engine.get_state(page_a).await.unwrap();
	let state_b = engine.get_state(page_b).await.unwrap();
	assert_eq!(state_a, PageState::InReview);
	assert_eq!(state_b, PageState::Archived);
}
