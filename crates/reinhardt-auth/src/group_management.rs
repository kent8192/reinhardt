//! Group Management
//!
//! Provides group management and permission assignment functionality.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

/// Group management error
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GroupManagementError {
	/// Group not found
	GroupNotFound,
	/// Group already exists
	GroupAlreadyExists,
	/// Invalid group name
	InvalidGroupName,
	/// User not found
	UserNotFound,
	/// Other error
	Other(String),
}

impl std::fmt::Display for GroupManagementError {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			GroupManagementError::GroupNotFound => write!(f, "Group not found"),
			GroupManagementError::GroupAlreadyExists => write!(f, "Group already exists"),
			GroupManagementError::InvalidGroupName => write!(f, "Invalid group name"),
			GroupManagementError::UserNotFound => write!(f, "User not found"),
			GroupManagementError::Other(msg) => write!(f, "Error: {}", msg),
		}
	}
}

impl std::error::Error for GroupManagementError {}

/// Group management result
pub type GroupManagementResult<T> = Result<T, GroupManagementError>;

/// User group
///
/// # Examples
///
/// ```
/// use reinhardt_auth::group_management::Group;
/// use uuid::Uuid;
///
/// let group = Group {
///     id: Uuid::new_v4(),
///     name: "Editors".to_string(),
///     description: Some("Content editors".to_string()),
/// };
///
/// assert_eq!(group.name, "Editors");
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Group {
	pub id: Uuid,
	pub name: String,
	pub description: Option<String>,
}

/// Group creation data
///
/// # Examples
///
/// ```
/// use reinhardt_auth::group_management::CreateGroupData;
///
/// let data = CreateGroupData {
///     name: "Admins".to_string(),
///     description: Some("System administrators".to_string()),
/// };
///
/// assert_eq!(data.name, "Admins");
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateGroupData {
	pub name: String,
	pub description: Option<String>,
}

/// Group manager
///
/// Manages groups and their permissions.
///
/// # Examples
///
/// ```
/// use reinhardt_auth::group_management::{GroupManager, CreateGroupData};
///
/// #[tokio::main]
/// async fn main() {
///     let mut manager = GroupManager::new();
///
///     // Create group
///     let group_data = CreateGroupData {
///         name: "Editors".to_string(),
///         description: Some("Content editors".to_string()),
///     };
///
///     let group = manager.create_group(group_data).await.unwrap();
///     assert_eq!(group.name, "Editors");
///
///     // Add permissions to group
///     manager.add_group_permission(&group.id.to_string(), "blog.add_article").await.unwrap();
///     manager.add_group_permission(&group.id.to_string(), "blog.change_article").await.unwrap();
///
///     // Add user to group
///     manager.add_user_to_group("alice", &group.id.to_string()).await.unwrap();
///
///     // Check user permissions
///     let perms = manager.get_user_permissions("alice").await.unwrap();
///     assert!(perms.contains(&"blog.add_article".to_string()));
/// }
/// ```
pub struct GroupManager {
	groups: Arc<RwLock<HashMap<Uuid, Group>>>,
	group_index: Arc<RwLock<HashMap<String, Uuid>>>,
	group_permissions: Arc<RwLock<HashMap<Uuid, HashSet<String>>>>,
	user_groups: Arc<RwLock<HashMap<String, HashSet<Uuid>>>>,
}

impl GroupManager {
	/// Create a new group manager
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_auth::group_management::GroupManager;
	///
	/// let manager = GroupManager::new();
	/// ```
	pub fn new() -> Self {
		Self {
			groups: Arc::new(RwLock::new(HashMap::new())),
			group_index: Arc::new(RwLock::new(HashMap::new())),
			group_permissions: Arc::new(RwLock::new(HashMap::new())),
			user_groups: Arc::new(RwLock::new(HashMap::new())),
		}
	}

	/// Create a new group
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_auth::group_management::{GroupManager, CreateGroupData};
	///
	/// #[tokio::main]
	/// async fn main() {
	///     let mut manager = GroupManager::new();
	///
	///     let group_data = CreateGroupData {
	///         name: "Moderators".to_string(),
	///         description: Some("Content moderators".to_string()),
	///     };
	///
	///     let group = manager.create_group(group_data).await.unwrap();
	///     assert_eq!(group.name, "Moderators");
	/// }
	/// ```
	pub async fn create_group(&mut self, data: CreateGroupData) -> GroupManagementResult<Group> {
		// Validate group name
		if data.name.is_empty() || data.name.len() < 2 {
			return Err(GroupManagementError::InvalidGroupName);
		}

		// Check if group already exists
		let group_index = self.group_index.read().await;
		if group_index.contains_key(&data.name) {
			return Err(GroupManagementError::GroupAlreadyExists);
		}
		drop(group_index);

		// Create group
		let group = Group {
			id: Uuid::new_v4(),
			name: data.name.clone(),
			description: data.description,
		};

		// Store group
		let mut groups = self.groups.write().await;
		let mut group_index = self.group_index.write().await;

		groups.insert(group.id, group.clone());
		group_index.insert(data.name, group.id);

		Ok(group)
	}

	/// Get group by ID
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_auth::group_management::{GroupManager, CreateGroupData};
	///
	/// #[tokio::main]
	/// async fn main() {
	///     let mut manager = GroupManager::new();
	///
	///     let group_data = CreateGroupData {
	///         name: "Viewers".to_string(),
	///         description: None,
	///     };
	///
	///     let group = manager.create_group(group_data).await.unwrap();
	///     let retrieved = manager.get_group(&group.id.to_string()).await.unwrap();
	///     assert_eq!(retrieved.name, "Viewers");
	/// }
	/// ```
	pub async fn get_group(&self, group_id: &str) -> GroupManagementResult<Group> {
		let uuid = Uuid::parse_str(group_id)
			.map_err(|_| GroupManagementError::Other("Invalid UUID".to_string()))?;

		let groups = self.groups.read().await;
		groups
			.get(&uuid)
			.cloned()
			.ok_or(GroupManagementError::GroupNotFound)
	}

	/// Get group by name
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_auth::group_management::{GroupManager, CreateGroupData};
	///
	/// #[tokio::main]
	/// async fn main() {
	///     let mut manager = GroupManager::new();
	///
	///     let group_data = CreateGroupData {
	///         name: "Contributors".to_string(),
	///         description: None,
	///     };
	///
	///     manager.create_group(group_data).await.unwrap();
	///     let retrieved = manager.get_group_by_name("Contributors").await.unwrap();
	///     assert_eq!(retrieved.name, "Contributors");
	/// }
	/// ```
	pub async fn get_group_by_name(&self, name: &str) -> GroupManagementResult<Group> {
		let group_index = self.group_index.read().await;
		let group_id = group_index
			.get(name)
			.ok_or(GroupManagementError::GroupNotFound)?;

		let groups = self.groups.read().await;
		groups
			.get(group_id)
			.cloned()
			.ok_or(GroupManagementError::GroupNotFound)
	}

	/// Delete group
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_auth::group_management::{GroupManager, CreateGroupData};
	///
	/// #[tokio::main]
	/// async fn main() {
	///     let mut manager = GroupManager::new();
	///
	///     let group_data = CreateGroupData {
	///         name: "TempGroup".to_string(),
	///         description: None,
	///     };
	///
	///     let group = manager.create_group(group_data).await.unwrap();
	///     manager.delete_group(&group.id.to_string()).await.unwrap();
	///     assert!(manager.get_group(&group.id.to_string()).await.is_err());
	/// }
	/// ```
	pub async fn delete_group(&mut self, group_id: &str) -> GroupManagementResult<()> {
		let uuid = Uuid::parse_str(group_id)
			.map_err(|_| GroupManagementError::Other("Invalid UUID".to_string()))?;

		let mut groups = self.groups.write().await;
		let group = groups
			.get(&uuid)
			.ok_or(GroupManagementError::GroupNotFound)?
			.clone();

		let mut group_index = self.group_index.write().await;
		let mut group_permissions = self.group_permissions.write().await;
		let mut user_groups = self.user_groups.write().await;

		groups.remove(&uuid);
		group_index.remove(&group.name);
		group_permissions.remove(&uuid);

		// Remove group from all users
		for user_group_set in user_groups.values_mut() {
			user_group_set.remove(&uuid);
		}

		Ok(())
	}

	/// Add permission to group
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_auth::group_management::{GroupManager, CreateGroupData};
	///
	/// #[tokio::main]
	/// async fn main() {
	///     let mut manager = GroupManager::new();
	///
	///     let group_data = CreateGroupData {
	///         name: "Writers".to_string(),
	///         description: None,
	///     };
	///
	///     let group = manager.create_group(group_data).await.unwrap();
	///     manager.add_group_permission(&group.id.to_string(), "blog.add_article").await.unwrap();
	///
	///     let perms = manager.get_group_permissions(&group.id.to_string()).await.unwrap();
	///     assert!(perms.contains(&"blog.add_article".to_string()));
	/// }
	/// ```
	pub async fn add_group_permission(
		&mut self,
		group_id: &str,
		permission: &str,
	) -> GroupManagementResult<()> {
		let uuid = Uuid::parse_str(group_id)
			.map_err(|_| GroupManagementError::Other("Invalid UUID".to_string()))?;

		// Check if group exists
		let groups = self.groups.read().await;
		if !groups.contains_key(&uuid) {
			return Err(GroupManagementError::GroupNotFound);
		}
		drop(groups);

		let mut group_permissions = self.group_permissions.write().await;
		group_permissions
			.entry(uuid)
			.or_default()
			.insert(permission.to_string());

		Ok(())
	}

	/// Remove permission from group
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_auth::group_management::{GroupManager, CreateGroupData};
	///
	/// #[tokio::main]
	/// async fn main() {
	///     let mut manager = GroupManager::new();
	///
	///     let group_data = CreateGroupData {
	///         name: "Reviewers".to_string(),
	///         description: None,
	///     };
	///
	///     let group = manager.create_group(group_data).await.unwrap();
	///     manager.add_group_permission(&group.id.to_string(), "blog.view_article").await.unwrap();
	///     manager.remove_group_permission(&group.id.to_string(), "blog.view_article").await.unwrap();
	///
	///     let perms = manager.get_group_permissions(&group.id.to_string()).await.unwrap();
	///     assert!(!perms.contains(&"blog.view_article".to_string()));
	/// }
	/// ```
	pub async fn remove_group_permission(
		&mut self,
		group_id: &str,
		permission: &str,
	) -> GroupManagementResult<()> {
		let uuid = Uuid::parse_str(group_id)
			.map_err(|_| GroupManagementError::Other("Invalid UUID".to_string()))?;

		let mut group_permissions = self.group_permissions.write().await;
		if let Some(perms) = group_permissions.get_mut(&uuid) {
			perms.remove(permission);
		}

		Ok(())
	}

	/// Get group permissions
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_auth::group_management::{GroupManager, CreateGroupData};
	///
	/// #[tokio::main]
	/// async fn main() {
	///     let mut manager = GroupManager::new();
	///
	///     let group_data = CreateGroupData {
	///         name: "Publishers".to_string(),
	///         description: None,
	///     };
	///
	///     let group = manager.create_group(group_data).await.unwrap();
	///     manager.add_group_permission(&group.id.to_string(), "blog.add_article").await.unwrap();
	///     manager.add_group_permission(&group.id.to_string(), "blog.publish_article").await.unwrap();
	///
	///     let perms = manager.get_group_permissions(&group.id.to_string()).await.unwrap();
	///     assert_eq!(perms.len(), 2);
	/// }
	/// ```
	pub async fn get_group_permissions(
		&self,
		group_id: &str,
	) -> GroupManagementResult<Vec<String>> {
		let uuid = Uuid::parse_str(group_id)
			.map_err(|_| GroupManagementError::Other("Invalid UUID".to_string()))?;

		let group_permissions = self.group_permissions.read().await;
		Ok(group_permissions
			.get(&uuid)
			.map(|perms| perms.iter().cloned().collect())
			.unwrap_or_default())
	}

	/// Add user to group
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_auth::group_management::{GroupManager, CreateGroupData};
	///
	/// #[tokio::main]
	/// async fn main() {
	///     let mut manager = GroupManager::new();
	///
	///     let group_data = CreateGroupData {
	///         name: "Members".to_string(),
	///         description: None,
	///     };
	///
	///     let group = manager.create_group(group_data).await.unwrap();
	///     manager.add_user_to_group("alice", &group.id.to_string()).await.unwrap();
	///
	///     let groups = manager.get_user_groups("alice").await.unwrap();
	///     assert_eq!(groups.len(), 1);
	/// }
	/// ```
	pub async fn add_user_to_group(
		&mut self,
		username: &str,
		group_id: &str,
	) -> GroupManagementResult<()> {
		let uuid = Uuid::parse_str(group_id)
			.map_err(|_| GroupManagementError::Other("Invalid UUID".to_string()))?;

		// Check if group exists
		let groups = self.groups.read().await;
		if !groups.contains_key(&uuid) {
			return Err(GroupManagementError::GroupNotFound);
		}
		drop(groups);

		let mut user_groups = self.user_groups.write().await;
		user_groups
			.entry(username.to_string())
			.or_default()
			.insert(uuid);

		Ok(())
	}

	/// Remove user from group
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_auth::group_management::{GroupManager, CreateGroupData};
	///
	/// #[tokio::main]
	/// async fn main() {
	///     let mut manager = GroupManager::new();
	///
	///     let group_data = CreateGroupData {
	///         name: "Staff".to_string(),
	///         description: None,
	///     };
	///
	///     let group = manager.create_group(group_data).await.unwrap();
	///     manager.add_user_to_group("bob", &group.id.to_string()).await.unwrap();
	///     manager.remove_user_from_group("bob", &group.id.to_string()).await.unwrap();
	///
	///     let groups = manager.get_user_groups("bob").await.unwrap();
	///     assert_eq!(groups.len(), 0);
	/// }
	/// ```
	pub async fn remove_user_from_group(
		&mut self,
		username: &str,
		group_id: &str,
	) -> GroupManagementResult<()> {
		let uuid = Uuid::parse_str(group_id)
			.map_err(|_| GroupManagementError::Other("Invalid UUID".to_string()))?;

		let mut user_groups = self.user_groups.write().await;
		if let Some(groups) = user_groups.get_mut(username) {
			groups.remove(&uuid);
		}

		Ok(())
	}

	/// Get user groups
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_auth::group_management::{GroupManager, CreateGroupData};
	///
	/// #[tokio::main]
	/// async fn main() {
	///     let mut manager = GroupManager::new();
	///
	///     let group_data1 = CreateGroupData {
	///         name: "Team1".to_string(),
	///         description: None,
	///     };
	///     let group_data2 = CreateGroupData {
	///         name: "Team2".to_string(),
	///         description: None,
	///     };
	///
	///     let group1 = manager.create_group(group_data1).await.unwrap();
	///     let group2 = manager.create_group(group_data2).await.unwrap();
	///
	///     manager.add_user_to_group("charlie", &group1.id.to_string()).await.unwrap();
	///     manager.add_user_to_group("charlie", &group2.id.to_string()).await.unwrap();
	///
	///     let groups = manager.get_user_groups("charlie").await.unwrap();
	///     assert_eq!(groups.len(), 2);
	/// }
	/// ```
	pub async fn get_user_groups(&self, username: &str) -> GroupManagementResult<Vec<Group>> {
		let user_groups = self.user_groups.read().await;
		let group_ids = user_groups.get(username).cloned().unwrap_or_default();

		let groups = self.groups.read().await;
		Ok(group_ids
			.iter()
			.filter_map(|id| groups.get(id).cloned())
			.collect())
	}

	/// Get user permissions from all groups
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_auth::group_management::{GroupManager, CreateGroupData};
	///
	/// #[tokio::main]
	/// async fn main() {
	///     let mut manager = GroupManager::new();
	///
	///     let group_data = CreateGroupData {
	///         name: "Developers".to_string(),
	///         description: None,
	///     };
	///
	///     let group = manager.create_group(group_data).await.unwrap();
	///     manager.add_group_permission(&group.id.to_string(), "code.commit").await.unwrap();
	///     manager.add_group_permission(&group.id.to_string(), "code.review").await.unwrap();
	///     manager.add_user_to_group("diana", &group.id.to_string()).await.unwrap();
	///
	///     let perms = manager.get_user_permissions("diana").await.unwrap();
	///     assert!(perms.contains(&"code.commit".to_string()));
	///     assert!(perms.contains(&"code.review".to_string()));
	/// }
	/// ```
	pub async fn get_user_permissions(&self, username: &str) -> GroupManagementResult<Vec<String>> {
		let user_groups = self.user_groups.read().await;
		let group_ids = user_groups.get(username).cloned().unwrap_or_default();

		let group_permissions = self.group_permissions.read().await;
		let mut all_permissions = HashSet::new();

		for group_id in group_ids {
			if let Some(perms) = group_permissions.get(&group_id) {
				all_permissions.extend(perms.clone());
			}
		}

		Ok(all_permissions.into_iter().collect())
	}

	/// List all groups
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_auth::group_management::{GroupManager, CreateGroupData};
	///
	/// #[tokio::main]
	/// async fn main() {
	///     let mut manager = GroupManager::new();
	///
	///     let group_data1 = CreateGroupData {
	///         name: "Group1".to_string(),
	///         description: None,
	///     };
	///     let group_data2 = CreateGroupData {
	///         name: "Group2".to_string(),
	///         description: None,
	///     };
	///
	///     manager.create_group(group_data1).await.unwrap();
	///     manager.create_group(group_data2).await.unwrap();
	///
	///     let groups = manager.list_groups().await;
	///     assert_eq!(groups.len(), 2);
	/// }
	/// ```
	pub async fn list_groups(&self) -> Vec<Group> {
		let groups = self.groups.read().await;
		groups.values().cloned().collect()
	}
}

impl Default for GroupManager {
	fn default() -> Self {
		Self::new()
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[tokio::test]
	async fn test_create_group() {
		let mut manager = GroupManager::new();

		let group_data = CreateGroupData {
			name: "Editors".to_string(),
			description: Some("Content editors".to_string()),
		};

		let group = manager.create_group(group_data).await.unwrap();
		assert_eq!(group.name, "Editors");
		assert_eq!(group.description, Some("Content editors".to_string()));
	}

	#[tokio::test]
	async fn test_get_group() {
		let mut manager = GroupManager::new();

		let group_data = CreateGroupData {
			name: "Moderators".to_string(),
			description: None,
		};

		let group = manager.create_group(group_data).await.unwrap();
		let retrieved = manager.get_group(&group.id.to_string()).await.unwrap();
		assert_eq!(retrieved.name, "Moderators");
	}

	#[tokio::test]
	async fn test_get_group_by_name() {
		let mut manager = GroupManager::new();

		let group_data = CreateGroupData {
			name: "Viewers".to_string(),
			description: None,
		};

		manager.create_group(group_data).await.unwrap();
		let retrieved = manager.get_group_by_name("Viewers").await.unwrap();
		assert_eq!(retrieved.name, "Viewers");
	}

	#[tokio::test]
	async fn test_delete_group() {
		let mut manager = GroupManager::new();

		let group_data = CreateGroupData {
			name: "TempGroup".to_string(),
			description: None,
		};

		let group = manager.create_group(group_data).await.unwrap();
		manager.delete_group(&group.id.to_string()).await.unwrap();
		let result = manager.get_group(&group.id.to_string()).await;
		assert!(result.is_err());
	}

	#[tokio::test]
	async fn test_group_permissions() {
		let mut manager = GroupManager::new();

		let group_data = CreateGroupData {
			name: "Writers".to_string(),
			description: None,
		};

		let group = manager.create_group(group_data).await.unwrap();
		manager
			.add_group_permission(&group.id.to_string(), "blog.add_article")
			.await
			.unwrap();
		manager
			.add_group_permission(&group.id.to_string(), "blog.change_article")
			.await
			.unwrap();

		let perms = manager
			.get_group_permissions(&group.id.to_string())
			.await
			.unwrap();
		assert_eq!(perms.len(), 2);
		assert!(perms.contains(&"blog.add_article".to_string()));
		assert!(perms.contains(&"blog.change_article".to_string()));
	}

	#[tokio::test]
	async fn test_user_groups() {
		let mut manager = GroupManager::new();

		let group_data1 = CreateGroupData {
			name: "Team1".to_string(),
			description: None,
		};
		let group_data2 = CreateGroupData {
			name: "Team2".to_string(),
			description: None,
		};

		let group1 = manager.create_group(group_data1).await.unwrap();
		let group2 = manager.create_group(group_data2).await.unwrap();

		manager
			.add_user_to_group("alice", &group1.id.to_string())
			.await
			.unwrap();
		manager
			.add_user_to_group("alice", &group2.id.to_string())
			.await
			.unwrap();

		let groups = manager.get_user_groups("alice").await.unwrap();
		assert_eq!(groups.len(), 2);
	}

	#[tokio::test]
	async fn test_user_permissions() {
		let mut manager = GroupManager::new();

		let group_data = CreateGroupData {
			name: "Developers".to_string(),
			description: None,
		};

		let group = manager.create_group(group_data).await.unwrap();
		manager
			.add_group_permission(&group.id.to_string(), "code.commit")
			.await
			.unwrap();
		manager
			.add_group_permission(&group.id.to_string(), "code.review")
			.await
			.unwrap();
		manager
			.add_user_to_group("bob", &group.id.to_string())
			.await
			.unwrap();

		let perms = manager.get_user_permissions("bob").await.unwrap();
		assert_eq!(perms.len(), 2);
		assert!(perms.contains(&"code.commit".to_string()));
		assert!(perms.contains(&"code.review".to_string()));
	}

	#[tokio::test]
	async fn test_remove_user_from_group() {
		let mut manager = GroupManager::new();

		let group_data = CreateGroupData {
			name: "Staff".to_string(),
			description: None,
		};

		let group = manager.create_group(group_data).await.unwrap();
		manager
			.add_user_to_group("charlie", &group.id.to_string())
			.await
			.unwrap();
		manager
			.remove_user_from_group("charlie", &group.id.to_string())
			.await
			.unwrap();

		let groups = manager.get_user_groups("charlie").await.unwrap();
		assert_eq!(groups.len(), 0);
	}

	#[tokio::test]
	async fn test_list_groups() {
		let mut manager = GroupManager::new();

		let group_data1 = CreateGroupData {
			name: "Group1".to_string(),
			description: None,
		};
		let group_data2 = CreateGroupData {
			name: "Group2".to_string(),
			description: None,
		};

		manager.create_group(group_data1).await.unwrap();
		manager.create_group(group_data2).await.unwrap();

		let groups = manager.list_groups().await;
		assert_eq!(groups.len(), 2);
	}
}
