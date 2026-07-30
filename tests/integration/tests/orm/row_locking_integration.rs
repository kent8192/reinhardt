//! Real-database contracts for transaction-scoped `QuerySet` row locking.
//!
//! PostgreSQL and MySQL exercise their native `FOR UPDATE`, `NOWAIT`, and
//! `SKIP LOCKED` forms. CockroachDB exercises the PostgreSQL-compatible forms
//! without explicit `OF` targets, which its checked capability contract rejects.
//! Every locking read goes through `QuerySet::select_for_update` and a
//! caller-owned `AtomicTransaction`.

use reinhardt_core::exception::{DatabaseError, DatabaseErrorKind, Error};
use reinhardt_db::{
	backends::DatabaseConnection as BackendsConnection,
	orm::{DatabaseConnectionLease, QuerySet},
};
use reinhardt_macros::model;
use reinhardt_test::fixtures::{
	postgres_container,
	testcontainers::{cockroachdb_container, mysql_container},
};
use serde::{Deserialize, Serialize};
use serial_test::serial;
use std::time::Duration;
use tokio::{
	sync::oneshot,
	task::JoinHandle,
	time::{sleep, timeout},
};

const LOCK_WAIT_OBSERVATION: Duration = Duration::from_millis(250);
const NOWAIT_DEADLINE: Duration = Duration::from_secs(2);

#[model(app_label = "row_locking", table_name = "row_lock_items")]
#[derive(Clone, Debug, Serialize, Deserialize)]
struct RowLockItem {
	#[field(primary_key = true)]
	id: i64,

	value: i64,
}

struct HeldRowLock {
	release: oneshot::Sender<()>,
	task: JoinHandle<Result<(), Error>>,
}

async fn connect(
	url: &str,
) -> (
	DatabaseConnectionLease,
	reinhardt_db::orm::DatabaseConnection,
) {
	let owner = BackendsConnection::connect(url)
		.await
		.expect("the framework backend must connect to the row-lock fixture");
	let lease = DatabaseConnectionLease::register(owner)
		.expect("the fixture connection must register with the ORM");
	let connection = lease.handle();
	(lease, connection)
}

async fn reset_rows(connection: reinhardt_db::orm::DatabaseConnection) {
	connection
		.execute("DELETE FROM row_lock_items", vec![])
		.await
		.expect("the previous row-lock scenario must be cleared");
	connection
		.execute(
			"INSERT INTO row_lock_items (id, value) VALUES (1, 10), (2, 20)",
			vec![],
		)
		.await
		.expect("the row-lock fixture rows must be inserted");
}

async fn hold_first_row(
	connection: reinhardt_db::orm::DatabaseConnection,
	rollback: bool,
) -> HeldRowLock {
	let (acquired_tx, acquired_rx) = oneshot::channel();
	let (release_tx, release_rx) = oneshot::channel();
	let task = tokio::spawn(async move {
		connection
			.atomic(async move |transaction| {
				let rows = QuerySet::<RowLockItem>::new()
					.filter(RowLockItem::field_id().eq(1_i64))
					.select_for_update()
					.all_with_executor(transaction)
					.await
					.map_err(Error::from)?;
				assert_eq!(rows.len(), 1);
				acquired_tx
					.send(())
					.expect("the lock holder must announce acquisition once");
				release_rx
					.await
					.expect("the lock holder must receive its release signal");
				if rollback {
					return Err(Error::from(DatabaseError::new(
						DatabaseErrorKind::Transaction,
						"intentional rollback after holding the row lock",
					)));
				}
				Ok(())
			})
			.await
	});
	acquired_rx
		.await
		.expect("the lock holder task must acquire the first row");
	HeldRowLock {
		release: release_tx,
		task,
	}
}

async fn assert_blocking_until_transaction_end(
	connection: reinhardt_db::orm::DatabaseConnection,
	rollback: bool,
) {
	reset_rows(connection).await;
	let holder = hold_first_row(connection, rollback).await;
	let (attempting_tx, attempting_rx) = oneshot::channel();
	let mut waiter = tokio::spawn(async move {
		connection
			.atomic(async move |transaction| {
				attempting_tx
					.send(())
					.expect("the waiter must announce its locking attempt once");
				QuerySet::<RowLockItem>::new()
					.filter(RowLockItem::field_id().eq(1_i64))
					.select_for_update()
					.all_with_executor(transaction)
					.await
					.map_err(Error::from)
			})
			.await
	});

	attempting_rx
		.await
		.expect("the waiting task must reach its locking query");
	sleep(LOCK_WAIT_OBSERVATION).await;
	assert!(
		!waiter.is_finished(),
		"a second blocking lock must wait for the first transaction to finish"
	);

	holder
		.release
		.send(())
		.expect("the holder must still be waiting for transaction completion");
	let holder_result = holder.task.await.expect("the holder task must not panic");
	if rollback {
		assert!(
			holder_result.is_err(),
			"the rollback scenario must leave the atomic scope through an error"
		);
	} else {
		holder_result.expect("the commit scenario must commit the atomic scope");
	}
	let rows = timeout(NOWAIT_DEADLINE, &mut waiter)
		.await
		.expect("the waiting lock must proceed after commit or rollback")
		.expect("the waiter task must not panic")
		.expect("the waiting transaction must acquire the released row");
	assert_eq!(rows.len(), 1);
}

async fn assert_nowait_fails_immediately(
	connection: reinhardt_db::orm::DatabaseConnection,
	expected_sqlstate: &str,
) {
	reset_rows(connection).await;
	let holder = hold_first_row(connection, false).await;

	let result = timeout(NOWAIT_DEADLINE, async {
		connection
			.atomic(async |transaction| {
				QuerySet::<RowLockItem>::new()
					.filter(RowLockItem::field_id().eq(1_i64))
					.select_for_update()
					.nowait()
					.all_with_executor(transaction)
					.await
					.map_err(Error::from)
			})
			.await
	})
	.await;

	holder
		.release
		.send(())
		.expect("the holder must remain active through the NOWAIT attempt");
	holder
		.task
		.await
		.expect("the holder task must not panic")
		.expect("the holder transaction must commit");
	let nowait_result = result.expect("NOWAIT must return before the deadline");
	let error = nowait_result.expect_err("NOWAIT must report lock contention instead of waiting");
	let database_error = error
		.database_error()
		.expect("NOWAIT contention must be reported as a database error");
	assert_eq!(
		database_error.code(),
		Some(expected_sqlstate),
		"NOWAIT must report the backend lock-contention SQLSTATE"
	);
}

async fn assert_skip_locked_omits_locked_row(connection: reinhardt_db::orm::DatabaseConnection) {
	reset_rows(connection).await;
	let holder = hold_first_row(connection, false).await;

	let rows = connection
		.atomic(async |transaction| {
			QuerySet::<RowLockItem>::new()
				.select_for_update()
				.skip_locked()
				.all_with_executor(transaction)
				.await
				.map_err(Error::from)
		})
		.await
		.expect("SKIP LOCKED must return the rows that remain available");

	holder
		.release
		.send(())
		.expect("the holder must remain active through the SKIP LOCKED query");
	holder
		.task
		.await
		.expect("the holder task must not panic")
		.expect("the holder transaction must commit");
	assert_eq!(
		rows.iter().map(|row| row.id).collect::<Vec<_>>(),
		vec![2],
		"SKIP LOCKED must omit the row held by the concurrent transaction"
	);
}

async fn run_row_lock_contract(url: &str, expected_nowait_sqlstate: &str) {
	let (_lease, connection) = connect(url).await;
	connection
		.execute(
			"CREATE TABLE row_lock_items (id BIGINT PRIMARY KEY, value BIGINT NOT NULL)",
			vec![],
		)
		.await
		.expect("the row-lock fixture table must be created");

	assert_blocking_until_transaction_end(connection, false).await;
	assert_blocking_until_transaction_end(connection, true).await;
	assert_nowait_fails_immediately(connection, expected_nowait_sqlstate).await;
	assert_skip_locked_omits_locked_row(connection).await;
}

#[tokio::test]
#[serial(row_locking_integration)]
async fn postgres_row_locks_follow_transaction_boundaries_and_wait_policies() {
	let (_container, _pool, _port, url) = postgres_container().await;
	run_row_lock_contract(&url, "55P03").await;
}

#[tokio::test]
#[serial(row_locking_integration)]
async fn mysql_row_locks_follow_transaction_boundaries_and_wait_policies() {
	let (_container, _pool, _port, url) = mysql_container().await;
	run_row_lock_contract(&url, "3572").await;
}

#[tokio::test]
#[serial(row_locking_integration)]
async fn cockroachdb_row_locks_follow_transaction_boundaries_and_wait_policies() {
	let (_container, _pool, _port, url) = cockroachdb_container().await;
	run_row_lock_contract(&url, "55P03").await;
}
