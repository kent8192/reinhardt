//! # Reinhardt Throttling
//!
//! Rate limiting and throttling for Reinhardt framework
//!
//! This crate provides flexible rate limiting capabilities with multiple algorithms
//! and backend storage options.
//!
//! ## Features
//!
//! - **Token Bucket**: Allows burst traffic while maintaining average rate
//! - **Leaky Bucket**: Smooths burst traffic by processing at constant rate
//! - **Adaptive Throttling**: Dynamically adjusts rates based on system load
//! - **Geo-based Limiting**: Different rates per geographic region
//! - **Time-of-day Limiting**: Different rates for peak/off-peak hours
//!
//! ## Backends
//!
//! - **Memory**: In-memory storage (default)
//! - **Redis**: Distributed rate limiting with Redis (feature: `redis-backend`)

pub mod adaptive;
pub mod anon;
pub mod backend;
pub mod burst;
pub mod geo;
pub mod key_validation;
pub mod leaky_bucket;
pub mod scoped;
pub mod throttle;
pub mod tiered;
pub mod time_of_day;
pub mod time_provider;
pub mod token_bucket;
pub mod user;

pub use adaptive::{AdaptiveConfig, AdaptiveThrottle, LoadMetrics};
pub use anon::AnonRateThrottle;
pub use backend::{MemoryBackend, ThrottleBackend};
pub use burst::BurstRateThrottle;
pub use geo::{GeoRateConfig, GeoRateThrottle};
pub use leaky_bucket::{LeakyBucketConfig, LeakyBucketThrottle};
pub use scoped::ScopedRateThrottle;
pub use throttle::{Throttle, ThrottleError, ThrottleResult};
pub use tiered::{Tier, TieredRateThrottle};
pub use time_of_day::{TimeOfDayConfig, TimeOfDayThrottle, TimeRange};
pub use time_provider::{MockTimeProvider, SystemTimeProvider, TimeProvider};
pub use token_bucket::{TokenBucket, TokenBucketConfig, TokenBucketConfigBuilder};
pub use user::UserRateThrottle;

#[cfg(feature = "redis-backend")]
pub use backend::RedisThrottleBackend;
