//! REST API integration tests module
//!
//! Cross-crate integration tests for REST API components.

mod rest {
	mod hyperlinked_serializer_tests;
	mod multi_term_tests;
	mod nested_orm_tests;
	mod nested_serializer_tests;
	mod openapi_derive_schema_tests;
	mod openapi_macro_integration_tests;
	mod openapi_parameter_recognition_tests;
	mod openapi_schema_generation_integration;
	mod ordering_field_tests;
	mod query_filter_tests;
	mod schema_integration_tests;
	mod searchable_tests;
	mod serializers_filters_integration;
	mod versioning_routers_integration;
}
