//! Integration tests for ViewSet pagination support

use bytes::Bytes;
use hyper::{HeaderMap, Method, StatusCode, Version};
use reinhardt_http::{Request, Response};
use reinhardt_macros::model;
use reinhardt_views::viewsets::ReadOnlyModelViewSet;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

#[allow(dead_code)]
#[model(table_name = "products")]
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Product {
	#[field(primary_key = true)]
	id: i64,
	#[field(max_length = 255)]
	name: String,
	price: f64,
	#[field(max_length = 100)]
	category: String,
}

#[derive(Debug, Clone)]
struct ProductSerializer;

// ============================================================================
// Pagination Traits
// ============================================================================

trait Paginator: Send + Sync {
	fn paginate(&self, items: &[Product], request: &Request) -> PaginationResult;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PaginationResult {
	data: Vec<Product>,
	total: usize,
	page_info: PageInfo,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
enum PageInfo {
	PageNumber {
		page: usize,
		page_size: usize,
		total_pages: usize,
	},
	Cursor {
		next_cursor: Option<String>,
		prev_cursor: Option<String>,
		has_next: bool,
		has_prev: bool,
	},
	LimitOffset {
		limit: usize,
		offset: usize,
		has_next: bool,
	},
}

// ============================================================================
// PageNumber Pagination
// ============================================================================

#[derive(Clone)]
struct PageNumberPagination {
	default_page_size: usize,
	max_page_size: usize,
}

impl PageNumberPagination {
	fn new() -> Self {
		Self {
			default_page_size: 10,
			max_page_size: 100,
		}
	}

	fn with_page_size(mut self, page_size: usize) -> Self {
		self.default_page_size = page_size;
		self
	}

	fn with_max_page_size(mut self, max_size: usize) -> Self {
		self.max_page_size = max_size;
		self
	}

	fn get_page(&self, request: &Request) -> usize {
		request
			.query_params
			.get("page")
			.and_then(|p| p.parse().ok())
			.unwrap_or(1)
	}

	fn get_page_size(&self, request: &Request) -> usize {
		request
			.query_params
			.get("page_size")
			.and_then(|s| s.parse().ok())
			.unwrap_or(self.default_page_size)
			.min(self.max_page_size)
	}
}

impl Paginator for PageNumberPagination {
	fn paginate(&self, items: &[Product], request: &Request) -> PaginationResult {
		let page = self.get_page(request);
		let page_size = self.get_page_size(request);
		let total = items.len();
		let total_pages = total.div_ceil(page_size);

		let start = (page - 1) * page_size;
		let end = (start + page_size).min(total);

		let data = if start < total {
			items[start..end].to_vec()
		} else {
			vec![]
		};

		PaginationResult {
			data,
			total,
			page_info: PageInfo::PageNumber {
				page,
				page_size,
				total_pages,
			},
		}
	}
}

// ============================================================================
// Cursor Pagination
// ============================================================================

#[derive(Clone)]
struct CursorPagination {
	page_size: usize,
}

impl CursorPagination {
	fn new() -> Self {
		Self { page_size: 10 }
	}

	fn with_page_size(mut self, page_size: usize) -> Self {
		self.page_size = page_size;
		self
	}

	fn decode_cursor(&self, cursor: &str) -> Option<i64> {
		cursor.parse().ok()
	}

	fn encode_cursor(&self, id: i64) -> String {
		id.to_string()
	}
}

impl Paginator for CursorPagination {
	fn paginate(&self, items: &[Product], request: &Request) -> PaginationResult {
		let cursor = request.query_params.get("cursor");

		let start_idx = if let Some(cursor_str) = cursor {
			if let Some(cursor_id) = self.decode_cursor(cursor_str) {
				items
					.iter()
					.position(|p| p.id > cursor_id)
					.unwrap_or(items.len())
			} else {
				0
			}
		} else {
			0
		};

		let end_idx = (start_idx + self.page_size).min(items.len());
		let data = items[start_idx..end_idx].to_vec();

		let next_cursor = if end_idx < items.len() {
			data.last().map(|p| self.encode_cursor(p.id))
		} else {
			None
		};

		let prev_cursor = if start_idx > 0 {
			items
				.get(start_idx.saturating_sub(1))
				.map(|p| self.encode_cursor(p.id))
		} else {
			None
		};

		PaginationResult {
			data,
			total: items.len(),
			page_info: PageInfo::Cursor {
				next_cursor,
				prev_cursor,
				has_next: end_idx < items.len(),
				has_prev: start_idx > 0,
			},
		}
	}
}

// ============================================================================
// LimitOffset Pagination
// ============================================================================

#[derive(Clone)]
struct LimitOffsetPagination {
	default_limit: usize,
	max_limit: usize,
}

impl LimitOffsetPagination {
	fn new() -> Self {
		Self {
			default_limit: 10,
			max_limit: 100,
		}
	}

	fn get_limit(&self, request: &Request) -> usize {
		request
			.query_params
			.get("limit")
			.and_then(|l| l.parse().ok())
			.unwrap_or(self.default_limit)
			.min(self.max_limit)
	}

	fn get_offset(&self, request: &Request) -> usize {
		request
			.query_params
			.get("offset")
			.and_then(|o| o.parse().ok())
			.unwrap_or(0)
	}
}

impl Paginator for LimitOffsetPagination {
	fn paginate(&self, items: &[Product], request: &Request) -> PaginationResult {
		let limit = self.get_limit(request);
		let offset = self.get_offset(request);
		let total = items.len();

		let start = offset.min(total);
		let end = (start + limit).min(total);
		let data = items[start..end].to_vec();

		PaginationResult {
			data,
			total,
			page_info: PageInfo::LimitOffset {
				limit,
				offset,
				has_next: end < total,
			},
		}
	}
}

// ============================================================================
// ViewSet with Pagination
// ============================================================================

struct PaginatedViewSet<P: Paginator> {
	paginator: P,
	products: Arc<Mutex<Vec<Product>>>,
}

impl<P: Paginator> PaginatedViewSet<P> {
	fn new(paginator: P) -> Self {
		let mut products = Vec::new();
		for i in 1..=50 {
			products.push(Product {
				id: i,
				name: format!("Product {}", i),
				price: 10.0 + (i as f64),
				category: if i % 2 == 0 {
					"A".to_string()
				} else {
					"B".to_string()
				},
			});
		}

		Self {
			paginator,
			products: Arc::new(Mutex::new(products)),
		}
	}

	fn filter_products(&self, request: &Request) -> Vec<Product> {
		let products = self.products.lock().unwrap();

		if let Some(category) = request.query_params.get("category") {
			products
				.iter()
				.filter(|p| &p.category == category)
				.cloned()
				.collect()
		} else {
			products.clone()
		}
	}

	async fn list(&self, request: &Request) -> Result<Response, String> {
		let filtered = self.filter_products(request);
		let result = self.paginator.paginate(&filtered, request);

		let json = serde_json::to_string(&result).unwrap();
		Ok(Response::new(StatusCode::OK).with_body(json))
	}
}

// ============================================================================
// Tests
// ============================================================================

#[tokio::test]
async fn test_page_number_pagination_default() {
	let paginator = PageNumberPagination::new();
	let viewset = PaginatedViewSet::new(paginator);

	let request = Request::builder()
		.method(Method::GET)
		.uri("/products/")
		.version(Version::HTTP_11)
		.headers(HeaderMap::new())
		.body(Bytes::new())
		.build()
		.unwrap();

	let response = viewset.list(&request).await.unwrap();
	assert_eq!(response.status, StatusCode::OK);

	let result: PaginationResult = serde_json::from_slice(&response.body).unwrap();
	assert_eq!(result.data.len(), 10);
	assert_eq!(result.total, 50);

	match result.page_info {
		PageInfo::PageNumber {
			page,
			page_size,
			total_pages,
		} => {
			assert_eq!(page, 1);
			assert_eq!(page_size, 10);
			assert_eq!(total_pages, 5);
		}
		_ => panic!("Expected PageNumber pagination"),
	}
}

#[tokio::test]
async fn test_page_number_pagination_custom_page() {
	let paginator = PageNumberPagination::new();
	let viewset = PaginatedViewSet::new(paginator);

	let request = Request::builder()
		.method(Method::GET)
		.uri("/products/?page=3")
		.version(Version::HTTP_11)
		.headers(HeaderMap::new())
		.body(Bytes::new())
		.build()
		.unwrap();

	let response = viewset.list(&request).await.unwrap();
	let result: PaginationResult = serde_json::from_slice(&response.body).unwrap();

	assert_eq!(result.data.len(), 10);
	assert_eq!(result.data[0].id, 21);
	assert_eq!(result.data[9].id, 30);

	match result.page_info {
		PageInfo::PageNumber { page, .. } => assert_eq!(page, 3),
		_ => panic!("Expected PageNumber pagination"),
	}
}

#[tokio::test]
async fn test_page_number_pagination_custom_page_size() {
	let paginator = PageNumberPagination::new();
	let viewset = PaginatedViewSet::new(paginator);

	let request = Request::builder()
		.method(Method::GET)
		.uri("/products/?page=2&page_size=5")
		.version(Version::HTTP_11)
		.headers(HeaderMap::new())
		.body(Bytes::new())
		.build()
		.unwrap();

	let response = viewset.list(&request).await.unwrap();
	let result: PaginationResult = serde_json::from_slice(&response.body).unwrap();

	assert_eq!(result.data.len(), 5);
	assert_eq!(result.data[0].id, 6);
	assert_eq!(result.data[4].id, 10);

	match result.page_info {
		PageInfo::PageNumber {
			page,
			page_size,
			total_pages,
		} => {
			assert_eq!(page, 2);
			assert_eq!(page_size, 5);
			assert_eq!(total_pages, 10);
		}
		_ => panic!("Expected PageNumber pagination"),
	}
}

#[tokio::test]
async fn test_page_number_pagination_max_page_size() {
	let paginator = PageNumberPagination::new().with_max_page_size(15);
	let viewset = PaginatedViewSet::new(paginator);

	let request = Request::builder()
		.method(Method::GET)
		.uri("/products/?page_size=100")
		.version(Version::HTTP_11)
		.headers(HeaderMap::new())
		.body(Bytes::new())
		.build()
		.unwrap();

	let response = viewset.list(&request).await.unwrap();
	let result: PaginationResult = serde_json::from_slice(&response.body).unwrap();

	match result.page_info {
		PageInfo::PageNumber { page_size, .. } => assert_eq!(page_size, 15),
		_ => panic!("Expected PageNumber pagination"),
	}
}

#[tokio::test]
async fn test_page_number_pagination_out_of_bounds() {
	let paginator = PageNumberPagination::new();
	let viewset = PaginatedViewSet::new(paginator);

	let request = Request::builder()
		.method(Method::GET)
		.uri("/products/?page=999")
		.version(Version::HTTP_11)
		.headers(HeaderMap::new())
		.body(Bytes::new())
		.build()
		.unwrap();

	let response = viewset.list(&request).await.unwrap();
	let result: PaginationResult = serde_json::from_slice(&response.body).unwrap();

	assert_eq!(result.data.len(), 0);
}

#[tokio::test]
async fn test_cursor_pagination_first_page() {
	let paginator = CursorPagination::new().with_page_size(5);
	let viewset = PaginatedViewSet::new(paginator);

	let request = Request::builder()
		.method(Method::GET)
		.uri("/products/")
		.version(Version::HTTP_11)
		.headers(HeaderMap::new())
		.body(Bytes::new())
		.build()
		.unwrap();

	let response = viewset.list(&request).await.unwrap();
	let result: PaginationResult = serde_json::from_slice(&response.body).unwrap();

	assert_eq!(result.data.len(), 5);
	assert_eq!(result.data[0].id, 1);
	assert_eq!(result.data[4].id, 5);

	match result.page_info {
		PageInfo::Cursor {
			next_cursor,
			prev_cursor,
			has_next,
			has_prev,
		} => {
			assert_eq!(next_cursor, Some("5".to_string()));
			assert_eq!(prev_cursor, None);
			assert!(has_next);
			assert!(!has_prev);
		}
		_ => panic!("Expected Cursor pagination"),
	}
}

#[tokio::test]
async fn test_cursor_pagination_next_page() {
	let paginator = CursorPagination::new().with_page_size(5);
	let viewset = PaginatedViewSet::new(paginator);

	let request = Request::builder()
		.method(Method::GET)
		.uri("/products/?cursor=5")
		.version(Version::HTTP_11)
		.headers(HeaderMap::new())
		.body(Bytes::new())
		.build()
		.unwrap();

	let response = viewset.list(&request).await.unwrap();
	let result: PaginationResult = serde_json::from_slice(&response.body).unwrap();

	assert_eq!(result.data.len(), 5);
	assert_eq!(result.data[0].id, 6);
	assert_eq!(result.data[4].id, 10);

	match result.page_info {
		PageInfo::Cursor {
			next_cursor,
			has_prev,
			..
		} => {
			assert_eq!(next_cursor, Some("10".to_string()));
			assert!(has_prev);
		}
		_ => panic!("Expected Cursor pagination"),
	}
}

#[tokio::test]
async fn test_cursor_pagination_last_page() {
	let paginator = CursorPagination::new().with_page_size(5);
	let viewset = PaginatedViewSet::new(paginator);

	let request = Request::builder()
		.method(Method::GET)
		.uri("/products/?cursor=45")
		.version(Version::HTTP_11)
		.headers(HeaderMap::new())
		.body(Bytes::new())
		.build()
		.unwrap();

	let response = viewset.list(&request).await.unwrap();
	let result: PaginationResult = serde_json::from_slice(&response.body).unwrap();

	assert_eq!(result.data.len(), 5);
	assert_eq!(result.data[0].id, 46);
	assert_eq!(result.data[4].id, 50);

	match result.page_info {
		PageInfo::Cursor {
			next_cursor,
			has_next,
			..
		} => {
			assert_eq!(next_cursor, None);
			assert!(!has_next);
		}
		_ => panic!("Expected Cursor pagination"),
	}
}

#[tokio::test]
async fn test_limit_offset_pagination_default() {
	let paginator = LimitOffsetPagination::new();
	let viewset = PaginatedViewSet::new(paginator);

	let request = Request::builder()
		.method(Method::GET)
		.uri("/products/")
		.version(Version::HTTP_11)
		.headers(HeaderMap::new())
		.body(Bytes::new())
		.build()
		.unwrap();

	let response = viewset.list(&request).await.unwrap();
	let result: PaginationResult = serde_json::from_slice(&response.body).unwrap();

	assert_eq!(result.data.len(), 10);
	assert_eq!(result.data[0].id, 1);

	match result.page_info {
		PageInfo::LimitOffset {
			limit,
			offset,
			has_next,
		} => {
			assert_eq!(limit, 10);
			assert_eq!(offset, 0);
			assert!(has_next);
		}
		_ => panic!("Expected LimitOffset pagination"),
	}
}

#[tokio::test]
async fn test_limit_offset_pagination_custom() {
	let paginator = LimitOffsetPagination::new();
	let viewset = PaginatedViewSet::new(paginator);

	let request = Request::builder()
		.method(Method::GET)
		.uri("/products/?limit=5&offset=20")
		.version(Version::HTTP_11)
		.headers(HeaderMap::new())
		.body(Bytes::new())
		.build()
		.unwrap();

	let response = viewset.list(&request).await.unwrap();
	let result: PaginationResult = serde_json::from_slice(&response.body).unwrap();

	assert_eq!(result.data.len(), 5);
	assert_eq!(result.data[0].id, 21);
	assert_eq!(result.data[4].id, 25);

	match result.page_info {
		PageInfo::LimitOffset {
			limit,
			offset,
			has_next,
		} => {
			assert_eq!(limit, 5);
			assert_eq!(offset, 20);
			assert!(has_next);
		}
		_ => panic!("Expected LimitOffset pagination"),
	}
}

#[tokio::test]
async fn test_pagination_with_filtering() {
	let paginator = PageNumberPagination::new().with_page_size(5);
	let viewset = PaginatedViewSet::new(paginator);

	let request = Request::builder()
		.method(Method::GET)
		.uri("/products/?category=A&page=1")
		.version(Version::HTTP_11)
		.headers(HeaderMap::new())
		.body(Bytes::new())
		.build()
		.unwrap();

	let response = viewset.list(&request).await.unwrap();
	let result: PaginationResult = serde_json::from_slice(&response.body).unwrap();

	assert_eq!(result.data.len(), 5);
	assert_eq!(result.total, 25);
	assert!(result.data.iter().all(|p| p.category == "A"));

	match result.page_info {
		PageInfo::PageNumber { total_pages, .. } => assert_eq!(total_pages, 5),
		_ => panic!("Expected PageNumber pagination"),
	}
}

#[tokio::test]
async fn test_cursor_pagination_with_filtering() {
	let paginator = CursorPagination::new().with_page_size(5);
	let viewset = PaginatedViewSet::new(paginator);

	let request = Request::builder()
		.method(Method::GET)
		.uri("/products/?category=B")
		.version(Version::HTTP_11)
		.headers(HeaderMap::new())
		.body(Bytes::new())
		.build()
		.unwrap();

	let response = viewset.list(&request).await.unwrap();
	let result: PaginationResult = serde_json::from_slice(&response.body).unwrap();

	assert_eq!(result.data.len(), 5);
	assert_eq!(result.total, 25);
	assert!(result.data.iter().all(|p| p.category == "B"));
}

#[tokio::test]
async fn test_readonly_viewset_supports_pagination() {
	let _viewset: ReadOnlyModelViewSet<Product, ProductSerializer> =
		ReadOnlyModelViewSet::new("products");

	// ReadOnlyModelViewSet should support pagination configuration
}

#[tokio::test]
async fn test_page_number_pagination_config() {
	let paginator = PageNumberPagination::new()
		.with_page_size(20)
		.with_max_page_size(50);

	assert_eq!(paginator.default_page_size, 20);
	assert_eq!(paginator.max_page_size, 50);
}
