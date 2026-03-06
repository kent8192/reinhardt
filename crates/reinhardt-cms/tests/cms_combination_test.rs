//! Combination tests for interactions between independent CMS modules

use reinhardt_cms::media::{CropMode, MediaManager, RenditionSpec};
use reinhardt_cms::pages::PageTree;
use reinhardt_cms::permissions::{PermissionChecker, PermissionType, Principal};
use reinhardt_cms::workflow::{PageState, WorkflowEngine, WorkflowTransition};
use rstest::rstest;
use uuid::Uuid;

#[rstest]
#[tokio::test]
async fn test_page_tree_with_workflow_transitions() {
	// Arrange
	let mut tree = PageTree::new();
	let mut workflow = WorkflowEngine::new();
	let user_id = Uuid::new_v4();

	// Act - Create page in tree and manage workflow with same page_id
	let page = tree
		.add_page(None, "Home".to_string(), "home".to_string())
		.await
		.unwrap();
	let state_review = workflow
		.transition(page.id, WorkflowTransition::SubmitForReview, user_id)
		.await
		.unwrap();
	let state_approved = workflow
		.transition(page.id, WorkflowTransition::Approve, user_id)
		.await
		.unwrap();
	let state_published = workflow
		.transition(page.id, WorkflowTransition::Publish, user_id)
		.await
		.unwrap();

	// Assert - Page exists in tree and workflow reached Published
	let children = tree.get_children(page.id).await.unwrap();
	assert_eq!(children.len(), 0);
	assert_eq!(page.path, "/home");
	assert_eq!(state_review, PageState::InReview);
	assert_eq!(state_approved, PageState::Approved);
	assert_eq!(state_published, PageState::Published);
}

#[rstest]
#[tokio::test]
async fn test_pages_permissions_combined() {
	// Arrange
	let mut tree = PageTree::new();
	let mut checker = PermissionChecker::new();
	let user_id = Uuid::new_v4();

	let page1 = tree
		.add_page(None, "Page 1".to_string(), "page1".to_string())
		.await
		.unwrap();
	let page2 = tree
		.add_page(None, "Page 2".to_string(), "page2".to_string())
		.await
		.unwrap();
	let page3 = tree
		.add_page(None, "Page 3".to_string(), "page3".to_string())
		.await
		.unwrap();

	// Act - Grant different permissions per page
	checker
		.grant_permission(
			page1.id,
			Principal::User(user_id),
			PermissionType::View,
			false,
		)
		.await
		.unwrap();
	checker
		.grant_permission(
			page2.id,
			Principal::User(user_id),
			PermissionType::Edit,
			false,
		)
		.await
		.unwrap();
	checker
		.grant_permission(
			page3.id,
			Principal::User(user_id),
			PermissionType::Publish,
			false,
		)
		.await
		.unwrap();

	// Assert - Permissions are isolated per page
	assert_eq!(
		checker
			.check_permission(user_id, page1.id, PermissionType::View)
			.await
			.unwrap(),
		true
	);
	assert_eq!(
		checker
			.check_permission(user_id, page1.id, PermissionType::Edit)
			.await
			.unwrap(),
		false
	);
	assert_eq!(
		checker
			.check_permission(user_id, page2.id, PermissionType::Edit)
			.await
			.unwrap(),
		true
	);
	assert_eq!(
		checker
			.check_permission(user_id, page2.id, PermissionType::View)
			.await
			.unwrap(),
		false
	);
	assert_eq!(
		checker
			.check_permission(user_id, page3.id, PermissionType::Publish)
			.await
			.unwrap(),
		true
	);
	assert_eq!(
		checker
			.check_permission(user_id, page3.id, PermissionType::View)
			.await
			.unwrap(),
		false
	);
}

#[rstest]
#[tokio::test]
async fn test_workflow_state_does_not_affect_page_tree() {
	// Arrange
	let mut tree = PageTree::new();
	let mut workflow = WorkflowEngine::new();
	let user_id = Uuid::new_v4();

	let page = tree
		.add_page(None, "Temporary".to_string(), "temp".to_string())
		.await
		.unwrap();
	let page_id = page.id;

	// Set workflow state
	workflow
		.transition(page_id, WorkflowTransition::SubmitForReview, user_id)
		.await
		.unwrap();

	// Act - Delete page from tree
	tree.delete_page(page_id, false).await.unwrap();

	// Assert - Workflow state is still accessible after page tree deletion
	let state = workflow.get_state(page_id).await.unwrap();
	assert_eq!(state, PageState::InReview);
}

#[rstest]
#[tokio::test]
async fn test_permission_and_workflow_independent() {
	// Arrange
	let mut checker = PermissionChecker::new();
	let mut workflow = WorkflowEngine::new();
	let page_id = Uuid::new_v4();
	let user_id = Uuid::new_v4();

	// Act - Grant permission and transition workflow independently
	checker
		.grant_permission(
			page_id,
			Principal::User(user_id),
			PermissionType::Edit,
			false,
		)
		.await
		.unwrap();
	let state = workflow
		.transition(page_id, WorkflowTransition::SubmitForReview, user_id)
		.await
		.unwrap();

	// Assert - Both operations succeed independently
	let has_edit = checker
		.check_permission(user_id, page_id, PermissionType::Edit)
		.await
		.unwrap();
	let current_state = workflow.get_state(page_id).await.unwrap();
	assert_eq!(has_edit, true);
	assert_eq!(state, PageState::InReview);
	assert_eq!(current_state, PageState::InReview);
}

#[rstest]
#[tokio::test]
async fn test_media_rendition_after_manager_default() {
	// Arrange
	let mut manager = MediaManager::default();
	let spec = RenditionSpec {
		width: Some(150),
		height: Some(150),
		mode: CropMode::Fit,
		format: None,
		quality: None,
	};

	// Act - Upload and create rendition
	let media = manager
		.upload("default_test.png".to_string(), vec![1u8; 200])
		.await
		.unwrap();
	let rendition = manager.get_rendition(media.id, spec).await.unwrap();

	// Assert
	assert_eq!(rendition.media_id, media.id);
	assert_eq!(rendition.width, 150);
	assert_eq!(rendition.height, 150);
	assert_eq!(media.mime_type, "image/png");
	assert_eq!(media.size, 200);
}
