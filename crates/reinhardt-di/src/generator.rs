//! Generator-based dependency resolution
//!
//! This module provides generator syntax for lazy dependency resolution,
//! allowing dependencies to be resolved on-demand in a streaming fashion.
//!
//! # Note
//!
//! **Workaround for Unstable Native Async Yield:**
//!
//! Rust's native async generators (with `yield` keyword) are not yet stable as of 2025.
//! This implementation uses the `genawaiter` crate as a workaround to provide
//! generator-like functionality on stable Rust.
//!
//! When Rust's native async generators become stable, this implementation should be
//! migrated to use the native syntax for better performance and ergonomics.
//!
//! Tracking issue: <https://github.com/rust-lang/rust/issues/79024>
//!
//! # Examples
//!
//! ```rust,no_run
//! # #[cfg(feature = "generator")]
//! # use reinhardt_di::generator::DependencyGenerator;
//! # #[cfg(feature = "generator")]
//! # async fn example() {
//! // Create a generator that yields dependencies one by one
//! // let gen = DependencyGenerator::new(|co| async move {
//! //     let db = resolve_database().await;
//! //     co.yield_(db).await;
//! //
//! //     let cache = resolve_cache().await;
//! //     co.yield_(cache).await;
//! //
//! //     let service = resolve_service().await;
//! //     co.yield_(service).await;
//! // });
//! //
//! // // Consume dependencies as they become available
//! // while let Some(dep) = gen.next().await {
//! //     // Use dependency
//! // }
//! # }
//! ```

#[cfg(feature = "generator")]
use genawaiter::GeneratorState;
#[cfg(feature = "generator")]
use genawaiter::sync::{Co, Gen};
#[cfg(feature = "generator")]
use std::future::Future;
#[cfg(feature = "generator")]
use std::marker::PhantomData;
#[cfg(feature = "generator")]
use std::pin::Pin;

/// Generator-based dependency resolver
///
/// Provides lazy, streaming dependency resolution using generators.
///
/// # Note
///
/// This uses `genawaiter` as a workaround for unstable native async yield.
#[cfg(feature = "generator")]
pub struct DependencyGenerator<T, R> {
	generator: Gen<T, (), Pin<Box<dyn Future<Output = R> + Send + 'static>>>,
	_phantom: PhantomData<(T, R)>,
}

#[cfg(feature = "generator")]
impl<T, R> DependencyGenerator<T, R>
where
	T: 'static,
	R: 'static,
{
	/// Create a new dependency generator
	///
	/// # Arguments
	///
	/// * `producer` - Async function that yields dependencies using the `Co` (coroutine) handle
	///
	/// # Examples
	///
	/// ```rust,no_run
	/// # #[cfg(feature = "generator")]
	/// # use reinhardt_di::generator::DependencyGenerator;
	/// # #[cfg(feature = "generator")]
	/// # async fn example() {
	/// // let gen = DependencyGenerator::new(|co| async move {
	/// //     let db = Database::connect().await;
	/// //     co.yield_(db).await;
	/// // });
	/// # }
	/// ```
	pub fn new<F>(producer: F) -> Self
	where
		F: FnOnce(Co<T>) -> Pin<Box<dyn Future<Output = R> + Send + 'static>> + Send + 'static,
	{
		Self {
			generator: Gen::new(producer),
			_phantom: PhantomData,
		}
	}

	/// Get the next dependency from the generator
	///
	/// Returns `None` when the generator is exhausted.
	pub async fn next(&mut self) -> Option<T> {
		match self.generator.async_resume().await {
			GeneratorState::Yielded(value) => Some(value),
			GeneratorState::Complete(_) => None,
		}
	}

	/// Collect all remaining dependencies into a vector
	pub async fn collect(mut self) -> Vec<T> {
		let mut deps = Vec::new();
		while let Some(dep) = self.next().await {
			deps.push(dep);
		}
		deps
	}
}

/// Dependency stream for request-scoped resolution
///
/// Provides async streaming of dependencies with lazy evaluation.
///
/// # Note
///
/// This uses `genawaiter` as a workaround for unstable native async yield.
#[cfg(feature = "generator")]
pub struct DependencyStream<T> {
	generator: Gen<T, (), Pin<Box<dyn Future<Output = ()> + Send + 'static>>>,
	/// Buffer for peeked values to avoid consuming elements during `is_empty()` checks
	peeked: Option<T>,
	_phantom: PhantomData<T>,
}

#[cfg(feature = "generator")]
impl<T> DependencyStream<T>
where
	T: 'static,
{
	/// Create a new dependency stream
	pub fn new<F>(producer: F) -> Self
	where
		F: FnOnce(Co<T>) -> Pin<Box<dyn Future<Output = ()> + Send + 'static>> + Send + 'static,
	{
		Self {
			generator: Gen::new(producer),
			peeked: None,
			_phantom: PhantomData,
		}
	}

	/// Stream the next dependency
	pub async fn next(&mut self) -> Option<T> {
		// Return peeked value first if available
		if let Some(value) = self.peeked.take() {
			return Some(value);
		}
		match self.generator.async_resume().await {
			GeneratorState::Yielded(value) => Some(value),
			GeneratorState::Complete(_) => None,
		}
	}

	/// Check if stream has more dependencies without consuming elements.
	///
	/// This method peeks at the next value and buffers it internally,
	/// so subsequent calls to `next()` will return the peeked value first.
	pub async fn is_empty(&mut self) -> bool {
		if self.peeked.is_some() {
			return false;
		}
		match self.generator.async_resume().await {
			GeneratorState::Yielded(value) => {
				self.peeked = Some(value);
				false
			}
			GeneratorState::Complete(_) => true,
		}
	}
}

/// Request-scoped dependency resolver with generator
///
/// Resolves dependencies lazily for a specific request context.
///
/// # Note
///
/// This uses `genawaiter` as a workaround for unstable native async yield.
#[cfg(feature = "generator")]
pub struct RequestScopedGenerator<T> {
	request_id: String,
	stream: DependencyStream<T>,
}

#[cfg(feature = "generator")]
impl<T> RequestScopedGenerator<T>
where
	T: 'static,
{
	/// Create a new request-scoped generator
	pub fn new(request_id: String, stream: DependencyStream<T>) -> Self {
		Self { request_id, stream }
	}

	/// Get request ID
	pub fn request_id(&self) -> &str {
		&self.request_id
	}

	/// Resolve next dependency for this request
	pub async fn resolve_next(&mut self) -> Option<T> {
		self.stream.next().await
	}
}

#[cfg(feature = "generator")]
#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;

	#[rstest]
	#[tokio::test]
	async fn test_dependency_generator_basic() {
		let mut generator = DependencyGenerator::new(|co| {
			Box::pin(async move {
				co.yield_(1).await;
				co.yield_(2).await;
				co.yield_(3).await;
			})
		});

		assert_eq!(generator.next().await, Some(1));
		assert_eq!(generator.next().await, Some(2));
		assert_eq!(generator.next().await, Some(3));
		assert_eq!(generator.next().await, None);
	}

	#[rstest]
	#[tokio::test]
	async fn test_dependency_generator_collect() {
		let generator = DependencyGenerator::new(|co| {
			Box::pin(async move {
				co.yield_(1).await;
				co.yield_(2).await;
				co.yield_(3).await;
			})
		});

		let deps = generator.collect().await;
		assert_eq!(deps, vec![1, 2, 3]);
	}

	#[rstest]
	#[tokio::test]
	async fn test_dependency_stream() {
		let mut stream = DependencyStream::new(|co| {
			Box::pin(async move {
				co.yield_("db".to_string()).await;
				co.yield_("cache".to_string()).await;
			})
		});

		assert_eq!(stream.next().await, Some("db".to_string()));
		assert_eq!(stream.next().await, Some("cache".to_string()));
		assert_eq!(stream.next().await, None);
	}

	#[rstest]
	#[tokio::test]
	async fn test_request_scoped_generator() {
		let stream = DependencyStream::new(|co| {
			Box::pin(async move {
				co.yield_("dependency1".to_string()).await;
				co.yield_("dependency2".to_string()).await;
			})
		});

		let mut generator = RequestScopedGenerator::new("request-123".to_string(), stream);

		assert_eq!(generator.request_id(), "request-123");
		assert_eq!(
			generator.resolve_next().await,
			Some("dependency1".to_string())
		);
		assert_eq!(
			generator.resolve_next().await,
			Some("dependency2".to_string())
		);
		assert_eq!(generator.resolve_next().await, None);
	}

	#[rstest]
	#[tokio::test]
	async fn test_dependency_stream_is_empty_does_not_consume() {
		// Arrange
		let mut stream = DependencyStream::new(|co| {
			Box::pin(async move {
				co.yield_("first".to_string()).await;
				co.yield_("second".to_string()).await;
			})
		});

		// Act - is_empty should peek without consuming
		let empty = stream.is_empty().await;

		// Assert - stream is not empty
		assert!(!empty);
		// The peeked element should still be available via next()
		assert_eq!(stream.next().await, Some("first".to_string()));
		assert_eq!(stream.next().await, Some("second".to_string()));
		assert_eq!(stream.next().await, None);
	}

	#[rstest]
	#[tokio::test]
	async fn test_dependency_stream_is_empty_on_exhausted_stream() {
		// Arrange
		let mut stream: DependencyStream<i32> =
			DependencyStream::new(|_co| Box::pin(async move {}));

		// Act & Assert - empty stream should return true
		assert!(stream.is_empty().await);
	}

	#[rstest]
	#[tokio::test]
	async fn test_dependency_stream_is_empty_after_partial_consumption() {
		// Arrange
		let mut stream = DependencyStream::new(|co| {
			Box::pin(async move {
				co.yield_(1).await;
				co.yield_(2).await;
			})
		});

		// Consume first element
		assert_eq!(stream.next().await, Some(1));

		// Act - check is_empty after partial consumption
		assert!(!stream.is_empty().await);

		// Assert - second element should still be available
		assert_eq!(stream.next().await, Some(2));
		assert!(stream.is_empty().await);
	}

	#[rstest]
	#[tokio::test]
	async fn test_dependency_stream_multiple_is_empty_calls() {
		// Arrange
		let mut stream = DependencyStream::new(|co| {
			Box::pin(async move {
				co.yield_(42).await;
			})
		});

		// Act - calling is_empty multiple times should not consume
		assert!(!stream.is_empty().await);
		assert!(!stream.is_empty().await);
		assert!(!stream.is_empty().await);

		// Assert - element should still be available
		assert_eq!(stream.next().await, Some(42));
		assert_eq!(stream.next().await, None);
	}

	#[rstest]
	#[tokio::test]
	async fn test_async_operations_in_generator() {
		let generator = DependencyGenerator::new(|co| {
			Box::pin(async move {
				// Simulate async database connection
				co.yield_("database".to_string()).await;

				// Simulate async cache connection
				co.yield_("cache".to_string()).await;
			})
		});

		let deps = generator.collect().await;
		assert_eq!(deps, vec!["database".to_string(), "cache".to_string()]);
	}

	// Regression test for #453: is_empty() must not consume stream elements.
	// Previously, is_empty() advanced the generator without buffering the peeked
	// value, causing the first element to be silently dropped on the next next() call.
	#[rstest]
	#[tokio::test]
	async fn test_is_empty_does_not_drop_first_element_regression() {
		// Arrange - stream with 3 elements; the bug would drop element "a" after is_empty()
		let mut stream = DependencyStream::new(|co| {
			Box::pin(async move {
				co.yield_("a".to_string()).await;
				co.yield_("b".to_string()).await;
				co.yield_("c".to_string()).await;
			})
		});

		// Act - call is_empty() before consuming any element
		let result = stream.is_empty().await;

		// Assert - stream must not be empty, and ALL three elements must still be reachable
		assert!(
			!result,
			"Regression #453: stream with 3 elements must not be empty"
		);
		assert_eq!(
			stream.next().await,
			Some("a".to_string()),
			"Regression #453: first element must not be dropped after is_empty()"
		);
		assert_eq!(stream.next().await, Some("b".to_string()));
		assert_eq!(stream.next().await, Some("c".to_string()));
		assert_eq!(stream.next().await, None);
	}

	// Regression test for #453: repeated is_empty() calls must buffer only one value.
	// Each repeated call should return the same peeked value without advancing the generator further.
	#[rstest]
	#[tokio::test]
	async fn test_repeated_is_empty_buffers_single_value_regression() {
		// Arrange - stream with 2 elements
		let mut stream = DependencyStream::new(|co| {
			Box::pin(async move {
				co.yield_(10_u32).await;
				co.yield_(20_u32).await;
			})
		});

		// Act - call is_empty() three times in a row
		assert!(
			!stream.is_empty().await,
			"Regression #453: first is_empty() call should return false"
		);
		assert!(
			!stream.is_empty().await,
			"Regression #453: second is_empty() call should return false (peeked slot not overwritten)"
		);
		assert!(
			!stream.is_empty().await,
			"Regression #453: third is_empty() call should return false"
		);

		// Assert - all elements must still be reachable in order
		assert_eq!(
			stream.next().await,
			Some(10_u32),
			"Regression #453: element 10 must survive three is_empty() calls"
		);
		assert_eq!(stream.next().await, Some(20_u32));
		assert_eq!(stream.next().await, None);
	}
}
