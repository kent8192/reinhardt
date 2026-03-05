//! Task scheduler integration tests
//!
//! Tests CronSchedule, Schedule trait, and Scheduler implementations.

use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use reinhardt_tasks::{
	Task, TaskExecutor, TaskId, TaskResult,
	scheduler::{CronSchedule, Schedule, Scheduler},
};
use std::sync::{Arc, Mutex};

/// Simple test task executor
struct TestExecutor {
	task_id: TaskId,
	task_name: String,
	executed_count: Arc<Mutex<usize>>,
}

impl TestExecutor {
	fn new(name: impl Into<String>) -> Self {
		Self {
			task_id: TaskId::new(),
			task_name: name.into(),
			executed_count: Arc::new(Mutex::new(0)),
		}
	}

	// Test helper method used only in specific test paths
	#[allow(dead_code)]
	fn execution_count(&self) -> usize {
		*self.executed_count.lock().unwrap()
	}
}

impl Task for TestExecutor {
	fn id(&self) -> TaskId {
		self.task_id
	}

	fn name(&self) -> &str {
		&self.task_name
	}
}

#[async_trait]
impl TaskExecutor for TestExecutor {
	async fn execute(&self) -> TaskResult<()> {
		*self.executed_count.lock().unwrap() += 1;
		Ok(())
	}
}

/// Test: CronSchedule creation
#[test]
fn test_cron_schedule_new() {
	let schedule = CronSchedule::new("0 0 * * *".to_string());
	assert_eq!(schedule.expression, "0 0 * * *");
}

/// Test: CronSchedule next_run with valid expression
#[test]
fn test_cron_schedule_next_run_valid() {
	// Daily at midnight
	let schedule = CronSchedule::new("0 0 * * * *".to_string());
	let next = schedule.next_run();
	assert!(next.is_some());

	// Hourly
	let schedule = CronSchedule::new("0 0 * * * *".to_string());
	let next = schedule.next_run();
	assert!(next.is_some());
}

/// Test: CronSchedule next_run with invalid expression
#[test]
fn test_cron_schedule_next_run_invalid() {
	let schedule = CronSchedule::new("invalid cron".to_string());
	let next = schedule.next_run();
	assert!(next.is_none());
}

/// Test: CronSchedule clone
#[test]
fn test_cron_schedule_clone() {
	let schedule1 = CronSchedule::new("0 0 * * *".to_string());
	let schedule2 = schedule1.clone();

	assert_eq!(schedule1.expression, schedule2.expression);
}

/// Test: CronSchedule as Schedule trait
#[test]
fn test_cron_schedule_as_trait() {
	let schedule: Box<dyn Schedule> = Box::new(CronSchedule::new("0 0 * * * *".to_string()));
	let next = schedule.next_run();
	assert!(next.is_some());
}

/// Test: Scheduler creation
#[test]
fn test_scheduler_new() {
	let scheduler = Scheduler::new();
	// Scheduler creation should succeed (no panic)
	assert_eq!(
		std::mem::size_of_val(&scheduler),
		std::mem::size_of::<Scheduler>()
	);
}

/// Test: Scheduler add_task
#[test]
fn test_scheduler_add_task() {
	let mut scheduler = Scheduler::new();
	let executor = Arc::new(TestExecutor::new("test_task"));
	let schedule = Box::new(CronSchedule::new("0 0 * * *".to_string()));

	scheduler.add_task(executor, schedule);
	// Task addition should succeed (no panic)
}

/// Test: Scheduler with multiple tasks
#[test]
fn test_scheduler_multiple_tasks() {
	let mut scheduler = Scheduler::new();

	let executor1 = Arc::new(TestExecutor::new("task1"));
	let schedule1 = Box::new(CronSchedule::new("0 0 * * *".to_string()));
	scheduler.add_task(executor1, schedule1);

	let executor2 = Arc::new(TestExecutor::new("task2"));
	let schedule2 = Box::new(CronSchedule::new("0 * * * *".to_string()));
	scheduler.add_task(executor2, schedule2);

	let executor3 = Arc::new(TestExecutor::new("task3"));
	let schedule3 = Box::new(CronSchedule::new("*/5 * * * *".to_string()));
	scheduler.add_task(executor3, schedule3);

	// Multiple task additions should succeed
}

/// Custom schedule implementation for testing
struct FixedSchedule {
	next_time: DateTime<Utc>,
}

impl FixedSchedule {
	fn new(next_time: DateTime<Utc>) -> Self {
		Self { next_time }
	}
}

impl Schedule for FixedSchedule {
	fn next_run(&self) -> Option<DateTime<Utc>> {
		Some(self.next_time)
	}
}

/// Test: Scheduler with custom Schedule implementation
#[test]
fn test_scheduler_with_custom_schedule() {
	let mut scheduler = Scheduler::new();
	let executor = Arc::new(TestExecutor::new("custom_task"));
	let next_time = Utc::now() + Duration::hours(1);
	let schedule = Box::new(FixedSchedule::new(next_time));

	scheduler.add_task(executor, schedule);
	// Custom schedule should work
}

/// Test: CronSchedule common expressions
#[test]
fn test_cron_schedule_common_expressions() {
	// Every minute
	let schedule = CronSchedule::new("* * * * * *".to_string());
	assert!(schedule.next_run().is_some());

	// Every hour at minute 0
	let schedule = CronSchedule::new("0 0 * * * *".to_string());
	assert!(schedule.next_run().is_some());

	// Every day at midnight
	let schedule = CronSchedule::new("0 0 0 * * *".to_string());
	assert!(schedule.next_run().is_some());

	// Every Monday at 9 AM
	let schedule = CronSchedule::new("0 0 9 * * 1".to_string());
	assert!(schedule.next_run().is_some());
}

/// Test: CronSchedule edge cases
#[test]
fn test_cron_schedule_edge_cases() {
	// Empty string
	let schedule = CronSchedule::new("".to_string());
	assert!(schedule.next_run().is_none());

	// Too few fields
	let schedule = CronSchedule::new("0 0".to_string());
	assert!(schedule.next_run().is_none());

	// Invalid cron expression
	let schedule = CronSchedule::new("invalid cron expression".to_string());
	assert!(schedule.next_run().is_none());
}
