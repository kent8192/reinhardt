//! # Reinhardt Metadata
//!
//! Metadata API for handling OPTIONS requests in Reinhardt framework.
//!
//! ## Features
//!
//! - **BaseMetadata**: Base trait for metadata providers
//! - **SimpleMetadata**: Default metadata implementation that returns view and field information
//! - **OpenAPI Schema Generation**: Convert field metadata to OpenAPI 3.0 schemas
//! - **Type Inference**: Automatic schema inference from Rust types
//! - **Validation Patterns**: Pre-defined regex patterns for common validation scenarios
//! - **Field Dependencies**: Define conditional requirements and field relationships
//! - Automatic field type detection
//! - Action-based metadata (POST, PUT, etc.)
//!
//! ## Example
//!
//! ```rust
//! use reinhardt_rest::metadata::{BaseMetadata, SimpleMetadata, MetadataOptions};
//!
//! let metadata = SimpleMetadata::new();
//! let mut options = MetadataOptions::default();
//! options.name = "User List".to_string();
//! options.description = "List all users".to_string();
//! options.allowed_methods = vec!["GET".to_string(), "POST".to_string()];
//! options.renders = vec!["application/json".to_string()];
//! options.parses = vec!["application/json".to_string()];
//! options.serializer_fields = None;
//! ```
//!
//! ## OpenAPI Schema Generation Example
//!
//! ```rust
//! use reinhardt_rest::metadata::{FieldInfoBuilder, FieldType, generate_field_schema};
//!
//! let field = FieldInfoBuilder::new(FieldType::String)
//!     .required(true)
//!     .min_length(3)
//!     .max_length(50)
//!     .build();
//!
//! let schema = generate_field_schema(&field);
//! assert_eq!(schema.schema_type, Some("string".to_string()));
//! ```
//!
//! ## Type Inference Example
//!
//! ```rust
//! use reinhardt_rest::metadata::SchemaInferencer;
//!
//! let inferencer = SchemaInferencer::new();
//! let schema = inferencer.infer_openapi_schema("Vec<String>");
//! assert_eq!(schema.schema_type, Some("array".to_string()));
//! ```
//!
//! ## Validation Pattern Example
//!
//! ```rust
//! use reinhardt_rest::metadata::ValidationPattern;
//!
//! let pattern = ValidationPattern::email();
//! assert!(pattern.is_valid("user@example.com"));
//! ```
//!
//! ## Field Dependencies Example
//!
//! ```rust
//! use reinhardt_rest::metadata::{DependencyManager, FieldDependency};
//!
//! let mut manager = DependencyManager::new();
//! manager.add_dependency(FieldDependency::requires("country", vec!["address"]));
//! ```

mod base;
mod dependencies;
mod fields;
mod inferencer;
mod options;
mod patterns;
mod response;
mod schema;
mod types;
mod validators;

// Re-export all public items
pub use base::{BaseMetadata, SimpleMetadata};
pub use dependencies::{DependencyManager, DependencyType, FieldDependency};
pub use fields::{FieldInfo, FieldInfoBuilder};
pub use inferencer::SchemaInferencer;
pub use options::{MetadataOptions, SerializerFieldInfo};
pub use patterns::ValidationPattern;
pub use response::{ActionMetadata, MetadataResponse};
pub use schema::{FieldSchema, generate_field_schema, generate_object_schema};
pub use types::{ChoiceInfo, FieldType, MetadataError};
pub use validators::FieldValidator;
