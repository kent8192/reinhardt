use crate::proto::common::Empty;
use crate::proto::user::{
	CreateUserRequest, DeleteUserRequest, GetUserRequest, ListUsersRequest, ListUsersResponse,
	UpdateUserRequest, User, user_service_server::UserService,
};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tonic::{Request, Response, Status};
use uuid::Uuid;

/// In-memory User storage
#[derive(Clone)]
pub struct UserStorage {
	users: Arc<RwLock<HashMap<String, User>>>,
}

impl UserStorage {
	/// Create a new storage
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_grpc::services::UserStorage;
	///
	/// let storage = UserStorage::new();
	/// ```
	pub fn new() -> Self {
		Self {
			users: Arc::new(RwLock::new(HashMap::new())),
		}
	}

	/// Add a user
	pub async fn add_user(&self, user: User) {
		self.users.write().await.insert(user.id.clone(), user);
	}

	/// Get a user
	pub async fn get_user(&self, id: &str) -> Option<User> {
		self.users.read().await.get(id).cloned()
	}

	/// Get all users
	pub async fn list_users(&self) -> Vec<User> {
		self.users.read().await.values().cloned().collect()
	}

	/// Delete a user
	pub async fn delete_user(&self, id: &str) -> bool {
		self.users.write().await.remove(id).is_some()
	}
}

impl Default for UserStorage {
	fn default() -> Self {
		Self::new()
	}
}

/// Implementation of UserService
pub struct UserServiceImpl {
	storage: UserStorage,
}

impl UserServiceImpl {
	/// Create a new service
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_grpc::services::{UserServiceImpl, UserStorage};
	///
	/// let storage = UserStorage::new();
	/// let service = UserServiceImpl::new(storage);
	/// ```
	pub fn new(storage: UserStorage) -> Self {
		Self { storage }
	}
}

#[tonic::async_trait]
impl UserService for UserServiceImpl {
	async fn create_user(
		&self,
		request: Request<CreateUserRequest>,
	) -> Result<Response<User>, Status> {
		let req = request.into_inner();

		let user = User {
			id: Uuid::now_v7().to_string(),
			name: req.name,
			email: req.email,
			active: true,
			created_at: None,
			updated_at: None,
		};

		self.storage.add_user(user.clone()).await;
		Ok(Response::new(user))
	}

	async fn get_user(&self, request: Request<GetUserRequest>) -> Result<Response<User>, Status> {
		let req = request.into_inner();

		match self.storage.get_user(&req.id).await {
			Some(user) => Ok(Response::new(user)),
			None => Err(Status::not_found(format!("User not found: {}", req.id))),
		}
	}

	async fn list_users(
		&self,
		_request: Request<ListUsersRequest>,
	) -> Result<Response<ListUsersResponse>, Status> {
		let users = self.storage.list_users().await;
		let total = users.len() as i32;
		let response = ListUsersResponse { users, total };
		Ok(Response::new(response))
	}

	async fn update_user(
		&self,
		request: Request<UpdateUserRequest>,
	) -> Result<Response<User>, Status> {
		let req = request.into_inner();

		let mut user = self
			.storage
			.get_user(&req.id)
			.await
			.ok_or_else(|| Status::not_found(format!("User not found: {}", req.id)))?;

		if let Some(name) = req.name {
			user.name = name;
		}
		if let Some(email) = req.email {
			user.email = email;
		}
		if let Some(active) = req.active {
			user.active = active;
		}

		self.storage.add_user(user.clone()).await;
		Ok(Response::new(user))
	}

	async fn delete_user(
		&self,
		request: Request<DeleteUserRequest>,
	) -> Result<Response<Empty>, Status> {
		let req = request.into_inner();

		if self.storage.delete_user(&req.id).await {
			Ok(Response::new(Empty {}))
		} else {
			Err(Status::not_found(format!("User not found: {}", req.id)))
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[tokio::test]
	async fn test_storage_add_get() {
		let storage = UserStorage::new();
		let user = User {
			id: "test-1".to_string(),
			name: "Test User".to_string(),
			email: "test@example.com".to_string(),
			active: true,
			created_at: None,
			updated_at: None,
		};

		storage.add_user(user.clone()).await;
		let retrieved = storage.get_user("test-1").await;

		assert!(retrieved.is_some());
		assert_eq!(retrieved.unwrap().name, "Test User");
	}

	#[tokio::test]
	async fn test_storage_list() {
		let storage = UserStorage::new();
		assert_eq!(storage.list_users().await.len(), 0);

		storage
			.add_user(User {
				id: "1".to_string(),
				name: "User1".to_string(),
				email: "user1@example.com".to_string(),
				active: true,
				created_at: None,
				updated_at: None,
			})
			.await;

		assert_eq!(storage.list_users().await.len(), 1);
	}

	#[tokio::test]
	async fn test_storage_delete() {
		let storage = UserStorage::new();
		storage
			.add_user(User {
				id: "del-1".to_string(),
				name: "Delete Me".to_string(),
				email: "delete@example.com".to_string(),
				active: true,
				created_at: None,
				updated_at: None,
			})
			.await;

		assert!(storage.delete_user("del-1").await);
		assert!(!storage.delete_user("del-1").await);
	}

	#[tokio::test]
	async fn test_service_create_user() {
		let storage = UserStorage::new();
		let service = UserServiceImpl::new(storage.clone());

		let request = Request::new(CreateUserRequest {
			name: "Alice".to_string(),
			email: "alice@example.com".to_string(),
		});

		let response = service.create_user(request).await.unwrap();
		let user = response.into_inner();

		assert_eq!(user.name, "Alice");
		assert_eq!(user.email, "alice@example.com");
		assert!(user.active);
	}

	#[tokio::test]
	async fn test_service_get_user() {
		let storage = UserStorage::new();
		storage
			.add_user(User {
				id: "get-test-1".to_string(),
				name: "Bob".to_string(),
				email: "bob@example.com".to_string(),
				active: true,
				created_at: None,
				updated_at: None,
			})
			.await;

		let service = UserServiceImpl::new(storage);

		let request = Request::new(GetUserRequest {
			id: "get-test-1".to_string(),
		});

		let response = service.get_user(request).await.unwrap();
		let user = response.into_inner();

		assert_eq!(user.name, "Bob");
	}

	#[tokio::test]
	async fn test_service_get_user_not_found() {
		let storage = UserStorage::new();
		let service = UserServiceImpl::new(storage);

		let request = Request::new(GetUserRequest {
			id: "nonexistent".to_string(),
		});

		let result = service.get_user(request).await;
		assert!(result.is_err());
		assert_eq!(result.unwrap_err().code(), tonic::Code::NotFound);
	}

	#[tokio::test]
	async fn test_service_update_user() {
		let storage = UserStorage::new();
		storage
			.add_user(User {
				id: "update-test-1".to_string(),
				name: "Charlie".to_string(),
				email: "charlie@example.com".to_string(),
				active: true,
				created_at: None,
				updated_at: None,
			})
			.await;

		let service = UserServiceImpl::new(storage);

		let request = Request::new(UpdateUserRequest {
			id: "update-test-1".to_string(),
			name: Some("Charles".to_string()),
			email: None,
			active: Some(false),
		});

		let response = service.update_user(request).await.unwrap();
		let user = response.into_inner();

		assert_eq!(user.name, "Charles");
		assert!(!user.active);
		assert_eq!(user.email, "charlie@example.com");
	}

	#[tokio::test]
	async fn test_service_delete_user() {
		let storage = UserStorage::new();
		storage
			.add_user(User {
				id: "delete-test-1".to_string(),
				name: "David".to_string(),
				email: "david@example.com".to_string(),
				active: true,
				created_at: None,
				updated_at: None,
			})
			.await;

		let service = UserServiceImpl::new(storage.clone());

		let request = Request::new(DeleteUserRequest {
			id: "delete-test-1".to_string(),
		});

		let response = service.delete_user(request).await;
		assert!(response.is_ok());

		// Confirm deletion
		assert!(storage.get_user("delete-test-1").await.is_none());
	}

	#[tokio::test]
	async fn test_service_list_users() {
		let storage = UserStorage::new();

		for i in 0..3 {
			storage
				.add_user(User {
					id: format!("list-{}", i),
					name: format!("User{}", i),
					email: format!("user{}@example.com", i),
					active: true,
					created_at: None,
					updated_at: None,
				})
				.await;
		}

		let service = UserServiceImpl::new(storage);

		let request = Request::new(ListUsersRequest {
			page: 0,
			page_size: 10,
		});

		let response = service.list_users(request).await.unwrap();
		let list_response = response.into_inner();

		assert_eq!(list_response.users.len(), 3);
		assert_eq!(list_response.total, 3);
	}
}
