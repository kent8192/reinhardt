//! # Reinhardt OpenAPI
//!
//! OpenAPI 3.0 schema generation for Reinhardt REST APIs.
//!
//! ## Overview
//!
//! This crate provides automatic OpenAPI documentation generation for Reinhardt
//! REST APIs, including schema derivation, Swagger UI integration, and ViewSet
//! inspection.
//!
//! ## Features
//!
//! - **OpenAPI 3.0**: Full OpenAPI 3.0 specification support
//! - **Auto-generation**: Automatic schema generation from ViewSets
//! - **Customization**: Override and extend generated schemas
//! - **Swagger UI**: Built-in Swagger UI and ReDoc integration
//! - **YAML/JSON**: Export schemas in both formats
//! - **Schema Registry**: Centralized schema management with `$ref` references
//! - **Enum Support**: Tagged, adjacently tagged, and untagged enum handling
//! - **Serde Integration**: Support for `#[serde(rename)]`, `#[serde(skip)]`, and more
//!
//! ## Quick Start
//!
//! ```rust,no_run
//! use reinhardt_rest::openapi::{SchemaGenerator, OpenApiSchema};
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! // Generate schema from ViewSets
//! let generator = SchemaGenerator::new()
//!     .title("My API")
//!     .version("1.0.0")
//!     .description("API documentation");
//!
//! let schema = generator.generate()?;
//! let json = schema.to_json()?;
//! # Ok(())
//! # }
//! ```
//!
//! ## Schema Derive Macro
//!
//! The `#[derive(Schema)]` macro generates OpenAPI schema definitions from Rust types.
//!
//! ### Basic Usage
//!
//! ```rust,ignore
//! use reinhardt_rest::openapi::Schema;
//!
//! #[derive(Schema)]
//! struct User {
//!     id: i64,
//!     username: String,
//!     email: String,
//!     #[schema(example = "true")]
//!     is_active: bool,
//! }
//! ```
//!
//! ### Schema Attributes
//!
//! Field-level attributes:
//!
//! - `#[schema(example = "...")]`: Provide example value for documentation
//! - `#[schema(skip)]`: Exclude field from schema
//! - `#[schema(rename = "...")]`: Rename field in schema
//! - `#[schema(description = "...")]`: Add field description
//! - `#[schema(nullable)]`: Mark field as nullable
//! - `#[schema(format = "...")]`: Specify format (e.g., "email", "uri", "date-time")
//!
//! Container-level attributes:
//!
//! - `#[schema(rename_all = "...")]`: Apply case transformation (camelCase, snake_case, etc.)
//!
//! ### Serde Integration
//!
//! The Schema derive macro automatically respects serde attributes:
//!
//! ```rust,ignore
//! use serde::{Deserialize, Serialize};
//! use reinhardt_rest::openapi::Schema;
//!
//! #[derive(Serialize, Deserialize, Schema)]
//! #[serde(rename_all = "camelCase")]
//! struct UserResponse {
//!     user_id: i64,           // Becomes "userId" in schema
//!     #[serde(skip)]
//!     internal_field: String, // Excluded from schema
//!     #[serde(rename = "mail")]
//!     email: String,          // Becomes "mail" in schema
//! }
//! ```
//!
//! ### Enum Schemas
//!
//! Support for various enum representations:
//!
//! ```rust,ignore
//! use reinhardt_rest::openapi::Schema;
//!
//! // Simple enum (string schema)
//! #[derive(Schema)]
//! enum Status {
//!     Active,
//!     Inactive,
//!     Pending,
//! }
//!
//! // Tagged enum (object schema with discriminator)
//! #[derive(Schema)]
//! #[serde(tag = "type")]
//! enum Event {
//!     Created { id: i64 },
//!     Updated { id: i64, changes: Vec<String> },
//!     Deleted { id: i64 },
//! }
//! ```
//!
//! ## Schema Registry
//!
//! Manage and reference schemas centrally:
//!
//! ```rust,ignore
//! use reinhardt_rest::openapi::SchemaRegistry;
//!
//! let mut registry = SchemaRegistry::new();
//!
//! // Register a schema
//! registry.register::<User>();
//!
//! // Get reference to schema
//! let user_ref = registry.get_ref::<User>(); // Returns "#/components/schemas/User"
//! ```
//!
//! ## Swagger UI Integration
//!
//! ```rust,ignore
//! use reinhardt_rest::openapi::{SwaggerUI, RedocUI};
//!
//! // Swagger UI endpoint
//! let swagger = SwaggerUI::new("/api/openapi.json")
//!     .path("/docs")
//!     .title("API Documentation");
//!
//! // ReDoc endpoint
//! let redoc = RedocUI::new("/api/openapi.json")
//!     .path("/redoc");
//! ```

pub mod auto_schema;
pub mod config;
pub mod endpoint_inspector;
pub mod endpoints;
pub mod enum_schema;
pub mod generator;
// Allow module_inception: Re-exporting openapi submodule from openapi.rs
// is intentional for compatibility with existing imports (`reinhardt_rest::openapi::OpenAPI`)
#[allow(clippy::module_inception)]
pub mod openapi;
pub mod param_metadata;
pub mod registry;
pub mod schema_registration;
pub mod serde_attrs;
pub mod swagger;

use thiserror::Error;

pub use auto_schema::{SchemaObject, ToSchema};
#[allow(deprecated)] // Re-export deprecated OpenApiConfig for backward compatibility
pub use config::OpenApiConfig;
pub use endpoint_inspector::EndpointInspector;
pub use endpoints::generate_openapi_schema;
pub use enum_schema::{EnumSchemaBuilder, EnumTagging};
pub use generator::SchemaGenerator;
pub use openapi::{
	ArrayBuilder, Components, ComponentsExt, Header, Info, MediaType, ObjectBuilder, OpenApiSchema,
	OpenApiSchemaExt, Operation, OperationExt, Parameter, ParameterExt,
	ParameterIn as ParameterLocation, PathItem, PathItemExt, RefOr, RequestBody, Required,
	Response, ResponsesExt, Schema, SchemaExt, Server, Tag,
};
pub use param_metadata::{CookieParam, HeaderParam, ParameterMetadata, PathParam, QueryParam};
pub use registry::SchemaRegistry;
pub use reinhardt_openapi_macros::Schema;
pub use schema_registration::SchemaRegistration;
pub use serde_attrs::{FieldMetadata, RenameAll, SchemaBuilderExt};
pub use swagger::{RedocUI, SwaggerUI};
pub use utoipa::Number;

// Re-export utoipa and inventory for macro-generated code
pub use inventory;
pub use utoipa;

/// Errors that can occur during OpenAPI schema operations.
#[derive(Debug, Error)]
pub enum SchemaError {
	/// The schema definition is invalid.
	#[error("Invalid schema: {0}")]
	InvalidSchema(String),

	/// A serialization error occurred while processing the schema.
	#[error("Serialization error: {0}")]
	SerializationError(String),

	/// An error occurred in the schema inspector.
	#[error("Inspector error: {0}")]
	InspectorError(String),

	/// A JSON parsing or generation error.
	#[error("JSON error: {0}")]
	JsonError(#[from] serde_json::Error),
}

/// Result type for OpenAPI schema operations.
pub type SchemaResult<T> = std::result::Result<T, SchemaError>;
