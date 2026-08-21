//! # reinhardt-core-serializers
//!
//! Core serialization and deserialization functionality for Reinhardt framework.
//!
//! This crate provides the foundational serialization infrastructure that is
//! ORM-agnostic and can be used across different layers of the framework.
//!
//! ## Features
//!
//! - **Serializer Traits**: Core `Serializer` and `Deserializer` traits
//! - **Field Types**: Django REST Framework-inspired fields with a public
//!   [`FieldErrorMessages`](crate::serializers::fields::FieldErrorMessages)
//!   member so struct-update construction keeps compiling
//! - **Validation**: Field-level and object-level validation support
//! - **Recursive Serialization**: Depth tracking and circular reference detection
//! - **Arena Allocation**: Memory-efficient serialization for deeply nested structures
//!
//! ## Feature Flags
//!
//! - `json` (default): JSON serialization support
//! - `xml`: XML serialization support
//! - `yaml`: YAML serialization support
//!
//! ## Examples
//!
//! ```rust
//! use reinhardt_core::serializers::{Serializer, JsonSerializer};
//! use serde::{Serialize, Deserialize};
//!
//! #[derive(Serialize, Deserialize)]
//! struct User {
//!     id: i64,
//!     name: String,
//! }
//!
//! let user = User { id: 1, name: "Alice".to_string() };
//! let serializer = JsonSerializer::<User>::new();
//! let json = serializer.serialize(&user).unwrap();
//! ```

pub mod arena;
pub mod fields;
pub mod recursive;
pub mod serializer;
pub mod validator;

// Re-export commonly used types
pub use serializer::{Deserializer, JsonSerializer, Serializer, SerializerError, ValidatorError};
pub use validator::{
	FieldLevelValidation, FieldValidator, ObjectLevelValidation, ObjectValidator, ValidationError,
	ValidationResult, validate_fields,
};
