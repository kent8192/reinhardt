//! Page-level permissions and access control
//!
//! Fine-grained permissions system for pages, inspired by Wagtail's permission model.

use crate::error::{CmsError, CmsResult};
use crate::pages::PageId;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// User identifier
pub type UserId = Uuid;

/// Group identifier
pub type GroupId = Uuid;

/// Permission type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PermissionType {
	/// Can view page
	View,

	/// Can edit page
	Edit,

	/// Can publish page
	Publish,

	/// Can delete page
	Delete,

	/// Can add child pages
	AddChild,

	/// Can manage permissions
	ManagePermissions,
}

/// Page permission entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PagePermission {
	/// Permission ID
	pub id: Uuid,

	/// Page this permission applies to
	pub page_id: PageId,

	/// User or group this permission is granted to
	pub principal: Principal,

	/// Permission type
	pub permission: PermissionType,

	/// Does this permission apply to descendants?
	pub recursive: bool,
}

/// Principal (user or group) that can be granted permissions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Principal {
	/// Individual user
	User(UserId),

	/// Group of users
	Group(GroupId),

	/// All authenticated users
	Authenticated,

	/// Anyone (including anonymous)
	Anyone,
}

/// Permission checker
pub struct PermissionChecker {
	/// Storage for page permissions
	permissions: std::collections::HashMap<Uuid, PagePermission>,
}

impl PermissionChecker {
	/// Create a new permission checker
	pub fn new() -> Self {
		Self {
			permissions: std::collections::HashMap::new(),
		}
	}

	/// Check if a user has a specific permission on a page
	pub async fn check_permission(
		&self,
		user_id: UserId,
		page_id: PageId,
		permission: PermissionType,
	) -> CmsResult<bool> {
		// Check if any permission matches
		for perm in self.permissions.values() {
			if perm.page_id == page_id && perm.permission == permission {
				match &perm.principal {
					Principal::User(uid) => {
						if *uid == user_id {
							return Ok(true);
						}
					}
					Principal::Group(_) => {
						// TODO: Check if user is member of the group
						// For now, skip group checking
					}
					Principal::Authenticated => {
						// Any authenticated user has this permission
						return Ok(true);
					}
					Principal::Anyone => {
						// Anyone (including anonymous) has this permission
						return Ok(true);
					}
				}
			}
		}

		Ok(false)
	}

	/// Grant a permission
	pub async fn grant_permission(
		&mut self,
		page_id: PageId,
		principal: Principal,
		permission: PermissionType,
		recursive: bool,
	) -> CmsResult<PagePermission> {
		let id = Uuid::new_v4();

		let page_permission = PagePermission {
			id,
			page_id,
			principal,
			permission,
			recursive,
		};

		self.permissions.insert(id, page_permission.clone());

		Ok(page_permission)
	}

	/// Revoke a permission
	pub async fn revoke_permission(&mut self, permission_id: Uuid) -> CmsResult<()> {
		self.permissions
			.remove(&permission_id)
			.ok_or_else(|| CmsError::PermissionDenied("Permission not found".to_string()))?;

		Ok(())
	}

	/// Get all permissions for a page
	pub async fn get_page_permissions(&self, page_id: PageId) -> CmsResult<Vec<PagePermission>> {
		Ok(self
			.permissions
			.values()
			.filter(|perm| perm.page_id == page_id)
			.cloned()
			.collect())
	}
}

impl Default for PermissionChecker {
	fn default() -> Self {
		Self::new()
	}
}
