//! Performance benchmarks for Reinhardt testing utilities
//!
//! These benchmarks test the performance of various testing utilities
//! including database operations, serialization, template rendering,
//! and other critical paths.

use bytes::Bytes;
use criterion::{Criterion, criterion_group, criterion_main};
use http::{HeaderMap, StatusCode};
use reinhardt_db::orm::{CascadeOption, LoadingStrategy, Relationship, RelationshipType};
use reinhardt_rest::serializers::{CharField, EmailField, IntegerField, JsonSerializer};
use reinhardt_test::{APIClient, APIRequestFactory, MockFunction, Spy, TestResponse};
use reinhardt_urls::proxy::{
	AssociationProxy, CollectionProxy, ProxyTarget, ScalarProxy, ScalarValue,
};
use serde::{Deserialize, Serialize};
use std::hint::black_box;
use tokio::runtime::Runtime;

/// Test models for benchmarking
#[derive(Debug, Clone, Serialize, Deserialize)]
struct BenchmarkUser {
	id: Option<i64>,
	name: String,
	email: String,
	posts: Vec<BenchmarkPost>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BenchmarkPost {
	id: Option<i64>,
	user_id: i64,
	title: String,
	content: String,
	tags: Vec<String>,
}

reinhardt_test::impl_test_model!(BenchmarkUser, i64, "benchmark_users");
reinhardt_test::impl_test_model!(BenchmarkPost, i64, "benchmark_posts");

/// Benchmark API client operations
fn benchmark_api_client_operations(c: &mut Criterion) {
	let rt = Runtime::new().unwrap();

	c.bench_function("api_client_create", |b| {
		b.iter(|| {
			black_box(APIClient::new());
		});
	});

	c.bench_function("api_client_set_header", |b| {
		b.iter(|| {
			rt.block_on(async {
				let client = APIClient::new();
				let _ = client
					.set_header("Authorization", "Bearer test_token")
					.await;
			});
		});
	});

	c.bench_function("api_client_get_request", |b| {
		let client = APIClient::new();
		b.iter(|| {
			rt.block_on(async {
				let _ = client.get("/api/users").await;
			});
		});
	});

	c.bench_function("api_client_post_request", |b| {
		let client = APIClient::new();
		let data = serde_json::json!({"name": "Test User", "email": "test@example.com"});
		b.iter(|| {
			rt.block_on(async {
				let _ = client.post("/api/users", &data, "application/json").await;
			});
		});
	});
}

/// Benchmark request factory operations
fn benchmark_request_factory_operations(c: &mut Criterion) {
	c.bench_function("request_factory_create", |b| {
		b.iter(|| {
			black_box(APIRequestFactory::new());
		});
	});

	c.bench_function("request_factory_build_get", |b| {
		let factory = APIRequestFactory::new();
		b.iter(|| {
			let _ = black_box(factory.get("/api/users").build());
		});
	});

	c.bench_function("request_factory_build_post", |b| {
		let factory = APIRequestFactory::new();
		b.iter(|| {
			let _ = black_box(factory.post("/api/users").build());
		});
	});

	c.bench_function("request_factory_with_headers", |b| {
		let factory = APIRequestFactory::new();
		b.iter(|| {
			let _ = black_box(
				factory
					.get("/api/users")
					.header("Authorization", "Bearer token")
					.unwrap()
					.header("Content-Type", "application/json")
					.unwrap()
					.build(),
			);
		});
	});
}

/// Benchmark mock and spy operations
fn benchmark_mock_spy_operations(c: &mut Criterion) {
	c.bench_function("mock_function_create", |b| {
		b.iter(|| {
			black_box(MockFunction::<i32>::new());
		});
	});

	c.bench_function("mock_function_call", |b| {
		let rt = Runtime::new().unwrap();
		let mock = MockFunction::<i32>::new();
		b.iter(|| {
			rt.block_on(async {
				black_box(mock.call(vec![serde_json::Value::Number(42.into())]).await);
			});
		});
	});

	c.bench_function("spy_create", |b| {
		b.iter(|| {
			black_box(Spy::<()>::new());
		});
	});

	c.bench_function("spy_record_call", |b| {
		let rt = Runtime::new().unwrap();
		let spy = Spy::<()>::new();
		b.iter(|| {
			rt.block_on(async {
				spy.record_call(vec![serde_json::Value::Number(42.into())])
					.await;
			});
		});
	});

	c.bench_function("spy_get_calls", |b| {
		let rt = Runtime::new().unwrap();
		let spy = Spy::<()>::new();
		rt.block_on(async {
			for i in 0..100 {
				spy.record_call(vec![serde_json::Value::Number(i.into())])
					.await;
			}
		});
		b.iter(|| {
			rt.block_on(async {
				black_box(spy.get_calls().await);
			});
		});
	});
}

/// Benchmark test response operations
fn benchmark_test_response_operations(c: &mut Criterion) {
	c.bench_function("test_response_create", |b| {
		b.iter(|| {
			black_box(TestResponse::with_body(
				StatusCode::OK,
				HeaderMap::new(),
				Bytes::from("{}"),
			));
		});
	});

	c.bench_function("test_response_assert_status", |b| {
		let response = TestResponse::with_body(StatusCode::OK, HeaderMap::new(), Bytes::from("{}"));
		b.iter(|| {
			black_box(response.status());
		});
	});

	c.bench_function("test_response_parse_json", |b| {
		let response = TestResponse::with_body(
			StatusCode::OK,
			HeaderMap::new(),
			Bytes::from(r#"{"name": "Test User", "age": 30}"#),
		);
		b.iter(|| {
			let _ = black_box(response.json::<serde_json::Value>());
		});
	});
}

// For settings benchmarks, see settings_benchmarks.rs

/// Benchmark proxy operations
fn benchmark_proxy_operations(c: &mut Criterion) {
	c.bench_function("association_proxy_create", |b| {
		b.iter(|| {
			black_box(AssociationProxy::<(), ()>::new("posts", "title"));
		});
	});

	c.bench_function("collection_proxy_create", |b| {
		b.iter(|| {
			black_box(CollectionProxy::new("posts", "title"));
		});
	});

	c.bench_function("scalar_proxy_create", |b| {
		b.iter(|| {
			black_box(ScalarProxy::new("profile", "bio"));
		});
	});

	c.bench_function("scalar_value_conversion", |b| {
		b.iter(|| {
			let value = ScalarValue::String("test".to_string());
			let _ = black_box(value.as_string());
		});
	});

	c.bench_function("proxy_target_serialization", |b| {
		b.iter(|| {
			let target = ProxyTarget::Collection(vec![
				ScalarValue::String("test".to_string()),
				ScalarValue::Integer(42),
			]);
			let _ = black_box(serde_json::to_string(&target));
		});
	});
}

/// Benchmark ORM relationship operations
fn benchmark_orm_relationship_operations(c: &mut Criterion) {
	c.bench_function("relationship_create", |b| {
		b.iter(|| {
			black_box(
				Relationship::<BenchmarkUser, BenchmarkPost>::new(
					"posts",
					RelationshipType::OneToMany,
				)
				.with_foreign_key("user_id")
				.with_lazy(LoadingStrategy::Lazy),
			);
		});
	});

	c.bench_function("relationship_with_cascade", |b| {
		b.iter(|| {
			black_box(
				Relationship::<BenchmarkUser, BenchmarkPost>::new(
					"posts",
					RelationshipType::OneToMany,
				)
				.with_cascade("all, delete-orphan")
				.with_foreign_key("user_id"),
			);
		});
	});

	c.bench_function("relationship_sql_generation", |b| {
		use reinhardt_db::orm::types::DatabaseDialect;
		let relationship =
			Relationship::<BenchmarkUser, BenchmarkPost>::new("posts", RelationshipType::OneToMany)
				.with_foreign_key("user_id")
				.with_lazy(LoadingStrategy::Lazy);

		b.iter(|| {
			black_box(relationship.load_sql("users.id", DatabaseDialect::PostgreSQL));
		});
	});

	c.bench_function("cascade_option_parsing", |b| {
		b.iter(|| {
			black_box(CascadeOption::parse("all, delete-orphan, save-update"));
		});
	});
}

// For template benchmarks, see template_benchmarks.rs

// For JWT and permission benchmarks, see auth_benchmarks.rs

/// Benchmark serialization operations
fn benchmark_serialization_operations(c: &mut Criterion) {
	c.bench_function("json_serializer_create", |b| {
		b.iter(|| {
			black_box(JsonSerializer::<serde_json::Value>::new());
		});
	});

	c.bench_function("char_field_validation", |b| {
		let field = CharField::new().max_length(100);
		b.iter(|| {
			let _ = black_box(field.validate("test_string"));
		});
	});

	c.bench_function("integer_field_validation", |b| {
		let field = IntegerField::new().min_value(0).max_value(100);
		b.iter(|| {
			let _ = black_box(field.validate(42));
		});
	});

	c.bench_function("email_field_validation", |b| {
		let field = EmailField::new();
		b.iter(|| {
			let _ = black_box(field.validate("test@example.com"));
		});
	});
}

// For concurrent operation benchmarks, see concurrent_benchmarks.rs

/// Benchmark memory usage
fn benchmark_memory_usage(c: &mut Criterion) {
	c.bench_function("large_collection_proxy", |b| {
		b.iter(|| {
			let proxy = CollectionProxy::new("large_collection", "data");
			black_box(proxy);
		});
	});
}

criterion_group!(
	benches,
	benchmark_api_client_operations,
	benchmark_request_factory_operations,
	benchmark_mock_spy_operations,
	benchmark_test_response_operations,
	benchmark_proxy_operations,
	benchmark_orm_relationship_operations,
	benchmark_serialization_operations,
	benchmark_memory_usage
);

criterion_main!(benches);
