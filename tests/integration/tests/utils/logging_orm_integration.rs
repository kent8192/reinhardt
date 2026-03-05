//! ORM Transaction Logging Integration Tests
//!
//! Integration tests for reinhardt-logging's ORM transaction logging
//! working with reinhardt-orm. These tests verify that database transaction
//! lifecycle events (begin, commit, rollback, savepoints) are properly logged.
//!
//! Tests for logging transaction lifecycle events (begin, commit, rollback, savepoints).
//! Based on SQLAlchemy's transaction logging tests.

use reinhardt_db::orm::{IsolationLevel, Savepoint, Transaction};
use reinhardt_utils::logging::handlers::MemoryHandler;
use reinhardt_utils::logging::{LogLevel, Logger};
use std::sync::Arc;

/// Transaction wrapper that logs all operations
pub(crate) struct LoggingTransaction {
	inner: Transaction,
	logger: Arc<Logger>,
}

impl LoggingTransaction {
	pub(crate) fn new(logger: Arc<Logger>) -> Self {
		Self {
			inner: Transaction::new(),
			logger,
		}
	}

	pub(crate) fn with_isolation_level(mut self, level: IsolationLevel) -> Self {
		self.inner = self.inner.with_isolation_level(level);
		self
	}

	pub(crate) async fn begin(&mut self) -> Result<String, String> {
		let result = self.inner.begin();

		if let Ok(ref sql) = result {
			self.logger
				.debug(format!("Transaction BEGIN: {}", sql))
				.await;
		}

		result
	}

	pub(crate) async fn commit(&mut self) -> Result<String, String> {
		let result = self.inner.commit();

		if let Ok(ref sql) = result {
			self.logger
				.debug(format!("Transaction COMMIT: {}", sql))
				.await;
		}

		result
	}

	pub(crate) async fn rollback(&mut self) -> Result<String, String> {
		let result = self.inner.rollback();

		if let Ok(ref sql) = result {
			self.logger
				.info(format!("Transaction ROLLBACK: {}", sql))
				.await;
		}

		result
	}
}

#[tokio::test]
async fn test_transaction_begin_logged() {
	// Transaction start should be logged at DEBUG level
	let logger = Arc::new(Logger::new("sqlalchemy.engine".to_string()));
	let handler = MemoryHandler::new(LogLevel::Debug);
	let memory = handler.clone();

	logger.add_handler(Arc::new(handler)).await;
	logger.set_level(LogLevel::Debug).await;

	let mut txn = LoggingTransaction::new(logger.clone());
	let result = txn.begin().await;

	assert!(result.is_ok());
	tokio::time::sleep(tokio::time::Duration::from_millis(20)).await;

	let records = memory.get_records();
	assert_eq!(records.len(), 1);
	assert_eq!(records[0].level, LogLevel::Debug);
	assert!(records[0].message.contains("Transaction BEGIN"));
	assert!(records[0].message.contains("BEGIN TRANSACTION"));
}

#[tokio::test]
async fn test_transaction_commit_logged() {
	// Transaction commit should be logged at DEBUG level
	let logger = Arc::new(Logger::new("sqlalchemy.engine".to_string()));
	let handler = MemoryHandler::new(LogLevel::Debug);
	let memory = handler.clone();

	logger.add_handler(Arc::new(handler)).await;
	logger.set_level(LogLevel::Debug).await;

	let mut txn = LoggingTransaction::new(logger.clone());
	txn.begin().await.unwrap();
	let result = txn.commit().await;

	assert!(result.is_ok());
	tokio::time::sleep(tokio::time::Duration::from_millis(20)).await;

	let records = memory.get_records();
	assert_eq!(records.len(), 2); // BEGIN + COMMIT
	assert_eq!(records[1].level, LogLevel::Debug);
	assert!(records[1].message.contains("Transaction COMMIT"));
	assert!(records[1].message.contains("COMMIT"));
}

#[tokio::test]
async fn test_transaction_rollback_logged() {
	// Transaction rollback should be logged at INFO level (more important than commit)
	let logger = Arc::new(Logger::new("sqlalchemy.engine".to_string()));
	let handler = MemoryHandler::new(LogLevel::Debug);
	let memory = handler.clone();

	logger.add_handler(Arc::new(handler)).await;
	logger.set_level(LogLevel::Debug).await;

	let mut txn = LoggingTransaction::new(logger.clone());
	txn.begin().await.unwrap();
	let result = txn.rollback().await;

	assert!(result.is_ok());
	tokio::time::sleep(tokio::time::Duration::from_millis(20)).await;

	let records = memory.get_records();
	assert_eq!(records.len(), 2); // BEGIN + ROLLBACK
	assert_eq!(records[0].level, LogLevel::Debug); // BEGIN
	assert_eq!(records[1].level, LogLevel::Info); // ROLLBACK
	assert!(records[1].message.contains("Transaction ROLLBACK"));
	assert!(records[1].message.contains("ROLLBACK"));
}

#[tokio::test]
async fn test_savepoint_created_logged() {
	// Savepoint creation (nested transaction) should be logged
	let logger = Arc::new(Logger::new("sqlalchemy.engine".to_string()));
	let handler = MemoryHandler::new(LogLevel::Debug);
	let memory = handler.clone();

	logger.add_handler(Arc::new(handler)).await;
	logger.set_level(LogLevel::Debug).await;

	let mut txn = LoggingTransaction::new(logger.clone());

	// Start first transaction
	txn.begin().await.unwrap();

	// Start nested transaction (creates savepoint)
	let result = txn.begin().await;
	assert!(result.is_ok());

	tokio::time::sleep(tokio::time::Duration::from_millis(20)).await;

	let records = memory.get_records();
	assert_eq!(records.len(), 2); // BEGIN + SAVEPOINT
	assert!(records[1].message.contains("SAVEPOINT"));
}

#[tokio::test]
async fn test_savepoint_rollback_logged() {
	// Savepoint rollback should be logged
	let logger = Arc::new(Logger::new("sqlalchemy.engine".to_string()));
	let handler = MemoryHandler::new(LogLevel::Debug);
	let memory = handler.clone();

	logger.add_handler(Arc::new(handler)).await;
	logger.set_level(LogLevel::Debug).await;

	let mut txn = LoggingTransaction::new(logger.clone());

	txn.begin().await.unwrap(); // BEGIN TRANSACTION
	txn.begin().await.unwrap(); // SAVEPOINT sp_2
	txn.rollback().await.unwrap(); // ROLLBACK TO SAVEPOINT sp_2

	tokio::time::sleep(tokio::time::Duration::from_millis(20)).await;

	let records = memory.get_records();
	assert_eq!(records.len(), 3);
	assert!(records[2].message.contains("ROLLBACK TO SAVEPOINT"));
}

#[tokio::test]
async fn test_nested_transaction_logging() {
	// Multiple levels of nested transactions should all be logged
	let logger = Arc::new(Logger::new("sqlalchemy.engine".to_string()));
	let handler = MemoryHandler::new(LogLevel::Debug);
	let memory = handler.clone();

	logger.add_handler(Arc::new(handler)).await;
	logger.set_level(LogLevel::Debug).await;

	let mut txn = LoggingTransaction::new(logger.clone());

	txn.begin().await.unwrap(); // Level 1
	txn.begin().await.unwrap(); // Level 2 (savepoint)
	txn.begin().await.unwrap(); // Level 3 (savepoint)

	tokio::time::sleep(tokio::time::Duration::from_millis(20)).await;

	let records = memory.get_records();
	assert_eq!(records.len(), 3);
	assert!(records[0].message.contains("BEGIN TRANSACTION"));
	assert!(records[1].message.contains("SAVEPOINT \"sp_2\""));
	assert!(records[2].message.contains("SAVEPOINT \"sp_3\""));
}

#[tokio::test]
async fn test_isolation_level_logged() {
	// Transaction with isolation level should include it in log
	let logger = Arc::new(Logger::new("sqlalchemy.engine".to_string()));
	let handler = MemoryHandler::new(LogLevel::Debug);
	let memory = handler.clone();

	logger.add_handler(Arc::new(handler)).await;
	logger.set_level(LogLevel::Debug).await;

	let mut txn =
		LoggingTransaction::new(logger.clone()).with_isolation_level(IsolationLevel::Serializable);

	txn.begin().await.unwrap();

	tokio::time::sleep(tokio::time::Duration::from_millis(20)).await;

	let records = memory.get_records();
	assert_eq!(records.len(), 1);
	assert!(records[0].message.contains("ISOLATION LEVEL"));
	assert!(records[0].message.contains("SERIALIZABLE"));
}

#[tokio::test]
async fn test_transaction_logger_level_filtering() {
	// Logger level should filter transaction logs appropriately
	let logger = Arc::new(Logger::new("sqlalchemy.engine".to_string()));
	let handler = MemoryHandler::new(LogLevel::Info);
	let memory = handler.clone();

	logger.add_handler(Arc::new(handler)).await;
	logger.set_level(LogLevel::Info).await; // Set to INFO, so DEBUG messages won't appear

	let mut txn = LoggingTransaction::new(logger.clone());

	txn.begin().await.unwrap(); // DEBUG - should not log
	txn.commit().await.unwrap(); // DEBUG - should not log

	tokio::time::sleep(tokio::time::Duration::from_millis(20)).await;

	let records = memory.get_records();
	assert_eq!(records.len(), 0); // No DEBUG logs should appear

	// Now test rollback which logs at INFO
	let mut txn2 = LoggingTransaction::new(logger.clone());
	txn2.begin().await.unwrap();
	txn2.rollback().await.unwrap(); // INFO - should log

	tokio::time::sleep(tokio::time::Duration::from_millis(20)).await;

	let records = memory.get_records();
	assert_eq!(records.len(), 1); // Only the INFO rollback should appear
	assert_eq!(records[0].level, LogLevel::Info);
}

#[tokio::test]
async fn test_full_transaction_lifecycle() {
	// Test complete transaction lifecycle with all operations
	let logger = Arc::new(Logger::new("sqlalchemy.engine".to_string()));
	let handler = MemoryHandler::new(LogLevel::Debug);
	let memory = handler.clone();

	logger.add_handler(Arc::new(handler)).await;
	logger.set_level(LogLevel::Debug).await;

	let mut txn = LoggingTransaction::new(logger.clone());

	txn.begin().await.unwrap(); // BEGIN
	txn.begin().await.unwrap(); // SAVEPOINT sp_2
	txn.commit().await.unwrap(); // RELEASE SAVEPOINT sp_2
	txn.commit().await.unwrap(); // COMMIT

	tokio::time::sleep(tokio::time::Duration::from_millis(20)).await;

	let records = memory.get_records();
	assert_eq!(records.len(), 4);
	assert!(records[0].message.contains("BEGIN TRANSACTION"));
	assert!(records[1].message.contains("SAVEPOINT \"sp_2\""));
	assert!(records[2].message.contains("RELEASE SAVEPOINT \"sp_2\""));
	assert!(records[3].message.contains("COMMIT"));
}

#[tokio::test]
async fn test_savepoint_object() {
	// Test Savepoint utility functions
	let savepoint = Savepoint::new("test_sp", 1);

	assert_eq!(savepoint.name(), "test_sp");
	assert_eq!(savepoint.depth, 1);
	assert_eq!(savepoint.to_sql(), "SAVEPOINT \"test_sp\"");
	assert_eq!(savepoint.release_sql(), "RELEASE SAVEPOINT \"test_sp\"");
	assert_eq!(
		savepoint.rollback_sql(),
		"ROLLBACK TO SAVEPOINT \"test_sp\""
	);
}

#[tokio::test]
async fn test_multiple_transactions_separate_loggers() {
	// Multiple transactions with different loggers should log independently
	let logger1 = Arc::new(Logger::new("sqlalchemy.engine.connection1".to_string()));
	let handler1 = MemoryHandler::new(LogLevel::Debug);
	let memory1 = handler1.clone();
	logger1.add_handler(Arc::new(handler1)).await;
	logger1.set_level(LogLevel::Debug).await;

	let logger2 = Arc::new(Logger::new("sqlalchemy.engine.connection2".to_string()));
	let handler2 = MemoryHandler::new(LogLevel::Debug);
	let memory2 = handler2.clone();
	logger2.add_handler(Arc::new(handler2)).await;
	logger2.set_level(LogLevel::Debug).await;

	let mut txn1 = LoggingTransaction::new(logger1.clone());
	let mut txn2 = LoggingTransaction::new(logger2.clone());

	txn1.begin().await.unwrap();
	txn2.begin().await.unwrap();

	tokio::time::sleep(tokio::time::Duration::from_millis(20)).await;

	// Each logger should have logged its own transaction
	let records1 = memory1.get_records();
	let records2 = memory2.get_records();

	assert_eq!(records1.len(), 1);
	assert_eq!(records2.len(), 1);
	assert!(records1[0].logger_name.contains("connection1"));
	assert!(records2[0].logger_name.contains("connection2"));
}
