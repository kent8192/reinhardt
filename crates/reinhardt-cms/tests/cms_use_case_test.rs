//! End-to-end use case tests combining multiple CMS modules

use reinhardt_cms::admin::{AdminPageRegistry, PageEditor, PageTypeDescriptor};
use reinhardt_cms::blocks::{Block, BlockLibrary, BlockType, StreamBlock, StreamField};
use reinhardt_cms::error::{CmsError, CmsResult};
use reinhardt_cms::media::{CropMode, MediaManager, RenditionSpec};
use reinhardt_cms::pages::{Page, PageTree};
use reinhardt_cms::permissions::{PermissionChecker, PermissionType, Principal};
use reinhardt_cms::workflow::{PageState, WorkflowEngine, WorkflowTransition};
use rstest::rstest;
use serde_json::{Value as JsonValue, json};
use uuid::Uuid;

// Test helper: simple text block
struct TextBlock {
	text: String,
}

impl Block for TextBlock {
	fn block_type(&self) -> BlockType {
		"text".to_string()
	}

	fn render(&self) -> CmsResult<String> {
		Ok(format!("<p>{}</p>", self.text))
	}

	fn to_json(&self) -> CmsResult<JsonValue> {
		Ok(json!({"text": self.text}))
	}

	fn from_json(value: JsonValue) -> CmsResult<Self> {
		Ok(Self {
			text: value["text"].as_str().unwrap_or("").to_string(),
		})
	}
}

// Test helper: page type descriptor
struct TestPageType {
	type_name: String,
	label: String,
	icon: String,
}

impl PageTypeDescriptor for TestPageType {
	fn type_name(&self) -> &str {
		&self.type_name
	}

	fn label(&self) -> &str {
		&self.label
	}

	fn icon(&self) -> &str {
		&self.icon
	}

	fn can_create_at(&self, _parent: Option<&dyn Page>) -> bool {
		true
	}
}

#[rstest]
#[tokio::test]
async fn test_blog_post_creation_workflow() {
	// Arrange
	let mut tree = PageTree::new();
	let mut workflow = WorkflowEngine::new();
	let author_id = Uuid::new_v4();

	let mut library = BlockLibrary::new();
	library.register("text".to_string(), |data| {
		Ok(Box::new(TextBlock::from_json(data)?))
	});

	// Act - Create page in tree
	let page = tree
		.add_page(None, "Blog".to_string(), "blog".to_string())
		.await
		.unwrap();

	// Act - Create StreamField with blocks
	let mut field = StreamField::new();
	field.add_block(StreamBlock {
		block_type: "text".to_string(),
		data: json!({"text": "Hello World"}),
		id: None,
	});
	let rendered = field.render(&library).unwrap();

	// Act - Transition through workflow
	let state1 = workflow
		.transition(page.id, WorkflowTransition::SubmitForReview, author_id)
		.await
		.unwrap();
	let state2 = workflow
		.transition(page.id, WorkflowTransition::Approve, author_id)
		.await
		.unwrap();
	let state3 = workflow
		.transition(page.id, WorkflowTransition::Publish, author_id)
		.await
		.unwrap();

	// Assert
	assert_eq!(page.path, "/blog");
	assert_eq!(rendered, "<p>Hello World</p>");
	assert_eq!(state1, PageState::InReview);
	assert_eq!(state2, PageState::Approved);
	assert_eq!(state3, PageState::Published);
}

#[rstest]
#[tokio::test]
async fn test_content_editor_save_edit_cycle() {
	// Arrange
	let mut editor = PageEditor::new();
	let page_id = Uuid::new_v4();
	let initial_data = json!({
		"title": "Initial Title",
		"slug": "initial-slug",
		"content": "Initial content"
	});
	let updated_data = json!({
		"title": "Updated Title",
		"slug": "updated-slug",
		"content": "Updated content"
	});

	// Act - Save and render initial data
	editor.save_page(page_id, initial_data).await.unwrap();
	let form_v1 = editor.render_edit_form(page_id).await.unwrap();

	// Act - Update and render new data
	editor.save_page(page_id, updated_data).await.unwrap();
	let form_v2 = editor.render_edit_form(page_id).await.unwrap();

	// Assert
	assert_ne!(form_v1, form_v2);
	assert_ne!(form_v1, "");
	assert_ne!(form_v2, "");
}

#[rstest]
#[tokio::test]
async fn test_image_gallery_with_multiple_renditions() {
	// Arrange
	let mut manager = MediaManager::new();
	let filenames = vec!["photo1.jpg", "photo2.png", "photo3.webp"];
	let specs = vec![
		RenditionSpec {
			width: Some(100),
			height: Some(100),
			mode: CropMode::Fill,
			format: None,
			quality: None,
		},
		RenditionSpec {
			width: Some(200),
			height: Some(200),
			mode: CropMode::Fit,
			format: None,
			quality: None,
		},
		RenditionSpec {
			width: Some(300),
			height: None,
			mode: CropMode::Width,
			format: None,
			quality: None,
		},
	];

	// Act - Upload 3 images
	let mut media_ids = Vec::new();
	for filename in &filenames {
		let media = manager
			.upload(filename.to_string(), vec![0u8; 100])
			.await
			.unwrap();
		media_ids.push(media.id);
	}

	// Act - Create 3 renditions per image = 9 total
	let mut rendition_count = 0u32;
	for media_id in &media_ids {
		for spec in &specs {
			let rendition = manager
				.get_rendition(*media_id, spec.clone())
				.await
				.unwrap();
			assert_eq!(rendition.media_id, *media_id);
			rendition_count += 1;
		}
	}

	// Assert
	assert_eq!(media_ids.len(), 3);
	assert_eq!(rendition_count, 9);
}

#[rstest]
#[tokio::test]
async fn test_permission_based_page_access_control() {
	// Arrange
	let mut checker = PermissionChecker::new();
	let page_id = Uuid::new_v4();
	let user_a = Uuid::new_v4();
	let user_b = Uuid::new_v4();

	// Act - Grant View to User A, Edit to User B
	checker
		.grant_permission(
			page_id,
			Principal::User(user_a),
			PermissionType::View,
			false,
		)
		.await
		.unwrap();
	checker
		.grant_permission(
			page_id,
			Principal::User(user_b),
			PermissionType::Edit,
			false,
		)
		.await
		.unwrap();

	// Assert - User A has View but not Edit
	let a_view = checker
		.check_permission(user_a, page_id, PermissionType::View)
		.await
		.unwrap();
	let a_edit = checker
		.check_permission(user_a, page_id, PermissionType::Edit)
		.await
		.unwrap();
	assert_eq!(a_view, true);
	assert_eq!(a_edit, false);

	// Assert - User B has Edit but not View
	let b_edit = checker
		.check_permission(user_b, page_id, PermissionType::Edit)
		.await
		.unwrap();
	let b_view = checker
		.check_permission(user_b, page_id, PermissionType::View)
		.await
		.unwrap();
	assert_eq!(b_edit, true);
	assert_eq!(b_view, false);
}

#[rstest]
#[tokio::test]
async fn test_page_revision_history_and_rollback() {
	// Arrange
	let mut engine = WorkflowEngine::new();
	let page_id = Uuid::new_v4();
	let author_id = Uuid::new_v4();
	let mut version_ids = Vec::new();

	// Act - Create 5 versions with different content
	for i in 1..=5 {
		let content = json!({"body": format!("Version {} content", i)});
		let version = engine
			.create_version(page_id, author_id, content, Some(format!("Version {}", i)))
			.await
			.unwrap();
		version_ids.push(version.id);
	}

	// Act - Restore version 2
	let restored = engine
		.restore_version(page_id, version_ids[1], author_id)
		.await
		.unwrap();

	// Assert
	let versions = engine.get_versions(page_id).await.unwrap();
	assert_eq!(versions.len(), 6);
	assert_eq!(restored.version_number, 6);
	assert_eq!(restored.content, json!({"body": "Version 2 content"}));
}

#[rstest]
#[tokio::test]
async fn test_multi_author_version_tracking() {
	// Arrange
	let mut engine = WorkflowEngine::new();
	let page_id = Uuid::new_v4();
	let author_a = Uuid::new_v4();
	let author_b = Uuid::new_v4();
	let author_c = Uuid::new_v4();

	// Act - Each author creates a version
	let v1 = engine
		.create_version(page_id, author_a, json!({"by": "A"}), None)
		.await
		.unwrap();
	let v2 = engine
		.create_version(page_id, author_b, json!({"by": "B"}), None)
		.await
		.unwrap();
	let v3 = engine
		.create_version(page_id, author_c, json!({"by": "C"}), None)
		.await
		.unwrap();

	// Assert
	let versions = engine.get_versions(page_id).await.unwrap();
	assert_eq!(versions.len(), 3);
	assert_eq!(v1.author_id, author_a);
	assert_eq!(v2.author_id, author_b);
	assert_eq!(v3.author_id, author_c);
	assert_eq!(versions[0].version_number, 1);
	assert_eq!(versions[1].version_number, 2);
	assert_eq!(versions[2].version_number, 3);
}

#[rstest]
#[tokio::test]
async fn test_page_tree_restructuring() {
	// Arrange
	let mut tree = PageTree::new();
	let root = tree
		.add_page(None, "Root".to_string(), "root".to_string())
		.await
		.unwrap();
	let child1 = tree
		.add_page(Some(root.id), "Child 1".to_string(), "child1".to_string())
		.await
		.unwrap();
	let child2 = tree
		.add_page(Some(root.id), "Child 2".to_string(), "child2".to_string())
		.await
		.unwrap();
	let grandchild = tree
		.add_page(
			Some(child1.id),
			"Grandchild".to_string(),
			"grandchild".to_string(),
		)
		.await
		.unwrap();

	// Act - Move grandchild under child2
	tree.move_page(grandchild.id, Some(child2.id))
		.await
		.unwrap();

	// Assert
	let child2_children = tree.get_children(child2.id).await.unwrap();
	let child1_children = tree.get_children(child1.id).await.unwrap();
	assert_eq!(child2_children.len(), 1);
	assert_eq!(child2_children[0].id, grandchild.id);
	assert_eq!(child2_children[0].path, "/root/child2/grandchild");
	assert_eq!(child2_children[0].depth, 2);
	assert_eq!(child1_children.len(), 0);
}

#[rstest]
#[tokio::test]
async fn test_media_lifecycle_upload_rendition_delete() {
	// Arrange
	let mut manager = MediaManager::new();
	let spec = RenditionSpec {
		width: Some(100),
		height: Some(100),
		mode: CropMode::Fill,
		format: None,
		quality: None,
	};

	// Act - Upload and create rendition
	let media = manager
		.upload("test.jpg".to_string(), vec![0u8; 50])
		.await
		.unwrap();
	let media_id = media.id;
	let rendition = manager.get_rendition(media_id, spec.clone()).await.unwrap();
	assert_eq!(rendition.media_id, media_id);

	// Act - Delete media
	manager.delete(media_id).await.unwrap();

	// Assert - Media no longer accessible
	let get_result = manager.get(media_id).await;
	let err = get_result.unwrap_err();
	assert!(matches!(err, CmsError::MediaNotFound(_)));

	// Assert - Renditions also removed (get_rendition fails because media is gone)
	let rendition_result = manager.get_rendition(media_id, spec).await;
	let rendition_err = rendition_result.unwrap_err();
	assert!(matches!(rendition_err, CmsError::MediaNotFound(_)));
}

#[rstest]
fn test_admin_page_type_hierarchy_control() {
	// Arrange
	let mut registry = AdminPageRegistry::new();
	let blog_type = TestPageType {
		type_name: "blog".to_string(),
		label: "Blog Page".to_string(),
		icon: "icon-blog".to_string(),
	};
	let article_type = TestPageType {
		type_name: "article".to_string(),
		label: "Article Page".to_string(),
		icon: "icon-article".to_string(),
	};

	// Act
	registry.register(blog_type);
	registry.register(article_type);

	// Assert
	let blog = registry.get("blog").unwrap();
	assert_eq!(blog.type_name(), "blog");
	assert_eq!(blog.label(), "Blog Page");
	assert_eq!(blog.can_create_at(None), true);

	let article = registry.get("article").unwrap();
	assert_eq!(article.type_name(), "article");
	assert_eq!(article.label(), "Article Page");
	assert_eq!(article.can_create_at(None), true);
}

#[rstest]
#[tokio::test]
async fn test_content_moderation_reject_flow() {
	// Arrange
	let mut engine = WorkflowEngine::new();
	let page_id = Uuid::new_v4();
	let user_id = Uuid::new_v4();

	// Act - Submit for review then reject
	let state_review = engine
		.transition(page_id, WorkflowTransition::SubmitForReview, user_id)
		.await
		.unwrap();
	let state_rejected = engine
		.transition(page_id, WorkflowTransition::Reject, user_id)
		.await
		.unwrap();

	// Assert - State is Rejected
	assert_eq!(state_review, PageState::InReview);
	assert_eq!(state_rejected, PageState::Rejected);
	let current_state = engine.get_state(page_id).await.unwrap();
	assert_eq!(current_state, PageState::Rejected);

	// Assert - Cannot submit for review from Rejected state
	let result = engine
		.transition(page_id, WorkflowTransition::SubmitForReview, user_id)
		.await;
	let err = result.unwrap_err();
	assert!(matches!(err, CmsError::InvalidWorkflowTransition(_)));
}
