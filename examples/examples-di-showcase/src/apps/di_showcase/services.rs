//! Custom Injectable service types for DI demonstration
//!
//! This module demonstrates how to create custom services that can be
//! automatically injected into HTTP handlers using `#[inject]`.

use reinhardt::async_trait::async_trait;
use reinhardt::di::{DiResult, Injectable, InjectionContext};
use std::sync::atomic::{AtomicU64, Ordering};

// ---------------------------------------------------------------------------
// Pattern 1: Injectable with Default values
// ---------------------------------------------------------------------------

/// A configuration service that provides application-level settings.
///
/// Demonstrates explicit `Injectable` implementation using `Default` values.
#[derive(Debug, Clone)]
pub struct AppConfig {
	pub app_name: String,
	pub version: String,
	pub max_items_per_page: usize,
}

impl Default for AppConfig {
	fn default() -> Self {
		Self {
			app_name: "DI Showcase".to_string(),
			version: "1.0.0".to_string(),
			max_items_per_page: 50,
		}
	}
}

#[async_trait]
impl Injectable for AppConfig {
	async fn inject(_ctx: &InjectionContext) -> DiResult<Self> {
		Ok(Self::default())
	}
}

// ---------------------------------------------------------------------------
// Pattern 2: Custom Injectable with explicit implementation
// ---------------------------------------------------------------------------

/// A greeting service that builds personalized messages.
///
/// Demonstrates explicit `Injectable` implementation with custom initialization.
#[derive(Debug, Clone)]
pub struct GreetingService {
	pub template: String,
}

#[async_trait]
impl Injectable for GreetingService {
	async fn inject(_ctx: &InjectionContext) -> DiResult<Self> {
		Ok(Self {
			template: "Hello, {}! Welcome to Reinhardt.".to_string(),
		})
	}
}

impl GreetingService {
	/// Format a greeting for the given name
	pub fn greet(&self, name: &str) -> String {
		self.template.replace("{}", name)
	}
}

// ---------------------------------------------------------------------------
// Pattern 3: Nested dependency (Service depends on another Injectable)
// ---------------------------------------------------------------------------

/// A request counter that tracks invocations across the application lifetime.
///
/// Uses `AtomicU64` for thread-safe counting without locking.
#[derive(Debug)]
pub struct RequestCounter {
	counter: AtomicU64,
}

impl Clone for RequestCounter {
	fn clone(&self) -> Self {
		Self {
			counter: AtomicU64::new(self.counter.load(Ordering::Relaxed)),
		}
	}
}

impl Default for RequestCounter {
	fn default() -> Self {
		Self {
			counter: AtomicU64::new(0),
		}
	}
}

#[async_trait]
impl Injectable for RequestCounter {
	async fn inject(_ctx: &InjectionContext) -> DiResult<Self> {
		Ok(Self::default())
	}
}

impl RequestCounter {
	/// Increment and return the new count
	pub fn increment(&self) -> u64 {
		self.counter.fetch_add(1, Ordering::Relaxed) + 1
	}

	/// Get the current count without incrementing
	pub fn current(&self) -> u64 {
		self.counter.load(Ordering::Relaxed)
	}
}

// ---------------------------------------------------------------------------
// Pattern 4: Service with nested Injectable dependency
// ---------------------------------------------------------------------------

/// A dashboard service that composes multiple injected dependencies.
///
/// Demonstrates how one service can depend on others via `InjectionContext`.
#[derive(Debug, Clone)]
pub struct DashboardService {
	pub app_config: AppConfig,
	pub greeting: GreetingService,
}

#[async_trait]
impl Injectable for DashboardService {
	async fn inject(ctx: &InjectionContext) -> DiResult<Self> {
		// Resolve nested dependencies from the injection context
		let app_config = AppConfig::inject(ctx).await?;
		let greeting = GreetingService::inject(ctx).await?;

		Ok(Self {
			app_config,
			greeting,
		})
	}
}

impl DashboardService {
	/// Build a dashboard summary
	pub fn summary(&self, user_name: &str) -> String {
		let greeting = self.greeting.greet(user_name);
		format!(
			"{} | App: {} v{}",
			greeting, self.app_config.app_name, self.app_config.version
		)
	}
}
