//! gRPC service module.
//!
//! This module provides gRPC infrastructure including service adapters,
//! protobuf types, and server configuration.
//!
//! # Examples
//!
//! ```rust,no_run
//! use reinhardt::grpc::{GrpcServiceAdapter, GrpcServerConfig};
//! ```

#[cfg(feature = "grpc")]
pub use reinhardt_grpc::*;
