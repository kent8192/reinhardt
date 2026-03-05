//! Test resource management with automatic setup and teardown
//!
//! This module provides traits and helpers for managing test resources
//! with automatic cleanup, similar to pytest fixtures or JUnit's BeforeEach/AfterEach.
//!
//! ## Overview
//!
//! - `TestResource`: Per-test setup/teardown (BeforeEach/AfterEach pattern)
//! - `TeardownGuard`: RAII guard for automatic resource cleanup
//! - `SuiteResource`: Suite-wide shared resources (BeforeAll/AfterAll pattern)
//! - `SuiteGuard`: Reference-counted guard with automatic cleanup when last user drops
//!
//! ## Examples
//!
//! ### Per-test resource (BeforeEach/AfterEach)
//!
//! ```rust
//! use reinhardt_testkit::resource::{TestResource, TeardownGuard};
//! use rstest::*;
//!
//! struct TestEnv {
//!     temp_dir: std::path::PathBuf,
//! }
//!
//! impl TestResource for TestEnv {
//!     fn setup() -> Self {
//!         let temp = tempfile::tempdir().unwrap();
//!         Self { temp_dir: temp.path().to_path_buf() }
//!     }
//!
//!     fn teardown(&mut self) {
//!         // Cleanup code here
//!         let _ = std::fs::remove_dir_all(&self.temp_dir);
//!     }
//! }
//!
//! #[fixture]
//! fn ctx() -> TeardownGuard<TestEnv> {
//!     TeardownGuard::new()
//! }
//!
//! #[rstest]
//! fn test_something(ctx: TeardownGuard<TestEnv>) {
//!     // ctx.temp_dir is available
//!     // teardown() is automatically called when ctx goes out of scope
//! }
//! ```
//!
//! ### Suite-wide resource (BeforeAll/AfterAll)
//!
//! ```rust,no_run
//! use reinhardt_testkit::resource::{SuiteResource, SuiteGuard, acquire_suite};
//! use rstest::*;
//! use std::sync::{OnceLock, Mutex, Weak};
//!
//! struct DatabaseSuite {
//!     connection_string: String,
//! }
//!
//! impl SuiteResource for DatabaseSuite {
//!     fn init() -> Self {
//!         // Expensive setup (e.g., start test database)
//!         Self { connection_string: "test_db".to_string() }
//!     }
//! }
//!
//! impl Drop for DatabaseSuite {
//!     fn drop(&mut self) {
//!         // Cleanup when last test completes
//!         println!("Dropping suite resource");
//!     }
//! }
//!
//! static SUITE: OnceLock<Mutex<Weak<DatabaseSuite>>> = OnceLock::new();
//!
//! #[fixture]
//! fn suite() -> SuiteGuard<DatabaseSuite> {
//!     acquire_suite(&SUITE)
//! }
//!
//! #[rstest]
//! fn test_with_database(suite: SuiteGuard<DatabaseSuite>) {
//!     // suite.connection_string is available
//!     // Drop is called automatically when last test finishes
//! }
//! ```

use async_dropper::{AsyncDrop, AsyncDropper};
use std::ops::{Deref, DerefMut};
use std::sync::{Arc, Mutex, OnceLock, Weak};

/// Per-test resource with setup and teardown hooks
///
/// Implement this trait to define test resources that need
/// initialization before each test and cleanup after each test.
///
/// # Examples
///
/// ```rust
/// use reinhardt_testkit::resource::TestResource;
///
/// struct TestEnv {
///     data: Vec<String>,
/// }
///
/// impl TestResource for TestEnv {
///     fn setup() -> Self {
///         Self { data: vec![] }
///     }
///
///     fn teardown(&mut self) {
///         self.data.clear();
///     }
/// }
/// ```
pub trait TestResource: Sized {
	/// Setup hook called before each test (BeforeEach)
	fn setup() -> Self;

	/// Teardown hook called after each test (AfterEach)
	///
	/// This is called automatically by `TeardownGuard::drop`,
	/// ensuring cleanup even if the test panics.
	fn teardown(&mut self);
}

/// RAII guard for automatic test resource cleanup
///
/// This guard ensures `teardown()` is called when the guard
/// goes out of scope, even if the test panics.
///
/// # Examples
///
/// ```rust
/// use reinhardt_testkit::resource::{TestResource, TeardownGuard};
/// use rstest::*;
///
/// struct TestEnv;
///
/// impl TestResource for TestEnv {
///     fn setup() -> Self { Self }
///     fn teardown(&mut self) { }
/// }
///
/// #[fixture]
/// fn ctx() -> TeardownGuard<TestEnv> {
///     TeardownGuard::new()
/// }
///
/// #[rstest]
/// fn test_example(ctx: TeardownGuard<TestEnv>) {
///     // Test code here
///     // teardown() is automatically called
/// }
/// ```
pub struct TeardownGuard<F: TestResource>(F);

impl<F: TestResource> TeardownGuard<F> {
	/// Create a new teardown guard with resource setup
	pub fn new() -> Self {
		Self(F::setup())
	}
}

impl<F: TestResource> Default for TeardownGuard<F> {
	fn default() -> Self {
		Self::new()
	}
}

impl<F: TestResource> Drop for TeardownGuard<F> {
	fn drop(&mut self) {
		self.0.teardown();
	}
}

impl<F: TestResource> Deref for TeardownGuard<F> {
	type Target = F;

	fn deref(&self) -> &F {
		&self.0
	}
}

impl<F: TestResource> DerefMut for TeardownGuard<F> {
	fn deref_mut(&mut self) -> &mut F {
		&mut self.0
	}
}

/// Suite-wide shared resource (BeforeAll/AfterAll pattern)
///
/// Implement this trait for resources that should be shared
/// across multiple tests and cleaned up when all tests complete.
///
/// # Examples
///
/// ```rust,no_run
/// use reinhardt_testkit::resource::SuiteResource;
///
/// struct DatabaseSuite {
///     url: String,
/// }
///
/// impl SuiteResource for DatabaseSuite {
///     fn init() -> Self {
///         // Expensive setup
///         Self { url: "postgres://localhost/test".to_string() }
///     }
/// }
///
/// impl Drop for DatabaseSuite {
///     fn drop(&mut self) {
///         // Cleanup when last test finishes
///         println!("Shutting down test database");
///     }
/// }
/// ```
pub trait SuiteResource: Send + Sync + 'static {
	/// Initialize suite resource (BeforeAll)
	///
	/// This is called once when the first test needs the resource.
	fn init() -> Self;
}

/// Guard for suite-wide shared resource
///
/// Uses `OnceLock + Weak<Arc<T>>` pattern to ensure:
/// - Resource is initialized only once
/// - Resource is dropped when last test completes
///
/// # Examples
///
/// ```rust,no_run
/// use reinhardt_testkit::resource::{SuiteResource, SuiteGuard, acquire_suite};
/// use rstest::*;
/// use std::sync::{OnceLock, Mutex, Weak};
///
/// struct MySuite;
///
/// impl SuiteResource for MySuite {
///     fn init() -> Self { Self }
/// }
///
/// static SUITE: OnceLock<Mutex<Weak<MySuite>>> = OnceLock::new();
///
/// #[fixture]
/// fn suite() -> SuiteGuard<MySuite> {
///     acquire_suite(&SUITE)
/// }
/// ```
pub struct SuiteGuard<T: SuiteResource> {
	arc: Arc<T>,
}

impl<T: SuiteResource> Deref for SuiteGuard<T> {
	type Target = T;

	fn deref(&self) -> &T {
		&self.arc
	}
}

/// Acquire suite-wide shared resource
///
/// This function uses `OnceLock + Weak<Arc<T>>` pattern to:
/// 1. Initialize resource once on first call
/// 2. Reuse existing resource for subsequent calls
/// 3. Drop resource when last guard is dropped
///
/// # Examples
///
/// ```rust,no_run
/// use reinhardt_testkit::resource::{SuiteResource, acquire_suite};
/// use std::sync::{OnceLock, Mutex, Weak};
///
/// struct MySuite {
///     value: i32,
/// }
///
/// impl SuiteResource for MySuite {
///     fn init() -> Self {
///         Self { value: 42 }
///     }
/// }
///
/// static SUITE: OnceLock<Mutex<Weak<MySuite>>> = OnceLock::new();
///
/// let guard1 = acquire_suite(&SUITE);
/// let guard2 = acquire_suite(&SUITE);  // Reuses same instance
/// assert_eq!(guard1.value, 42);
/// assert_eq!(guard2.value, 42);
/// ```
pub fn acquire_suite<T: SuiteResource>(cell: &'static OnceLock<Mutex<Weak<T>>>) -> SuiteGuard<T> {
	let mutex = cell.get_or_init(|| Mutex::new(Weak::new()));
	let mut weak = mutex
		.lock()
		.expect("Suite resource mutex poisoned - a test panicked while holding the lock");

	// Try to upgrade existing Weak reference
	if let Some(existing) = weak.upgrade() {
		return SuiteGuard { arc: existing };
	}

	// Initialize new resource
	let arc = Arc::new(T::init());
	*weak = Arc::downgrade(&arc);

	SuiteGuard { arc }
}

/// Async version of TestResource for async setup/teardown
///
/// Implement this trait for test resources that require
/// asynchronous initialization or cleanup.
///
/// # Examples
///
/// ```rust
/// use reinhardt_testkit::resource::AsyncTestResource;
///
/// struct AsyncTestEnv {
///     connection: String,
/// }
///
/// #[async_trait::async_trait]
/// impl AsyncTestResource for AsyncTestEnv {
///     async fn setup() -> Self {
///         // Async initialization
///         Self { connection: "test_db".to_string() }
///     }
///
///     async fn teardown(self) {
///         // Async cleanup
///     }
/// }
/// ```
#[async_trait::async_trait]
pub trait AsyncTestResource: Sized + Send {
	/// Async setup hook called before each test
	async fn setup() -> Self;

	/// Async teardown hook called after each test
	///
	/// Takes ownership of self to ensure cleanup.
	async fn teardown(self);
}

// Internal wrapper for async drop implementation
struct AsyncResourceWrapper<F: AsyncTestResource> {
	resource: Option<F>,
}

impl<F: AsyncTestResource> Default for AsyncResourceWrapper<F> {
	fn default() -> Self {
		Self { resource: None }
	}
}

#[async_trait::async_trait]
impl<F: AsyncTestResource> AsyncDrop for AsyncResourceWrapper<F> {
	async fn async_drop(&mut self) {
		if let Some(resource) = self.resource.take() {
			resource.teardown().await;
		}
	}
}

/// RAII guard for async test resource cleanup using async-dropper
///
/// This guard automatically calls `teardown()` when dropped, using the `async-dropper` crate
/// to properly handle async cleanup in Drop.
///
/// # Important
///
/// **Requires multi-threaded tokio runtime.** Use `#[tokio::test(flavor = "multi_thread")]`
/// instead of `#[tokio::test]` because async-dropper uses blocking operations internally.
///
/// # Examples
///
/// ```rust
/// use reinhardt_testkit::resource::{AsyncTestResource, AsyncTeardownGuard};
///
/// struct AsyncTestEnv {
///     value: i32,
/// }
///
/// #[async_trait::async_trait]
/// impl AsyncTestResource for AsyncTestEnv {
///     async fn setup() -> Self {
///         Self { value: 42 }
///     }
///     async fn teardown(self) {
///         // Cleanup code here
///     }
/// }
///
/// #[tokio::test(flavor = "multi_thread")]
/// async fn test_example() {
///     let guard = AsyncTeardownGuard::<AsyncTestEnv>::new().await;
///     assert_eq!(guard.value, 42);
///     // Cleanup automatically called when guard is dropped
/// }
/// ```
pub struct AsyncTeardownGuard<F: AsyncTestResource + 'static> {
	inner: AsyncDropper<AsyncResourceWrapper<F>>,
}

impl<F: AsyncTestResource + 'static> AsyncTeardownGuard<F> {
	/// Create a new async teardown guard with resource setup
	pub async fn new() -> Self {
		let resource = F::setup().await;
		let wrapper = AsyncResourceWrapper {
			resource: Some(resource),
		};
		Self {
			inner: AsyncDropper::new(wrapper),
		}
	}
}

impl<F: AsyncTestResource + 'static> Deref for AsyncTeardownGuard<F> {
	type Target = F;

	fn deref(&self) -> &F {
		// Access resource through AsyncDropper -> AsyncResourceWrapper -> F
		self.inner
			.inner()
			.resource
			.as_ref()
			.expect("Resource already dropped")
	}
}

impl<F: AsyncTestResource + 'static> DerefMut for AsyncTeardownGuard<F> {
	fn deref_mut(&mut self) -> &mut F {
		// Access resource through AsyncDropper -> AsyncResourceWrapper -> F
		self.inner
			.inner_mut()
			.resource
			.as_mut()
			.expect("Resource already dropped")
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	struct Counter {
		setup_count: usize,
		teardown_count: usize,
	}

	impl TestResource for Counter {
		fn setup() -> Self {
			Self {
				setup_count: 1,
				teardown_count: 0,
			}
		}

		fn teardown(&mut self) {
			self.teardown_count += 1;
		}
	}

	#[test]
	fn test_teardown_guard() {
		let mut guard = TeardownGuard::<Counter>::new();
		assert_eq!(guard.setup_count, 1);
		assert_eq!(guard.teardown_count, 0);

		// Manually trigger teardown for testing
		guard.teardown();
		assert_eq!(guard.teardown_count, 1);
	}

	struct SuiteCounter {
		value: i32,
	}

	impl SuiteResource for SuiteCounter {
		fn init() -> Self {
			Self { value: 42 }
		}
	}

	#[test]
	fn test_suite_guard() {
		static SUITE: OnceLock<Mutex<Weak<SuiteCounter>>> = OnceLock::new();

		let guard1 = acquire_suite(&SUITE);
		assert_eq!(guard1.value, 42);

		let guard2 = acquire_suite(&SUITE);
		assert_eq!(guard2.value, 42);

		// Both guards should point to the same instance
		assert!(Arc::ptr_eq(&guard1.arc, &guard2.arc));
	}

	struct AsyncCounter {
		value: i32,
	}

	#[async_trait::async_trait]
	impl AsyncTestResource for AsyncCounter {
		async fn setup() -> Self {
			Self { value: 42 }
		}

		async fn teardown(self) {
			// Cleanup
		}
	}

	#[tokio::test(flavor = "multi_thread")]
	async fn test_async_teardown_guard_auto_cleanup() {
		// Test automatic cleanup when guard is dropped in tokio runtime
		{
			let guard = AsyncTeardownGuard::<AsyncCounter>::new().await;
			assert_eq!(guard.value, 42);
			// Guard is automatically cleaned up when dropped
		}
		// async-dropper ensures cleanup completes before continuing
	}
}
