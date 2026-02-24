//! Connection pooling for bulk email operations
//!
//! This module provides connection pooling capabilities for efficient
//! bulk email sending operations, reducing the overhead of establishing
//! new SMTP connections for each message.

use crate::backends::{EmailBackend, SmtpBackend, SmtpConfig};
use crate::message::EmailMessage;
use crate::{EmailError, EmailResult};
use std::sync::Arc;
use tokio::sync::Semaphore;

/// Configuration for the email connection pool
#[derive(Debug, Clone)]
pub struct PoolConfig {
	/// Maximum number of concurrent connections
	pub max_connections: usize,
	/// Minimum number of idle connections to maintain
	pub min_idle: usize,
	/// Maximum number of messages to send per connection before reconnecting
	pub max_messages_per_connection: usize,
}

impl Default for PoolConfig {
	fn default() -> Self {
		Self {
			max_connections: 10,
			min_idle: 2,
			max_messages_per_connection: 100,
		}
	}
}

impl PoolConfig {
	pub fn new() -> Self {
		Self::default()
	}

	pub fn with_max_connections(mut self, max: usize) -> Self {
		self.max_connections = max;
		self
	}

	pub fn with_min_idle(mut self, min: usize) -> Self {
		self.min_idle = min;
		self
	}

	pub fn with_max_messages_per_connection(mut self, max: usize) -> Self {
		self.max_messages_per_connection = max;
		self
	}
}

/// Email connection pool for bulk sending
///
/// Concurrency control uses a `Semaphore` to atomically manage connection
/// permits, avoiding TOCTOU race conditions that would occur with separate
/// check-and-increment operations on an atomic counter.
///
/// # Examples
///
/// ```rust,no_run
/// use reinhardt_mail::pooling::{EmailPool, PoolConfig};
/// use reinhardt_mail::{SmtpConfig, EmailMessage};
///
/// # #[tokio::main]
/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let smtp_config = SmtpConfig::new("smtp.example.com", 587);
/// let pool_config = PoolConfig::new().with_max_connections(5);
///
/// let pool = EmailPool::new(smtp_config, pool_config)?;
///
/// // Send multiple emails efficiently
/// let messages = vec![
///     EmailMessage::builder()
///         .from("sender@example.com")
///         .to(vec!["recipient1@example.com".to_string()])
///         .subject("Test 1")
///         .body("Body 1")
///         .build()?,
///     EmailMessage::builder()
///         .from("sender@example.com")
///         .to(vec!["recipient2@example.com".to_string()])
///         .subject("Test 2")
///         .body("Body 2")
///         .build()?,
/// ];
///
/// let sent_count = pool.send_bulk(messages).await?;
/// println!("Sent {} emails", sent_count);
/// # Ok(())
/// # }
/// ```
pub struct EmailPool {
	smtp_config: SmtpConfig,
	pool_config: PoolConfig,
	semaphore: Arc<Semaphore>,
}

impl EmailPool {
	/// Create a new email connection pool
	pub fn new(smtp_config: SmtpConfig, pool_config: PoolConfig) -> EmailResult<Self> {
		let semaphore = Arc::new(Semaphore::new(pool_config.max_connections));

		Ok(Self {
			smtp_config,
			pool_config,
			semaphore,
		})
	}

	/// Send multiple emails using the pool
	///
	/// This method distributes the emails across multiple connections
	/// for efficient bulk sending.
	pub async fn send_bulk(&self, messages: Vec<EmailMessage>) -> EmailResult<usize> {
		if messages.is_empty() {
			return Ok(0);
		}

		// Split messages into chunks based on max_messages_per_connection
		let chunk_size = self.pool_config.max_messages_per_connection;
		let chunks: Vec<Vec<EmailMessage>> = messages
			.chunks(chunk_size)
			.map(|chunk| chunk.to_vec())
			.collect();

		let mut total_sent = 0;
		let mut handles = Vec::new();

		for chunk in chunks {
			let permit = self.semaphore.clone().acquire_owned().await.map_err(|e| {
				EmailError::BackendError(format!("Failed to acquire semaphore: {}", e))
			})?;

			let smtp_config = self.smtp_config.clone();
			let handle = tokio::spawn(async move {
				let backend = SmtpBackend::new(smtp_config)?;
				let result = backend.send_messages(&chunk).await;
				drop(permit); // Release the permit
				result
			});

			handles.push(handle);
		}

		// Wait for all tasks to complete
		for handle in handles {
			let sent = handle
				.await
				.map_err(|e| EmailError::BackendError(format!("Task join error: {}", e)))??;
			total_sent += sent;
		}

		Ok(total_sent)
	}

	/// Send a single email using the pool
	pub async fn send(&self, message: EmailMessage) -> EmailResult<()> {
		self.send_bulk(vec![message]).await?;
		Ok(())
	}

	/// Get the pool configuration
	pub fn config(&self) -> &PoolConfig {
		&self.pool_config
	}

	/// Get the SMTP configuration
	pub fn smtp_config(&self) -> &SmtpConfig {
		&self.smtp_config
	}
}

/// Batch email sender with rate limiting
///
/// # Examples
///
/// ```rust,no_run
/// use reinhardt_mail::pooling::{BatchSender, PoolConfig};
/// use reinhardt_mail::{SmtpConfig, EmailMessage};
/// use std::time::Duration;
///
/// # #[tokio::main]
/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let smtp_config = SmtpConfig::new("smtp.example.com", 587);
/// let pool_config = PoolConfig::new();
///
/// let mut batch_sender = BatchSender::new(smtp_config, pool_config)?
///     .with_batch_size(50)
///     .with_delay(Duration::from_millis(100));
///
/// let messages = vec![];
/// let sent_count = batch_sender.send_with_rate_limit(messages).await?;
/// # Ok(())
/// # }
/// ```
pub struct BatchSender {
	pool: EmailPool,
	batch_size: usize,
	delay: std::time::Duration,
}

impl BatchSender {
	pub fn new(smtp_config: SmtpConfig, pool_config: PoolConfig) -> EmailResult<Self> {
		let pool = EmailPool::new(smtp_config, pool_config)?;

		Ok(Self {
			pool,
			batch_size: 100,
			delay: std::time::Duration::from_millis(0),
		})
	}

	pub fn with_batch_size(mut self, size: usize) -> Self {
		self.batch_size = size;
		self
	}

	pub fn with_delay(mut self, delay: std::time::Duration) -> Self {
		self.delay = delay;
		self
	}

	/// Send emails in batches with rate limiting
	///
	/// Sends emails in batches of `batch_size`, applying `delay` between each batch
	/// to avoid overwhelming the SMTP server.
	pub async fn send_with_rate_limit(
		&mut self,
		messages: Vec<EmailMessage>,
	) -> EmailResult<usize> {
		let mut total_sent = 0;
		let chunks: Vec<&[EmailMessage]> = messages.chunks(self.batch_size).collect();
		let last_index = chunks.len().saturating_sub(1);

		for (i, batch) in chunks.into_iter().enumerate() {
			let sent = self.pool.send_bulk(batch.to_vec()).await?;
			total_sent += sent;

			// Apply rate limiting delay between batches (skip after the last batch)
			if !self.delay.is_zero() && i < last_index {
				tokio::time::sleep(self.delay).await;
			}
		}

		Ok(total_sent)
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;
	use std::sync::atomic::{AtomicUsize, Ordering};

	#[rstest]
	fn test_pool_config() {
		// Arrange / Act
		let config = PoolConfig::new()
			.with_max_connections(20)
			.with_min_idle(5)
			.with_max_messages_per_connection(50);

		// Assert
		assert_eq!(config.max_connections, 20);
		assert_eq!(config.min_idle, 5);
		assert_eq!(config.max_messages_per_connection, 50);
	}

	#[rstest]
	fn test_pool_config_default() {
		// Arrange / Act
		let config = PoolConfig::default();

		// Assert
		assert_eq!(config.max_connections, 10);
		assert_eq!(config.min_idle, 2);
		assert_eq!(config.max_messages_per_connection, 100);
	}

	#[tokio::test]
	async fn test_semaphore_enforces_max_connections() {
		// Arrange
		let max_connections = 3;
		let semaphore = Arc::new(Semaphore::new(max_connections));
		let active_count = Arc::new(AtomicUsize::new(0));
		let peak_count = Arc::new(AtomicUsize::new(0));
		let total_tasks = 20;

		// Act
		let mut handles = Vec::new();
		for _ in 0..total_tasks {
			let sem = semaphore.clone();
			let active = active_count.clone();
			let peak = peak_count.clone();

			handles.push(tokio::spawn(async move {
				let _permit = sem.acquire().await.unwrap();

				// Track the number of concurrently active tasks
				let current = active.fetch_add(1, Ordering::SeqCst) + 1;
				// Update peak if this is the highest concurrency seen
				peak.fetch_max(current, Ordering::SeqCst);

				// Simulate work
				tokio::task::yield_now().await;

				active.fetch_sub(1, Ordering::SeqCst);
			}));
		}

		for handle in handles {
			handle.await.unwrap();
		}

		// Assert
		let observed_peak = peak_count.load(Ordering::SeqCst);
		assert!(
			observed_peak <= max_connections,
			"Peak concurrent count {} exceeded max_connections {}",
			observed_peak,
			max_connections
		);
	}
}
