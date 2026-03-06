//! Property-based tests for page tree functionality

use proptest::prelude::*;
use reinhardt_cms::pages::PageTree;
use uuid::Uuid;

proptest! {
	#[test]
	fn prop_page_root_depth_always_zero(slug in "[a-z]{1,50}") {
		let rt = tokio::runtime::Runtime::new().unwrap();

		// Arrange & Act
		let page = rt.block_on(async {
			let mut tree = PageTree::new();
			tree.add_page(None, slug.clone(), slug).await.unwrap()
		});

		// Assert
		prop_assert_eq!(page.depth, 0);
	}

	#[test]
	fn prop_page_child_depth_equals_parent_plus_one(
		parent_slug in "[a-z]{1,50}",
		child_slug in "[a-z]{1,50}",
	) {
		let rt = tokio::runtime::Runtime::new().unwrap();

		// Arrange & Act
		let (parent, child) = rt.block_on(async {
			let mut tree = PageTree::new();
			let parent = tree
				.add_page(None, parent_slug.clone(), parent_slug)
				.await
				.unwrap();
			let child = tree
				.add_page(Some(parent.id), child_slug.clone(), child_slug)
				.await
				.unwrap();
			(parent, child)
		});

		// Assert
		prop_assert_eq!(child.depth, parent.depth + 1);
	}

	#[test]
	fn prop_page_path_starts_with_slash(slug in "[a-z]{1,50}") {
		let rt = tokio::runtime::Runtime::new().unwrap();

		// Arrange & Act
		let page = rt.block_on(async {
			let mut tree = PageTree::new();
			tree.add_page(None, slug.clone(), slug).await.unwrap()
		});

		// Assert
		prop_assert!(page.path.starts_with('/'));
	}

	#[test]
	fn prop_page_path_ends_with_slug(slug in "[a-z]{1,50}") {
		let rt = tokio::runtime::Runtime::new().unwrap();

		// Arrange & Act
		let page = rt.block_on(async {
			let mut tree = PageTree::new();
			tree.add_page(None, slug.clone(), slug).await.unwrap()
		});

		// Assert
		prop_assert!(page.path.ends_with(&page.slug));
	}

	#[test]
	fn fuzz_page_tree_add_random_slugs(slugs in proptest::collection::vec(".*", 1..20)) {
		let rt = tokio::runtime::Runtime::new().unwrap();

		// Arrange, Act, Assert - verify arbitrary slugs never cause panics
		rt.block_on(async {
			let mut tree = PageTree::new();
			for slug in slugs {
				let _ = tree.add_page(None, slug.clone(), slug).await;
			}
		});
	}

	#[test]
	fn fuzz_page_tree_operations_sequence(ops in proptest::collection::vec(0..3u8, 1..30)) {
		let rt = tokio::runtime::Runtime::new().unwrap();

		// Arrange, Act, Assert - verify random operation sequences never panic
		rt.block_on(async {
			let mut tree = PageTree::new();
			let mut page_ids: Vec<Uuid> = Vec::new();

			for op in ops {
				match op {
					0 => {
						// Add page with optional parent
						let parent = if page_ids.is_empty() {
							None
						} else {
							Some(page_ids[0])
						};
						if let Ok(page) = tree
							.add_page(parent, "p".to_string(), "p".to_string())
							.await
						{
							page_ids.push(page.id);
						}
					}
					1 => {
						// Delete a page non-recursively
						if let Some(&id) = page_ids.first() {
							let _ = tree.delete_page(id, false).await;
							page_ids.remove(0);
						}
					}
					_ => {
						// Move a page to another
						if page_ids.len() >= 2 {
							let _ = tree.move_page(page_ids[0], Some(page_ids[1])).await;
						}
					}
				}
			}
		});
	}
}
