//! # Reinhardt REST
//!
//! **Unified REST API framework** for Reinhardt.
//!
//! This crate provides a complete REST API framework by integrating:
//! - **Serializers**: Data serialization and validation (from reinhardt-serializers)
//! - **Parsers**: Request body parsing (from reinhardt-parsers)
//! - **Authentication**: JWT, Token, Session, Basic auth (from reinhardt-auth)
//! - **Routers**: Automatic URL routing for ViewSets (from reinhardt-routers)
//! - **Browsable API**: HTML interface for API exploration (from reinhardt-browsable-api)
//!
//! ## Quick Start
//!
//! ```rust
//! use reinhardt_rest::parsers::JSONParser;
//!
//! // Create a JSON parser for handling JSON request bodies
//! let parser = JSONParser::new();
//! ```
//!
//! For router integration, see the `reinhardt_urls::routers` crate.
//!
//! ## Architecture
//!
//! Key modules in this crate:
//!
//! - [`browsable_api`]: HTML interface for interactive API exploration
//! - [`filters`]: Query parameter filtering for list endpoints
//! - [`metadata`]: API metadata and schema introspection utilities
//! - [`serializers`]: Data serialization, deserialization, and validation
//! - [`throttling`]: Request rate limiting and throttle policies
//! - [`versioning`]: API versioning strategies (URL path, header, query)
//! - [`authentication`]: REST authentication backends (JWT, Token, Session, Basic)
//! - [`response`]: Typed API response wrappers and pagination support
//! - [`schema`]: OpenAPI schema generation (requires `openapi` feature)
//!
//! ## Feature Flags
//!
//! | Feature | Default | Description |
//! |---------|---------|-------------|
//! | `serializers` | enabled | Data serialization and validation components |
//! | `parsers` | enabled | Request body parsing (JSON, Form, Multipart) |
//! | `jwt` | disabled | JWT authentication backend |
//! | `filters` | disabled | Query parameter filtering for querysets |
//! | `throttling` | disabled | Request rate limiting policies |
//! | `versioning` | disabled | API versioning strategies |
//! | `metadata` | disabled | API introspection and schema metadata |
//! | `pagination` | disabled | Cursor and page-number pagination |
//! | `browsable-api` | disabled | HTML browsable API interface |
//! | `openapi` | disabled | OpenAPI/Swagger schema generation and UI |
//! | `rest-full` | disabled | Enables all REST features |
//!
//! ## Testing
//!
//! This crate contains unit tests for the integrated modules.
//! Integration tests are located in `tests/integration/`.

pub mod browsable_api;
pub mod filters;
pub mod metadata;
pub mod serializers;
pub mod throttling;
pub mod versioning;

// Re-export internal crates (2024 edition module system)
// These modules represent the internal crates that are now part of reinhardt-rest

// Parsers module - now part of reinhardt-rest
#[cfg(feature = "parsers")]
pub use reinhardt_core::parsers;

// Re-export other internal crates
pub use reinhardt_core::negotiation;
pub use reinhardt_core::pagination;

// Core modules (merged from rest-core)
pub mod authentication;
pub mod response;
// NOTE: routers module removed to avoid circular dependency with reinhardt-urls
// Use reinhardt-urls::routers directly instead

#[cfg(feature = "openapi")]
pub mod schema;

// Re-export authentication types
pub use authentication::{
	AllowAny, AnonymousUser, AuthBackend, AuthResult, IsAdminUser, IsAuthenticated,
	IsAuthenticatedOrReadOnly, Permission, SimpleUser, User,
};

// Re-export JWT types conditionally
#[cfg(feature = "jwt")]
pub use authentication::{Claims, JwtAuth};

// Re-export response types
pub use response::{ApiResponse, IntoApiResponse, PaginatedResponse, ResponseBuilder};

// Re-export from specialized crates
pub use crate::browsable_api::*;

// Re-export integrated modules at top level for convenience
#[cfg(feature = "serializers")]
pub use crate::serializers::{
	ContentNegotiator, Deserializer, JsonSerializer, ModelSerializer, Serializer, SerializerError,
	UniqueTogetherValidator, UniqueValidator,
};

#[cfg(feature = "parsers")]
pub use reinhardt_core::parsers::{
	FileUploadParser, FormParser, JSONParser, MediaType, MultiPartParser, ParseError, ParseResult,
	Parser,
};

// OpenAPI module - integrated from former reinhardt-openapi subcrate
#[cfg(feature = "openapi")]
pub mod openapi;

// Re-export commonly used OpenAPI types
#[cfg(feature = "openapi")]
pub use crate::openapi::{
	ComponentsExt, EnumSchemaBuilder, EnumTagging, Info, OpenApiSchema, Operation, Parameter,
	ParameterLocation, PathItem, RequestBody, Response, Schema, SchemaExt, SchemaGenerator,
	SchemaRegistry, Server, ToSchema,
};

// Re-export builders
#[cfg(feature = "openapi")]
pub use crate::openapi::openapi::{
	ArrayBuilder, ComponentsBuilder, InfoBuilder, ObjectBuilder, OpenApiBuilder, OperationBuilder,
	ParameterBuilder, PathItemBuilder, PathsBuilder, RequestBodyBuilder, ResponsesBuilder,
	ServerBuilder, TagBuilder,
};

// Re-export OpenAPI ResponseBuilder with alias to avoid conflict with rest_core::ResponseBuilder
#[cfg(feature = "openapi")]
pub use crate::openapi::openapi::ResponseBuilder as OpenApiResponseBuilder;

// Re-export UI components
#[cfg(feature = "openapi")]
pub use crate::openapi::swagger::SwaggerUI;

#[cfg(test)]
mod tests {
	#[test]
	fn test_serializers_module_available() {
		#[cfg(feature = "serializers")]
		{
			use crate::JsonSerializer;
			use serde::{Deserialize, Serialize};

			#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
			struct TestData {
				name: String,
			}

			let _serializer = JsonSerializer::<TestData>::new();
		}
	}

	#[test]
	fn test_parsers_module_available() {
		#[cfg(feature = "parsers")]
		{
			use crate::JSONParser;
			let _parser = JSONParser::new();
		}
	}
}
