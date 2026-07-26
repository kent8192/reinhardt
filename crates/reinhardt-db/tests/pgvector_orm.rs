// The model macro emits the framework's native cfg gate into this standalone
// integration-test crate, where Cargo does not declare that cfg name.
#![allow(unexpected_cfgs)]

use std::error::Error as _;

use reinhardt_core::macros::model;
use reinhardt_db::{
	backends::{DatabaseConnection as BackendsConnection, error::DatabaseErrorKind},
	migrations::{
		DatabaseMigrationExecutor, Migration, MigrationAutodetector, MigrationError, ProjectState,
		model_registry::global_registry, operations::postgres::CreateExtension,
	},
	orm::{
		DatabaseConnectionLease, Model, QueryValue, Vector,
		query::{Filter, FilterOperator, FilterValue, QuerySet},
	},
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use testcontainers::{
	GenericImage, ImageExt,
	core::{IntoContainerPort, WaitFor},
	runners::AsyncRunner,
};

const APP_LABEL: &str = "task9_pgvector";
const TABLE_NAME: &str = "task9_pgvector_documents";
const HNSW_INDEX_NAME: &str = "task9_pgvector_documents_embedding_cosine_hnsw";
const IVFFLAT_INDEX_NAME: &str = "task9_pgvector_documents_summary_l2_ivfflat";

#[model(app_label = "task9_pgvector", table_name = "task9_pgvector_documents")]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct PgvectorDocument {
	#[field(primary_key = true)]
	id: Option<i64>,
	#[field(max_length = 64)]
	name: String,
	#[field(index(
		name = "task9_pgvector_documents_embedding_cosine_hnsw",
		method = "hnsw",
		opclass = "vector_cosine_ops",
		m = 16,
		ef_construction = 64
	))]
	embedding: Vector<3>,
	#[field(index(
		name = "task9_pgvector_documents_summary_l2_ivfflat",
		method = "ivfflat",
		opclass = "vector_l2_ops",
		lists = 10
	))]
	summary: Vector<3>,
}

fn vector(values: [f32; 3]) -> Vector<3> {
	let vector = Vector::try_from_slice(&values).expect("test vectors should be valid");
	assert_eq!(vector.as_slice().len(), 3);
	assert_eq!(
		vector
			.as_slice()
			.iter()
			.map(|value| value.is_finite())
			.collect::<Vec<_>>(),
		vec![true, true, true]
	);
	vector
}

fn model_migrations() -> Vec<Migration> {
	let metadata = global_registry()
		.get_model(APP_LABEL, "PgvectorDocument")
		.expect("the derived pgvector model should be registered");
	assert_eq!(metadata.table_name, TABLE_NAME);
	let mut target = ProjectState::new();
	target.add_model(metadata.to_model_state());
	MigrationAutodetector::new(ProjectState::new(), target)
		.try_generate_migrations()
		.expect("registered model metadata should generate a migration")
}

async fn create_model_schema(owner: BackendsConnection) {
	let mut migrations = model_migrations();
	assert_eq!(migrations.len(), 1);
	let mut migration = migrations
		.pop()
		.expect("the pgvector model should produce one migration");
	migration.operations.insert(
		0,
		CreateExtension::new("vector")
			.into_operation()
			.expect("the extension should convert into a migration operation"),
	);
	assert!(matches!(
		migration.operations.first(),
		Some(reinhardt_db::migrations::Operation::CreateExtension { name, .. })
			if name == "vector"
	));
	let mut executor = DatabaseMigrationExecutor::new(owner);
	executor
		.apply_migrations(&[migration])
		.await
		.expect("the ordered pgvector migration should apply");
}

fn by_id(id: i64) -> QuerySet<PgvectorDocument> {
	QuerySet::new().filter(Filter::new("id", FilterOperator::Eq, FilterValue::Int(id)))
}

#[tokio::test]
async fn native_pgvector_workflow_round_trips_models_and_typed_distance_queries() {
	let container = GenericImage::new("pgvector/pgvector", "pg16")
		.with_exposed_port(5432.tcp())
		.with_wait_for(WaitFor::message_on_stderr(
			"database system is ready to accept connections",
		))
		.with_startup_timeout(std::time::Duration::from_secs(120))
		.with_env_var("POSTGRES_PASSWORD", "task9")
		.with_env_var("POSTGRES_DB", "task9_pgvector")
		.start()
		.await
		.expect("pgvector PostgreSQL container should start");
	let port = container
		.get_host_port_ipv4(5432)
		.await
		.expect("pgvector PostgreSQL port should be exposed");
	let database_url =
		format!("postgres://postgres:task9@localhost:{port}/task9_pgvector?sslmode=disable");

	let owner = BackendsConnection::connect_postgres(&database_url)
		.await
		.expect("Reinhardt should connect to pgvector PostgreSQL");
	let lease = DatabaseConnectionLease::register(owner.clone())
		.expect("the pgvector ORM connection should register");
	let mut connection = lease.handle();
	create_model_schema(owner).await;

	let column = connection
		.query_one(
			"SELECT format_type(attribute.atttypid, attribute.atttypmod) AS column_type \
			 FROM pg_attribute AS attribute \
			 JOIN pg_class AS table_class ON table_class.oid = attribute.attrelid \
			 WHERE table_class.relname = 'task9_pgvector_documents' \
			   AND attribute.attname = 'embedding'",
			Vec::new(),
		)
		.await
		.expect("the vector column should be introspectable");
	assert_eq!(
		column.get::<String>("column_type").as_deref(),
		Some("vector(3)")
	);

	let index = connection
		.query_one(
			"SELECT index_class.relname AS index_name, \
			        access_method.amname AS access_method, \
			        operator_class.opcname AS operator_class, \
			        attribute.attname AS column_name, \
			        index_catalog.indnkeyatts::bigint AS key_columns, \
			        ARRAY(SELECT option FROM unnest(index_class.reloptions) AS option ORDER BY option) \
			          AS options \
			 FROM pg_class AS index_class \
			 JOIN pg_index AS index_catalog ON index_catalog.indexrelid = index_class.oid \
			 JOIN pg_class AS table_class ON table_class.oid = index_catalog.indrelid \
			 JOIN pg_am AS access_method ON access_method.oid = index_class.relam \
			 JOIN pg_opclass AS operator_class ON operator_class.oid = index_catalog.indclass[0] \
			 JOIN pg_attribute AS attribute \
			   ON attribute.attrelid = table_class.oid \
			  AND attribute.attnum = index_catalog.indkey[0] \
			 WHERE index_class.relname = 'task9_pgvector_documents_embedding_cosine_hnsw'",
			Vec::new(),
		)
		.await
		.expect("the HNSW index should be introspectable");
	assert_eq!(
		index.data,
		json!({
			"access_method": "hnsw",
			"column_name": "embedding",
			"index_name": HNSW_INDEX_NAME,
			"key_columns": 1,
			"operator_class": "vector_cosine_ops",
			"options": ["ef_construction=64", "m=16"],
		})
	);

	let ivfflat_index = connection
		.query_one(
			"SELECT index_class.relname AS index_name, \
			        access_method.amname AS access_method, \
			        operator_class.opcname AS operator_class, \
			        attribute.attname AS column_name, \
			        index_catalog.indnkeyatts::bigint AS key_columns, \
			        ARRAY(SELECT option FROM unnest(index_class.reloptions) AS option ORDER BY option) \
			          AS options \
			 FROM pg_class AS index_class \
			 JOIN pg_index AS index_catalog ON index_catalog.indexrelid = index_class.oid \
			 JOIN pg_class AS table_class ON table_class.oid = index_catalog.indrelid \
			 JOIN pg_am AS access_method ON access_method.oid = index_class.relam \
			 JOIN pg_opclass AS operator_class ON operator_class.oid = index_catalog.indclass[0] \
			 JOIN pg_attribute AS attribute \
			   ON attribute.attrelid = table_class.oid \
			  AND attribute.attnum = index_catalog.indkey[0] \
			 WHERE index_class.relname = 'task9_pgvector_documents_summary_l2_ivfflat'",
			Vec::new(),
		)
		.await
		.expect("the IVFFlat index should be introspectable");
	assert_eq!(
		ivfflat_index.data,
		json!({
			"access_method": "ivfflat",
			"column_name": "summary",
			"index_name": IVFFLAT_INDEX_NAME,
			"key_columns": 1,
			"operator_class": "vector_l2_ops",
			"options": ["lists=10"],
		})
	);

	let manager = PgvectorDocument::objects();
	let first = manager
		.create_with_conn(
			&mut connection,
			&PgvectorDocument {
				id: None,
				name: "first".to_owned(),
				embedding: vector([1.0, 0.0, 0.0]),
				summary: vector([1.0, 0.0, 0.0]),
			},
		)
		.await
		.expect("the first model should be inserted through the ORM");
	let second = manager
		.create_with_conn(
			&mut connection,
			&PgvectorDocument {
				id: None,
				name: "second".to_owned(),
				embedding: vector([1.0, 1.0, 0.0]),
				summary: vector([1.0, 1.0, 0.0]),
			},
		)
		.await
		.expect("the second model should be inserted through the ORM");
	let mut third = manager
		.create_with_conn(
			&mut connection,
			&PgvectorDocument {
				id: None,
				name: "third".to_owned(),
				embedding: vector([0.0, 1.0, 0.0]),
				summary: vector([0.0, 1.0, 0.0]),
			},
		)
		.await
		.expect("the third model should be inserted through the ORM");

	let first_id = first
		.id
		.expect("insert should return the first primary key");
	let second_id = second
		.id
		.expect("insert should return the second primary key");
	let third_id = third
		.id
		.expect("insert should return the third primary key");
	let first_read = by_id(first_id)
		.get_with_db(&mut connection)
		.await
		.expect("the first vector should read through the ORM");
	assert_eq!(first_read.embedding.as_slice(), &[1.0, 0.0, 0.0]);

	third.embedding = vector([-1.0, 0.0, 0.0]);
	manager
		.update_with_conn(&mut connection, &third)
		.await
		.expect("the replacement vector should update through the ORM");
	let third_read = by_id(third_id)
		.get_with_db(&mut connection)
		.await
		.expect("the replacement vector should read through the ORM");
	assert_eq!(third_read.embedding.as_slice(), &[-1.0, 0.0, 0.0]);

	let target = vector([1.0, 0.0, 0.0]);
	let l2_rows = QuerySet::<PgvectorDocument>::new()
		.values(&["id"])
		.select_expr(
			"distance",
			PgvectorDocument::new_fields()
				.embedding
				.l2_distance(target.clone()),
		)
		.order_by(
			PgvectorDocument::new_fields()
				.embedding
				.l2_distance(target.clone())
				.asc(),
		)
		.rows_with_db(&mut connection)
		.await
		.expect("typed L2 projection should execute through the ORM");
	assert_eq!(
		l2_rows
			.iter()
			.map(|row| row.get::<i64>("id").expect("projected id should decode"))
			.collect::<Vec<_>>(),
		vec![first_id, second_id, third_id]
	);
	assert_eq!(
		l2_rows
			.iter()
			.map(|row| {
				row.get::<f64>("distance")
					.expect("projected L2 distance should decode")
			})
			.collect::<Vec<_>>(),
		vec![0.0, 1.0, 2.0]
	);

	let inner_product_target = vector([2.0, 1.0, 0.0]);
	let negative_inner_product_rows = QuerySet::<PgvectorDocument>::new()
		.values(&["id"])
		.select_expr(
			"negative_inner_product",
			PgvectorDocument::new_fields()
				.embedding
				.negative_inner_product(inner_product_target.clone()),
		)
		.order_by(
			PgvectorDocument::new_fields()
				.embedding
				.negative_inner_product(inner_product_target)
				.asc(),
		)
		.rows_with_db(&mut connection)
		.await
		.expect("typed negative inner-product projection should execute through the ORM");
	assert_eq!(
		negative_inner_product_rows
			.iter()
			.map(|row| row.get::<i64>("id").expect("projected id should decode"))
			.collect::<Vec<_>>(),
		vec![second_id, first_id, third_id]
	);
	assert_eq!(
		negative_inner_product_rows
			.iter()
			.map(|row| {
				row.get::<f64>("negative_inner_product")
					.expect("projected negative inner product should decode")
			})
			.collect::<Vec<_>>(),
		vec![-3.0, -2.0, 2.0]
	);

	let ordered = QuerySet::<PgvectorDocument>::new()
		.order_by(
			PgvectorDocument::new_fields()
				.embedding
				.cosine_distance(target.clone())
				.asc(),
		)
		.all_with_db(&mut connection)
		.await
		.expect("typed cosine ordering should execute with a bound vector");
	assert_eq!(
		ordered
			.iter()
			.map(|document| document.id.expect("persisted rows should have ids"))
			.collect::<Vec<_>>(),
		vec![first_id, second_id, third_id]
	);

	let selected = QuerySet::<PgvectorDocument>::new()
		.filter(
			PgvectorDocument::new_fields()
				.embedding
				.cosine_distance(target.clone())
				.lt(0.5),
		)
		.order_by(
			PgvectorDocument::new_fields()
				.embedding
				.cosine_distance(target.clone())
				.asc(),
		)
		.all_with_db(&mut connection)
		.await
		.expect("typed cosine filtering should execute with a bound vector");
	assert_eq!(
		selected
			.iter()
			.map(|document| document.id.expect("persisted rows should have ids"))
			.collect::<Vec<_>>(),
		vec![first_id, second_id]
	);

	let distance_rows = QuerySet::<PgvectorDocument>::new()
		.values(&["id"])
		.select_expr(
			"distance",
			PgvectorDocument::new_fields()
				.embedding
				.cosine_distance(target.clone()),
		)
		.order_by(
			PgvectorDocument::new_fields()
				.embedding
				.cosine_distance(target)
				.asc(),
		)
		.rows_with_db(&mut connection)
		.await
		.expect("typed distance projection should execute through the ORM");
	assert_eq!(
		distance_rows
			.iter()
			.map(|row| row.get::<i64>("id").expect("projected id should decode"))
			.collect::<Vec<_>>(),
		vec![first_id, second_id, third_id]
	);
	let distances = distance_rows
		.iter()
		.map(|row| {
			row.get::<f64>("distance")
				.expect("projected distance should decode")
		})
		.collect::<Vec<_>>();
	assert_eq!(distances[0], 0.0);
	// PostgreSQL computes cosine distance from f32 vector elements, so compare
	// the irrational result with a tight deterministic floating-point tolerance.
	assert!((distances[1] - (1.0 - 1.0 / 2.0_f64.sqrt())).abs() < 1.0e-12);
	assert_eq!(distances[2], 2.0);

	let dimension_error = match connection
		.query_one(
			"SELECT embedding <-> $1 AS distance \
			 FROM task9_pgvector_documents \
			 ORDER BY id \
			 LIMIT 1",
			vec![QueryValue::Vector(vec![1.0, 0.0])],
		)
		.await
	{
		Ok(_) => panic!("PostgreSQL should reject a two-dimensional bound vector"),
		Err(error) => error,
	};
	let dimension_database_error = dimension_error
		.database_error()
		.expect("the server failure should retain structured database context");
	assert_eq!(dimension_database_error.kind(), DatabaseErrorKind::Query);
	assert_eq!(dimension_database_error.code(), Some("22000"));
	let dimension_message = dimension_database_error.message().to_ascii_lowercase();
	assert!(
		dimension_message.contains("vector") && dimension_message.contains("dimension"),
		"the PostgreSQL diagnostic should identify a vector dimension mismatch: \
		 {dimension_message}"
	);
	assert_eq!(
		dimension_message
			.split(|character: char| !character.is_ascii_digit())
			.filter(|token| !token.is_empty())
			.collect::<Vec<_>>(),
		vec!["3", "2"],
		"the PostgreSQL diagnostic should report expected dimension 3 before actual dimension 2"
	);
	assert!(
		dimension_error
			.source()
			.and_then(|source| source.downcast_ref::<sqlx::Error>())
			.is_some()
	);

	connection
		.execute(
			"CREATE DATABASE task9_pgvector_without_extension",
			Vec::new(),
		)
		.await
		.expect("the separate missing-extension database should be created");
	let missing_database_url = format!(
		"postgres://postgres:task9@localhost:{port}/task9_pgvector_without_extension?sslmode=disable"
	);
	let missing_owner = BackendsConnection::connect_postgres(&missing_database_url)
		.await
		.expect("Reinhardt should connect to the missing-extension database");
	let mut missing_executor = DatabaseMigrationExecutor::new(missing_owner);
	let error = missing_executor
		.apply_migrations(&model_migrations())
		.await
		.expect_err("native vector DDL should fail without the extension");
	let MigrationError::DatabaseError(database_error) = &error else {
		panic!("expected a structured database migration error, got {error:?}");
	};
	assert_eq!(database_error.kind(), DatabaseErrorKind::Query);
	assert_eq!(database_error.code(), Some("42704"));
	assert_eq!(
		database_error.message(),
		"type \"vector\" does not exist. Install the pgvector extension explicitly with \
		 CreateExtension::new(\"vector\") before this operation"
	);
	assert!(
		error
			.source()
			.and_then(|source| source.downcast_ref::<reinhardt_db::backends::QueryDatabaseError>())
			.is_some()
	);
	assert!(
		database_error
			.source()
			.and_then(|source| source.downcast_ref::<sqlx::Error>())
			.is_some()
	);

	assert_eq!(
		connection.backend(),
		reinhardt_db::orm::DatabaseBackend::Postgres
	);
}
