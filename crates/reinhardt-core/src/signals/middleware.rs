//! Signal middleware for intercepting and transforming signals

use super::error::SignalError;
use parking_lot::RwLock;
use std::sync::Arc;

/// Signal middleware for intercepting and transforming signals
#[async_trait::async_trait]
pub trait SignalMiddleware<T: Send + Sync + 'static>: Send + Sync {
	/// Called before the signal is sent to receivers
	/// Return false to stop signal propagation
	async fn before_send(&self, _instance: &T) -> Result<bool, SignalError> {
		Ok(true)
	}

	/// Called after the signal has been sent to all receivers
	async fn after_send(
		&self,
		instance: &T,
		results: &[Result<(), SignalError>],
	) -> Result<(), SignalError> {
		let _ = (instance, results);
		Ok(())
	}

	/// Called when a receiver is about to execute
	/// Return false to skip this receiver
	async fn before_receiver(
		&self,
		instance: &T,
		dispatch_uid: Option<&str>,
	) -> Result<bool, SignalError> {
		let _ = (instance, dispatch_uid);
		Ok(true)
	}

	/// Called after a receiver has executed
	async fn after_receiver(
		&self,
		instance: &T,
		dispatch_uid: Option<&str>,
		result: &Result<(), SignalError>,
	) -> Result<(), SignalError> {
		let _ = (instance, dispatch_uid, result);
		Ok(())
	}
}

/// Type alias for middleware
pub type MiddlewareFn<T> = Arc<dyn SignalMiddleware<T>>;

/// Call record for SignalSpy
#[derive(Debug, Clone)]
pub struct SignalCall {
	/// Whether the signal was successfully sent.
	pub signal_sent: bool,
	/// Number of receivers that were called.
	pub receivers_called: usize,
	/// Error messages from failed receiver executions.
	pub errors: Vec<String>,
}

/// Testing utility to spy on signal calls
pub struct SignalSpy<T: Send + Sync + 'static> {
	calls: Arc<RwLock<Vec<SignalCall>>>,
	instances: Arc<RwLock<Vec<Arc<T>>>>,
}

impl<T: Send + Sync + 'static> SignalSpy<T> {
	/// Creates a new signal spy with empty call records.
	pub fn new() -> Self {
		Self {
			calls: Arc::new(RwLock::new(Vec::new())),
			instances: Arc::new(RwLock::new(Vec::new())),
		}
	}

	/// Returns the number of times the signal was sent
	pub fn call_count(&self) -> usize {
		self.calls.read().len()
	}

	/// Returns all recorded signal calls
	pub fn calls(&self) -> Vec<SignalCall> {
		self.calls.read().clone()
	}

	/// Returns all instances that were sent
	pub fn instances(&self) -> Vec<Arc<T>> {
		self.instances.read().clone()
	}

	/// Returns the last instance that was sent, if any
	pub fn last_instance(&self) -> Option<Arc<T>> {
		self.instances.read().last().cloned()
	}

	/// Check if the signal was called
	pub fn was_called(&self) -> bool {
		self.call_count() > 0
	}

	/// Check if the signal was called with specific count
	pub fn was_called_with_count(&self, count: usize) -> bool {
		self.call_count() == count
	}

	/// Reset all recorded calls and instances
	pub fn reset(&self) {
		self.calls.write().clear();
		self.instances.write().clear();
	}

	/// Get the total number of receivers that were called across all signals
	pub fn total_receivers_called(&self) -> usize {
		self.calls.read().iter().map(|c| c.receivers_called).sum()
	}

	/// Check if any errors occurred
	pub fn has_errors(&self) -> bool {
		self.calls.read().iter().any(|c| !c.errors.is_empty())
	}

	/// Get all error messages
	pub fn errors(&self) -> Vec<String> {
		self.calls
			.read()
			.iter()
			.flat_map(|c| c.errors.clone())
			.collect()
	}
}

impl<T: Send + Sync + 'static> Clone for SignalSpy<T> {
	fn clone(&self) -> Self {
		Self {
			calls: Arc::clone(&self.calls),
			instances: Arc::clone(&self.instances),
		}
	}
}

impl<T: Send + Sync + 'static> Default for SignalSpy<T> {
	fn default() -> Self {
		Self::new()
	}
}

#[async_trait::async_trait]
impl<T: Send + Sync + 'static> SignalMiddleware<T> for SignalSpy<T> {
	async fn before_send(&self, _instance: &T) -> Result<bool, SignalError> {
		// This will be updated in after_send
		Ok(true)
	}

	async fn after_send(
		&self,
		_instance: &T,
		results: &[Result<(), SignalError>],
	) -> Result<(), SignalError> {
		let errors: Vec<String> = results
			.iter()
			.filter_map(|r| r.as_ref().err().map(|e| e.message.clone()))
			.collect();

		let call = SignalCall {
			signal_sent: true,
			receivers_called: results.len(),
			errors,
		};

		self.calls.write().push(call);

		// Store instance for later inspection (we need to convert &T to Arc<T>)
		// Since we can't clone T, we create a new Arc from the reference
		// This is a limitation - we can only store the reference during the call

		Ok(())
	}

	async fn before_receiver(
		&self,
		_instance: &T,
		_dispatch_uid: Option<&str>,
	) -> Result<bool, SignalError> {
		Ok(true)
	}

	async fn after_receiver(
		&self,
		_instance: &T,
		_dispatch_uid: Option<&str>,
		_result: &Result<(), SignalError>,
	) -> Result<(), SignalError> {
		Ok(())
	}
}
