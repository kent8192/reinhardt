//! Page tree and hierarchical page management
//!
//! This module provides hierarchical page models inspired by Wagtail's page tree.
//! Pages can have parent-child relationships and automatic URL routing based on hierarchy.

use crate::error::{CmsError, CmsResult};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Page identifier
pub type PageId = Uuid;

/// Represents a page node in the CMS tree
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageNode {
	/// Unique page identifier
	pub id: PageId,

	/// Parent page ID (None for root pages)
	pub parent_id: Option<PageId>,

	/// Page title
	pub title: String,

	/// URL slug (used for routing)
	pub slug: String,

	/// Full path from root (e.g., "/blog/2024/my-post")
	pub path: String,

	/// Tree depth (0 for root pages)
	pub depth: u32,

	/// Sort order among siblings
	pub sort_order: i32,

	/// Is this page published?
	pub is_published: bool,

	/// Creation timestamp
	pub created_at: chrono::DateTime<chrono::Utc>,

	/// Last update timestamp
	pub updated_at: chrono::DateTime<chrono::Utc>,
}

/// Page trait that all CMS pages must implement
pub trait Page: Send + Sync {
	/// Get page ID
	fn id(&self) -> PageId;

	/// Get parent page ID
	fn parent_id(&self) -> Option<PageId>;

	/// Get page title
	fn title(&self) -> &str;

	/// Get page slug
	fn slug(&self) -> &str;

	/// Get full path
	fn path(&self) -> &str;

	/// Render this page to HTML
	fn render(&self) -> CmsResult<String>;
}

use std::collections::HashMap;

/// Page tree manager for hierarchical operations
pub struct PageTree {
	pages: HashMap<PageId, PageNode>,
}

impl PageTree {
	/// Create a new page tree manager
	pub fn new() -> Self {
		Self {
			pages: HashMap::new(),
		}
	}

	/// Add a new page to the tree
	pub async fn add_page(
		&mut self,
		parent_id: Option<PageId>,
		title: String,
		slug: String,
	) -> CmsResult<PageNode> {
		let id = Uuid::new_v4();

		// Calculate depth and path
		let (depth, path) = if let Some(parent_id) = parent_id {
			let parent = self
				.pages
				.get(&parent_id)
				.ok_or_else(|| CmsError::PageNotFound(format!("{}", parent_id)))?;
			(parent.depth + 1, format!("{}/{}", parent.path, slug))
		} else {
			(0, format!("/{}", slug))
		};

		let page = PageNode {
			id,
			parent_id,
			title,
			slug,
			path,
			depth,
			sort_order: 0,
			is_published: false,
			created_at: chrono::Utc::now(),
			updated_at: chrono::Utc::now(),
		};

		self.pages.insert(id, page.clone());
		Ok(page)
	}

	/// Move a page to a new parent
	pub async fn move_page(
		&mut self,
		page_id: PageId,
		new_parent_id: Option<PageId>,
	) -> CmsResult<()> {
		// Check if page exists
		if !self.pages.contains_key(&page_id) {
			return Err(CmsError::PageNotFound(format!("{}", page_id)));
		}

		// Check if new parent exists (if specified)
		if let Some(new_parent_id) = new_parent_id
			&& !self.pages.contains_key(&new_parent_id)
		{
			return Err(CmsError::PageNotFound(format!("{}", new_parent_id)));
		}

		// Get page slug before mutable borrow
		let page_slug = self.pages.get(&page_id).unwrap().slug.clone();

		// Recalculate path and depth
		let (new_depth, new_path) = if let Some(parent_id) = new_parent_id {
			let parent = self.pages.get(&parent_id).unwrap();
			(parent.depth + 1, format!("{}/{}", parent.path, page_slug))
		} else {
			(0, format!("/{}", page_slug))
		};

		// Update page's parent, path, and depth
		let page = self.pages.get_mut(&page_id).unwrap();
		page.parent_id = new_parent_id;
		page.depth = new_depth;
		page.path = new_path;
		page.updated_at = chrono::Utc::now();

		Ok(())
	}

	/// Get all children of a page
	pub async fn get_children(&self, parent_id: PageId) -> CmsResult<Vec<PageNode>> {
		let children: Vec<PageNode> = self
			.pages
			.values()
			.filter(|p| p.parent_id == Some(parent_id))
			.cloned()
			.collect();
		Ok(children)
	}

	/// Get ancestors of a page (breadcrumb trail)
	pub async fn get_ancestors(&self, page_id: PageId) -> CmsResult<Vec<PageNode>> {
		let mut ancestors = Vec::new();
		let mut current_id = page_id;

		while let Some(page) = self.pages.get(&current_id) {
			if let Some(parent_id) = page.parent_id {
				if let Some(parent) = self.pages.get(&parent_id) {
					ancestors.push(parent.clone());
					current_id = parent_id;
				} else {
					break;
				}
			} else {
				break;
			}
		}

		ancestors.reverse();
		Ok(ancestors)
	}

	/// Delete a page and optionally its children
	pub async fn delete_page(&mut self, page_id: PageId, recursive: bool) -> CmsResult<()> {
		if !self.pages.contains_key(&page_id) {
			return Err(CmsError::PageNotFound(format!("{}", page_id)));
		}

		if recursive {
			// Collect all descendant IDs first
			let descendants = self.get_descendants(page_id);
			for descendant_id in descendants {
				self.pages.remove(&descendant_id);
			}
		}

		self.pages.remove(&page_id);
		Ok(())
	}

	// Helper method to get all descendants of a page
	fn get_descendants(&self, page_id: PageId) -> Vec<PageId> {
		let mut descendants = Vec::new();
		let children: Vec<PageId> = self
			.pages
			.values()
			.filter(|p| p.parent_id == Some(page_id))
			.map(|p| p.id)
			.collect();

		for child_id in children {
			descendants.push(child_id);
			descendants.extend(self.get_descendants(child_id));
		}

		descendants
	}
}

impl Default for PageTree {
	fn default() -> Self {
		Self::new()
	}
}
