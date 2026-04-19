//! Unit tests for `Depends<T>`, `Option<Depends<T>>`, and `InjectionMetadata`

use async_trait::async_trait;
use reinhardt_di::injected::{DependencyScope, InjectionMetadata};
use reinhardt_di::{Depends, DiResult, Injectable, InjectionContext};
use reinhardt_test::fixtures::*;
use rstest::*;

// Test type definitions
#[derive(Clone, Debug, PartialEq)]
struct TestData {
	value: String,
}

// Injectable implementation is needed for Depends::resolve (registry-based resolution)
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
async fn depends_wraps_value() {
	// Arrange
	let data = TestData {
		value: "wrapped".to_string(),
	};

	// Act
	let depends = Depends::from_value(data);

	// Assert
	assert_eq!(depends.value, "wrapped");
}

#[rstest]
#[tokio::test]
async fn depends_metadata_stores_scope() {
	// Arrange
	let data = TestData {
		value: "metadata_test".to_string(),
	};
	let depends = Depends::from_value(data);

	// Act
	let metadata = depends.metadata();

	// Assert
	assert_eq!(metadata.scope, DependencyScope::Request);
	assert!(!metadata.cached);
}

#[rstest]
#[tokio::test]
async fn option_depends_some_value() {
	// Arrange
	let data = TestData {
		value: "optional_some".to_string(),
	};
	let depends = Depends::from_value(data);

	// Act
	let optional: Option<Depends<TestData>> = Some(depends);

	// Assert
	assert!(optional.is_some());
	assert_eq!(optional.unwrap().value, "optional_some");
}

#[rstest]
#[tokio::test]
async fn option_depends_none_value() {
	// Act
	let optional: Option<Depends<TestData>> = None;

	// Assert
	assert!(optional.is_none());
}

#[rstest]
#[tokio::test]
async fn depends_scope_singleton() {
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
async fn depends_scope_request(injection_context: InjectionContext) {
	// Act
	let depends = Depends::<TestData>::resolve(&injection_context, true)
		.await
		.unwrap();

	// Assert
	let metadata = depends.metadata();
	assert_eq!(metadata.scope, DependencyScope::Request);
	assert!(metadata.cached);
}
