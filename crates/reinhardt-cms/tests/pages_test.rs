//! Tests for page tree functionality

use reinhardt_cms::error::CmsError;
use reinhardt_cms::pages::{PageNode, PageTree};
use rstest::*;
use uuid::Uuid;

#[rstest]
#[tokio::test]
async fn test_create_root_page() {
	// Given: A new page tree
	let mut tree = PageTree::new();

	// When: Adding a root page
	let result = tree
		.add_page(None, "Home".to_string(), "home".to_string())
		.await;

	// Then: The page should be created successfully
	assert!(result.is_ok());
	let page = result.unwrap();
	assert_eq!(page.title, "Home");
	assert_eq!(page.slug, "home");
	assert_eq!(page.path, "/home");
	assert_eq!(page.depth, 0);
	assert!(page.parent_id.is_none());
}

#[rstest]
#[tokio::test]
async fn test_create_child_page() {
	// Given: A page tree with a root page
	let mut tree = PageTree::new();
	let root = tree
		.add_page(None, "Home".to_string(), "home".to_string())
		.await
		.unwrap();

	// When: Adding a child page
	let result = tree
		.add_page(Some(root.id), "About".to_string(), "about".to_string())
		.await;

	// Then: The child page should be created with correct hierarchy
	assert!(result.is_ok());
	let child = result.unwrap();
	assert_eq!(child.title, "About");
	assert_eq!(child.slug, "about");
	assert_eq!(child.path, "/home/about");
	assert_eq!(child.depth, 1);
	assert_eq!(child.parent_id, Some(root.id));
}

#[rstest]
#[tokio::test]
async fn test_get_children() {
	// Given: A page tree with parent and children
	let mut tree = PageTree::new();
	let root = tree
		.add_page(None, "Home".to_string(), "home".to_string())
		.await
		.unwrap();

	tree.add_page(Some(root.id), "About".to_string(), "about".to_string())
		.await
		.unwrap();
	tree.add_page(Some(root.id), "Contact".to_string(), "contact".to_string())
		.await
		.unwrap();

	// When: Getting children of the root page
	let children = tree.get_children(root.id).await;

	// Then: All children should be returned
	assert!(children.is_ok());
	let children = children.unwrap();
	assert_eq!(children.len(), 2);
	assert!(children.iter().any(|p| p.slug == "about"));
	assert!(children.iter().any(|p| p.slug == "contact"));
}

#[rstest]
#[tokio::test]
async fn test_move_page() {
	// Given: A page tree with multiple pages
	let mut tree = PageTree::new();
	let root1 = tree
		.add_page(None, "Blog".to_string(), "blog".to_string())
		.await
		.unwrap();
	let root2 = tree
		.add_page(None, "News".to_string(), "news".to_string())
		.await
		.unwrap();
	let child = tree
		.add_page(Some(root1.id), "Article".to_string(), "article".to_string())
		.await
		.unwrap();

	// When: Moving the article from Blog to News
	let result = tree.move_page(child.id, Some(root2.id)).await;

	// Then: The page should be moved successfully
	assert!(result.is_ok());

	let children = tree.get_children(root2.id).await.unwrap();
	assert_eq!(children.len(), 1);
	assert_eq!(children[0].slug, "article");
}

#[rstest]
#[tokio::test]
async fn test_get_ancestors() {
	// Given: A three-level page hierarchy
	let mut tree = PageTree::new();
	let root = tree
		.add_page(None, "Home".to_string(), "home".to_string())
		.await
		.unwrap();
	let child = tree
		.add_page(Some(root.id), "Blog".to_string(), "blog".to_string())
		.await
		.unwrap();
	let grandchild = tree
		.add_page(Some(child.id), "Post".to_string(), "post".to_string())
		.await
		.unwrap();

	// When: Getting ancestors of the grandchild
	let ancestors = tree.get_ancestors(grandchild.id).await;

	// Then: All ancestors should be returned in order
	assert!(ancestors.is_ok());
	let ancestors = ancestors.unwrap();
	assert_eq!(ancestors.len(), 2);
	assert_eq!(ancestors[0].slug, "home");
	assert_eq!(ancestors[1].slug, "blog");
}

#[rstest]
#[tokio::test]
async fn test_delete_page() {
	// Given: A page tree with a page
	let mut tree = PageTree::new();
	let page = tree
		.add_page(None, "Temp".to_string(), "temp".to_string())
		.await
		.unwrap();

	// When: Deleting the page
	let result = tree.delete_page(page.id, false).await;

	// Then: The page should be deleted successfully
	assert!(result.is_ok());
}

#[rstest]
#[tokio::test]
async fn test_delete_page_recursive() {
	// Given: A page tree with parent and children
	let mut tree = PageTree::new();
	let root = tree
		.add_page(None, "Parent".to_string(), "parent".to_string())
		.await
		.unwrap();
	tree.add_page(Some(root.id), "Child".to_string(), "child".to_string())
		.await
		.unwrap();

	// When: Deleting the parent recursively
	let result = tree.delete_page(root.id, true).await;

	// Then: The parent and all children should be deleted
	assert!(result.is_ok());
}

// --- Happy Path ---

#[rstest]
#[tokio::test]
async fn test_create_deeply_nested_page() {
	// Arrange
	let mut tree = PageTree::new();

	// Act
	let level0 = tree
		.add_page(None, "L0".to_string(), "l0".to_string())
		.await
		.unwrap();
	let level1 = tree
		.add_page(Some(level0.id), "L1".to_string(), "l1".to_string())
		.await
		.unwrap();
	let level2 = tree
		.add_page(Some(level1.id), "L2".to_string(), "l2".to_string())
		.await
		.unwrap();
	let level3 = tree
		.add_page(Some(level2.id), "L3".to_string(), "l3".to_string())
		.await
		.unwrap();

	// Assert
	assert_eq!(level0.depth, 0);
	assert_eq!(level0.path, "/l0");
	assert_eq!(level1.depth, 1);
	assert_eq!(level1.path, "/l0/l1");
	assert_eq!(level2.depth, 2);
	assert_eq!(level2.path, "/l0/l1/l2");
	assert_eq!(level3.depth, 3);
	assert_eq!(level3.path, "/l0/l1/l2/l3");
}

#[rstest]
#[tokio::test]
async fn test_move_page_to_root() {
	// Arrange
	let mut tree = PageTree::new();
	let parent = tree
		.add_page(None, "Parent".to_string(), "parent".to_string())
		.await
		.unwrap();
	let child = tree
		.add_page(Some(parent.id), "Child".to_string(), "child".to_string())
		.await
		.unwrap();

	// Act
	tree.move_page(child.id, None).await.unwrap();

	// Assert
	let parent_children = tree.get_children(parent.id).await.unwrap();
	assert_eq!(parent_children.len(), 0);
	let ancestors = tree.get_ancestors(child.id).await.unwrap();
	assert_eq!(ancestors.len(), 0);
	// Verify depth and path indirectly via sub-page creation
	let grandchild = tree
		.add_page(Some(child.id), "GC".to_string(), "gc".to_string())
		.await
		.unwrap();
	assert_eq!(grandchild.depth, 1);
	assert_eq!(grandchild.path, "/child/gc");
}

// --- Error Path ---

#[rstest]
#[tokio::test]
async fn test_add_page_with_nonexistent_parent() {
	// Arrange
	let mut tree = PageTree::new();
	let fake_parent_id = Uuid::new_v4();

	// Act
	let result = tree
		.add_page(
			Some(fake_parent_id),
			"Orphan".to_string(),
			"orphan".to_string(),
		)
		.await;

	// Assert
	let err = result.unwrap_err();
	assert!(matches!(
		err,
		CmsError::PageNotFound(ref msg) if msg.contains(&fake_parent_id.to_string())
	));
}

#[rstest]
#[tokio::test]
async fn test_move_nonexistent_page() {
	// Arrange
	let mut tree = PageTree::new();
	let target = tree
		.add_page(None, "Target".to_string(), "target".to_string())
		.await
		.unwrap();
	let fake_page_id = Uuid::new_v4();

	// Act
	let result = tree.move_page(fake_page_id, Some(target.id)).await;

	// Assert
	let err = result.unwrap_err();
	assert!(matches!(err, CmsError::PageNotFound(_)));
}

#[rstest]
#[tokio::test]
async fn test_move_page_to_nonexistent_parent() {
	// Arrange
	let mut tree = PageTree::new();
	let page = tree
		.add_page(None, "Page".to_string(), "page".to_string())
		.await
		.unwrap();
	let fake_parent_id = Uuid::new_v4();

	// Act
	let result = tree.move_page(page.id, Some(fake_parent_id)).await;

	// Assert
	let err = result.unwrap_err();
	assert!(matches!(err, CmsError::PageNotFound(_)));
}

#[rstest]
#[tokio::test]
async fn test_delete_nonexistent_page() {
	// Arrange
	let mut tree = PageTree::new();
	let fake_page_id = Uuid::new_v4();

	// Act
	let result = tree.delete_page(fake_page_id, false).await;

	// Assert
	let err = result.unwrap_err();
	assert!(matches!(err, CmsError::PageNotFound(_)));
}

// --- Edge Cases ---

#[rstest]
#[tokio::test]
async fn test_add_page_empty_title_and_slug() {
	// Arrange
	let mut tree = PageTree::new();

	// Act
	let page = tree
		.add_page(None, String::new(), String::new())
		.await
		.unwrap();

	// Assert
	assert_eq!(page.title, "");
	assert_eq!(page.slug, "");
	assert_eq!(page.path, "/");
}

#[rstest]
#[tokio::test]
async fn test_add_page_slug_with_special_characters() {
	// Arrange
	let mut tree = PageTree::new();
	let unicode_slug = "日本語 スラッグ";

	// Act
	let page = tree
		.add_page(None, "Unicode Page".to_string(), unicode_slug.to_string())
		.await
		.unwrap();

	// Assert
	assert_eq!(page.slug, unicode_slug);
	assert_eq!(page.path, format!("/{}", unicode_slug));
}

#[rstest]
#[tokio::test]
async fn test_get_children_of_leaf_page() {
	// Arrange
	let mut tree = PageTree::new();
	let leaf = tree
		.add_page(None, "Leaf".to_string(), "leaf".to_string())
		.await
		.unwrap();

	// Act
	let children = tree.get_children(leaf.id).await.unwrap();

	// Assert
	assert_eq!(children.len(), 0);
}

#[rstest]
#[tokio::test]
async fn test_get_ancestors_of_root_page() {
	// Arrange
	let mut tree = PageTree::new();
	let root = tree
		.add_page(None, "Root".to_string(), "root".to_string())
		.await
		.unwrap();

	// Act
	let ancestors = tree.get_ancestors(root.id).await.unwrap();

	// Assert
	assert_eq!(ancestors.len(), 0);
}

#[rstest]
#[tokio::test]
async fn test_get_ancestors_of_nonexistent_page() {
	// Arrange
	let tree = PageTree::new();
	let fake_id = Uuid::new_v4();

	// Act
	let ancestors = tree.get_ancestors(fake_id).await.unwrap();

	// Assert
	assert_eq!(ancestors.len(), 0);
}

#[rstest]
#[tokio::test]
async fn test_get_children_of_nonexistent_page() {
	// Arrange
	let tree = PageTree::new();
	let fake_id = Uuid::new_v4();

	// Act
	let children = tree.get_children(fake_id).await.unwrap();

	// Assert
	assert_eq!(children.len(), 0);
}

// --- Sanity ---

#[rstest]
#[tokio::test]
async fn test_page_tree_default_trait() {
	// Arrange
	let mut tree = PageTree::default();

	// Act
	let page = tree
		.add_page(None, "Default".to_string(), "default".to_string())
		.await
		.unwrap();

	// Assert
	assert_eq!(page.title, "Default");
	assert_eq!(page.slug, "default");
	assert_eq!(page.depth, 0);
}

#[rstest]
#[tokio::test]
async fn test_page_node_serialization_roundtrip() {
	// Arrange
	let mut tree = PageTree::new();
	let original = tree
		.add_page(None, "Serde".to_string(), "serde".to_string())
		.await
		.unwrap();

	// Act
	let json = serde_json::to_string(&original).unwrap();
	let deserialized: PageNode = serde_json::from_str(&json).unwrap();

	// Assert
	assert_eq!(deserialized.id, original.id);
	assert_eq!(deserialized.parent_id, original.parent_id);
	assert_eq!(deserialized.title, original.title);
	assert_eq!(deserialized.slug, original.slug);
	assert_eq!(deserialized.path, original.path);
	assert_eq!(deserialized.depth, original.depth);
	assert_eq!(deserialized.sort_order, original.sort_order);
	assert_eq!(deserialized.is_published, original.is_published);
	assert_eq!(deserialized.created_at, original.created_at);
	assert_eq!(deserialized.updated_at, original.updated_at);
}

// --- Combination ---

#[rstest]
#[tokio::test]
async fn test_delete_parent_non_recursive_orphans_children() {
	// Arrange
	let mut tree = PageTree::new();
	let parent = tree
		.add_page(None, "Parent".to_string(), "parent".to_string())
		.await
		.unwrap();
	let child1 = tree
		.add_page(Some(parent.id), "Child1".to_string(), "child1".to_string())
		.await
		.unwrap();
	let child2 = tree
		.add_page(Some(parent.id), "Child2".to_string(), "child2".to_string())
		.await
		.unwrap();

	// Act
	tree.delete_page(parent.id, false).await.unwrap();

	// Assert - children are orphaned but still accessible
	let orphans = tree.get_children(parent.id).await.unwrap();
	assert_eq!(orphans.len(), 2);
	let orphan_ids: Vec<_> = orphans.iter().map(|p| p.id).collect();
	assert!(orphan_ids.contains(&child1.id));
	assert!(orphan_ids.contains(&child2.id));
}

#[rstest]
#[tokio::test]
async fn test_move_page_verify_old_parent_children_updated() {
	// Arrange
	let mut tree = PageTree::new();
	let old_parent = tree
		.add_page(None, "OldParent".to_string(), "old-parent".to_string())
		.await
		.unwrap();
	let new_parent = tree
		.add_page(None, "NewParent".to_string(), "new-parent".to_string())
		.await
		.unwrap();
	let child = tree
		.add_page(
			Some(old_parent.id),
			"Child".to_string(),
			"child".to_string(),
		)
		.await
		.unwrap();

	// Act
	tree.move_page(child.id, Some(new_parent.id)).await.unwrap();

	// Assert
	let old_children = tree.get_children(old_parent.id).await.unwrap();
	assert_eq!(old_children.len(), 0);
	let new_children = tree.get_children(new_parent.id).await.unwrap();
	assert_eq!(new_children.len(), 1);
	assert_eq!(new_children[0].id, child.id);
}

// --- Boundary Value ---

#[rstest]
#[case(1)]
#[case(5)]
#[case(10)]
#[case(50)]
#[tokio::test]
async fn test_page_tree_depth_boundaries(#[case] depth: usize) {
	// Arrange
	let mut tree = PageTree::new();
	let mut parent_id: Option<uuid::Uuid> = None;
	let mut last_page = None;

	// Act
	for i in 0..depth {
		let slug = format!("level{}", i);
		let page = tree.add_page(parent_id, slug.clone(), slug).await.unwrap();
		parent_id = Some(page.id);
		last_page = Some(page);
	}

	// Assert
	let deepest = last_page.unwrap();
	assert_eq!(deepest.depth as usize, depth - 1);
}

#[rstest]
#[case(0)]
#[case(1)]
#[case(255)]
#[case(1000)]
#[tokio::test]
async fn test_page_slug_length_boundaries(#[case] length: usize) {
	// Arrange
	let mut tree = PageTree::new();
	let slug = "a".repeat(length);

	// Act
	let page = tree
		.add_page(None, "Title".to_string(), slug.clone())
		.await
		.unwrap();

	// Assert
	assert_eq!(page.slug.len(), length);
	assert_eq!(page.slug, slug);
}

#[rstest]
#[case(0)]
#[case(1)]
#[case(10)]
#[case(100)]
#[tokio::test]
async fn test_children_count_boundaries(#[case] count: usize) {
	// Arrange
	let mut tree = PageTree::new();
	let parent = tree
		.add_page(None, "Parent".to_string(), "parent".to_string())
		.await
		.unwrap();

	// Act
	for i in 0..count {
		let slug = format!("child{}", i);
		tree.add_page(Some(parent.id), slug.clone(), slug)
			.await
			.unwrap();
	}

	// Assert
	let children = tree.get_children(parent.id).await.unwrap();
	assert_eq!(children.len(), count);
}

// --- Decision Table ---

#[rstest]
#[case(true, true, false)]
#[case(true, false, true)]
#[case(false, true, true)]
#[case(false, false, true)]
#[tokio::test]
async fn test_move_page_validation_decision_table(
	#[case] page_exists: bool,
	#[case] parent_exists: bool,
	#[case] expect_error: bool,
) {
	// Arrange
	let mut tree = PageTree::new();
	let real_page = tree
		.add_page(None, "Page".to_string(), "page".to_string())
		.await
		.unwrap();
	let real_parent = tree
		.add_page(None, "Parent".to_string(), "parent".to_string())
		.await
		.unwrap();

	let page_id = if page_exists {
		real_page.id
	} else {
		Uuid::new_v4()
	};
	let new_parent_id = if parent_exists {
		Some(real_parent.id)
	} else {
		Some(Uuid::new_v4())
	};

	// Act
	let result = tree.move_page(page_id, new_parent_id).await;

	// Assert
	assert_eq!(result.is_err(), expect_error);
	if let Err(err) = result {
		assert!(matches!(err, CmsError::PageNotFound(_)));
	}
}

#[rstest]
#[case(0, "home", "/home")]
#[case(1, "about", "/root/about")]
#[case(2, "post", "/root/child/post")]
#[tokio::test]
async fn test_page_path_calculation_decision_table(
	#[case] parent_depth: usize,
	#[case] slug: &str,
	#[case] expected_path: &str,
) {
	// Arrange
	let mut tree = PageTree::new();
	let ancestor_slugs = ["root", "child", "grandchild"];
	let mut parent_id: Option<uuid::Uuid> = None;
	for ancestor_slug in ancestor_slugs.iter().take(parent_depth) {
		let page = tree
			.add_page(
				parent_id,
				ancestor_slug.to_string(),
				ancestor_slug.to_string(),
			)
			.await
			.unwrap();
		parent_id = Some(page.id);
	}

	// Act
	let page = tree
		.add_page(parent_id, slug.to_string(), slug.to_string())
		.await
		.unwrap();

	// Assert
	assert_eq!(page.path, expected_path);
}

#[rstest]
#[case(false, false, 0)]
#[case(false, true, 0)]
#[case(true, false, 2)]
#[case(true, true, 0)]
#[tokio::test]
async fn test_delete_page_behavior_decision_table(
	#[case] has_children: bool,
	#[case] recursive: bool,
	#[case] expected_remaining_children: usize,
) {
	// Arrange
	let mut tree = PageTree::new();
	let parent = tree
		.add_page(None, "Parent".to_string(), "parent".to_string())
		.await
		.unwrap();
	if has_children {
		tree.add_page(Some(parent.id), "Child1".to_string(), "child1".to_string())
			.await
			.unwrap();
		tree.add_page(Some(parent.id), "Child2".to_string(), "child2".to_string())
			.await
			.unwrap();
	}

	// Act
	tree.delete_page(parent.id, recursive).await.unwrap();

	// Assert
	let remaining = tree.get_children(parent.id).await.unwrap();
	assert_eq!(remaining.len(), expected_remaining_children);
}
