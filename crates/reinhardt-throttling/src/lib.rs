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

/// Adaptive throttling that adjusts limits based on server load.
pub mod adaptive;
/// Rate throttle for anonymous (unauthenticated) requests.
pub mod anon;
/// Throttle storage backends (memory, Redis).
pub mod backend;
/// Burst rate throttle allowing short traffic spikes.
pub mod burst;
/// Geographic region-based rate limiting.
pub mod geo;
/// Throttle key validation and sanitization.
pub mod key_validation;
/// Leaky bucket rate limiting algorithm.
pub mod leaky_bucket;
/// Scope-based throttle for per-view rate limits.
pub mod scoped;
/// Core throttle trait and error types.
pub mod throttle;
/// Tiered rate limiting with multiple user tiers.
pub mod tiered;
/// Time-of-day based rate limiting.
pub mod time_of_day;
/// Pluggable time provider for testability.
pub mod time_provider;
/// Token bucket rate limiting algorithm.
pub mod token_bucket;
/// Rate throttle for authenticated user requests.
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
