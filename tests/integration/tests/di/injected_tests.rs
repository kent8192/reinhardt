//! Unit tests for Injected<T>, OptionalInjected<T>, and InjectionMetadata

use async_trait::async_trait;
use reinhardt_di::injected::{DependencyScope, Injected, InjectionMetadata, OptionalInjected};
use reinhardt_di::{DiResult, Injectable, InjectionContext};
use reinhardt_test::fixtures::*;
use rstest::*;

// Test type definitions
#[derive(Clone, Debug, PartialEq)]
struct TestData {
	value: String,
}

#[async_trait]
impl Injectable for TestData {
	async fn inject(_ctx: &InjectionContext) -> DiResult<Self> {
		Ok(TestData {
			value: "test_data".to_string(),
		})
	}
}

#[rstest]
#[tokio::test]
async fn injected_wraps_value() {
	// Arrange
	let data = TestData {
		value: "wrapped".to_string(),
	};

	// Act
	let injected = Injected::from_value(data);

	// Assert
	assert_eq!(injected.value, "wrapped");
}

#[rstest]
#[tokio::test]
async fn injected_metadata_stores_scope() {
	// Arrange
	let data = TestData {
		value: "metadata_test".to_string(),
	};
	let injected = Injected::from_value(data);

	// Act
	let metadata = injected.metadata();

	// Assert
	assert_eq!(metadata.scope, DependencyScope::Request);
	assert!(!metadata.cached);
}

#[rstest]
#[tokio::test]
async fn optional_injected_some_value() {
	// Arrange
	let data = TestData {
		value: "optional_some".to_string(),
	};
	let injected = Injected::from_value(data);

	// Act
	let optional: OptionalInjected<TestData> = Some(injected);

	// Assert
	assert!(optional.is_some());
	assert_eq!(optional.unwrap().value, "optional_some");
}

#[rstest]
#[tokio::test]
async fn optional_injected_none_value() {
	// Act
	let optional: OptionalInjected<TestData> = None;

	// Assert
	assert!(optional.is_none());
}

#[rstest]
#[tokio::test]
async fn injected_scope_singleton() {
	// Arrange
	let metadata = InjectionMetadata {
		scope: DependencyScope::Singleton,
		cached: true,
	};

	// Assert
	assert_eq!(metadata.scope, DependencyScope::Singleton);
	assert!(metadata.cached);
}

#[rstest]
#[tokio::test]
async fn injected_scope_request(injection_context: InjectionContext) {
	// Act
	let injected = Injected::<TestData>::resolve(&injection_context)
		.await
		.unwrap();

	// Assert
	let metadata = injected.metadata();
	assert_eq!(metadata.scope, DependencyScope::Request);
	assert!(metadata.cached);
}
