//! Live MySQL coverage for write-intent transaction isolation.

#![cfg(feature = "mysql")]

use std::time::Duration;

use reinhardt_core::exception::DatabaseErrorKind;
use reinhardt_db::orm::connection::{BackendsConnection, QueryValue, TransactionExecutor};
use testcontainers::{
	GenericImage, ImageExt,
	core::{IntoContainerPort, WaitFor},
	runners::AsyncRunner,
};

#[tokio::test]
async fn mysql_write_intent_avoids_missing_unique_key_gap_lock_deadlock() {
	let image = GenericImage::new("mysql", "8.0")
		.with_exposed_port(3306.tcp())
		.with_wait_for(WaitFor::message_on_stderr(
			"port: 3306  MySQL Community Server",
		))
		.with_startup_timeout(Duration::from_secs(120))
		.with_env_var("MYSQL_ROOT_PASSWORD", "test")
		.with_env_var("MYSQL_DATABASE", "write_intent_test");
	let container = image.start().await.expect("MySQL container should start");
	let port = container
		.get_host_port_ipv4(3306)
		.await
		.expect("MySQL port should be exposed");
	let url = format!("mysql://root:test@127.0.0.1:{port}/write_intent_test");

	let owner = connect_with_retry(&url).await;
	owner
		.execute(
			"CREATE TABLE write_intent_rows (
				id BIGINT NOT NULL AUTO_INCREMENT PRIMARY KEY,
				slug VARCHAR(255) NOT NULL UNIQUE
			) ENGINE=InnoDB",
			vec![],
		)
		.await
		.expect("write-intent test table should be created");
	let mut first = owner
		.begin_write()
		.await
		.expect("first write-intent transaction should begin");
	let mut second = owner
		.begin_write()
		.await
		.expect("second write-intent transaction should begin");
	let lookup_sql = "SELECT id, slug FROM write_intent_rows WHERE slug = ? FOR UPDATE";
	first
		.fetch_all(lookup_sql, vec![QueryValue::String("shared".to_owned())])
		.await
		.expect("first missing-row lock lookup should succeed");
	second
		.fetch_all(lookup_sql, vec![QueryValue::String("shared".to_owned())])
		.await
		.expect("second missing-row lock lookup should succeed");

	let insert_sql = "INSERT INTO write_intent_rows (slug) VALUES (?)";
	let first_insert = tokio::spawn(insert_and_finish(first, insert_sql));
	let second_insert = tokio::spawn(insert_and_finish(second, insert_sql));
	let (first_insert, second_insert) = tokio::time::timeout(Duration::from_secs(10), async {
		tokio::join!(first_insert, second_insert)
	})
	.await
	.expect("write-intent inserts should not wait for an InnoDB lock timeout");
	let first_insert = first_insert.expect("first insert task should join");
	let second_insert = second_insert.expect("second insert task should join");
	let results = [&first_insert, &second_insert];
	assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
	let errors = results
		.into_iter()
		.filter_map(|result| result.as_ref().err())
		.collect::<Vec<_>>();
	assert_eq!(errors.len(), 1);
	assert_eq!(
		errors[0].database_kind(),
		Some(DatabaseErrorKind::UniqueViolation),
		"the losing write-intent insert should observe the committed winner"
	);
}

async fn insert_and_finish(
	mut transaction: Box<dyn TransactionExecutor>,
	sql: &'static str,
) -> reinhardt_core::exception::Result<()> {
	let result = transaction
		.execute(sql, vec![QueryValue::String("shared".to_owned())])
		.await;
	match result {
		Ok(_) => {
			transaction.commit().await?;
			Ok(())
		}
		Err(error) => {
			let _ = transaction.rollback().await;
			Err(error)
		}
	}
}

async fn connect_with_retry(url: &str) -> BackendsConnection {
	let mut delay = Duration::from_millis(200);
	for attempt in 0..8 {
		match BackendsConnection::connect_mysql(url).await {
			Ok(connection) => return connection,
			Err(error) if attempt < 7 => {
				eprintln!("MySQL connection attempt {} failed: {error}", attempt + 1);
				tokio::time::sleep(delay).await;
				delay *= 2;
			}
			Err(error) => panic!("MySQL connection failed after 8 attempts: {error}"),
		}
	}
	unreachable!("the final MySQL connection attempt returns or panics")
}
