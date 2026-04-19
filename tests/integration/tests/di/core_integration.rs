//! Integration tests for reinhardt-di

use reinhardt_di::{Depends, DiResult, Injectable, InjectionContext, SingletonScope};
use std::sync::Arc;

// Test structures
#[derive(Clone, Debug, PartialEq)]
struct Database {
	connection_string: String,
}

#[derive(Clone, Debug, PartialEq)]
struct UserRepository {
	db: Arc<Database>,
}

#[derive(Clone, Debug, PartialEq)]
struct UserService {
	repo: Arc<UserRepository>,
}

// Injectable implementations
#[async_trait::async_trait]
impl Injectable for Database {
	async fn inject(_ctx: &InjectionContext) -> DiResult<Self> {
		Ok(Database {
			connection_string: "postgres://localhost/test".to_string(),
		})
	}
}

#[async_trait::async_trait]
impl Injectable for UserRepository {
	async fn inject(ctx: &InjectionContext) -> DiResult<Self> {
		let db = Database::inject(ctx).await?;
		Ok(UserRepository { db: Arc::new(db) })
	}
}

#[async_trait::async_trait]
impl Injectable for UserService {
	async fn inject(ctx: &InjectionContext) -> DiResult<Self> {
		let repo = UserRepository::inject(ctx).await?;
		Ok(UserService {
			repo: Arc::new(repo),
		})
	}
}

#[tokio::test]
async fn test_basic_injection() {
	let singleton = Arc::new(SingletonScope::new());
	let ctx = InjectionContext::builder(singleton).build();

	let db = Database::inject(&ctx).await.unwrap();
	assert_eq!(db.connection_string, "postgres://localhost/test");
}

#[tokio::test]
async fn test_nested_injection() {
	let singleton = Arc::new(SingletonScope::new());
	let ctx = InjectionContext::builder(singleton).build();

	let service = UserService::inject(&ctx).await.unwrap();
	assert_eq!(
		service.repo.db.connection_string,
		"postgres://localhost/test"
	);
}

#[tokio::test]
async fn test_depends_wrapper() {
	// Register Database in the global registry for Depends<T> resolution
	reinhardt_di::global_registry().register_async::<Database, _, _>(
		reinhardt_di::DependencyScope::Request,
		|_ctx| async {
			Ok(Database {
				connection_string: "postgres://localhost/test".to_string(),
			})
		},
	);

	let singleton = Arc::new(SingletonScope::new());
	let ctx = InjectionContext::builder(singleton).build();

	let db = Depends::<Database>::builder().resolve(&ctx).await.unwrap();
	assert_eq!(db.connection_string, "postgres://localhost/test");
}

#[tokio::test]
async fn test_request_scope_isolation() {
	let singleton = Arc::new(SingletonScope::new());

	// Create two separate request contexts
	let ctx1 = InjectionContext::builder(Arc::clone(&singleton)).build();
	let ctx2 = InjectionContext::builder(Arc::clone(&singleton)).build();

	// Set different values in each request scope
	ctx1.set_request("request1".to_string());
	ctx2.set_request("request2".to_string());

	// Verify isolation
	let val1: Option<Arc<String>> = ctx1.get_request();
	let val2: Option<Arc<String>> = ctx2.get_request();

	assert_eq!(*val1.unwrap(), "request1");
	assert_eq!(*val2.unwrap(), "request2");
}

#[tokio::test]
async fn test_singleton_scope_sharing() {
	let singleton = Arc::new(SingletonScope::new());

	// Set value in singleton scope
	singleton.set("shared_value".to_string());

	// Create two contexts sharing the same singleton
	let ctx1 = InjectionContext::builder(Arc::clone(&singleton)).build();
	let ctx2 = InjectionContext::builder(Arc::clone(&singleton)).build();

	// Both should see the same value
	let val1: Option<Arc<String>> = ctx1.get_singleton();
	let val2: Option<Arc<String>> = ctx2.get_singleton();

	assert_eq!(*val1.unwrap(), "shared_value");
	assert_eq!(*val2.unwrap(), "shared_value");
}

#[tokio::test]
async fn test_concurrent_request_scopes() {
	use tokio::task;

	let singleton = Arc::new(SingletonScope::new());

	let mut handles = vec![];

	for i in 0..10 {
		let singleton_clone = Arc::clone(&singleton);
		let handle = task::spawn(async move {
			let ctx = InjectionContext::builder(singleton_clone).build();
			ctx.set_request(i);

			tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

			let val: Option<Arc<i32>> = ctx.get_request();
			val.map(|v| *v)
		});
		handles.push(handle);
	}

	let results: Vec<_> = futures::future::join_all(handles)
		.await
		.into_iter()
		.map(|r| r.unwrap())
		.collect();

	for (i, result) in results.iter().enumerate() {
		assert_eq!(result.unwrap(), i as i32);
	}
}

#[tokio::test]
async fn test_di_integration_depends_clone() {
	// Register Database in the global registry for Depends<T> resolution
	reinhardt_di::global_registry().register_async::<Database, _, _>(
		reinhardt_di::DependencyScope::Request,
		|_ctx| async {
			Ok(Database {
				connection_string: "postgres://localhost/test".to_string(),
			})
		},
	);

	let singleton = Arc::new(SingletonScope::new());
	let ctx = InjectionContext::builder(singleton).build();

	let db1 = Depends::<Database>::builder().resolve(&ctx).await.unwrap();
	let db2 = db1.clone();

	// Both should point to the same underlying data
	assert_eq!(db1.connection_string, db2.connection_string);
}

#[tokio::test]
async fn test_mixed_scopes() {
	let singleton = Arc::new(SingletonScope::new());

	// Set singleton value
	singleton.set(100i32);

	// Create request context
	let ctx = InjectionContext::builder(Arc::clone(&singleton)).build();
	ctx.set_request(200i32);

	// Verify both scopes work
	let singleton_val: Option<Arc<i32>> = ctx.get_singleton();
	let request_val: Option<Arc<i32>> = ctx.get_request();

	assert_eq!(*singleton_val.unwrap(), 100);
	assert_eq!(*request_val.unwrap(), 200);
}

// Injectable-based tests

#[derive(Clone, Debug, PartialEq)]
struct RequestData {
	value: String,
}

#[async_trait::async_trait]
impl Injectable for RequestData {
	async fn inject(ctx: &InjectionContext) -> DiResult<Self> {
		// Check request scope first
		if let Some(cached) = ctx.get_request::<Self>() {
			return Ok((*cached).clone());
		}

		// Create new instance
		let instance = Self {
			value: "request-specific".to_string(),
		};

		// Cache in request scope
		ctx.set_request(instance.clone());

		Ok(instance)
	}
}

#[derive(Clone, Debug, PartialEq)]
struct SingletonData {
	value: String,
}

#[async_trait::async_trait]
impl Injectable for SingletonData {
	async fn inject(ctx: &InjectionContext) -> DiResult<Self> {
		// Check singleton scope first
		if let Some(cached) = ctx.get_singleton::<Self>() {
			return Ok((*cached).clone());
		}

		// Create new instance
		let instance = Self {
			value: "singleton-shared".to_string(),
		};

		// Cache in singleton scope
		ctx.set_singleton(instance.clone());

		Ok(instance)
	}
}

#[tokio::test]
async fn test_injectable_request_scope_isolation() {
	let singleton = Arc::new(SingletonScope::new());
	let ctx1 = InjectionContext::builder(Arc::clone(&singleton)).build();
	let ctx2 = InjectionContext::builder(Arc::clone(&singleton)).build();

	// Each context should have its own instance
	let data1 = RequestData::inject(&ctx1).await.unwrap();
	let data2 = RequestData::inject(&ctx2).await.unwrap();

	// Verify instances are independent
	assert_eq!(data1.value, "request-specific");
	assert_eq!(data2.value, "request-specific");

	// Verify caching works (same instance within same context)
	let data1_cached = RequestData::inject(&ctx1).await.unwrap();
	assert_eq!(data1, data1_cached);

	// Modify ctx1's data to verify isolation
	ctx1.set_request(RequestData {
		value: "modified".to_string(),
	});
	let data1_modified = RequestData::inject(&ctx1).await.unwrap();
	let data2_unchanged = RequestData::inject(&ctx2).await.unwrap();

	assert_eq!(data1_modified.value, "modified");
	assert_eq!(data2_unchanged.value, "request-specific");
}

#[tokio::test]
async fn test_injectable_singleton_scope_sharing() {
	let singleton = Arc::new(SingletonScope::new());

	let ctx1 = InjectionContext::builder(Arc::clone(&singleton)).build();
	let ctx2 = InjectionContext::builder(Arc::clone(&singleton)).build();

	// First injection creates singleton instance
	let data1 = SingletonData::inject(&ctx1).await.unwrap();
	assert_eq!(data1.value, "singleton-shared");

	// Second context should get the same instance
	let data2 = SingletonData::inject(&ctx2).await.unwrap();
	assert_eq!(data2.value, "singleton-shared");
	assert_eq!(data1, data2);
}

#[tokio::test]
async fn test_injectable_concurrent_requests() {
	use tokio::task;

	let singleton = Arc::new(SingletonScope::new());

	let mut handles = vec![];

	for i in 0..10 {
		let singleton_clone = Arc::clone(&singleton);
		let handle = task::spawn(async move {
			let ctx = InjectionContext::builder(singleton_clone).build();

			// Inject request-scoped data
			let data = RequestData::inject(&ctx).await.unwrap();

			tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

			// Verify data is still accessible and cached
			let data_cached = RequestData::inject(&ctx).await.unwrap();
			assert_eq!(data, data_cached);

			i
		});
		handles.push(handle);
	}

	let results: Vec<_> = futures::future::join_all(handles)
		.await
		.into_iter()
		.map(|r| r.unwrap())
		.collect();

	// All tasks should complete successfully
	assert_eq!(results.len(), 10);
	for (i, result) in results.iter().enumerate() {
		assert_eq!(*result, i);
	}
}

#[tokio::test]
async fn test_injectable_mixed_scopes() {
	let singleton = Arc::new(SingletonScope::new());
	let ctx = InjectionContext::builder(Arc::clone(&singleton)).build();

	// Inject both request-scoped and singleton-scoped data
	let request_data = RequestData::inject(&ctx).await.unwrap();
	let singleton_data = SingletonData::inject(&ctx).await.unwrap();

	assert_eq!(request_data.value, "request-specific");
	assert_eq!(singleton_data.value, "singleton-shared");

	// Create a new context with the same singleton
	let ctx2 = InjectionContext::builder(Arc::clone(&singleton)).build();

	// Singleton data should be shared
	let singleton_data2 = SingletonData::inject(&ctx2).await.unwrap();
	assert_eq!(singleton_data, singleton_data2);

	// Request data should be independent
	let request_data2 = RequestData::inject(&ctx2).await.unwrap();
	assert_eq!(request_data2.value, "request-specific");

	// Verify they are different instances (not the same as ctx)
	ctx2.set_request(RequestData {
		value: "ctx2-modified".to_string(),
	});
	let request_data2_modified = RequestData::inject(&ctx2).await.unwrap();
	let request_data_original = RequestData::inject(&ctx).await.unwrap();

	assert_eq!(request_data2_modified.value, "ctx2-modified");
	assert_eq!(request_data_original.value, "request-specific");
}
