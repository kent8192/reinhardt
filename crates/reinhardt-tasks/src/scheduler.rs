//! Task scheduling

use crate::TaskExecutor;
use chrono::{DateTime, Utc};
use cron::Schedule as CronParser;
use std::str::FromStr;
use std::sync::Arc;

/// Cron-like schedule for periodic tasks
///
/// # Example
///
/// ```rust
/// use reinhardt_tasks::CronSchedule;
///
/// let schedule = CronSchedule::new("0 0 * * *".to_string());
/// assert_eq!(schedule.expression, "0 0 * * *");
/// ```
#[derive(Debug, Clone)]
pub struct CronSchedule {
	pub expression: String,
}

impl CronSchedule {
	/// Create a new cron schedule
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_tasks::CronSchedule;
	///
	/// // Run every day at midnight
	/// let daily = CronSchedule::new("0 0 * * *".to_string());
	///
	/// // Run every hour
	/// let hourly = CronSchedule::new("0 * * * *".to_string());
	/// ```
	pub fn new(expression: String) -> Self {
		Self { expression }
	}

	/// Calculate next run time based on cron expression
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_tasks::CronSchedule;
	///
	/// let schedule = CronSchedule::new("0 0 * * *".to_string());
	/// // Returns the next midnight UTC (if parsing succeeds)
	/// let next = schedule.next_run();
	/// // Note: Result depends on CronParser implementation
	/// ```
	pub fn next_run(&self) -> Option<DateTime<Utc>> {
		// Parse cron expression
		let schedule = CronParser::from_str(&self.expression).ok()?;

		// Calculate next run time
		schedule.upcoming(Utc).next()
	}
}

pub trait Schedule: Send + Sync {
	fn next_run(&self) -> Option<DateTime<Utc>>;
}

impl Schedule for CronSchedule {
	fn next_run(&self) -> Option<DateTime<Utc>> {
		CronSchedule::next_run(self)
	}
}

/// Task scheduler for managing periodic tasks
///
/// # Example
///
/// ```rust
/// use reinhardt_tasks::Scheduler;
///
/// let scheduler = Scheduler::new();
/// // Add tasks and run scheduler
/// ```
// Fixes #786: added shutdown broadcast channel
pub struct Scheduler {
	tasks: Vec<(Arc<dyn TaskExecutor>, Box<dyn Schedule>)>,
	shutdown_tx: tokio::sync::broadcast::Sender<()>,
}

impl Scheduler {
	/// Create a new scheduler
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_tasks::Scheduler;
	///
	/// let scheduler = Scheduler::new();
	/// ```
	pub fn new() -> Self {
		let (shutdown_tx, _) = tokio::sync::broadcast::channel(1);
		Self {
			tasks: Vec::new(),
			shutdown_tx,
		}
	}

	/// Add a task with schedule
	///
	/// # Example
	///
	/// ```rust,no_run
	/// # use reinhardt_tasks::Scheduler;
	/// # struct CronSchedule { cron: String }
	/// # impl CronSchedule { fn new(s: String) -> Self { CronSchedule { cron: s } } }
	/// let mut scheduler = Scheduler::new();
	/// let schedule = CronSchedule::new("0 0 * * *".to_string());
	/// // scheduler.add_task(Box::new(my_task), Box::new(schedule));
	/// ```
	pub fn add_task(&mut self, task: Arc<dyn TaskExecutor>, schedule: Box<dyn Schedule>) {
		self.tasks.push((task, schedule));
	}

	/// Shut down the scheduler gracefully
	///
	/// Signals the scheduler to stop processing tasks. Already spawned
	/// tasks will continue to completion, but no new tasks will be started.
	///
	/// # Example
	///
	/// ```rust,no_run
	/// # use reinhardt_tasks::Scheduler;
	/// # #[tokio::main]
	/// # async fn main() {
	/// let scheduler = Scheduler::new();
	/// // ... add tasks ...
	/// scheduler.shutdown();
	/// # }
	/// ```
	// Fixes #786
	pub fn shutdown(&self) {
		let _ = self.shutdown_tx.send(());
	}

	/// Run the scheduler
	///
	/// This method continuously runs the scheduler, checking each task's schedule
	/// and executing tasks when their scheduled time arrives.
	///
	/// # Example
	///
	/// ```rust,no_run
	/// # use reinhardt_tasks::Scheduler;
	/// # #[tokio::main]
	/// # async fn main() {
	/// let mut scheduler = Scheduler::new();
	/// // Add tasks...
	/// scheduler.run().await;
	/// # }
	/// ```
	// Fixes #787: spawn each task execution as a separate tokio task
	// Fixes #786: check shutdown signal via tokio::select!
	pub async fn run(&self) {
		use tokio::time::{Duration, sleep};

		let mut shutdown_rx = self.shutdown_tx.subscribe();

		loop {
			let now = Utc::now();
			let mut next_check = None;

			// Check each task's schedule
			for (task, schedule) in &self.tasks {
				if let Some(next_run) = schedule.next_run() {
					// If it's time to run the task
					if next_run <= now {
						// Spawn each task execution concurrently instead of awaiting inline
						let task = Arc::clone(task);
						tokio::spawn(async move {
							if let Err(e) = task.execute().await {
								tracing::error!(error = %e, "Task execution failed");
							}
						});
					} else {
						// Track the earliest next run time
						match next_check {
							None => next_check = Some(next_run),
							Some(current) if next_run < current => next_check = Some(next_run),
							_ => {}
						}
					}
				}
			}

			// Sleep until the next scheduled task, or break on shutdown.
			// Enforce a minimum sleep of 100ms to prevent busy-looping when
			// next_run is in the past or very close to the current time.
			const MIN_SLEEP: Duration = Duration::from_millis(100);
			let sleep_duration = if let Some(next) = next_check {
				(next - now).to_std().unwrap_or(MIN_SLEEP).max(MIN_SLEEP)
			} else {
				// No tasks scheduled, check again in 60 seconds
				Duration::from_secs(60)
			};

			tokio::select! {
				_ = sleep(sleep_duration) => {}
				_ = shutdown_rx.recv() => {
					break;
				}
			}
		}
	}
}

impl Default for Scheduler {
	fn default() -> Self {
		Self::new()
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::{TaskId, TaskResult};
	use async_trait::async_trait;
	use rstest::rstest;
	use std::sync::atomic::{AtomicU64, Ordering};

	#[derive(Debug)]
	struct DummyTask {
		id: TaskId,
	}

	impl crate::Task for DummyTask {
		fn id(&self) -> TaskId {
			self.id
		}

		fn name(&self) -> &str {
			"dummy"
		}
	}

	#[async_trait]
	impl TaskExecutor for DummyTask {
		async fn execute(&self) -> TaskResult<()> {
			Ok(())
		}
	}

	/// A schedule that always returns a time in the past to trigger
	/// the busy-loop scenario. Used to verify the minimum sleep guard.
	struct PastSchedule;

	impl Schedule for PastSchedule {
		fn next_run(&self) -> Option<DateTime<Utc>> {
			// Return a time 1 hour in the past
			Some(Utc::now() - chrono::Duration::hours(1))
		}
	}

	/// A task that tracks how many times it has been executed.
	struct CountingTask {
		id: TaskId,
		count: Arc<AtomicU64>,
	}

	impl std::fmt::Debug for CountingTask {
		fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
			f.debug_struct("CountingTask")
				.field("id", &self.id)
				.finish()
		}
	}

	impl crate::Task for CountingTask {
		fn id(&self) -> TaskId {
			self.id
		}

		fn name(&self) -> &str {
			"counting"
		}
	}

	#[async_trait]
	impl TaskExecutor for CountingTask {
		async fn execute(&self) -> TaskResult<()> {
			self.count.fetch_add(1, Ordering::SeqCst);
			Ok(())
		}
	}

	#[rstest]
	#[tokio::test]
	async fn test_scheduler_shutdown() {
		// Arrange
		let scheduler = Arc::new(Scheduler::new());
		let scheduler_clone = Arc::clone(&scheduler);

		let handle = tokio::spawn(async move {
			scheduler_clone.run().await;
		});

		// Give the scheduler a moment to start its run loop
		tokio::time::sleep(std::time::Duration::from_millis(10)).await;

		// Act - signal shutdown via the public method
		scheduler.shutdown();

		// Assert - run() should exit without hanging
		tokio::time::timeout(std::time::Duration::from_secs(2), handle)
			.await
			.expect("scheduler should shut down within timeout")
			.expect("scheduler task should not panic");
	}

	#[rstest]
	#[tokio::test]
	async fn test_scheduler_does_not_busy_loop_when_next_run_is_in_the_past() {
		// Arrange - a schedule that always returns a past time would cause
		// a tight CPU-spinning loop without the minimum sleep guard.
		let count = Arc::new(AtomicU64::new(0));
		let task = Arc::new(CountingTask {
			id: TaskId::new(),
			count: Arc::clone(&count),
		});

		let mut scheduler = Scheduler::new();
		scheduler.add_task(task, Box::new(PastSchedule));
		let scheduler = Arc::new(scheduler);
		let scheduler_clone = Arc::clone(&scheduler);

		let handle = tokio::spawn(async move {
			scheduler_clone.run().await;
		});

		// Act - let the scheduler run for 500ms
		tokio::time::sleep(std::time::Duration::from_millis(500)).await;
		scheduler.shutdown();
		let _ = tokio::time::timeout(std::time::Duration::from_secs(2), handle).await;

		// Assert - with 100ms minimum sleep, at most ~5-6 executions in 500ms.
		// Without the guard, this would be thousands of executions.
		let execution_count = count.load(Ordering::SeqCst);
		assert!(
			execution_count <= 10,
			"Expected at most ~10 executions in 500ms with min sleep guard, got {}",
			execution_count
		);
	}

	// Regression test for #754: the scheduler MUST enforce MIN_SLEEP = 100ms to
	// prevent a busy-loop when next_run falls in the past (or very close to now).
	// Without MIN_SLEEP the run() loop would spin at CPU speed.
	#[rstest]
	#[tokio::test]
	async fn test_min_sleep_enforced_prevents_busy_loop_regression() {
		// Arrange - PastSchedule always returns a time 1 hour ago, simulating a
		// task that is perpetually "overdue". Without MIN_SLEEP the scheduler would
		// busy-loop and execute the task millions of times per second.
		let count = Arc::new(AtomicU64::new(0));
		let task = Arc::new(CountingTask {
			id: TaskId::new(),
			count: Arc::clone(&count),
		});

		let mut scheduler = Scheduler::new();
		scheduler.add_task(task, Box::new(PastSchedule));
		let scheduler = Arc::new(scheduler);
		let scheduler_clone = Arc::clone(&scheduler);

		let handle = tokio::spawn(async move {
			scheduler_clone.run().await;
		});

		// Act - observe execution count over exactly 300ms
		tokio::time::sleep(std::time::Duration::from_millis(300)).await;
		scheduler.shutdown();
		let _ = tokio::time::timeout(std::time::Duration::from_secs(2), handle).await;

		// Assert - MIN_SLEEP = 100ms means at most 3 loop iterations in 300ms,
		// yielding at most ~3-4 executions. A busy-loop without MIN_SLEEP would
		// produce many thousands of executions in the same window.
		let execution_count = count.load(Ordering::SeqCst);
		assert!(
			execution_count <= 6,
			"Regression #754: busy-loop guard must cap executions at ~3 in 300ms, got {}",
			execution_count
		);
	}
}
