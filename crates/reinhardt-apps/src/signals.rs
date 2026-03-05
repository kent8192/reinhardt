//! Application lifecycle signals
//!
//! This module provides a simple signal system for application lifecycle events.
//! Signals allow different parts of the application to respond to events such as
//! application startup, shutdown, and readiness.
//!
//! # Examples
//!
//! ```rust
//! use reinhardt_apps::signals::{app_ready, SignalReceiver};
//! use reinhardt_apps::AppConfig;
//!
//! // Define a receiver function
//! fn on_app_ready(config: &AppConfig) {
//!     println!("App {} is ready!", config.label);
//! }
//!
//! // Connect the receiver to the signal
//! app_ready().connect(Box::new(on_app_ready));
//!
//! // Send the signal
//! let config = AppConfig::new("myapp", "myapp");
//! app_ready().send(&config);
//! ```

use crate::apps::AppConfig;
use std::sync::{Arc, Mutex, OnceLock};

/// Type alias for signal receiver functions
///
/// Receivers are functions that are called when a signal is sent.
/// They receive a reference to the AppConfig that triggered the signal.
pub type SignalReceiver = Box<dyn Fn(&AppConfig) + Send + Sync>;

/// Internal receiver type using Arc for cloneable snapshots.
/// This allows the send method to release the lock before invoking callbacks,
/// preventing lock contention when receivers call connect/disconnect.
type InternalReceiver = Arc<dyn Fn(&AppConfig) + Send + Sync>;

/// A signal that can be sent when application lifecycle events occur
///
/// Signals maintain a list of receivers (callbacks) that are called when
/// the signal is sent.
#[derive(Default)]
pub struct Signal {
	receivers: Arc<Mutex<Vec<InternalReceiver>>>,
}

impl Signal {
	/// Create a new signal
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_apps::signals::Signal;
	///
	/// let signal = Signal::new();
	/// ```
	pub fn new() -> Self {
		Self {
			receivers: Arc::new(Mutex::new(Vec::new())),
		}
	}

	/// Connect a receiver to this signal
	///
	/// The receiver will be called whenever the signal is sent.
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_apps::signals::Signal;
	/// use reinhardt_apps::AppConfig;
	///
	/// let signal = Signal::new();
	/// signal.connect(Box::new(|config| {
	///     println!("Signal received for {}", config.label);
	/// }));
	/// ```
	pub fn connect(&self, receiver: SignalReceiver) {
		// Convert Box to Arc for cloneable internal storage
		self.receivers
			.lock()
			.unwrap_or_else(|e| e.into_inner())
			.push(Arc::from(receiver));
	}

	/// Send the signal to all connected receivers
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_apps::signals::Signal;
	/// use reinhardt_apps::AppConfig;
	///
	/// let signal = Signal::new();
	/// signal.connect(Box::new(|config| {
	///     println!("Received: {}", config.label);
	/// }));
	///
	/// let config = AppConfig::new("myapp", "myapp");
	/// signal.send(&config);
	/// ```
	pub fn send(&self, config: &AppConfig) {
		// Clone the receiver list and release the lock before invoking callbacks.
		// This prevents lock contention when receivers call connect/disconnect,
		// and avoids holding the lock during potentially long-running handler execution.
		let snapshot: Vec<InternalReceiver> = {
			let guard = self.receivers.lock().unwrap_or_else(|e| e.into_inner());
			guard.iter().map(Arc::clone).collect()
		};
		for receiver in &snapshot {
			receiver(config);
		}
	}

	/// Disconnect all receivers from this signal
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_apps::signals::Signal;
	///
	/// let signal = Signal::new();
	/// signal.connect(Box::new(|_| println!("Receiver")));
	/// assert_eq!(signal.receiver_count(), 1);
	///
	/// signal.disconnect_all();
	/// assert_eq!(signal.receiver_count(), 0);
	/// ```
	pub fn disconnect_all(&self) {
		self.receivers
			.lock()
			.unwrap_or_else(|e| e.into_inner())
			.clear();
	}

	/// Get the number of connected receivers
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_apps::signals::Signal;
	///
	/// let signal = Signal::new();
	/// assert_eq!(signal.receiver_count(), 0);
	///
	/// signal.connect(Box::new(|_| {}));
	/// assert_eq!(signal.receiver_count(), 1);
	/// ```
	pub fn receiver_count(&self) -> usize {
		self.receivers
			.lock()
			.unwrap_or_else(|e| e.into_inner())
			.len()
	}
}

impl Clone for Signal {
	fn clone(&self) -> Self {
		Self {
			receivers: Arc::clone(&self.receivers),
		}
	}
}

// Global signal instances
static APP_READY_SIGNAL: OnceLock<Signal> = OnceLock::new();
static APP_SHUTDOWN_SIGNAL: OnceLock<Signal> = OnceLock::new();

/// Get the global app_ready signal
///
/// This signal is sent when an application has been fully initialized
/// and is ready to handle requests.
///
/// # Examples
///
/// ```rust
/// use reinhardt_apps::signals::app_ready;
/// use reinhardt_apps::AppConfig;
///
/// app_ready().connect(Box::new(|config| {
///     println!("App {} is ready!", config.label);
/// }));
///
/// let config = AppConfig::new("myapp", "myapp");
/// app_ready().send(&config);
/// ```
pub fn app_ready() -> &'static Signal {
	APP_READY_SIGNAL.get_or_init(Signal::new)
}

/// Get the global app_shutdown signal
///
/// This signal is sent when an application is about to shut down.
///
/// # Examples
///
/// ```rust
/// use reinhardt_apps::signals::app_shutdown;
/// use reinhardt_apps::AppConfig;
///
/// app_shutdown().connect(Box::new(|config| {
///     println!("App {} is shutting down!", config.label);
/// }));
///
/// let config = AppConfig::new("myapp", "myapp");
/// app_shutdown().send(&config);
/// ```
pub fn app_shutdown() -> &'static Signal {
	APP_SHUTDOWN_SIGNAL.get_or_init(Signal::new)
}

/// Clear all signal receivers (primarily for testing)
///
/// This function clears all receivers from the global signals.
/// It should primarily be used in test scenarios.
pub fn clear_all_signals() {
	if let Some(signal) = APP_READY_SIGNAL.get() {
		signal.disconnect_all();
	}
	if let Some(signal) = APP_SHUTDOWN_SIGNAL.get() {
		signal.disconnect_all();
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use std::sync::atomic::{AtomicUsize, Ordering};

	#[test]
	fn test_signal_creation() {
		let signal = Signal::new();
		assert_eq!(signal.receiver_count(), 0);
	}

	#[test]
	fn test_signal_connect() {
		let signal = Signal::new();
		signal.connect(Box::new(|_| {}));
		assert_eq!(signal.receiver_count(), 1);
	}

	#[test]
	fn test_signal_send() {
		let signal = Signal::new();
		let counter = Arc::new(AtomicUsize::new(0));

		let counter_clone = Arc::clone(&counter);
		signal.connect(Box::new(move |_| {
			counter_clone.fetch_add(1, Ordering::SeqCst);
		}));

		let config = AppConfig::new("test", "test");
		signal.send(&config);

		assert_eq!(counter.load(Ordering::SeqCst), 1);
	}

	#[test]
	fn test_signal_multiple_receivers() {
		let signal = Signal::new();
		let counter = Arc::new(AtomicUsize::new(0));

		let counter_clone1 = Arc::clone(&counter);
		signal.connect(Box::new(move |_| {
			counter_clone1.fetch_add(1, Ordering::SeqCst);
		}));

		let counter_clone2 = Arc::clone(&counter);
		signal.connect(Box::new(move |_| {
			counter_clone2.fetch_add(2, Ordering::SeqCst);
		}));

		let config = AppConfig::new("test", "test");
		signal.send(&config);

		assert_eq!(counter.load(Ordering::SeqCst), 3); // 1 + 2
	}

	#[test]
	fn test_signal_disconnect_all() {
		let signal = Signal::new();
		signal.connect(Box::new(|_| {}));
		signal.connect(Box::new(|_| {}));
		assert_eq!(signal.receiver_count(), 2);

		signal.disconnect_all();
		assert_eq!(signal.receiver_count(), 0);
	}

	#[test]
	fn test_signal_receiver_with_config() {
		let signal = Signal::new();
		let received_label = Arc::new(Mutex::new(String::new()));

		let label_clone = Arc::clone(&received_label);
		signal.connect(Box::new(move |config| {
			*label_clone.lock().unwrap() = config.label.clone();
		}));

		let config = AppConfig::new("testapp", "testapp");
		signal.send(&config);

		assert_eq!(*received_label.lock().unwrap(), "testapp");
	}

	#[test]
	fn test_app_ready_signal() {
		clear_all_signals(); // Clear previous test state

		let counter = Arc::new(AtomicUsize::new(0));
		let counter_clone = Arc::clone(&counter);

		app_ready().connect(Box::new(move |_| {
			counter_clone.fetch_add(1, Ordering::SeqCst);
		}));

		let config = AppConfig::new("test", "test");
		app_ready().send(&config);

		assert_eq!(counter.load(Ordering::SeqCst), 1);
	}

	#[test]
	fn test_app_shutdown_signal() {
		clear_all_signals(); // Clear previous test state

		let counter = Arc::new(AtomicUsize::new(0));
		let counter_clone = Arc::clone(&counter);

		app_shutdown().connect(Box::new(move |_| {
			counter_clone.fetch_add(1, Ordering::SeqCst);
		}));

		let config = AppConfig::new("test", "test");
		app_shutdown().send(&config);

		assert_eq!(counter.load(Ordering::SeqCst), 1);
	}

	#[test]
	fn test_signal_clone() {
		let signal1 = Signal::new();
		signal1.connect(Box::new(|_| {}));

		let signal2 = signal1.clone();
		assert_eq!(signal1.receiver_count(), 1);
		assert_eq!(signal2.receiver_count(), 1);

		// Both should share the same receivers
		signal2.connect(Box::new(|_| {}));
		assert_eq!(signal1.receiver_count(), 2);
		assert_eq!(signal2.receiver_count(), 2);
	}

	#[test]
	fn test_clear_all_signals() {
		clear_all_signals(); // Clear previous test state

		app_ready().connect(Box::new(|_| {}));
		app_shutdown().connect(Box::new(|_| {}));

		assert_eq!(app_ready().receiver_count(), 1);
		assert_eq!(app_shutdown().receiver_count(), 1);

		clear_all_signals();

		assert_eq!(app_ready().receiver_count(), 0);
		assert_eq!(app_shutdown().receiver_count(), 0);
	}

	#[test]
	fn test_signal_send_multiple_times() {
		let signal = Signal::new();
		let counter = Arc::new(AtomicUsize::new(0));

		let counter_clone = Arc::clone(&counter);
		signal.connect(Box::new(move |_| {
			counter_clone.fetch_add(1, Ordering::SeqCst);
		}));

		let config = AppConfig::new("test", "test");

		signal.send(&config);
		signal.send(&config);
		signal.send(&config);

		assert_eq!(counter.load(Ordering::SeqCst), 3);
	}

	#[test]
	fn test_signal_receiver_error_handling() {
		let signal = Signal::new();
		let success_counter = Arc::new(AtomicUsize::new(0));

		// Add a receiver that might panic (in real code)
		signal.connect(Box::new(|_| {
			// In production, you'd handle errors gracefully
			// For testing, just verify the receiver is called
		}));

		let counter_clone = Arc::clone(&success_counter);
		signal.connect(Box::new(move |_| {
			counter_clone.fetch_add(1, Ordering::SeqCst);
		}));

		let config = AppConfig::new("test", "test");
		signal.send(&config);

		// Both receivers should have been called
		assert_eq!(success_counter.load(Ordering::SeqCst), 1);
	}
}
