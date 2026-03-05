//! Unit tests for Provider and ProviderFn

use reinhardt_di::InjectionContext;
use reinhardt_di::provider::{Provider, ProviderFn};
use reinhardt_test::fixtures::*;
use rstest::*;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

// Test type definitions
#[derive(Clone, Debug, PartialEq)]
struct TestValue {
	data: String,
}

#[rstest]
#[tokio::test]
async fn provider_fn_executes() {
	// Arrange
	let provider = || async {
		Ok(TestValue {
			data: "test".to_string(),
		})
	};

	// Act
	let future = provider.provide();
	let result = future.into_inner().await;

	// Assert
	assert!(result.is_ok());
	let any_box = result.unwrap();
	let value = any_box.downcast::<TestValue>().unwrap();
	assert_eq!(value.data, "test");
}

#[rstest]
#[tokio::test]
async fn provider_with_context(injection_context: InjectionContext) {
	// Arrange
	injection_context.set_request("context_data".to_string());

	let ctx_arc = Arc::new(injection_context.clone());
	let provider_fn = ProviderFn::new(Arc::new(move || {
		let ctx = Arc::clone(&ctx_arc);
		let future = async move {
			let data: Option<Arc<String>> = ctx.get_request();
			Ok(Box::new(TestValue {
				data: data.map(|s| (*s).clone()).unwrap_or_default(),
			}) as Box<dyn std::any::Any + Send + Sync>)
		};
		reinhardt_di::provider::ProviderFuture::new(Box::pin(future))
	}));

	// Act
	let future = provider_fn.as_fn()();
	let result = future.into_inner().await;

	// Assert
	assert!(result.is_ok());
	let any_box = result.unwrap();
	let value = any_box.downcast::<TestValue>().unwrap();
	assert_eq!(value.data, "context_data");
}

#[rstest]
#[tokio::test]
async fn provider_returns_error() {
	// Arrange
	let provider = || async {
		Err::<TestValue, reinhardt_di::DiError>(reinhardt_di::DiError::Internal {
			message: "provider error".to_string(),
		})
	};

	// Act
	let future = provider.provide();
	let result = future.into_inner().await;

	// Assert
	assert!(result.is_err());
}

#[rstest]
#[tokio::test]
async fn provider_cached(injection_context: InjectionContext) {
	// Arrange
	static CALL_COUNT: AtomicU32 = AtomicU32::new(0);
	CALL_COUNT.store(0, Ordering::SeqCst);

	let provider = || async {
		CALL_COUNT.fetch_add(1, Ordering::SeqCst);
		Ok(TestValue {
			data: "cached".to_string(),
		})
	};

	// Act - First call
	let future1 = provider.provide();
	let result1 = future1.into_inner().await.unwrap();
	let value1 = result1.downcast::<TestValue>().unwrap();

	injection_context.set_request((*value1).clone());

	// Get from cache
	let cached: Option<Arc<TestValue>> = injection_context.get_request();

	// Assert
	assert!(cached.is_some());
	assert_eq!(cached.unwrap().data, "cached");
	assert_eq!(CALL_COUNT.load(Ordering::SeqCst), 1);
}
