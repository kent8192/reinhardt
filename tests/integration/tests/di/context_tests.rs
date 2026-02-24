//! Unit tests for InjectionContext and InjectionContextBuilder

use reinhardt_di::{InjectionContext, SingletonScope};
use reinhardt_test::fixtures::*;
use rstest::*;
use std::sync::Arc;

#[cfg(feature = "params")]
use reinhardt_di::params::{ParamContext, Request};

// Test type definitions
#[derive(Clone, Debug, PartialEq)]
struct TestConfig {
	value: String,
}

#[rstest]
fn builder_creates_context_with_singleton_scope(singleton_scope: Arc<SingletonScope>) {
	// Arrange & Act
	let ctx = InjectionContext::builder(singleton_scope.clone()).build();

	// Assert
	assert!(Arc::ptr_eq(ctx.singleton_scope(), &singleton_scope));
}

#[cfg(feature = "params")]
#[rstest]
fn with_request_sets_http_request(singleton_scope: Arc<SingletonScope>) {
	// Arrange
	let request = Request::builder()
		.method(hyper::Method::GET)
		.uri("/test")
		.body(())
		.unwrap();

	// Act
	let ctx = InjectionContext::builder(singleton_scope)
		.with_request(request)
		.build();

	// Assert
	let retrieved_request = ctx.get_http_request();
	assert!(retrieved_request.is_some());
	assert_eq!(retrieved_request.unwrap().uri(), "/test");
}

#[cfg(feature = "params")]
#[rstest]
fn with_param_context_sets_param_context(singleton_scope: Arc<SingletonScope>) {
	// Arrange
	let param_context = ParamContext::new();

	// Act
	let ctx = InjectionContext::builder(singleton_scope)
		.with_param_context(param_context)
		.build();

	// Assert
	assert!(ctx.get_param_context().is_some());
}

#[rstest]
fn get_request_returns_none_when_not_set(injection_context: InjectionContext) {
	// Act
	let value: Option<Arc<TestConfig>> = injection_context.get_request();

	// Assert
	assert!(value.is_none());
}

#[rstest]
fn get_singleton_retrieves_value(singleton_scope: Arc<SingletonScope>) {
	// Arrange
	let config = TestConfig {
		value: "singleton_value".to_string(),
	};
	singleton_scope.set(config.clone());

	let ctx = InjectionContext::builder(singleton_scope).build();

	// Act
	let retrieved: Option<Arc<TestConfig>> = ctx.get_singleton();

	// Assert
	assert!(retrieved.is_some());
	assert_eq!(retrieved.unwrap().value, "singleton_value");
}

#[rstest]
fn set_singleton_stores_value(injection_context: InjectionContext) {
	// Arrange
	let config = TestConfig {
		value: "test_singleton".to_string(),
	};

	// Act
	injection_context.set_singleton(config.clone());

	// Assert
	let retrieved: Option<Arc<TestConfig>> = injection_context.get_singleton();
	assert!(retrieved.is_some());
	assert_eq!(retrieved.unwrap().value, "test_singleton");
}

#[rstest]
fn get_request_scope_retrieves_value(injection_context: InjectionContext) {
	// Arrange
	let config = TestConfig {
		value: "request_value".to_string(),
	};
	injection_context.set_request(config.clone());

	// Act
	let retrieved: Option<Arc<TestConfig>> = injection_context.get_request();

	// Assert
	assert!(retrieved.is_some());
	assert_eq!(retrieved.unwrap().value, "request_value");
}

#[rstest]
fn set_request_scope_stores_value(injection_context: InjectionContext) {
	// Arrange
	let config = TestConfig {
		value: "test_request".to_string(),
	};

	// Act
	injection_context.set_request(config.clone());

	// Assert
	let retrieved: Option<Arc<TestConfig>> = injection_context.get_request();
	assert!(retrieved.is_some());
	assert_eq!(retrieved.unwrap().value, "test_request");
}

#[rstest]
fn clone_creates_separate_request_scope(singleton_scope: Arc<SingletonScope>) {
	// Arrange
	let ctx1 = InjectionContext::builder(singleton_scope).build();
	let config1 = TestConfig {
		value: "ctx1_request".to_string(),
	};
	ctx1.set_request(config1.clone());

	// Act
	let ctx2 = ctx1.clone();
	let config2 = TestConfig {
		value: "ctx2_request".to_string(),
	};
	ctx2.set_request(config2.clone());

	// Assert
	let retrieved1: Option<Arc<TestConfig>> = ctx1.get_request();
	let retrieved2: Option<Arc<TestConfig>> = ctx2.get_request();

	assert_eq!(retrieved1.unwrap().value, "ctx1_request");
	assert_eq!(retrieved2.unwrap().value, "ctx2_request");
}

#[rstest]
fn clone_shares_singleton_scope(singleton_scope: Arc<SingletonScope>) {
	// Arrange
	let ctx1 = InjectionContext::builder(singleton_scope).build();
	let config = TestConfig {
		value: "shared_singleton".to_string(),
	};
	ctx1.set_singleton(config.clone());

	// Act
	let ctx2 = ctx1.clone();

	// Assert
	let retrieved1: Option<Arc<TestConfig>> = ctx1.get_singleton();
	let retrieved2: Option<Arc<TestConfig>> = ctx2.get_singleton();

	assert!(retrieved1.is_some());
	assert!(retrieved2.is_some());
	assert_eq!(retrieved1.as_ref().unwrap().value, "shared_singleton");
	assert_eq!(retrieved2.as_ref().unwrap().value, "shared_singleton");

	// Verify they point to the same Arc
	assert!(Arc::ptr_eq(
		retrieved1.as_ref().unwrap(),
		retrieved2.as_ref().unwrap()
	));
}
