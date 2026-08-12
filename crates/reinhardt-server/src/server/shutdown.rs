use std::future::Future;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tokio::sync::Notify;
use tokio::sync::broadcast;
use tokio::time::timeout;

/// Shutdown coordinator that manages graceful server shutdown
///
/// Handles signal listening, connection tracking, and graceful shutdown with timeout.
///
/// # Examples
///
/// ```
/// use reinhardt_server::server::shutdown::ShutdownCoordinator;
/// use std::time::Duration;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let coordinator = ShutdownCoordinator::new(Duration::from_secs(30));
///
/// // Start shutdown
/// coordinator.shutdown();
///
/// // Wait for shutdown to complete
/// coordinator.wait_for_shutdown().await;
/// # Ok(())
/// # }
/// ```
#[derive(Clone)]
pub struct ShutdownCoordinator {
	/// Broadcast channel for shutdown signal
	shutdown_tx: broadcast::Sender<()>,
	/// Atomic shutdown state shared by all coordinator clones.
	shutdown: Arc<AtomicBool>,
	/// Notification for graceful shutdown completion
	shutdown_complete: Arc<Notify>,
	/// Shutdown timeout duration
	timeout_duration: Duration,
}

impl ShutdownCoordinator {
	/// Create a new shutdown coordinator
	///
	/// # Arguments
	///
	/// * `timeout_duration` - Maximum time to wait for graceful shutdown
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_server::server::shutdown::ShutdownCoordinator;
	/// use std::time::Duration;
	///
	/// let coordinator = ShutdownCoordinator::new(Duration::from_secs(30));
	/// ```
	pub fn new(timeout_duration: Duration) -> Self {
		let (shutdown_tx, _) = broadcast::channel(1);
		let shutdown_complete = Arc::new(Notify::new());

		Self {
			shutdown_tx,
			shutdown: Arc::new(AtomicBool::new(false)),
			shutdown_complete,
			timeout_duration,
		}
	}

	/// Subscribe to shutdown signal
	///
	/// Returns a receiver that will be notified when shutdown is initiated.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_server::server::shutdown::ShutdownCoordinator;
	/// use std::time::Duration;
	///
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// let coordinator = ShutdownCoordinator::new(Duration::from_secs(30));
	/// let mut shutdown_rx = coordinator.subscribe();
	///
	/// // Wait for shutdown signal
	/// tokio::select! {
	///     _ = shutdown_rx.recv() => {
	///         println!("Shutdown signal received");
	///     }
	/// }
	/// # Ok(())
	/// # }
	/// ```
	pub fn subscribe(&self) -> broadcast::Receiver<()> {
		let receiver = self.shutdown_tx.subscribe();
		if self.is_shutdown() {
			let (late_tx, late_receiver) = broadcast::channel(1);
			let _ = late_tx.send(());
			return late_receiver;
		}
		receiver
	}

	/// Initiate graceful shutdown
	///
	/// Sends shutdown signal to all subscribers.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_server::server::shutdown::ShutdownCoordinator;
	/// use std::time::Duration;
	///
	/// let coordinator = ShutdownCoordinator::new(Duration::from_secs(30));
	/// coordinator.shutdown();
	/// ```
	pub fn shutdown(&self) {
		if self
			.shutdown
			.compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
			.is_ok()
		{
			let _ = self.shutdown_tx.send(());
		}
	}

	/// Returns whether shutdown has been initiated.
	pub fn is_shutdown(&self) -> bool {
		self.shutdown.load(Ordering::Acquire)
	}

	/// Notify that a component has completed shutdown
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_server::server::shutdown::ShutdownCoordinator;
	/// use std::time::Duration;
	///
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// let coordinator = ShutdownCoordinator::new(Duration::from_secs(30));
	///
	/// // Component finishes cleanup
	/// coordinator.notify_shutdown_complete();
	/// # Ok(())
	/// # }
	/// ```
	pub fn notify_shutdown_complete(&self) {
		self.shutdown_complete.notify_one();
	}

	/// Wait for graceful shutdown to complete
	///
	/// Waits for shutdown notification with timeout.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_server::server::shutdown::ShutdownCoordinator;
	/// use std::time::Duration;
	///
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// let coordinator = ShutdownCoordinator::new(Duration::from_secs(30));
	/// coordinator.shutdown();
	/// coordinator.wait_for_shutdown().await;
	/// # Ok(())
	/// # }
	/// ```
	pub async fn wait_for_shutdown(&self) {
		match timeout(self.timeout_duration, self.shutdown_complete.notified()).await {
			Ok(_) => {
				println!("Graceful shutdown completed");
			}
			Err(_) => {
				eprintln!(
					"Shutdown timeout after {:?}, forcing termination",
					self.timeout_duration
				);
			}
		}
	}

	/// Get shutdown timeout duration
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_server::server::shutdown::ShutdownCoordinator;
	/// use std::time::Duration;
	///
	/// let coordinator = ShutdownCoordinator::new(Duration::from_secs(30));
	/// assert_eq!(coordinator.timeout_duration(), Duration::from_secs(30));
	/// ```
	pub fn timeout_duration(&self) -> Duration {
		self.timeout_duration
	}
}

/// Listen for OS shutdown signals (SIGTERM, SIGINT)
///
/// Returns when a shutdown signal is received.
///
/// # Examples
///
/// ```no_run
/// use reinhardt_server::server::shutdown::shutdown_signal;
///
/// # async fn example() {
/// println!("Waiting for shutdown signal...");
/// shutdown_signal().await;
/// println!("Shutdown signal received!");
/// # }
/// ```
pub async fn shutdown_signal() {
	use tokio::signal;

	let ctrl_c = async {
		signal::ctrl_c()
			.await
			.expect("Failed to install Ctrl+C handler");
	};

	#[cfg(unix)]
	let terminate = async {
		signal::unix::signal(signal::unix::SignalKind::terminate())
			.expect("Failed to install SIGTERM handler")
			.recv()
			.await;
	};

	#[cfg(not(unix))]
	let terminate = std::future::pending::<()>();

	tokio::select! {
		_ = ctrl_c => {
			println!("Received Ctrl+C signal");
		}
		_ = terminate => {
			println!("Received SIGTERM signal");
		}
	}
}

/// Wraps a future with shutdown handling
///
/// Runs the future until completion or shutdown signal, whichever comes first.
///
/// # Examples
///
/// ```
/// use reinhardt_server::server::shutdown::{with_shutdown, ShutdownCoordinator};
/// use std::time::Duration;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let coordinator = ShutdownCoordinator::new(Duration::from_secs(30));
/// let mut shutdown_rx = coordinator.subscribe();
///
/// let work = async {
///     // Some long-running work
///     tokio::time::sleep(Duration::from_secs(10)).await;
/// };
///
/// with_shutdown(work, shutdown_rx).await;
/// # Ok(())
/// # }
/// ```
pub async fn with_shutdown<F>(
	future: F,
	mut shutdown_rx: broadcast::Receiver<()>,
) -> Option<F::Output>
where
	F: Future,
{
	tokio::select! {
		result = future => Some(result),
		_ = shutdown_rx.recv() => None,
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use std::time::Duration;

	#[tokio::test]
	async fn test_shutdown_coordinator_creation() {
		let coordinator = ShutdownCoordinator::new(Duration::from_secs(30));
		assert_eq!(coordinator.timeout_duration(), Duration::from_secs(30));
	}

	#[tokio::test]
	async fn test_shutdown_signal_propagation() {
		let coordinator = ShutdownCoordinator::new(Duration::from_secs(1));
		let mut rx = coordinator.subscribe();

		coordinator.shutdown();

		// Should receive shutdown signal
		let result = rx.recv().await;
		assert!(result.is_ok());
	}

	#[tokio::test]
	async fn test_late_subscriber_receives_shutdown_signal() {
		let coordinator = ShutdownCoordinator::new(Duration::from_secs(1));
		coordinator.shutdown();

		let mut rx = coordinator.subscribe();
		assert_eq!(rx.recv().await, Ok(()));
	}

	#[tokio::test]
	async fn test_multiple_subscribers() {
		let coordinator = ShutdownCoordinator::new(Duration::from_secs(1));
		let mut rx1 = coordinator.subscribe();
		let mut rx2 = coordinator.subscribe();

		coordinator.shutdown();

		// Both should receive the signal
		assert!(rx1.recv().await.is_ok());
		assert!(rx2.recv().await.is_ok());
	}

	#[tokio::test]
	async fn test_shutdown_is_atomic_and_broadcast_once() {
		let coordinator = ShutdownCoordinator::new(Duration::from_secs(1));
		let mut rx = coordinator.subscribe();

		assert!(!coordinator.is_shutdown());
		coordinator.shutdown();
		coordinator.shutdown();
		assert!(coordinator.is_shutdown());
		assert_eq!(rx.recv().await, Ok(()));
		assert_eq!(rx.try_recv(), Err(broadcast::error::TryRecvError::Empty));
	}

	#[tokio::test]
	async fn test_late_subscriber_does_not_rebroadcast_to_existing_receivers() {
		let coordinator = ShutdownCoordinator::new(Duration::from_secs(1));
		let mut existing = coordinator.subscribe();

		coordinator.shutdown();
		assert_eq!(existing.recv().await, Ok(()));

		let mut late = coordinator.subscribe();
		assert_eq!(late.recv().await, Ok(()));
		assert_eq!(
			existing.try_recv(),
			Err(broadcast::error::TryRecvError::Empty)
		);
	}

	#[tokio::test]
	async fn test_shutdown_notification() {
		let coordinator = ShutdownCoordinator::new(Duration::from_secs(1));

		// Spawn a task to notify shutdown complete
		let coordinator_clone = coordinator.clone();
		tokio::spawn(async move {
			coordinator_clone.notify_shutdown_complete();
		});

		// Should complete before timeout
		coordinator.wait_for_shutdown().await;
	}

	#[tokio::test]
	async fn test_shutdown_timeout() {
		let coordinator = ShutdownCoordinator::new(Duration::from_millis(100));

		// Don't notify - should timeout
		let start = std::time::Instant::now();
		coordinator.wait_for_shutdown().await;
		let elapsed = start.elapsed();

		// Should have waited for the timeout duration
		assert!(elapsed >= Duration::from_millis(100));
		assert!(elapsed < Duration::from_millis(200));
	}

	#[tokio::test]
	async fn test_with_shutdown_completes_normally() {
		let coordinator = ShutdownCoordinator::new(Duration::from_secs(1));
		let shutdown_rx = coordinator.subscribe();

		let work = async { 42 };

		let result = with_shutdown(work, shutdown_rx).await;
		assert_eq!(result, Some(42));
	}

	#[tokio::test]
	async fn test_with_shutdown_interrupted() {
		let coordinator = ShutdownCoordinator::new(Duration::from_secs(1));
		let shutdown_rx = coordinator.subscribe();

		let work = async {
			// Add delay to ensure shutdown signal arrives first
			tokio::time::sleep(Duration::from_millis(100)).await;
			42
		};

		// Shutdown immediately
		coordinator.shutdown();

		let result = with_shutdown(work, shutdown_rx).await;
		assert_eq!(result, None);
	}
}
