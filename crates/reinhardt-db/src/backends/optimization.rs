//! Database backend optimization utilities
//!
//! This module provides optimization features for database backends:
//! - Connection pooling configuration
//! - Query caching
//! - Batch operations

pub mod batch_ops;
pub mod connection_pool;
pub mod query_cache;

pub use batch_ops::{BatchInsertBuilder, BatchOperations, BatchUpdateBuilder, QuoteStyle};
pub use connection_pool::{OptimizedPoolBuilder, PoolOptimizationConfig};
pub use query_cache::{CachedQuery, QueryCache, QueryCacheConfig};
