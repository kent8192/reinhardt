//! # Reinhardt Signals
//!
//! Event-driven signal system for Reinhardt framework, inspired by Django signals.
//!
//! ## Signal Types
//!
//! ### Model Signals
//! - **pre_save**: Before saving a model instance
//! - **post_save**: After saving a model instance
//! - **pre_delete**: Before deleting a model instance
//! - **post_delete**: After deleting a model instance
//! - **pre_init**: At the beginning of a model's initialization
//! - **post_init**: At the end of a model's initialization
//! - **m2m_changed**: When many-to-many relationships change
//! - **class_prepared**: When a model class is prepared
//!
//! ### Migration Signals
//! - **pre_migrate**: Before running migrations
//! - **post_migrate**: After running migrations
//!
//! ### Request Signals
//! - **request_started**: When an HTTP request starts
//! - **request_finished**: When an HTTP request finishes
//! - **got_request_exception**: When an exception occurs during request handling
//!
//! ### Management Signals
//! - **setting_changed**: When a configuration setting is changed
//!
//! ## Example
//!
//! ```rust,no_run
//! use reinhardt_core::signals::post_save;
//!
//! # #[tokio::main]
//! # async fn main() -> Result<(), reinhardt_core::signals::SignalError> {
//! # #[derive(Clone)]
//! # struct User;
//! # let user = User;
//! // Connect a receiver
//! post_save::<User>().connect(|_instance| async move {
//!     println!("User saved");
//!     Ok(())
//! });
//!
//! // Send signal
//! post_save::<User>().send(user).await?;
//! # Ok(())
//! # }
//! ```
//!
//! ## Implemented Integration Features
//!
//! - **ORM Integration**: Automatic signal dispatch from ORM operations
//! - **Transaction Support**: Signals tied to database transaction lifecycle
//! - **Distributed Signals**: Cross-service signal dispatch via message brokers
//! - **WebSocket Signals**: Real-time signal propagation to clients
//! - **GraphQL Subscriptions**: Signal-based GraphQL subscription support
//!
// Module declarations
pub mod batching;
mod context;
mod core;
mod db_events;
pub mod debugger;
pub mod dispatch;
pub mod dlq;
pub mod doc_generator;
pub mod error;
pub mod history;
pub mod injectable_signal;
mod lifecycle_events;
mod middleware;
mod model_signals;
pub mod persistence;
pub mod profiler;
pub mod receiver_context;
pub mod receiver_registry;
mod registry;
pub mod replay;
mod request_events;
mod signal;
pub mod throttling;
pub mod visualization;

// Integration modules
pub mod distributed;
pub mod graphql_integration;
pub mod orm_integration;
pub mod transaction;
pub mod websocket_integration;

// Re-export core types
pub use context::{SignalContext, SignalMetrics};
pub use core::{AsyncSignalDispatcher, ReceiverFn, SignalDispatcher, SignalName};
pub use error::SignalError;
pub use injectable_signal::InjectableSignal;
pub use middleware::{MiddlewareFn, SignalCall, SignalMiddleware, SignalSpy};
pub use receiver_context::ReceiverContext;
pub use receiver_registry::{ReceiverRegistryEntry, auto_connect_receivers};
pub use registry::{get_signal, get_signal_with_string};
pub use signal::Signal;

// Re-export model signals
pub use model_signals::{post_delete, post_save, pre_delete, pre_save};

// Re-export db events
pub use db_events::{
	DbEvent, after_delete, after_insert, after_update, before_delete, before_insert, before_update,
};

// Re-export lifecycle events
pub use lifecycle_events::{
	ClassPreparedEvent, M2MAction, M2MChangeEvent, MigrationEvent, PostInitEvent, PreInitEvent,
	class_prepared, m2m_changed, post_init, post_migrate, pre_init, pre_migrate,
};

// Re-export request events
pub use request_events::{
	GotRequestExceptionEvent, RequestFinishedEvent, RequestStartedEvent, SettingChangedEvent,
	got_request_exception, request_finished, request_started, setting_changed,
};

// Re-export dispatch types
pub use dispatch::{SyncReceiverFn, SyncSignal};

/// Helper macro for connecting receivers with a simpler syntax
///
/// # Example
///
/// ```rust,no_run
/// use reinhardt_core::{connect_receiver, signals::{post_save, SignalError}};
/// use std::sync::Arc;
///
/// # #[derive(Clone)]
/// # struct User;
/// connect_receiver!(post_save::<User>(), on_user_saved);
///
/// async fn on_user_saved(_instance: Arc<User>) -> Result<(), SignalError> {
///     println!("User saved");
///     Ok(())
/// }
/// ```
#[macro_export]
macro_rules! connect_receiver {
	($signal:expr, $receiver:expr) => {
		$signal.connect($receiver)
	};
	($signal:expr, $receiver:expr, priority = $priority:expr) => {
		$signal.connect_with_priority($receiver, $priority)
	};
	($signal:expr, $receiver:expr, dispatch_uid = $uid:expr) => {
		$signal.connect_with_options($receiver, None, Some($uid.to_string()), 0)
	};
	($signal:expr, $receiver:expr, sender = $sender:ty) => {
		$signal.connect_with_options($receiver, Some(std::any::TypeId::of::<$sender>()), None, 0)
	};
	($signal:expr, $receiver:expr, priority = $priority:expr, dispatch_uid = $uid:expr) => {
		$signal.connect_with_options($receiver, None, Some($uid.to_string()), $priority)
	};
	($signal:expr, $receiver:expr, sender = $sender:ty, dispatch_uid = $uid:expr) => {
		$signal.connect_with_options(
			$receiver,
			Some(std::any::TypeId::of::<$sender>()),
			Some($uid.to_string()),
			0,
		)
	};
	($signal:expr, $receiver:expr, sender = $sender:ty, priority = $priority:expr) => {
		$signal.connect_with_options(
			$receiver,
			Some(std::any::TypeId::of::<$sender>()),
			None,
			$priority,
		)
	};
	($signal:expr, $receiver:expr, sender = $sender:ty, dispatch_uid = $uid:expr, priority = $priority:expr) => {
		$signal.connect_with_options(
			$receiver,
			Some(std::any::TypeId::of::<$sender>()),
			Some($uid.to_string()),
			$priority,
		)
	};
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
	async fn test_signal_connect_and_send() {
		let signal = Signal::new(SignalName::custom("test"));
		let counter = Arc::new(AtomicUsize::new(0));

		let counter_clone = Arc::clone(&counter);
		signal.connect(move |_instance: Arc<TestModel>| {
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

		signal.send(model).await.unwrap();

		assert_eq!(counter.load(Ordering::SeqCst), 1);
	}

	#[tokio::test]
	async fn test_signals_multiple_receivers() {
		let signal = Signal::new(SignalName::custom("test"));
		let counter = Arc::new(AtomicUsize::new(0));

		// Connect multiple receivers
		for _ in 0..3 {
			let counter_clone = Arc::clone(&counter);
			signal.connect(move |_instance: Arc<TestModel>| {
				let counter = Arc::clone(&counter_clone);
				async move {
					counter.fetch_add(1, Ordering::SeqCst);
					Ok(())
				}
			});
		}

		let model = TestModel {
			id: 1,
			name: "Test".to_string(),
		};

		signal.send(model).await.unwrap();

		assert_eq!(counter.load(Ordering::SeqCst), 3);
	}

	#[tokio::test]
	async fn test_signals_pre_post_save() {
		let pre_counter = Arc::new(AtomicUsize::new(0));
		let post_counter = Arc::new(AtomicUsize::new(0));

		// Connect to pre_save
		let pre_clone = Arc::clone(&pre_counter);
		pre_save::<TestModel>().connect(move |_instance| {
			let counter = Arc::clone(&pre_clone);
			async move {
				counter.fetch_add(1, Ordering::SeqCst);
				Ok(())
			}
		});

		// Connect to post_save
		let post_clone = Arc::clone(&post_counter);
		post_save::<TestModel>().connect(move |_instance| {
			let counter = Arc::clone(&post_clone);
			async move {
				counter.fetch_add(1, Ordering::SeqCst);
				Ok(())
			}
		});

		let model = TestModel {
			id: 1,
			name: "Test".to_string(),
		};

		// Simulate save operation
		pre_save::<TestModel>().send(model.clone()).await.unwrap();
		// ... actual save would happen here ...
		post_save::<TestModel>().send(model).await.unwrap();

		assert_eq!(pre_counter.load(Ordering::SeqCst), 1);
		assert_eq!(post_counter.load(Ordering::SeqCst), 1);
	}

	#[tokio::test]
	async fn test_signals_global_registry() {
		let signal1 = get_signal::<TestModel>(SignalName::custom("custom_signal"));
		let signal2 = get_signal::<TestModel>(SignalName::custom("custom_signal"));

		let counter = Arc::new(AtomicUsize::new(0));
		let counter_clone = Arc::clone(&counter);

		// Connect to signal1
		signal1.connect(move |_instance| {
			let counter = Arc::clone(&counter_clone);
			async move {
				counter.fetch_add(1, Ordering::SeqCst);
				Ok(())
			}
		});

		// Send through signal2 (should be the same signal)
		let model = TestModel {
			id: 1,
			name: "Test".to_string(),
		};
		signal2.send(model).await.unwrap();

		assert_eq!(counter.load(Ordering::SeqCst), 1);
	}

	#[tokio::test]
	async fn test_signal_spy() {
		let signal = Signal::new(SignalName::custom("test"));
		let spy = SignalSpy::new();
		signal.add_middleware(spy.clone());

		signal.connect(|_instance: Arc<TestModel>| async move { Ok(()) });

		assert!(!spy.was_called());
		assert_eq!(spy.call_count(), 0);

		let model = TestModel {
			id: 1,
			name: "Test".to_string(),
		};
		signal.send(model).await.unwrap();

		assert!(spy.was_called());
		assert_eq!(spy.call_count(), 1);
		assert_eq!(spy.total_receivers_called(), 1);
		assert!(!spy.has_errors());
	}

	#[tokio::test]
	async fn test_signal_chain() {
		let signal_a = Signal::new(SignalName::custom("signal_a"));
		let signal_b = Signal::new(SignalName::custom("signal_b"));

		let calls = Arc::new(Mutex::new(Vec::new()));

		// Track signal_a calls
		let calls_a = calls.clone();
		signal_a.connect(move |instance: Arc<TestModel>| {
			let calls = calls_a.clone();
			async move {
				calls.lock().push(format!("signal_a: {}", instance.id));
				Ok(())
			}
		});

		// Track signal_b calls
		let calls_b = calls.clone();
		signal_b.connect(move |instance: Arc<TestModel>| {
			let calls = calls_b.clone();
			async move {
				calls.lock().push(format!("signal_b: {}", instance.id));
				Ok(())
			}
		});

		// Chain signal_a to signal_b
		signal_a.chain(&signal_b);

		// Send to signal_a
		let model = TestModel {
			id: 1,
			name: "Test".to_string(),
		};
		signal_a.send(model).await.unwrap();

		// Wait for async execution

		let results = calls.lock();
		assert_eq!(results.len(), 2);
		assert_eq!(results[0], "signal_a: 1");
		assert_eq!(results[1], "signal_b: 1");
	}

	#[tokio::test]
	async fn test_signal_filter() {
		let signal = Signal::new(SignalName::custom("user_signal"));
		let admin_only = signal.filter(|model: &TestModel| model.id > 100);

		let admin_calls = Arc::new(Mutex::new(Vec::new()));
		let all_calls = Arc::new(Mutex::new(Vec::new()));

		let admin = admin_calls.clone();
		admin_only.connect(move |instance: Arc<TestModel>| {
			let admin = admin.clone();
			async move {
				admin.lock().push(instance.id);
				Ok(())
			}
		});

		let all = all_calls.clone();
		signal.connect(move |instance: Arc<TestModel>| {
			let all = all.clone();
			async move {
				all.lock().push(instance.id);
				Ok(())
			}
		});

		// Send regular user (id <= 100)
		signal
			.send(TestModel {
				id: 50,
				name: "Regular".to_string(),
			})
			.await
			.unwrap();

		// Send admin user (id > 100)
		signal
			.send(TestModel {
				id: 101,
				name: "Admin".to_string(),
			})
			.await
			.unwrap();

		let admin_results = admin_calls.lock();
		let all_results = all_calls.lock();

		assert_eq!(admin_results.len(), 1); // Only admin
		assert_eq!(admin_results[0], 101);

		assert_eq!(all_results.len(), 2); // Both
	}

	#[test]
	fn test_signal_name_validation() {
		// Valid names
		assert!(SignalName::custom_validated("my_custom_signal").is_ok());
		assert!(SignalName::custom_validated("user_created").is_ok());

		// Invalid: empty
		assert!(SignalName::custom_validated("").is_err());

		// Invalid: not snake_case
		assert!(SignalName::custom_validated("MySignal").is_err());

		// Invalid: reserved name
		assert!(SignalName::custom_validated("pre_save").is_err());
	}

	#[tokio::test]
	async fn test_all_signal_types_accessible() {
		// Model signals
		let _ = pre_save::<TestModel>();
		let _ = post_save::<TestModel>();
		let _ = pre_delete::<TestModel>();
		let _ = post_delete::<TestModel>();
		let _ = pre_init::<TestModel>();
		let _ = post_init::<TestModel>();
		let _ = m2m_changed::<TestModel, TestModel>();

		// Migration signals
		let _ = pre_migrate();
		let _ = post_migrate();

		// Class signal
		let _ = class_prepared();

		// Request signals
		let _ = request_started();
		let _ = request_finished();
		let _ = got_request_exception();

		// Management signals
		let _ = setting_changed();

		// DB events
		let _ = before_insert();
		let _ = after_insert();
		let _ = before_update();
		let _ = after_update();
		let _ = before_delete();
		let _ = after_delete();
	}
}
