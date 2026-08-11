#![cfg(all(feature = "postgres", feature = "mysql", feature = "sqlite"))]
#![allow(unexpected_cfgs)]

//! Live backend coverage for the public typed terminal aggregate API.
//!
//! Each fixture owns every resource used by its connection.  Keeping the lease,
//! container, and SQLite temporary directory in the fixture makes cleanup
//! deterministic through `Drop` even when an assertion fails.

use std::time::Duration;

use reinhardt_core::macros::model;
use reinhardt_db::backends::DatabaseConnection as BackendsConnection;
use reinhardt_db::orm::connection::{DatabaseConnection, DatabaseConnectionLease};
use reinhardt_db::orm::func;
use reinhardt_db::orm::query::{Filter, FilterOperator, FilterValue, QuerySet};
use reinhardt_db::orm::query_fields::{AnnotationExpressionKind, LabeledExpression};
use reinhardt_db::orm::{AggregateDateTime, AggregateValue, TypedExpression};
use serde::{Deserialize, Serialize};
use tempfile::TempDir;
use testcontainers::{
	ContainerAsync, GenericImage, ImageExt,
	core::{IntoContainerPort, WaitFor},
	runners::AsyncRunner,
};

#[path = "ui/typed_aggregation/support.rs"]
mod aggregate_support;

use aggregate_support::{ModelRecord, RelatedRecord};

const MAX_CONNECT_RETRIES: u32 = 7;

#[model(
	app_label = "typed_aggregation_backends",
	table_name = "aggregate_records"
)]
#[derive(Clone, Debug, Serialize, Deserialize)]
struct AggregateRecord {
	#[field(primary_key = true)]
	id: i64,
	#[field(db_column = "display_name", max_length = 255)]
	name: String,
	integer_value: Option<i64>,
	float_value: Option<f64>,
	decimal_value: Option<rust_decimal::Decimal>,
	external_uuid: uuid::Uuid,
	event_date: chrono::NaiveDate,
	event_time: chrono::NaiveTime,
	event_at: chrono::DateTime<chrono::Utc>,
}

enum BackendFixture {
	Postgres {
		connection: DatabaseConnection,
		_lease: DatabaseConnectionLease,
		_container: ContainerAsync<GenericImage>,
	},
	MySql {
		connection: DatabaseConnection,
		_lease: DatabaseConnectionLease,
		_container: ContainerAsync<GenericImage>,
	},
	Sqlite {
		connection: DatabaseConnection,
		_lease: DatabaseConnectionLease,
		_directory: TempDir,
	},
}

impl BackendFixture {
	fn connection(&self) -> DatabaseConnection {
		match self {
			Self::Postgres { connection, .. }
			| Self::MySql { connection, .. }
			| Self::Sqlite { connection, .. } => *connection,
		}
	}

	fn name(&self) -> &'static str {
		match self {
			Self::Postgres { .. } => "postgres",
			Self::MySql { .. } => "mysql",
			Self::Sqlite { .. } => "sqlite",
		}
	}
}

async fn connect_with_retry(url: &str) -> BackendsConnection {
	for attempt in 0..=MAX_CONNECT_RETRIES {
		match BackendsConnection::connect(url).await {
			Ok(connection) => return connection,
			Err(error) if attempt < MAX_CONNECT_RETRIES => {
				eprintln!(
					"aggregate backend connection attempt {} of {} failed: {error}",
					attempt + 1,
					MAX_CONNECT_RETRIES + 1,
				);
				tokio::time::sleep(Duration::from_millis(200 * 2_u64.pow(attempt + 1))).await;
			}
			Err(error) => panic!(
				"aggregate backend connection failed after {} attempts: {error}",
				MAX_CONNECT_RETRIES + 1,
			),
		}
	}
	unreachable!("the final connection attempt either returns or panics")
}

async fn postgres_fixture() -> BackendFixture {
	let container = GenericImage::new("postgres", "16-alpine")
		.with_exposed_port(5432.tcp())
		.with_wait_for(WaitFor::message_on_stderr(
			"database system is ready to accept connections",
		))
		.with_startup_timeout(Duration::from_secs(120))
		.with_env_var("POSTGRES_HOST_AUTH_METHOD", "trust")
		.start()
		.await
		.expect("PostgreSQL 16 container should start");
	let port = container
		.get_host_port_ipv4(5432)
		.await
		.expect("PostgreSQL port should be exposed");
	let owner = connect_with_retry(&format!(
		"postgres://postgres@127.0.0.1:{port}/postgres?sslmode=disable"
	))
	.await;
	let lease = DatabaseConnectionLease::register(owner).expect("PostgreSQL lease should register");
	BackendFixture::Postgres {
		connection: lease.handle(),
		_lease: lease,
		_container: container,
	}
}

async fn mysql_fixture() -> BackendFixture {
	let container = GenericImage::new("mysql", "8.0")
		.with_exposed_port(3306.tcp())
		.with_wait_for(WaitFor::message_on_stderr(
			"port: 3306  MySQL Community Server",
		))
		.with_startup_timeout(Duration::from_secs(120))
		.with_env_var("MYSQL_ROOT_PASSWORD", "test")
		.with_env_var("MYSQL_DATABASE", "typed_aggregation_backends")
		.start()
		.await
		.expect("MySQL 8 container should start");
	let port = container
		.get_host_port_ipv4(3306)
		.await
		.expect("MySQL port should be exposed");
	let owner = connect_with_retry(&format!(
		"mysql://root:test@127.0.0.1:{port}/typed_aggregation_backends"
	))
	.await;
	let lease = DatabaseConnectionLease::register(owner).expect("MySQL lease should register");
	BackendFixture::MySql {
		connection: lease.handle(),
		_lease: lease,
		_container: container,
	}
}

async fn sqlite_fixture() -> BackendFixture {
	let directory = tempfile::Builder::new()
		.prefix("reinhardt-typed-aggregation-")
		.tempdir_in("/tmp")
		.expect("SQLite temporary directory should be created under /tmp");
	let database_path = directory.path().join("aggregate.sqlite");
	let owner =
		BackendsConnection::connect_sqlite(&format!("sqlite:///{}", database_path.display()))
			.await
			.expect("SQLite connection should open");
	let lease = DatabaseConnectionLease::register(owner).expect("SQLite lease should register");
	BackendFixture::Sqlite {
		connection: lease.handle(),
		_lease: lease,
		_directory: directory,
	}
}

async fn create_schema(connection: &DatabaseConnection, backend: &str) {
	let float_type = if backend == "postgres" {
		"DOUBLE PRECISION"
	} else {
		"DOUBLE"
	};
	let uuid_type = if backend == "postgres" {
		"UUID"
	} else {
		"VARCHAR(36)"
	};
	let datetime_type = if backend == "postgres" {
		"TIMESTAMP WITH TIME ZONE"
	} else {
		"TIMESTAMP"
	};
	connection
		.execute(
			&format!(
				"CREATE TABLE aggregate_records (id BIGINT PRIMARY KEY, display_name VARCHAR(255) NOT NULL, integer_value BIGINT NULL, float_value {float_type} NULL, decimal_value DECIMAL(12, 2) NULL, external_uuid {uuid_type} NOT NULL, event_date DATE NOT NULL, event_time TIME NOT NULL, event_at {datetime_type} NOT NULL)"
			),
			Vec::new(),
		)
		.await
		.expect("aggregate_records schema should be created");
	connection
		.execute(
			"CREATE TABLE related_records (id BIGINT PRIMARY KEY, value_i64 BIGINT NOT NULL)",
			Vec::new(),
		)
		.await
		.expect("related_records schema should be created");
	connection
		.execute(
			"CREATE TABLE model_records (id BIGINT PRIMARY KEY, related_id BIGINT NULL)",
			Vec::new(),
		)
		.await
		.expect("model_records schema should be created");
}

async fn seed_data(connection: &DatabaseConnection, backend: &str) {
	let uuid_one = "00000000-0000-0000-0000-000000000001";
	let uuid_two = "00000000-0000-0000-0000-000000000002";
	let uuid_type_one = if backend == "postgres" {
		format!("'{uuid_one}'")
	} else {
		format!("'{uuid_one}'")
	};
	let uuid_type_two = if backend == "postgres" {
		format!("'{uuid_two}'")
	} else {
		format!("'{uuid_two}'")
	};
	let rows = [
		format!(
			"(1, 'alpha', 10, 1.5, 10.25, {uuid_type_one}, '2024-01-02', '03:04:05', '2024-01-02 03:04:05+00:00')"
		),
		format!(
			"(2, 'beta', NULL, 2.5, 20.50, {uuid_type_two}, '2024-01-03', '04:05:06', '2024-01-03 04:05:06+00:00')"
		),
		format!(
			"(3, 'gamma', 30, NULL, NULL, {uuid_type_two}, '2024-01-04', '05:06:07', '2024-01-04 05:06:07+00:00')"
		),
	];
	connection
		.execute(
			&format!(
				"INSERT INTO aggregate_records (id, display_name, integer_value, float_value, decimal_value, external_uuid, event_date, event_time, event_at) VALUES {}",
				rows.join(", ")
			),
			Vec::new(),
		)
		.await
		.expect("aggregate_records rows should be seeded");
	connection
		.execute(
			"INSERT INTO related_records (id, value_i64) VALUES (10, 100), (11, 200)",
			Vec::new(),
		)
		.await
		.expect("related_records rows should be seeded");
	connection
		.execute(
			"INSERT INTO model_records (id, related_id) VALUES (1, 10), (2, NULL), (3, 11), (4, 11)",
			Vec::new(),
		)
		.await
		.expect("model_records rows should be seeded");
}

fn label<M, V, K>(expression: TypedExpression<M, V, K>, name: &str) -> LabeledExpression<M, K>
where
	K: AnnotationExpressionKind,
{
	expression
		.label(name)
		.expect("aggregate label should be valid")
}

async fn run_matrix(fixture: BackendFixture) {
	let backend = fixture.name();
	let mut connection = fixture.connection();
	create_schema(&connection, backend).await;
	seed_data(&connection, backend).await;

	let aggregates = [
		label(func::count_all::<AggregateRecord>(), "row_count"),
		label(
			func::count(AggregateRecord::field_integer_value()),
			"integer_count",
		),
		label(
			func::sum(AggregateRecord::field_integer_value()),
			"integer_sum",
		),
		label(
			func::avg(AggregateRecord::field_integer_value()),
			"integer_average",
		),
		label(func::sum(AggregateRecord::field_float_value()), "float_sum"),
		label(
			func::avg(AggregateRecord::field_float_value()),
			"float_average",
		),
		label(
			func::sum(AggregateRecord::field_decimal_value()),
			"decimal_sum",
		),
		label(
			func::avg(AggregateRecord::field_decimal_value()),
			"decimal_average",
		),
		label(func::min(AggregateRecord::field_name()), "first_name"),
		label(func::max(AggregateRecord::field_name()), "last_name"),
		label(func::min(AggregateRecord::field_event_date()), "first_date"),
		label(func::max(AggregateRecord::field_event_date()), "last_date"),
		label(func::min(AggregateRecord::field_event_time()), "first_time"),
		label(func::max(AggregateRecord::field_event_time()), "last_time"),
		label(
			func::min(AggregateRecord::field_event_at()),
			"first_datetime",
		),
		label(
			func::max(AggregateRecord::field_event_at()),
			"last_datetime",
		),
	];
	let result = QuerySet::<AggregateRecord>::new()
		.aggregate_with_db(aggregates, &mut connection)
		.await
		.unwrap_or_else(|error| panic!("{backend} typed aggregate matrix failed: {error}"));
	assert_eq!(result.get_i64("row_count").unwrap(), 3);
	assert_eq!(result.get_i64("integer_count").unwrap(), 2);
	assert_eq!(result.get_i64("integer_sum").unwrap(), 40);
	assert_eq!(result.get_f64("integer_average").unwrap(), 20.0);
	assert_eq!(result.get_f64("float_sum").unwrap(), 4.0);
	assert_eq!(result.get_f64("float_average").unwrap(), 2.0);
	assert_eq!(
		result.get_decimal("decimal_sum").unwrap(),
		rust_decimal::Decimal::new(3075, 2)
	);
	assert_eq!(
		result.get_decimal("decimal_average").unwrap(),
		rust_decimal::Decimal::new(15375, 3)
	);
	assert!(
		matches!(result.get("first_name").unwrap(), AggregateValue::String(value) if value == "alpha")
	);
	assert!(
		matches!(result.get("last_name").unwrap(), AggregateValue::String(value) if value == "gamma")
	);
	assert!(
		matches!(result.get("first_date").unwrap(), AggregateValue::Date(value) if value.to_string() == "2024-01-02")
	);
	assert!(
		matches!(result.get("last_date").unwrap(), AggregateValue::Date(value) if value.to_string() == "2024-01-04")
	);
	assert!(
		matches!(result.get("first_time").unwrap(), AggregateValue::Time(value) if value.to_string().starts_with("03:04:05"))
	);
	assert!(
		matches!(result.get("last_time").unwrap(), AggregateValue::Time(value) if value.to_string().starts_with("05:06:07"))
	);
	assert!(
		matches!(result.get("first_datetime").unwrap(), AggregateValue::DateTime(AggregateDateTime::Utc(value)) if value.to_rfc3339().starts_with("2024-01-02T03:04:05"))
	);
	assert!(
		matches!(result.get("last_datetime").unwrap(), AggregateValue::DateTime(AggregateDateTime::Utc(value)) if value.to_rfc3339().starts_with("2024-01-04T05:06:07"))
	);
	assert_eq!(result.iter().map(|(label, _)| label).collect::<Vec<_>>(), {
		let mut labels = vec![
			"row_count",
			"integer_count",
			"integer_sum",
			"integer_average",
			"float_sum",
			"float_average",
			"decimal_sum",
			"decimal_average",
			"first_name",
			"last_name",
			"first_date",
			"last_date",
			"first_time",
			"last_time",
			"first_datetime",
			"last_datetime",
		];
		labels.sort_unstable();
		labels
	});

	// Uuid intentionally does not implement `OrderedAggregateStorage`: UUID
	// ordering is not portable across PostgreSQL, MySQL, and SQLite, so
	// `func::min`/`func::max` on a Uuid field is rejected at compile time
	// (see tests/ui/typed_aggregation/fail/uuid_min.rs).

	let relation_result = QuerySet::<ModelRecord>::new()
		.aggregate_with_db(
			[
				label(func::count(ModelRecord::rel_related()), "related_count"),
				label(
					func::count(ModelRecord::rel_related().field(RelatedRecord::field_i64()))
						.distinct(),
					"distinct_related_count",
				),
			],
			&mut connection,
		)
		.await
		.unwrap_or_else(|error| panic!("{backend} relation aggregate failed: {error}"));
	assert_eq!(relation_result.get_i64("related_count").unwrap(), 3);
	assert_eq!(
		relation_result.get_i64("distinct_related_count").unwrap(),
		2
	);

	let zero_relation_result = QuerySet::<ModelRecord>::new()
		.filter(Filter::new(
			"model_records.id",
			FilterOperator::Eq,
			FilterValue::Int(2),
		))
		.aggregate_with_db(
			label(func::count(ModelRecord::rel_related()), "related_count"),
			&mut connection,
		)
		.await
		.unwrap_or_else(|error| panic!("{backend} zero-child relation aggregate failed: {error}"));
	assert_eq!(zero_relation_result.get_i64("related_count").unwrap(), 0);

	let empty = QuerySet::<AggregateRecord>::new().filter(Filter::new(
		"id",
		FilterOperator::Eq,
		FilterValue::Int(999),
	));
	let empty_result = empty
		.aggregate_with_db(
			[
				label(func::count_all::<AggregateRecord>(), "count"),
				label(func::sum(AggregateRecord::field_integer_value()), "sum"),
			],
			&mut connection,
		)
		.await
		.expect("empty aggregate should execute one row");
	assert_eq!(empty_result.get_i64("count").unwrap(), 0);
	assert!(matches!(
		empty_result.get("sum").unwrap(),
		AggregateValue::Null
	));

	let grouped_sql = QuerySet::<AggregateRecord>::new()
		.annotate(label(func::count_all::<AggregateRecord>(), "grouped_count"))
		.expect("grouped annotation should be accepted")
		.to_sql()
		.expect("grouped annotation SQL should compile");
	assert!(grouped_sql.contains("GROUP BY"));
	assert!(grouped_sql.contains("display_name"));

	let sliced = QuerySet::<AggregateRecord>::new().limit(1);
	let sliced_result = sliced
		.aggregate_with_db(
			label(func::count_all::<AggregateRecord>(), "count"),
			&mut connection,
		)
		.await
		.expect("sliced aggregate should execute");
	assert_eq!(sliced_result.get_i64("count").unwrap(), 1);

	let none_result = QuerySet::<AggregateRecord>::new()
		.none()
		.aggregate(label(func::count_all::<AggregateRecord>(), "count"))
		.await
		.expect("none aggregate should short-circuit");
	assert_eq!(none_result.get_i64("count").unwrap(), 0);
}

#[tokio::test]
async fn typed_aggregates_postgres_mysql_and_sqlite_matrix() {
	let fixtures = vec![
		postgres_fixture().await,
		mysql_fixture().await,
		sqlite_fixture().await,
	];
	for fixture in fixtures {
		run_matrix(fixture).await;
	}
}
