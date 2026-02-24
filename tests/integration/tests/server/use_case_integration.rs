//! Comprehensive use case integration tests
//!
//! This module contains realistic end-to-end integration tests that simulate
//! real-world user workflows. Each test demonstrates a complete use case from
//! start to finish.
//!
//! # Test Scenarios
//!
//! 1. **RESTful API Full Workflow** - Complete CRUD operations lifecycle
//! 2. **File Upload/Download** - Multipart upload and streaming download
//! 3. **Real-time Chat** - WebSocket-based multi-client communication (feature-gated)
//! 4. **GraphQL Pagination** - Cursor-based pagination with filtering (feature-gated)
//! 5. **Session Management** - Cookie-based sessions with authentication

use bytes::Bytes;
use http::Method;
use reinhardt_http::Handler;
use reinhardt_http::{Request, Response};
use reinhardt_test::APIClient;
use reinhardt_test::fixtures::*;
use reinhardt_urls::routers::ServerRouter as Router;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

// ============================================================================
// Test 1: RESTful API Full Workflow (CRUD Operations)
// ============================================================================

/// Article resource for RESTful API testing
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct Article {
	id: u64,
	title: String,
	content: String,
	author: String,
}

/// In-memory article store
#[derive(Clone)]
struct ArticleStore {
	articles: Arc<Mutex<HashMap<u64, Article>>>,
	next_id: Arc<Mutex<u64>>,
}

impl ArticleStore {
	fn new() -> Self {
		Self {
			articles: Arc::new(Mutex::new(HashMap::new())),
			next_id: Arc::new(Mutex::new(1)),
		}
	}

	fn create(&self, title: String, content: String, author: String) -> Article {
		let mut next_id = self.next_id.lock().unwrap();
		let id = *next_id;
		*next_id += 1;

		let article = Article {
			id,
			title,
			content,
			author,
		};

		self.articles.lock().unwrap().insert(id, article.clone());
		article
	}

	fn list(&self) -> Vec<Article> {
		self.articles.lock().unwrap().values().cloned().collect()
	}

	fn get(&self, id: u64) -> Option<Article> {
		self.articles.lock().unwrap().get(&id).cloned()
	}

	fn update(&self, id: u64, title: String, content: String, author: String) -> Option<Article> {
		let mut articles = self.articles.lock().unwrap();
		if articles.contains_key(&id) {
			let article = Article {
				id,
				title,
				content,
				author,
			};
			articles.insert(id, article.clone());
			Some(article)
		} else {
			None
		}
	}

	fn delete(&self, id: u64) -> bool {
		self.articles.lock().unwrap().remove(&id).is_some()
	}
}

/// Create article request payload
#[derive(Deserialize)]
struct CreateArticleRequest {
	title: String,
	content: String,
	author: String,
}

/// Update article request payload
#[derive(Deserialize)]
struct UpdateArticleRequest {
	title: String,
	content: String,
	author: String,
}

// Global store for testing (thread-safe)
static ARTICLE_STORE: OnceLock<ArticleStore> = OnceLock::new();

fn get_article_store() -> &'static ArticleStore {
	ARTICLE_STORE.get().expect("ArticleStore not initialized")
}

/// Create article handler
#[derive(Clone)]
struct CreateArticleHandler;

#[async_trait::async_trait]
impl Handler for CreateArticleHandler {
	async fn handle(&self, request: Request) -> reinhardt_core::exception::Result<Response> {
		let payload: CreateArticleRequest =
			serde_json::from_slice(request.body()).map_err(|e| format!("Invalid JSON: {}", e))?;

		let store = get_article_store();
		let article = store.create(payload.title, payload.content, payload.author);
		Response::created().with_json(&article)
	}
}

/// List articles handler
#[derive(Clone)]
struct ListArticlesHandler;

#[async_trait::async_trait]
impl Handler for ListArticlesHandler {
	async fn handle(&self, _request: Request) -> reinhardt_core::exception::Result<Response> {
		let store = get_article_store();
		let articles = store.list();
		Response::ok().with_json(&articles)
	}
}

/// Get article handler
#[derive(Clone)]
struct GetArticleHandler;

#[async_trait::async_trait]
impl Handler for GetArticleHandler {
	async fn handle(&self, request: Request) -> reinhardt_core::exception::Result<Response> {
		// Extract ID from path
		let path = request.uri.path();
		let id: u64 = path
			.trim_start_matches("/articles/")
			.parse()
			.map_err(|_| "Invalid article ID")?;

		let store = get_article_store();
		match store.get(id) {
			Some(article) => Response::ok().with_json(&article),
			None => Ok(Response::not_found().with_body("Article not found")),
		}
	}
}

/// Update article handler
#[derive(Clone)]
struct UpdateArticleHandler;

#[async_trait::async_trait]
impl Handler for UpdateArticleHandler {
	async fn handle(&self, request: Request) -> reinhardt_core::exception::Result<Response> {
		// Extract ID from path
		let path = request.uri.path();
		let id: u64 = path
			.trim_start_matches("/articles/")
			.parse()
			.map_err(|_| "Invalid article ID")?;

		let payload: UpdateArticleRequest =
			serde_json::from_slice(request.body()).map_err(|e| format!("Invalid JSON: {}", e))?;

		let store = get_article_store();
		match store.update(id, payload.title, payload.content, payload.author) {
			Some(article) => Response::ok().with_json(&article),
			None => Ok(Response::not_found().with_body("Article not found")),
		}
	}
}

/// Delete article handler
#[derive(Clone)]
struct DeleteArticleHandler;

#[async_trait::async_trait]
impl Handler for DeleteArticleHandler {
	async fn handle(&self, request: Request) -> reinhardt_core::exception::Result<Response> {
		// Extract ID from path
		let path = request.uri.path();
		let id: u64 = path
			.trim_start_matches("/articles/")
			.parse()
			.map_err(|_| "Invalid article ID")?;

		let store = get_article_store();
		if store.delete(id) {
			Ok(Response::ok().with_body("Deleted"))
		} else {
			Ok(Response::not_found().with_body("Article not found"))
		}
	}
}

/// Test 1: RESTful API Full Workflow
///
/// This test simulates a complete CRUD lifecycle:
/// 1. Create a new article (POST)
/// 2. List all articles (GET)
/// 3. Get specific article (GET)
/// 4. Update the article (PUT)
/// 5. Delete the article (DELETE)
/// 6. Verify deletion (GET returns 404)
#[tokio::test]
async fn test_restful_api_full_workflow() {
	// Initialize store
	let store = ArticleStore::new();
	let _ = ARTICLE_STORE.set(store);

	// Register routes with Handler trait implementation
	let router = Router::new()
		.handler_with_method("/articles", Method::POST, CreateArticleHandler)
		.handler_with_method("/articles", Method::GET, ListArticlesHandler)
		.handler_with_method("/articles/{id}", Method::GET, GetArticleHandler)
		.handler_with_method("/articles/{id}", Method::PUT, UpdateArticleHandler)
		.handler_with_method("/articles/{id}", Method::DELETE, DeleteArticleHandler);

	let server = test_server_guard(router).await;
	let client = APIClient::with_base_url(&server.url);

	// Step 1: Create a new article
	let create_payload = serde_json::json!({
		"title": "Introduction to Rust",
		"content": "Rust is a systems programming language...",
		"author": "Alice"
	});

	let response = client
		.post("/articles", &create_payload, "json")
		.await
		.unwrap();

	assert_eq!(response.status_code(), 201);
	let created_article: Article = response.json().unwrap();
	assert_eq!(created_article.title, "Introduction to Rust");
	assert_eq!(created_article.author, "Alice");
	let article_id = created_article.id;

	// Step 2: List all articles
	let response = client.get("/articles").await.unwrap();

	assert_eq!(response.status_code(), 200);
	let articles: Vec<Article> = response.json().unwrap();
	assert_eq!(articles.len(), 1);
	assert_eq!(articles[0].id, article_id);

	// Step 3: Get specific article
	let response = client
		.get(&format!("/articles/{}", article_id))
		.await
		.unwrap();

	assert_eq!(response.status_code(), 200);
	let article: Article = response.json().unwrap();
	assert_eq!(article.id, article_id);
	assert_eq!(article.title, "Introduction to Rust");

	// Step 4: Update the article
	let update_payload = serde_json::json!({
		"title": "Advanced Rust Programming",
		"content": "Rust's ownership system ensures memory safety...",
		"author": "Alice"
	});

	let response = client
		.put(
			&format!("/articles/{}", article_id),
			&update_payload,
			"json",
		)
		.await
		.unwrap();

	assert_eq!(response.status_code(), 200);
	let updated_article: Article = response.json().unwrap();
	assert_eq!(updated_article.title, "Advanced Rust Programming");
	assert_eq!(updated_article.id, article_id);

	// Step 5: Delete the article
	let response = client
		.delete(&format!("/articles/{}", article_id))
		.await
		.unwrap();

	assert_eq!(response.status_code(), 200);

	// Step 6: Verify deletion
	let response = client
		.get(&format!("/articles/{}", article_id))
		.await
		.unwrap();

	assert_eq!(response.status_code(), 404);

	// Verify list is empty
	let response = client.get("/articles").await.unwrap();

	let articles: Vec<Article> = response.json().unwrap();
	assert_eq!(articles.len(), 0);
}

// ============================================================================
// Test 2: File Upload/Download
// ============================================================================

/// File storage for upload/download testing
#[derive(Clone)]
struct FileStore {
	files: Arc<Mutex<HashMap<String, Vec<u8>>>>,
}

impl FileStore {
	fn new() -> Self {
		Self {
			files: Arc::new(Mutex::new(HashMap::new())),
		}
	}

	fn save(&self, filename: String, data: Vec<u8>) {
		self.files.lock().unwrap().insert(filename, data);
	}

	fn get(&self, filename: &str) -> Option<Vec<u8>> {
		self.files.lock().unwrap().get(filename).cloned()
	}
}

// Global file store for testing
static FILE_STORE: OnceLock<FileStore> = OnceLock::new();

fn get_file_store() -> &'static FileStore {
	FILE_STORE.get().expect("FileStore not initialized")
}

/// File upload handler
#[derive(Clone)]
struct UploadFileHandler;

#[async_trait::async_trait]
impl Handler for UploadFileHandler {
	async fn handle(&self, request: Request) -> reinhardt_core::exception::Result<Response> {
		// Extract filename from query parameters
		let filename = request
			.uri
			.query()
			.and_then(|q| {
				q.split('&')
					.find(|pair| pair.starts_with("filename="))
					.and_then(|pair| pair.split('=').nth(1))
			})
			.ok_or_else(|| {
				reinhardt_core::exception::Error::Http("Missing filename parameter".into())
			})?;

		let data = request.body().to_vec();
		let store = get_file_store();
		store.save(filename.to_string(), data.clone());

		Response::ok().with_json(&serde_json::json!({
			"filename": filename,
			"size": data.len()
		}))
	}
}

/// File download handler
#[derive(Clone)]
struct DownloadFileHandler;

#[async_trait::async_trait]
impl Handler for DownloadFileHandler {
	async fn handle(&self, request: Request) -> reinhardt_core::exception::Result<Response> {
		// Extract filename from path
		let path = request.uri.path();
		let filename = path.trim_start_matches("/download/");

		let store = get_file_store();
		match store.get(filename) {
			Some(data) => Ok(Response::ok()
				.with_body(Bytes::from(data))
				.with_header("Content-Type", "application/octet-stream")
				.with_header(
					"Content-Disposition",
					&format!("attachment; filename=\"{}\"", filename),
				)),
			None => Ok(Response::not_found().with_body("File not found")),
		}
	}
}

/// Test 2: File Upload/Download
///
/// This test simulates file upload and download workflow:
/// 1. Upload a large file via multipart POST
/// 2. Download the file and verify content
/// 3. Upload another file
/// 4. Download and verify the second file
#[tokio::test]
async fn test_file_upload_download() {
	// Initialize store
	let store = FileStore::new();
	let _ = FILE_STORE.set(store);

	// Register routes with Handler trait implementation
	let router = Router::new()
		.handler_with_method("/upload", Method::POST, UploadFileHandler)
		.handler_with_method("/download/{filename}", Method::GET, DownloadFileHandler);

	let server = test_server_guard(router).await;
	let client = APIClient::with_base_url(&server.url);

	// Step 1: Upload a large file (1MB of data)
	let file_data = vec![b'A'; 1024 * 1024]; // 1MB
	let response = client
		.post_raw(
			"/upload?filename=large_file.bin",
			&file_data,
			"application/octet-stream",
		)
		.await
		.unwrap();

	assert_eq!(response.status_code(), 200);
	let upload_result: serde_json::Value = response.json().unwrap();
	assert_eq!(upload_result["filename"], "large_file.bin");
	assert_eq!(upload_result["size"], 1024 * 1024);

	// Step 2: Download the file and verify content
	let response = client.get("/download/large_file.bin").await.unwrap();

	assert_eq!(response.status_code(), 200);
	let downloaded_data = response.body();
	assert_eq!(downloaded_data.len(), 1024 * 1024);
	assert_eq!(downloaded_data.to_vec(), file_data);

	// Step 3: Upload another file (text file)
	let text_content = "Hello, this is a test file!";
	let response = client
		.post_raw(
			"/upload?filename=test.txt",
			text_content.as_bytes(),
			"text/plain",
		)
		.await
		.unwrap();

	assert_eq!(response.status_code(), 200);

	// Step 4: Download and verify the text file
	let response = client.get("/download/test.txt").await.unwrap();

	assert_eq!(response.status_code(), 200);
	let content = response.text();
	assert_eq!(content, text_content);

	// Verify non-existent file returns 404
	let response = client.get("/download/nonexistent.txt").await.unwrap();

	assert_eq!(response.status_code(), 404);
}

// ============================================================================
// Test 3: Real-time Chat (WebSocket) - Feature-gated
// ============================================================================

#[cfg(feature = "websocket")]
mod websocket_tests {
	use super::*;
	use futures_util::{SinkExt, StreamExt};
	use reinhardt_server::{ShutdownCoordinator, WebSocketHandler, WebSocketServer};
	use std::time::Duration;
	use tokio::net::TcpListener;
	use tokio_tungstenite::tungstenite::Message;

	/// Chat room for broadcasting messages
	#[derive(Clone)]
	struct ChatRoom {
		messages: Arc<Mutex<Vec<String>>>,
		broadcast_tx: Arc<Mutex<Option<tokio::sync::broadcast::Sender<String>>>>,
	}

	impl ChatRoom {
		fn new() -> Self {
			let (tx, _) = tokio::sync::broadcast::channel(100);
			Self {
				messages: Arc::new(Mutex::new(Vec::new())),
				broadcast_tx: Arc::new(Mutex::new(Some(tx))),
			}
		}

		fn send_message(&self, message: String) {
			self.messages.lock().unwrap().push(message.clone());
			if let Some(tx) = self.broadcast_tx.lock().unwrap().as_ref() {
				let _ = tx.send(message);
			}
		}

		fn subscribe(&self) -> tokio::sync::broadcast::Receiver<String> {
			self.broadcast_tx
				.lock()
				.unwrap()
				.as_ref()
				.unwrap()
				.subscribe()
		}
	}

	/// Chat handler for WebSocket connections
	#[derive(Clone)]
	struct ChatHandler {
		room: ChatRoom,
	}

	#[async_trait::async_trait]
	impl WebSocketHandler for ChatHandler {
		async fn handle_message(&self, message: String) -> Result<String, String> {
			// Broadcast message to all clients
			self.room.send_message(message.clone());
			Ok(format!("Broadcast: {}", message))
		}
	}

	/// Test 3: Real-time Chat
	///
	/// This test simulates a multi-client chat room:
	/// 1. Connect multiple WebSocket clients
	/// 2. Send messages from different clients
	/// 3. Verify message broadcasting
	/// 4. Disconnect clients gracefully
	#[tokio::test]
	async fn test_realtime_chat() {
		let room = ChatRoom::new();
		let handler = Arc::new(ChatHandler { room: room.clone() });

		// Setup WebSocket server
		let addr: std::net::SocketAddr = "127.0.0.1:0".parse().unwrap();
		let listener = TcpListener::bind(addr).await.unwrap();
		let actual_addr = listener.local_addr().unwrap();
		let url = format!("ws://{}", actual_addr);
		drop(listener);

		let coordinator = Arc::new(ShutdownCoordinator::new(Duration::from_secs(5)));
		let server_coordinator = (*coordinator).clone();

		let server_task = tokio::spawn(async move {
			let server = WebSocketServer::from_arc(handler);
			let _ = server
				.listen_with_shutdown(actual_addr, server_coordinator)
				.await;
		});

		tokio::time::sleep(Duration::from_millis(100)).await;

		// Connect client 1
		let (mut ws1, _) = tokio_tungstenite::connect_async(&url)
			.await
			.expect("Failed to connect client 1");

		// Connect client 2
		let (mut ws2, _) = tokio_tungstenite::connect_async(&url)
			.await
			.expect("Failed to connect client 2");

		// Subscribe to broadcast messages
		let mut rx1 = room.subscribe();
		let mut rx2 = room.subscribe();

		// Client 1 sends a message
		ws1.send(Message::Text("Hello from Client 1".into()))
			.await
			.unwrap();

		// Verify response
		if let Some(Ok(Message::Text(response))) = ws1.next().await {
			assert_eq!(response, "Broadcast: Hello from Client 1");
		}

		// Both clients should receive broadcast
		tokio::time::timeout(Duration::from_secs(1), rx1.recv())
			.await
			.unwrap()
			.unwrap();
		tokio::time::timeout(Duration::from_secs(1), rx2.recv())
			.await
			.unwrap()
			.unwrap();

		// Client 2 sends a message
		ws2.send(Message::Text("Hello from Client 2".into()))
			.await
			.unwrap();

		// Verify response
		if let Some(Ok(Message::Text(response))) = ws2.next().await {
			assert_eq!(response, "Broadcast: Hello from Client 2");
		}

		// Verify message history
		let messages = room.messages.lock().unwrap();
		assert_eq!(messages.len(), 2);
		assert_eq!(messages[0], "Hello from Client 1");
		assert_eq!(messages[1], "Hello from Client 2");

		// Close connections
		ws1.close(None).await.unwrap();
		ws2.close(None).await.unwrap();

		// Cleanup
		coordinator.shutdown();
		server_task.abort();
	}
}

// ============================================================================
// Test 4: GraphQL Pagination - Feature-gated
// ============================================================================

#[cfg(feature = "graphql")]
mod graphql_tests {
	use super::*;
	use async_graphql::*;
	use reinhardt_server::graphql_handler;
	use reinhardt_test::APIClient;
	use reinhardt_test::server::{shutdown_test_server, spawn_test_server};

	/// User for pagination testing
	#[derive(Debug, Clone, SimpleObject)]
	struct User {
		id: ID,
		name: String,
		email: String,
		age: i32,
	}

	/// Edge for cursor-based pagination
	#[derive(SimpleObject)]
	struct UserEdge {
		node: User,
		cursor: String,
	}

	/// Connection for cursor-based pagination
	#[derive(SimpleObject)]
	struct UserConnection {
		edges: Vec<UserEdge>,
		page_info: PageInfo,
	}

	/// Page info for pagination
	#[derive(SimpleObject)]
	struct PageInfo {
		has_next_page: bool,
		has_previous_page: bool,
		start_cursor: Option<String>,
		end_cursor: Option<String>,
	}

	/// User store
	#[derive(Clone)]
	struct UserStore {
		users: Arc<Mutex<Vec<User>>>,
	}

	impl UserStore {
		fn new() -> Self {
			let mut users = Vec::new();
			for i in 1..=20 {
				users.push(User {
					id: ID::from(i.to_string()),
					name: format!("User {}", i),
					email: format!("user{}@example.com", i),
					age: 20 + i,
				});
			}
			Self {
				users: Arc::new(Mutex::new(users)),
			}
		}

		fn get_users(
			&self,
			first: Option<i32>,
			after: Option<String>,
			min_age: Option<i32>,
		) -> UserConnection {
			let users = self.users.lock().unwrap();

			// Apply age filter
			let filtered: Vec<User> = users
				.iter()
				.filter(|u| {
					if let Some(age) = min_age {
						u.age >= age
					} else {
						true
					}
				})
				.cloned()
				.collect();

			// Handle cursor pagination
			let start_index = if let Some(cursor) = after {
				cursor.parse::<usize>().ok().map(|idx| idx + 1).unwrap_or(0)
			} else {
				0
			};

			let limit = first.unwrap_or(10).max(1) as usize;
			let end_index = (start_index + limit).min(filtered.len());

			let page_users: Vec<User> = filtered[start_index..end_index].to_vec();
			let edges: Vec<UserEdge> = page_users
				.iter()
				.enumerate()
				.map(|(idx, user)| UserEdge {
					node: user.clone(),
					cursor: (start_index + idx).to_string(),
				})
				.collect();

			let page_info = PageInfo {
				has_next_page: end_index < filtered.len(),
				has_previous_page: start_index > 0,
				start_cursor: edges.first().map(|e| e.cursor.clone()),
				end_cursor: edges.last().map(|e| e.cursor.clone()),
			};

			UserConnection { edges, page_info }
		}
	}

	struct QueryRoot {
		store: UserStore,
	}

	#[Object]
	impl QueryRoot {
		async fn users(
			&self,
			first: Option<i32>,
			after: Option<String>,
			min_age: Option<i32>,
		) -> UserConnection {
			self.store.get_users(first, after, min_age)
		}
	}

	/// Test 4: GraphQL Pagination
	///
	/// This test demonstrates cursor-based pagination with filtering:
	/// 1. Fetch first page of users
	/// 2. Fetch next page using cursor
	/// 3. Apply age filter and paginate
	/// 4. Verify page info (has_next_page, cursors)
	#[tokio::test]
	async fn test_graphql_pagination() {
		let store = UserStore::new();
		let query_root = QueryRoot { store };

		let handler = graphql_handler(query_root, EmptyMutation);
		let (url, server_handle) = spawn_test_server(handler).await;

		let client = APIClient::with_base_url(&url);

		// Step 1: Fetch first page (5 users)
		let query = r#"
			{
				users(first: 5) {
					edges {
						node { id name email age }
						cursor
					}
					pageInfo {
						hasNextPage
						hasPreviousPage
						startCursor
						endCursor
					}
				}
			}
		"#;

		let payload = serde_json::json!({ "query": query });
		let response = client.post("/graphql", &payload, "json").await.unwrap();

		assert_eq!(response.status_code(), 200);
		let result: serde_json::Value = response.json().unwrap();
		let page1 = &result["data"]["users"];

		assert_eq!(page1["edges"].as_array().unwrap().len(), 5);
		assert_eq!(page1["pageInfo"]["hasNextPage"], true);
		assert_eq!(page1["pageInfo"]["hasPreviousPage"], false);

		let end_cursor = page1["pageInfo"]["endCursor"].as_str().unwrap();

		// Step 2: Fetch next page using cursor
		let query = format!(
			r#"
			{{
				users(first: 5, after: "{}") {{
					edges {{
						node {{ id name age }}
						cursor
					}}
					pageInfo {{
						hasNextPage
						hasPreviousPage
					}}
				}}
			}}
		"#,
			end_cursor
		);

		let payload = serde_json::json!({ "query": query });
		let response = client.post("/graphql", &payload, "json").await.unwrap();

		let result: serde_json::Value = response.json().unwrap();
		let page2 = &result["data"]["users"];

		assert_eq!(page2["edges"].as_array().unwrap().len(), 5);
		assert_eq!(page2["pageInfo"]["hasNextPage"], true);
		assert_eq!(page2["pageInfo"]["hasPreviousPage"], true);

		// Step 3: Apply age filter (age >= 35) and paginate
		let query = r#"
			{
				users(first: 5, minAge: 35) {
					edges {
						node { id name age }
					}
					pageInfo {
						hasNextPage
					}
				}
			}
		"#;

		let payload = serde_json::json!({ "query": query });
		let response = client.post("/graphql", &payload, "json").await.unwrap();

		let result: serde_json::Value = response.json().unwrap();
		let filtered_page = &result["data"]["users"];

		// Users with age >= 35 (User 14-20, since age = 20 + i)
		assert_eq!(filtered_page["edges"].as_array().unwrap().len(), 5);

		// Verify all returned users have age >= 35
		for edge in filtered_page["edges"].as_array().unwrap() {
			let age = edge["node"]["age"].as_i64().unwrap();
			assert!(age >= 35);
		}

		// Cleanup
		shutdown_test_server(server_handle).await;
	}
}

// ============================================================================
// Test 5: Session Management
// ============================================================================

/// User session data
#[derive(Debug, Clone, Serialize, Deserialize)]
struct UserSession {
	user_id: String,
	username: String,
	is_authenticated: bool,
}

/// Session store for managing user sessions
#[derive(Clone)]
struct SessionStore {
	sessions: Arc<Mutex<HashMap<String, UserSession>>>,
}

impl SessionStore {
	fn new() -> Self {
		Self {
			sessions: Arc::new(Mutex::new(HashMap::new())),
		}
	}

	fn create_session(&self, user_id: String, username: String) -> String {
		let session_id = uuid::Uuid::new_v4().to_string();
		let session = UserSession {
			user_id,
			username,
			is_authenticated: true,
		};
		self.sessions
			.lock()
			.unwrap()
			.insert(session_id.clone(), session);
		session_id
	}

	fn get_session(&self, session_id: &str) -> Option<UserSession> {
		self.sessions.lock().unwrap().get(session_id).cloned()
	}

	fn delete_session(&self, session_id: &str) {
		self.sessions.lock().unwrap().remove(session_id);
	}
}

/// Login request payload
#[derive(Deserialize)]
struct LoginRequest {
	username: String,
	password: String,
}

// Global session store for testing
static SESSION_STORE: OnceLock<SessionStore> = OnceLock::new();

fn get_session_store() -> &'static SessionStore {
	SESSION_STORE.get().expect("SessionStore not initialized")
}

/// Login handler
#[derive(Clone)]
struct LoginHandler;

#[async_trait::async_trait]
impl Handler for LoginHandler {
	async fn handle(&self, request: Request) -> reinhardt_core::exception::Result<Response> {
		let payload: LoginRequest =
			serde_json::from_slice(request.body()).map_err(|e| format!("Invalid JSON: {}", e))?;

		let store = get_session_store();

		// Simple authentication (username == password for demo)
		if payload.username == payload.password {
			let session_id =
				store.create_session(payload.username.clone(), payload.username.clone());

			Ok(Response::ok()
				.with_header(
					"Set-Cookie",
					&format!("session_id={}; HttpOnly; Path=/", session_id),
				)
				.with_json(&serde_json::json!({
					"message": "Login successful",
					"username": payload.username
				}))?)
		} else {
			Ok(Response::unauthorized().with_body("Invalid credentials"))
		}
	}
}

/// Protected resource handler (requires authentication)
#[derive(Clone)]
struct ProtectedHandler;

#[async_trait::async_trait]
impl Handler for ProtectedHandler {
	async fn handle(&self, request: Request) -> reinhardt_core::exception::Result<Response> {
		let store = get_session_store();

		// Extract session_id from Cookie header
		let session_id = request
			.headers
			.get(hyper::header::COOKIE)
			.and_then(|v| v.to_str().ok())
			.and_then(|cookie_str| {
				cookie_str
					.split(';')
					.find(|c| c.trim().starts_with("session_id="))
					.and_then(|c| c.split('=').nth(1))
			});

		if let Some(sid) = session_id {
			if let Some(session) = store.get_session(sid) {
				return Response::ok().with_json(&serde_json::json!({
					"message": "Access granted",
					"user": session.username
				}));
			}
		}

		Ok(Response::unauthorized().with_body("Unauthorized"))
	}
}

/// Logout handler
#[derive(Clone)]
struct LogoutHandler;

#[async_trait::async_trait]
impl Handler for LogoutHandler {
	async fn handle(&self, request: Request) -> reinhardt_core::exception::Result<Response> {
		let store = get_session_store();

		// Extract session_id from Cookie header
		let session_id = request
			.headers
			.get(hyper::header::COOKIE)
			.and_then(|v| v.to_str().ok())
			.and_then(|cookie_str| {
				cookie_str
					.split(';')
					.find(|c| c.trim().starts_with("session_id="))
					.and_then(|c| c.split('=').nth(1))
			});

		if let Some(sid) = session_id {
			store.delete_session(sid);
		}

		Ok(Response::ok()
			.with_header("Set-Cookie", "session_id=; HttpOnly; Path=/; Max-Age=0")
			.with_body("Logged out"))
	}
}

/// Test 5: Session Management
///
/// This test demonstrates cookie-based session management:
/// 1. Login and receive session cookie
/// 2. Access protected resource with session cookie
/// 3. Verify unauthorized access without cookie
/// 4. Logout and clear session
/// 5. Verify protected resource is inaccessible after logout
#[tokio::test]
async fn test_session_management() {
	// Initialize store
	let store = SessionStore::new();
	let _ = SESSION_STORE.set(store);

	// Register routes with Handler trait implementation
	let router = Router::new()
		.handler_with_method("/login", Method::POST, LoginHandler)
		.handler_with_method("/protected", Method::GET, ProtectedHandler)
		.handler_with_method("/logout", Method::POST, LogoutHandler);

	let server = test_server_guard(router).await;

	// Create client with cookie jar
	let client = APIClient::builder()
		.base_url(&server.url)
		.cookie_store(true)
		.build();

	// Step 1: Login
	let login_payload = serde_json::json!({
		"username": "alice",
		"password": "alice"
	});

	let response = client.post("/login", &login_payload, "json").await.unwrap();

	assert_eq!(response.status_code(), 200);
	let result: serde_json::Value = response.json().unwrap();
	assert_eq!(result["message"], "Login successful");
	assert_eq!(result["username"], "alice");

	// Step 2: Access protected resource with session cookie
	let response = client.get("/protected").await.unwrap();

	assert_eq!(response.status_code(), 200);
	let result: serde_json::Value = response.json().unwrap();
	assert_eq!(result["message"], "Access granted");
	assert_eq!(result["user"], "alice");

	// Step 3: Verify unauthorized access without cookie (new client)
	let client_no_cookie = APIClient::with_base_url(&server.url);
	let response = client_no_cookie.get("/protected").await.unwrap();

	assert_eq!(response.status_code(), 401);

	// Step 4: Logout
	let response = client
		.post_raw("/logout", &[], "application/json")
		.await
		.unwrap();

	assert_eq!(response.status_code(), 200);

	// Step 5: Verify protected resource is inaccessible after logout
	let response = client.get("/protected").await.unwrap();

	assert_eq!(response.status_code(), 401);

	// Verify invalid credentials
	let invalid_payload = serde_json::json!({
		"username": "alice",
		"password": "wrong_password"
	});

	let response = client
		.post("/login", &invalid_payload, "json")
		.await
		.unwrap();

	assert_eq!(response.status_code(), 401);
}
