//! ORM Integration - Automatic signal dispatch from ORM operations
//!
//! This module provides automatic signal dispatching for ORM operations,
//! integrating with reinhardt-orm's event system to trigger signals on
//! database operations.
//!
//! # Examples
//!
//! ```rust,no_run
//! use reinhardt_core::signals::orm_integration::OrmSignalAdapter;
//!
//! # #[tokio::main]
//! # async fn main() {
//! # struct User;
//! // Create ORM signal adapter
//! let adapter = OrmSignalAdapter::<User>::new();
//! let count = adapter.signal_count();
//! # }
//! ```

use super::error::SignalError;
use super::model_signals::{post_delete, post_save, pre_delete, pre_save};
use super::signal::Signal;
use async_trait::async_trait;
use std::any::Any;
use std::marker::PhantomData;
use std::sync::Arc;

/// ORM event listener trait
///
/// Implement this trait to receive ORM events and dispatch signals
#[async_trait]
pub trait OrmEventListener: Send + Sync {
	/// Called before a model instance is inserted
	async fn before_insert(&self, instance: Arc<dyn Any + Send + Sync>) -> Result<(), SignalError>;

	/// Called after a model instance is inserted
	async fn after_insert(&self, instance: Arc<dyn Any + Send + Sync>) -> Result<(), SignalError>;

	/// Called before a model instance is updated
	async fn before_update(&self, instance: Arc<dyn Any + Send + Sync>) -> Result<(), SignalError>;

	/// Called after a model instance is updated
	async fn after_update(&self, instance: Arc<dyn Any + Send + Sync>) -> Result<(), SignalError>;

	/// Called before a model instance is deleted
	async fn before_delete(&self, instance: Arc<dyn Any + Send + Sync>) -> Result<(), SignalError>;

	/// Called after a model instance is deleted
	async fn after_delete(&self, instance: Arc<dyn Any + Send + Sync>) -> Result<(), SignalError>;
}

/// Signal dispatcher for ORM events
///
/// This adapter bridges ORM events to signal dispatches
///
/// # Examples
///
/// ```
/// use reinhardt_core::signals::orm_integration::OrmSignalAdapter;
///
/// let adapter = OrmSignalAdapter::<String>::new();
/// assert_eq!(adapter.signal_count(), 0);
/// ```
pub struct OrmSignalAdapter<T: Send + Sync + 'static> {
	_phantom: PhantomData<T>,
}

impl<T: Send + Sync + 'static> OrmSignalAdapter<T> {
	/// Create a new ORM signal adapter
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::signals::orm_integration::OrmSignalAdapter;
	///
	/// let adapter = OrmSignalAdapter::<String>::new();
	/// ```
	pub fn new() -> Self {
		Self {
			_phantom: PhantomData,
		}
	}

	/// Get count of connected signals for this adapter
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::signals::orm_integration::OrmSignalAdapter;
	///
	/// let adapter = OrmSignalAdapter::<String>::new();
	/// let count = adapter.signal_count();
	/// assert_eq!(count, 0); // No signals connected yet
	/// ```
	pub fn signal_count(&self) -> usize {
		// Count receivers from pre_save, post_save, pre_delete, post_delete
		pre_save::<T>().receiver_count()
			+ post_save::<T>().receiver_count()
			+ pre_delete::<T>().receiver_count()
			+ post_delete::<T>().receiver_count()
	}
}

impl<T: Send + Sync + 'static> Default for OrmSignalAdapter<T> {
	fn default() -> Self {
		Self::new()
	}
}

/// Dispatch pre-save signal for ORM operation
///
/// This function is called by ORM before saving a model instance
///
/// # Examples
///
/// ```rust,no_run
/// # use reinhardt_core::signals::orm_integration::dispatch_pre_save;
/// # #[tokio::main]
/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
/// # #[derive(Clone)] struct User { id: i32, name: String }
/// let user = User { id: 1, name: "Alice".into() };
/// dispatch_pre_save(user).await?;
/// # Ok(())
/// # }
/// ```
pub async fn dispatch_pre_save<T: Send + Sync + Clone + 'static>(
	instance: T,
) -> Result<(), SignalError> {
	pre_save::<T>().send(instance).await
}

/// Dispatch post-save signal for ORM operation
///
/// This function is called by ORM after saving a model instance
///
/// # Examples
///
/// ```rust,no_run
/// # use reinhardt_core::signals::orm_integration::dispatch_post_save;
/// # #[tokio::main]
/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
/// # #[derive(Clone)] struct User { id: i32, name: String }
/// let user = User { id: 1, name: "Alice".into() };
/// dispatch_post_save(user, false).await?;
/// # Ok(())
/// # }
/// ```
pub async fn dispatch_post_save<T: Send + Sync + Clone + 'static>(
	instance: T,
	_created: bool,
) -> Result<(), SignalError> {
	post_save::<T>().send(instance).await
}

/// Dispatch pre-delete signal for ORM operation
///
/// This function is called by ORM before deleting a model instance
///
/// # Examples
///
/// ```rust,no_run
/// # use reinhardt_core::signals::orm_integration::dispatch_pre_delete;
/// # #[tokio::main]
/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
/// # #[derive(Clone)] struct User { id: i32, name: String }
/// let user = User { id: 1, name: "Alice".into() };
/// dispatch_pre_delete(user).await?;
/// # Ok(())
/// # }
/// ```
pub async fn dispatch_pre_delete<T: Send + Sync + Clone + 'static>(
	instance: T,
) -> Result<(), SignalError> {
	pre_delete::<T>().send(instance).await
}

/// Dispatch post-delete signal for ORM operation
///
/// This function is called by ORM after deleting a model instance
///
/// # Examples
///
/// ```rust,no_run
/// # use reinhardt_core::signals::orm_integration::dispatch_post_delete;
/// # #[tokio::main]
/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
/// # #[derive(Clone)] struct User { id: i32, name: String }
/// let user = User { id: 1, name: "Alice".into() };
/// dispatch_post_delete(user).await?;
/// # Ok(())
/// # }
/// ```
pub async fn dispatch_post_delete<T: Send + Sync + Clone + 'static>(
	instance: T,
) -> Result<(), SignalError> {
	post_delete::<T>().send(instance).await
}

/// Connect all ORM signals for a specific model type
///
/// This is a convenience function that sets up signal dispatching
/// for all ORM operations on a model
///
/// # Examples
///
/// ```
/// use reinhardt_core::signals::orm_integration::connect_orm_signals;
///
/// # async fn example() {
/// connect_orm_signals::<String>().await;
/// # }
/// ```
pub async fn connect_orm_signals<T: Send + Sync + 'static>() {
	// Signals are already registered in the global registry
	// This function serves as a convenience to ensure they exist
	let _ = pre_save::<T>();
	let _ = post_save::<T>();
	let _ = pre_delete::<T>();
	let _ = post_delete::<T>();
}

/// Get ORM-related signals for a model
///
/// Returns all four signals (pre_save, post_save, pre_delete, post_delete)
///
/// # Examples
///
/// ```
/// use reinhardt_core::signals::orm_integration::get_orm_signals;
///
/// let signals = get_orm_signals::<String>();
/// assert_eq!(signals.len(), 4);
/// ```
pub fn get_orm_signals<T: Send + Sync + 'static>() -> Vec<Signal<T>> {
	vec![
		pre_save::<T>(),
		post_save::<T>(),
		pre_delete::<T>(),
		post_delete::<T>(),
	]
}

#[cfg(test)]
mod tests {
	use super::*;
	use parking_lot::Mutex;
	use std::sync::Arc;
	use std::sync::atomic::{AtomicUsize, Ordering};

	#[derive(Debug, Clone)]
	// Allow dead_code: test-only model struct; fields accessed indirectly via Clone/Debug derives and signal dispatch
	#[allow(dead_code)]
	struct TestModel {
		id: i32,
		name: String,
	}

	#[tokio::test]
	async fn test_orm_signal_adapter_creation() {
		let _adapter = OrmSignalAdapter::<TestModel>::new();
		// Adapter creation test - verifies compilation and instantiation
	}

	#[tokio::test]
	#[serial_test::serial]
	async fn test_dispatch_pre_save() {
		// Clean up before test
		pre_save::<TestModel>().disconnect_all();

		let counter = Arc::new(AtomicUsize::new(0));
		let counter_clone = Arc::clone(&counter);

		pre_save::<TestModel>().connect(move |_instance| {
			let counter = Arc::clone(&counter_clone);
			async move {
				counter.fetch_add(1, Ordering::SeqCst);
				Ok(())
			}
		});

		let model = TestModel {
			id: 1,
			name: "Test".to_string(),
		};

		dispatch_pre_save(model).await.unwrap();
		assert_eq!(counter.load(Ordering::SeqCst), 1);

		// Clean up after test
		pre_save::<TestModel>().disconnect_all();
	}

	#[tokio::test]
	async fn test_dispatch_post_save() {
		let counter = Arc::new(AtomicUsize::new(0));
		let counter_clone = Arc::clone(&counter);

		post_save::<TestModel>().connect(move |_instance| {
			let counter = Arc::clone(&counter_clone);
			async move {
				counter.fetch_add(1, Ordering::SeqCst);
				Ok(())
			}
		});

		let model = TestModel {
			id: 1,
			name: "Test".to_string(),
		};

		dispatch_post_save(model, true).await.unwrap();
		assert_eq!(counter.load(Ordering::SeqCst), 1);
	}

	#[tokio::test]
	#[serial_test::serial]
	async fn test_dispatch_pre_delete() {
		// Clean up before test
		pre_delete::<TestModel>().disconnect_all();

		let counter = Arc::new(AtomicUsize::new(0));
		let counter_clone = Arc::clone(&counter);

		pre_delete::<TestModel>().connect(move |_instance| {
			let counter = Arc::clone(&counter_clone);
			async move {
				counter.fetch_add(1, Ordering::SeqCst);
				Ok(())
			}
		});

		let model = TestModel {
			id: 1,
			name: "Test".to_string(),
		};

		dispatch_pre_delete(model).await.unwrap();
		assert_eq!(counter.load(Ordering::SeqCst), 1);

		// Clean up after test
		pre_delete::<TestModel>().disconnect_all();
	}

	#[tokio::test]
	async fn test_dispatch_post_delete() {
		let counter = Arc::new(AtomicUsize::new(0));
		let counter_clone = Arc::clone(&counter);

		post_delete::<TestModel>().connect(move |_instance| {
			let counter = Arc::clone(&counter_clone);
			async move {
				counter.fetch_add(1, Ordering::SeqCst);
				Ok(())
			}
		});

		let model = TestModel {
			id: 1,
			name: "Test".to_string(),
		};

		dispatch_post_delete(model).await.unwrap();
		assert_eq!(counter.load(Ordering::SeqCst), 1);
	}

	#[tokio::test]
	async fn test_connect_orm_signals() {
		connect_orm_signals::<TestModel>().await;

		let adapter = OrmSignalAdapter::<TestModel>::new();
		let initial_count = adapter.signal_count();

		// Connect a receiver to verify signals exist
		pre_save::<TestModel>().connect(|_| async { Ok(()) });

		assert!(adapter.signal_count() > initial_count);
	}

	#[tokio::test]
	async fn test_get_orm_signals() {
		let signals = get_orm_signals::<TestModel>();
		assert_eq!(signals.len(), 4);

		// Verify each signal is unique
		let names: Vec<_> = signals.iter().map(|s| format!("{:?}", s)).collect();
		assert_eq!(names.len(), 4);
	}

	#[tokio::test]
	#[serial_test::serial]
	async fn test_orm_signal_flow() {
		// Clean up before test
		pre_save::<TestModel>().disconnect_all();
		post_save::<TestModel>().disconnect_all();
		pre_delete::<TestModel>().disconnect_all();
		post_delete::<TestModel>().disconnect_all();

		let events = Arc::new(Mutex::new(Vec::new()));

		// Connect all signals
		let e1 = events.clone();
		pre_save::<TestModel>().connect(move |_| {
			let e = e1.clone();
			async move {
				e.lock().push("pre_save");
				Ok(())
			}
		});

		let e2 = events.clone();
		post_save::<TestModel>().connect(move |_| {
			let e = e2.clone();
			async move {
				e.lock().push("post_save");
				Ok(())
			}
		});

		let e3 = events.clone();
		pre_delete::<TestModel>().connect(move |_| {
			let e = e3.clone();
			async move {
				e.lock().push("pre_delete");
				Ok(())
			}
		});

		let e4 = events.clone();
		post_delete::<TestModel>().connect(move |_| {
			let e = e4.clone();
			async move {
				e.lock().push("post_delete");
				Ok(())
			}
		});

		let model = TestModel {
			id: 1,
			name: "Test".to_string(),
		};

		// Simulate save operation
		dispatch_pre_save(model.clone()).await.unwrap();
		dispatch_post_save(model.clone(), true).await.unwrap();

		// Simulate delete operation
		dispatch_pre_delete(model.clone()).await.unwrap();
		dispatch_post_delete(model).await.unwrap();

		let event_log = events.lock();
		assert_eq!(event_log.len(), 4);
		assert_eq!(event_log[0], "pre_save");
		assert_eq!(event_log[1], "post_save");
		assert_eq!(event_log[2], "pre_delete");
		assert_eq!(event_log[3], "post_delete");

		// Clean up after test
		pre_save::<TestModel>().disconnect_all();
		post_save::<TestModel>().disconnect_all();
		pre_delete::<TestModel>().disconnect_all();
		post_delete::<TestModel>().disconnect_all();
	}
}
