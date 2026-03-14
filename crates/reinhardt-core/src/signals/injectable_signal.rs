//! Context-aware signal receivers
//!
//! This module provides the `InjectableSignal` extension trait that enables
//! context-aware receivers without modifying the core Signal type.
//!
//! # Usage
//!
//! ```rust,no_run
//! use reinhardt_core::signals::{Signal, SignalName, SignalError, InjectableSignal};
//! use std::sync::Arc;
//!
//! #[derive(Clone, Debug)]
//! struct User;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), SignalError> {
//!     // Create a signal
//!     let signal = Signal::<User>::new(SignalName::custom("user_created"));
//!
//!     // Connect a receiver with context
//!     signal.connect_with_context(|instance, ctx| async move {
//!         println!("User {:?} saved", instance);
//!         Ok(())
//!     });
//!
//!     // Send signal
//!     let user = User;
//!     signal.send(user).await?;
//!     Ok(())
//! }
//! ```

use super::receiver_context::ReceiverContext;
use super::signal::Signal;
use async_trait::async_trait;
use std::future::Future;
use std::sync::Arc;

use super::error::SignalError;

/// Extension trait for Signals that enables context-aware receivers
///
/// This trait is automatically implemented for all Signal types.
/// It provides helper methods to connect receivers that receive a context.
#[async_trait]
pub trait InjectableSignal<T: Send + Sync + 'static> {
	/// Connect a receiver function that receives a ReceiverContext
	///
	/// The receiver context can be used to pass additional information
	/// to the receiver.
	///
	/// # Examples
	///
	/// ```rust,no_run
	/// use reinhardt_core::signals::{Signal, SignalName, SignalError, InjectableSignal, ReceiverContext};
	/// use std::sync::Arc;
	///
	/// #[derive(Clone, Debug)]
	/// struct User;
	///
	/// let signal = Signal::<User>::new(SignalName::custom("test"));
	/// signal.connect_with_context(|instance, ctx| async move {
	///     println!("Processing {:?}", instance);
	///     Ok(())
	/// });
	/// ```
	fn connect_with_context<F, Fut>(&self, receiver: F)
	where
		F: Fn(Arc<T>, ReceiverContext) -> Fut + Send + Sync + 'static,
		Fut: Future<Output = Result<(), SignalError>> + Send + 'static;
}

#[async_trait]
impl<T: Send + Sync + 'static> InjectableSignal<T> for Signal<T> {
	fn connect_with_context<F, Fut>(&self, receiver: F)
	where
		F: Fn(Arc<T>, ReceiverContext) -> Fut + Send + Sync + 'static,
		Fut: Future<Output = Result<(), SignalError>> + Send + 'static,
	{
		// Wrap the context-aware receiver to work with the standard connect API
		self.connect(move |instance: Arc<T>| {
			let ctx = ReceiverContext::new();
			receiver(instance, ctx)
		});
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::signals::SignalName;

	// Allow dead_code: test-only model struct; fields accessed indirectly via Clone/Debug derives and signal dispatch
	#[allow(dead_code)]
	#[derive(Debug, Clone)]
	struct TestModel {
		id: i32,
		name: String,
	}

	#[tokio::test]
	async fn test_connect_with_context() {
		let signal = Signal::new(SignalName::custom("test_context"));

		let counter = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
		let counter_clone = counter.clone();

		signal.connect_with_context(move |_instance: Arc<TestModel>, ctx: ReceiverContext| {
			let counter = counter_clone.clone();
			async move {
				// Context should be available but without DI context
				assert!(!ctx.has_di_context());
				counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
				Ok(())
			}
		});

		let model = TestModel {
			id: 1,
			name: "Test".to_string(),
		};

		// Send without DI context (using standard send)
		signal.send(model).await.unwrap();

		assert_eq!(counter.load(std::sync::atomic::Ordering::SeqCst), 1);
	}
}
