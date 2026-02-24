//! Tests for feature flags in reinhardt-grpc
//!
//! These tests verify conditional compilation based on feature flags,
//! particularly the `di` (dependency injection) feature.

// ============================================================================
// Tests with di feature enabled
// ============================================================================

#[cfg(feature = "di")]
mod di_feature_enabled {
	use reinhardt_di::InjectionContext;
	use reinhardt_grpc::GrpcRequestExt;
	use std::sync::Arc;
	use tonic::Request;

	#[test]
	fn test_di_feature_enabled_compilation() {
		// This test verifies that the code compiles when di feature is enabled
		// Simply compiling this module is sufficient to test

		// Verify that GrpcRequestExt is available
		assert!(true, "GrpcRequestExt is available when di feature enabled");
	}

	#[test]
	fn test_grpc_request_ext_get_di_context() {
		// Create a request with DI context extensions
		let mut request = Request::new(());

		// Create InjectionContext using builder pattern
		let singleton_scope = reinhardt_di::SingletonScope::new();
		let di_context = Arc::new(InjectionContext::builder(singleton_scope).build());

		// Add DI context to extensions
		request.extensions_mut().insert(di_context.clone());

		// Get DI context using GrpcRequestExt trait
		let extracted = request.get_di_context::<Arc<InjectionContext>>();

		assert!(
			extracted.is_some(),
			"Should be able to extract DI context when present"
		);
	}

	#[test]
	fn test_grpc_request_ext_missing_context() {
		// Create a request without DI context extensions
		let request = Request::new(());

		// Get DI context using GrpcRequestExt trait
		let extracted = request.get_di_context::<Arc<InjectionContext>>();

		assert!(
			extracted.is_none(),
			"Should return None when DI context is missing"
		);
	}

	// Note: Testing the grpc_handler macro requires a more complex setup
	// with actual handler functions. This will be done in integration tests.
}

// ============================================================================
// Tests with di feature disabled
// ============================================================================

#[cfg(not(feature = "di"))]
mod di_feature_disabled {
	use async_trait::async_trait;
	use reinhardt_grpc::adapter::{GrpcServiceAdapter, GrpcSubscriptionAdapter};
	use reinhardt_grpc::error::{GrpcError, GrpcResult};

	// ===== Mock implementations for compilation testing =====
	struct MockAdapter;

	// Allow: mock implementations for compilation testing only (never called)
	#[async_trait]
	#[allow(clippy::unimplemented)]
	impl GrpcServiceAdapter for MockAdapter {
		type Input = String;
		type Output = String;
		type Error = GrpcError;

		async fn call(&self, _input: Self::Input) -> Result<Self::Output, Self::Error> {
			unimplemented!("Mock implementation for compilation test only")
		}
	}

	struct MockSubscriptionAdapter;

	// Allow: mock implementations for compilation testing only (never called)
	#[allow(clippy::unimplemented)]
	impl GrpcSubscriptionAdapter for MockSubscriptionAdapter {
		type Proto = String;
		type GraphQL = String;
		type Error = GrpcError;

		fn map_event(&self, _proto: Self::Proto) -> Option<Self::GraphQL> {
			unimplemented!("Mock implementation for compilation test only")
		}

		fn handle_error(&self, _error: Self::Error) -> String {
			unimplemented!("Mock implementation for compilation test only")
		}
	}

	// ===== Tests =====
	#[test]
	fn test_di_feature_disabled_compilation() {
		// This test verifies that basic gRPC functionality works without di feature

		// Verify that adapter traits are available
		let _service_trait: &dyn GrpcServiceAdapter<Input = String, Output = String, Error = GrpcError> =
			&MockAdapter;
		let _subscription_trait: &dyn GrpcSubscriptionAdapter<Proto = String, GraphQL = String, Error = GrpcError> =
			&MockSubscriptionAdapter;

		// Verify that error types are available
		let _error = GrpcError::Connection("test".to_string());
		let _result: GrpcResult<String> = Ok("test".to_string());

		assert!(true, "Basic gRPC functionality compiles without di feature");
	}

	#[test]
	fn test_basic_grpc_without_di() {
		// This test verifies that users can define adapters without DI
		// Local struct definitions are intentionally not instantiated - this is a compile-time test

		#[allow(dead_code)] // Compile-time test only - struct is never constructed
		struct MockAdapter;

		#[async_trait]
		impl GrpcServiceAdapter for MockAdapter {
			type Input = String;
			type Output = String;
			type Error = GrpcError;

			async fn call(&self, input: Self::Input) -> Result<Self::Output, Self::Error> {
				Ok(format!("Processed: {}", input))
			}
		}

		#[allow(dead_code)] // Compile-time test only - struct is never constructed
		struct MockSubscriptionAdapter;

		impl GrpcSubscriptionAdapter for MockSubscriptionAdapter {
			type Proto = String;
			type GraphQL = String;
			type Error = GrpcError;

			fn map_event(&self, proto: Self::Proto) -> Option<Self::GraphQL> {
				Some(format!("Mapped: {}", proto))
			}

			fn handle_error(&self, error: Self::Error) -> String {
				format!("Handled: {}", error)
			}
		}

		// The test passes if the code compiles
		assert!(true, "Can define and use adapters without DI");
	}
}

// ============================================================================
// Common tests (run regardless of feature flags)
// ============================================================================

#[cfg(test)]
mod common_tests {
	use prost::Message;
	use reinhardt_grpc::proto::common;

	#[test]
	fn test_proto_types_available_with_all_features() {
		// Proto types should always be available regardless of feature flags

		let timestamp = common::Timestamp {
			seconds: 1000,
			nanos: 500_000_000,
		};

		let encoded = timestamp.encode_to_vec();
		let decoded = common::Timestamp::decode(&encoded[..]);

		assert!(
			decoded.is_ok(),
			"Proto types should work with any feature combination"
		);
	}
}
