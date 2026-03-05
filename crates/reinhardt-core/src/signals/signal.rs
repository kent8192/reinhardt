//! Core Signal implementation

use super::context::{MetricsCollector, SignalContext, SignalMetrics};
use super::core::{AsyncSignalDispatcher, ReceiverFn, SignalDispatcher, SignalName};
use super::error::SignalError;
use super::middleware::{MiddlewareFn, SignalMiddleware};
use parking_lot::RwLock;
use std::any::TypeId;
use std::fmt;
use std::future::Future;
use std::sync::Arc;
use std::time::Instant;

/// Type alias for predicate functions
type PredicateFn<T> = Arc<dyn Fn(&T) -> bool + Send + Sync>;

/// Information about a connected receiver
pub(crate) struct ReceiverInfo<T: Send + Sync + 'static> {
	pub(crate) receiver: ReceiverFn<T>,
	pub(crate) sender_type_id: Option<TypeId>,
	pub(crate) dispatch_uid: Option<String>,
	pub(crate) priority: i32,                     // Higher values execute first
	pub(crate) predicate: Option<PredicateFn<T>>, // Optional condition for execution
}

impl<T: Send + Sync + 'static> Clone for ReceiverInfo<T> {
	fn clone(&self) -> Self {
		Self {
			receiver: Arc::clone(&self.receiver),
			sender_type_id: self.sender_type_id,
			dispatch_uid: self.dispatch_uid.clone(),
			priority: self.priority,
			predicate: self.predicate.clone(),
		}
	}
}

/// A signal that can dispatch events to connected receivers
pub struct Signal<T: Send + Sync + 'static> {
	receivers: Arc<RwLock<Vec<ReceiverInfo<T>>>>,
	middlewares: Arc<RwLock<Vec<MiddlewareFn<T>>>>,
	context: SignalContext,
	metrics: Arc<MetricsCollector>,
	name: String,
}

impl<T: Send + Sync + 'static> Signal<T> {
	/// Create a new signal with a type-safe name
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::signals::{Signal, SignalName};
	///
	/// // Use built-in signal names
	/// let signal = Signal::<String>::new(SignalName::PRE_SAVE);
	///
	/// // Use custom signal names
	/// let custom = Signal::<String>::new(SignalName::custom("my_signal"));
	/// ```
	pub fn new(name: SignalName) -> Self {
		Self {
			receivers: Arc::new(RwLock::new(Vec::new())),
			middlewares: Arc::new(RwLock::new(Vec::new())),
			context: SignalContext::new(),
			metrics: Arc::new(MetricsCollector::new()),
			name: name.as_str().to_string(),
		}
	}

	/// Create a new signal with a string name (for backward compatibility)
	///
	/// Consider using `Signal::new(SignalName::custom(name))` for type safety.
	#[doc(hidden)]
	pub fn new_with_string(name: impl Into<String>) -> Self {
		Self {
			receivers: Arc::new(RwLock::new(Vec::new())),
			middlewares: Arc::new(RwLock::new(Vec::new())),
			context: SignalContext::new(),
			metrics: Arc::new(MetricsCollector::new()),
			name: name.into(),
		}
	}

	/// Get the current metrics for this signal
	pub fn metrics(&self) -> SignalMetrics {
		self.metrics.snapshot()
	}

	/// Reset metrics for this signal
	pub fn reset_metrics(&self) {
		self.metrics.reset();
	}

	/// Add middleware to this signal
	pub fn add_middleware<M>(&self, middleware: M)
	where
		M: SignalMiddleware<T> + 'static,
	{
		let mut middlewares = self.middlewares.write();
		middlewares.push(Arc::new(middleware));
	}

	/// Get the signal context
	pub fn context(&self) -> &SignalContext {
		&self.context
	}

	/// Connect a receiver function to this signal with full options
	///
	/// # Arguments
	/// * `receiver` - The receiver function to connect
	/// * `sender_type_id` - Optional TypeId to filter by sender type
	/// * `dispatch_uid` - Optional unique identifier to prevent duplicate registration
	/// * `priority` - Execution priority (higher values execute first, default: 0)
	pub fn connect_with_options<F, Fut>(
		&self,
		receiver: F,
		sender_type_id: Option<TypeId>,
		dispatch_uid: Option<String>,
		priority: i32,
	) where
		F: Fn(Arc<T>) -> Fut + Send + Sync + 'static,
		Fut: Future<Output = Result<(), SignalError>> + Send + 'static,
	{
		self.connect_with_full_options::<F, Fut, fn(&T) -> bool>(
			receiver,
			sender_type_id,
			dispatch_uid,
			priority,
			None,
		);
	}

	/// Connect a receiver with all available options including predicate
	///
	/// # Arguments
	/// * `receiver` - The receiver function to connect
	/// * `sender_type_id` - Optional TypeId to filter by sender type
	/// * `dispatch_uid` - Optional unique identifier to prevent duplicate registration
	/// * `priority` - Execution priority (higher values execute first, default: 0)
	/// * `predicate` - Optional condition that must be true for receiver to execute
	pub fn connect_with_full_options<F, Fut, P>(
		&self,
		receiver: F,
		sender_type_id: Option<TypeId>,
		dispatch_uid: Option<String>,
		priority: i32,
		predicate: Option<P>,
	) where
		F: Fn(Arc<T>) -> Fut + Send + Sync + 'static,
		Fut: Future<Output = Result<(), SignalError>> + Send + 'static,
		P: Fn(&T) -> bool + Send + Sync + 'static,
	{
		let boxed: ReceiverFn<T> = Arc::new(move |instance| Box::pin(receiver(instance)));
		let pred: Option<PredicateFn<T>> = predicate.map(|p| Arc::new(p) as PredicateFn<T>);
		let mut receivers = self.receivers.write();

		// Remove existing receiver with same dispatch_uid
		if let Some(ref uid) = dispatch_uid {
			receivers.retain(|r| r.dispatch_uid.as_ref() != Some(uid));
		}

		receivers.push(ReceiverInfo {
			receiver: boxed,
			sender_type_id,
			dispatch_uid,
			priority,
			predicate: pred,
		});

		// Sort by priority (descending - higher priority first)
		receivers.sort_by(|a, b| b.priority.cmp(&a.priority));
	}

	/// Connect a receiver function to this signal (simple version)
	pub fn connect<F, Fut>(&self, receiver: F)
	where
		F: Fn(Arc<T>) -> Fut + Send + Sync + 'static,
		Fut: Future<Output = Result<(), SignalError>> + Send + 'static,
	{
		self.connect_with_options(receiver, None, None, 0);
	}

	/// Connect a receiver with priority
	pub fn connect_with_priority<F, Fut>(&self, receiver: F, priority: i32)
	where
		F: Fn(Arc<T>) -> Fut + Send + Sync + 'static,
		Fut: Future<Output = Result<(), SignalError>> + Send + 'static,
	{
		self.connect_with_options(receiver, None, None, priority);
	}

	/// Chain this signal to another signal
	/// When this signal is sent, it will also trigger the chained signal
	///
	/// # Example
	/// ```rust,no_run
	/// # use reinhardt_core::signals::{Signal, SignalName};
	/// # use serde::{Serialize, Deserialize};
	/// # #[derive(Debug, Clone, Serialize, Deserialize)]
	/// # struct User { id: Option<i64> }
	/// # #[tokio::main]
	/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
	/// let signal_a = Signal::<User>::new(SignalName::custom("user_created"));
	/// let signal_b = Signal::<User>::new(SignalName::custom("user_notified"));
	///
	/// signal_a.chain(&signal_b);
	///
	/// // Now sending signal_a will also trigger signal_b
	/// # let user = User { id: None };
	/// signal_a.send(user).await?;
	/// # Ok(())
	/// # }
	/// ```
	pub fn chain(&self, next: &Signal<T>)
	where
		T: Clone,
	{
		let next_clone = next.clone();
		self.connect(move |instance| {
			let next = next_clone.clone();
			async move {
				let value = (*instance).clone();
				next.send(value).await
			}
		});
	}

	/// Chain this signal to another signal with transformation
	/// Allows transforming the instance before passing to the next signal
	///
	/// # Example
	/// ```rust,no_run
	/// # use reinhardt_core::signals::{Signal, SignalName};
	/// # use std::sync::Arc;
	/// # struct User { id: i64 }
	/// # struct NotificationPayload { user_id: i64 }
	/// # impl From<Arc<User>> for NotificationPayload {
	/// #     fn from(user: Arc<User>) -> Self {
	/// #         Self { user_id: user.id }
	/// #     }
	/// # }
	/// # let signal_a = Signal::<User>::new(SignalName::custom("user_created"));
	/// # let signal_b = Signal::<NotificationPayload>::new(SignalName::custom("notification"));
	/// signal_a.chain_with(&signal_b, |user| {
	///     // Transform User to NotificationPayload
	///     NotificationPayload::from(user)
	/// });
	/// ```
	pub fn chain_with<U, F>(&self, next: &Signal<U>, transform: F)
	where
		U: Send + Sync + 'static,
		F: Fn(Arc<T>) -> U + Send + Sync + 'static,
	{
		let next_clone = next.clone();
		let transform = Arc::new(transform);
		self.connect(move |instance| {
			let next = next_clone.clone();
			let transform = transform.clone();
			async move {
				let transformed = transform(instance);
				next.send(transformed).await
			}
		});
	}

	/// Merge multiple signals into one
	/// Returns a new signal that triggers when any of the source signals trigger
	///
	/// # Example
	/// ```rust,no_run
	/// # use reinhardt_core::signals::{Signal, SignalName};
	/// # use serde::{Serialize, Deserialize};
	/// # #[derive(Debug, Clone, Serialize, Deserialize)]
	/// # struct User { id: Option<i64> }
	/// # #[tokio::main]
	/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
	/// # let signal_a = Signal::<User>::new(SignalName::custom("a"));
	/// # let signal_b = Signal::<User>::new(SignalName::custom("b"));
	/// # let signal_c = Signal::<User>::new(SignalName::custom("c"));
	/// let merged = Signal::merge(vec![&signal_a, &signal_b, &signal_c]);
	///
	/// merged.connect(|instance| async move {
	///     println!("Any of the three signals was triggered!");
	///     Ok(())
	/// });
	/// # Ok(())
	/// # }
	/// ```
	pub fn merge(signals: Vec<&Signal<T>>) -> Signal<T>
	where
		T: Clone,
	{
		let merged = Signal::new(SignalName::custom("merged_signal"));

		for signal in signals {
			let merged_clone = merged.clone();
			signal.connect(move |instance| {
				let merged = merged_clone.clone();
				async move {
					let value = (*instance).clone();
					merged.send(value).await
				}
			});
		}

		merged
	}

	/// Filter signal emissions based on a predicate
	/// Returns a new signal that only triggers when the predicate returns true
	///
	/// # Example
	/// ```rust,no_run
	/// # use reinhardt_core::signals::{Signal, SignalName};
	/// # #[derive(Clone)]
	/// # struct User { is_admin: bool }
	/// # #[tokio::main]
	/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
	/// # let user_signal = Signal::<User>::new(SignalName::custom("user"));
	/// let admin_only = user_signal.filter(|user| user.is_admin);
	///
	/// admin_only.connect(|admin_user| async move {
	///     println!("Admin user action!");
	///     Ok(())
	/// });
	/// # Ok(())
	/// # }
	/// ```
	pub fn filter<P>(&self, predicate: P) -> Signal<T>
	where
		P: Fn(&T) -> bool + Send + Sync + 'static,
		T: Clone,
	{
		let filtered = Signal::new_with_string(format!("{}_filtered", self.name));
		let predicate = Arc::new(predicate);
		let filtered_clone = filtered.clone();

		self.connect(move |instance| {
			let filtered = filtered_clone.clone();
			let predicate = predicate.clone();
			async move {
				if predicate(&*instance) {
					let value = (*instance).clone();
					filtered.send(value).await
				} else {
					Ok(())
				}
			}
		});

		filtered
	}

	/// Transform signal values
	/// Returns a new signal with transformed values
	///
	/// # Example
	/// ```rust,no_run
	/// # use reinhardt_core::signals::{Signal, SignalName};
	/// # use std::sync::Arc;
	/// # struct User { id: i64 }
	/// # #[tokio::main]
	/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
	/// # let user_signal = Signal::<User>::new(SignalName::custom("user"));
	/// let user_ids = user_signal.map(|user| user.id);
	///
	/// user_ids.connect(|id| async move {
	///     println!("User ID: {}", id);
	///     Ok(())
	/// });
	/// # Ok(())
	/// # }
	/// ```
	pub fn map<U, F>(&self, transform: F) -> Signal<U>
	where
		U: Send + Sync + 'static,
		F: Fn(Arc<T>) -> U + Send + Sync + 'static,
	{
		let mapped = Signal::new_with_string(format!("{}_mapped", self.name));
		let transform = Arc::new(transform);
		let mapped_clone = mapped.clone();

		self.connect(move |instance| {
			let mapped = mapped_clone.clone();
			let transform = transform.clone();
			async move {
				let transformed = transform(instance);
				mapped.send(transformed).await
			}
		});

		mapped
	}

	/// Connect a receiver with a predicate condition
	///
	/// The receiver will only execute if the predicate returns true for the instance
	pub fn connect_if<F, Fut, P>(&self, receiver: F, predicate: P)
	where
		F: Fn(Arc<T>) -> Fut + Send + Sync + 'static,
		Fut: Future<Output = Result<(), SignalError>> + Send + 'static,
		P: Fn(&T) -> bool + Send + Sync + 'static,
	{
		self.connect_with_full_options(receiver, None, None, 0, Some(predicate));
	}

	/// Disconnect a receiver by dispatch_uid
	pub fn disconnect(&self, dispatch_uid: &str) -> bool {
		let mut receivers = self.receivers.write();
		let original_len = receivers.len();
		receivers.retain(|r| r.dispatch_uid.as_deref() != Some(dispatch_uid));
		receivers.len() < original_len
	}

	/// Send signal to all connected receivers
	///
	/// # Arguments
	/// * `instance` - The instance to send
	/// * `sender_type_id` - Optional TypeId of the sender for filtering
	pub async fn send_with_sender(
		&self,
		instance: T,
		sender_type_id: Option<TypeId>,
	) -> Result<(), SignalError> {
		// Record send event
		self.metrics.record_send();

		let instance = Arc::new(instance);
		let receivers = self.receivers.read().clone();
		let middlewares = self.middlewares.read().clone();

		// Execute before_send middleware hooks
		for middleware in &middlewares {
			let should_continue = middleware.before_send(&instance).await?;
			if !should_continue {
				return Ok(()); // Middleware stopped signal propagation
			}
		}

		let mut results = Vec::new();

		for receiver_info in receivers {
			// Check sender type match
			if let Some(expected_type_id) = receiver_info.sender_type_id {
				if let Some(actual_type_id) = sender_type_id {
					if expected_type_id != actual_type_id {
						continue; // Type mismatch, skip this receiver
					}
				} else {
					continue; // Receiver expects a specific sender, but None was provided
				}
			}

			// Check predicate condition
			if let Some(ref predicate) = receiver_info.predicate
				&& !predicate(&instance)
			{
				continue; // Predicate failed, skip this receiver
			}

			// Execute before_receiver middleware hooks
			let dispatch_uid_ref = receiver_info.dispatch_uid.as_deref();
			let mut should_execute = true;
			for middleware in &middlewares {
				let can_execute = middleware
					.before_receiver(&instance, dispatch_uid_ref)
					.await?;
				if !can_execute {
					should_execute = false;
					break;
				}
			}

			if !should_execute {
				continue; // Middleware skipped this receiver
			}

			// Execute receiver and measure time
			let start = Instant::now();
			let result = (receiver_info.receiver)(Arc::clone(&instance)).await;
			let duration = start.elapsed();

			// Record metrics
			self.metrics
				.record_receiver_execution(duration, result.is_ok());

			// Execute after_receiver middleware hooks
			for middleware in &middlewares {
				middleware
					.after_receiver(&instance, dispatch_uid_ref, &result)
					.await?;
			}

			// Stop on first error (not robust mode)
			result?;
			results.push(Ok(()));
		}

		// Execute after_send middleware hooks
		for middleware in &middlewares {
			middleware.after_send(&instance, &results).await?;
		}

		Ok(())
	}

	/// Send signal to all connected receivers (simple version)
	pub async fn send(&self, instance: T) -> Result<(), SignalError> {
		self.send_with_sender(instance, None).await
	}

	/// Send signal robustly, catching errors
	pub async fn send_robust(
		&self,
		instance: T,
		sender_type_id: Option<TypeId>,
	) -> Vec<Result<(), SignalError>> {
		// Record send event
		self.metrics.record_send();

		let instance = Arc::new(instance);
		let receivers = self.receivers.read().clone();
		let middlewares = self.middlewares.read().clone();
		let mut results = Vec::new();

		// Execute before_send middleware hooks (ignore errors in robust mode)
		for middleware in &middlewares {
			if let Ok(should_continue) = middleware.before_send(&instance).await
				&& !should_continue
			{
				return results; // Middleware stopped signal propagation
			}
		}

		for receiver_info in receivers {
			// Check sender type match
			if let Some(expected_type_id) = receiver_info.sender_type_id {
				if let Some(actual_type_id) = sender_type_id {
					if expected_type_id != actual_type_id {
						continue; // Type mismatch, skip this receiver
					}
				} else {
					continue; // Receiver expects a specific sender, but None was provided
				}
			}

			// Check predicate condition
			if let Some(ref predicate) = receiver_info.predicate
				&& !predicate(&instance)
			{
				continue; // Predicate failed, skip this receiver
			}

			// Execute before_receiver middleware hooks
			let dispatch_uid_ref = receiver_info.dispatch_uid.as_deref();
			let mut should_execute = true;
			for middleware in &middlewares {
				if let Ok(can_execute) = middleware
					.before_receiver(&instance, dispatch_uid_ref)
					.await && !can_execute
				{
					should_execute = false;
					break;
				}
			}

			if !should_execute {
				continue; // Middleware skipped this receiver
			}

			// Execute receiver and measure time
			let start = Instant::now();
			let result = (receiver_info.receiver)(Arc::clone(&instance)).await;
			let duration = start.elapsed();

			// Record metrics
			self.metrics
				.record_receiver_execution(duration, result.is_ok());

			// Execute after_receiver middleware hooks (ignore errors)
			for middleware in &middlewares {
				let _ = middleware
					.after_receiver(&instance, dispatch_uid_ref, &result)
					.await;
			}

			results.push(result);
		}

		// Execute after_send middleware hooks
		for middleware in &middlewares {
			if let Err(e) = middleware.after_send(&instance, &results).await {
				eprintln!("Signal after_send middleware error: {}", e);
			}
		}

		results
	}

	/// Send signal asynchronously (fire and forget)
	#[cfg(not(target_arch = "wasm32"))]
	pub fn send_async(&self, instance: T) {
		let instance = Arc::new(instance);
		let receivers = self.receivers.read().clone();

		tokio::spawn(async move {
			for receiver_info in receivers {
				let _ = (receiver_info.receiver)(Arc::clone(&instance)).await;
			}
		});
	}

	/// Get number of connected receivers
	pub fn receiver_count(&self) -> usize {
		self.receivers.read().len()
	}

	/// Clear all receivers
	pub fn disconnect_all(&self) {
		self.receivers.write().clear();
	}
}

impl<T: Send + Sync + 'static> Clone for Signal<T> {
	fn clone(&self) -> Self {
		Self {
			receivers: Arc::clone(&self.receivers),
			middlewares: Arc::clone(&self.middlewares),
			context: self.context.clone(),
			metrics: Arc::clone(&self.metrics),
			name: self.name.clone(),
		}
	}
}

impl<T: Send + Sync + 'static> fmt::Debug for Signal<T> {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_struct("Signal")
			.field("name", &self.name)
			.field("receiver_count", &self.receiver_count())
			.finish()
	}
}

// Implement SignalDispatcher trait for Signal
impl<T: Send + Sync + 'static> SignalDispatcher<T> for Signal<T> {
	fn receiver_count(&self) -> usize {
		Signal::receiver_count(self)
	}

	fn disconnect_all(&self) {
		Signal::disconnect_all(self)
	}

	fn disconnect(&self, dispatch_uid: &str) -> bool {
		Signal::disconnect(self, dispatch_uid)
	}
}

// Implement AsyncSignalDispatcher trait for Signal
#[async_trait::async_trait]
impl<T: Send + Sync + 'static> AsyncSignalDispatcher<T> for Signal<T> {
	async fn send(&self, instance: T) -> Result<(), SignalError> {
		Signal::send(self, instance).await
	}

	async fn send_with_sender(
		&self,
		instance: T,
		sender_type_id: Option<TypeId>,
	) -> Result<(), SignalError> {
		Signal::send_with_sender(self, instance, sender_type_id).await
	}

	async fn send_robust(
		&self,
		instance: T,
		sender_type_id: Option<TypeId>,
	) -> Vec<Result<(), SignalError>> {
		Signal::send_robust(self, instance, sender_type_id).await
	}
}
