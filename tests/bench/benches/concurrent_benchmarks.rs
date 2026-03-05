//! Concurrent operations benchmarks
//!
//! Benchmarks for concurrent and parallel operations:
//! - APIClient concurrent requests
//! - Spy concurrent recording
//! - MockFunction concurrent calls

use criterion::{Criterion, criterion_group, criterion_main};
use futures::future::join_all;
use reinhardt_test::{APIClient, MockFunction, Spy};
use std::hint::black_box;
use std::sync::Arc;
use tokio::runtime::Runtime;

fn benchmark_api_client_concurrent(c: &mut Criterion) {
	let rt = Runtime::new().unwrap();

	// Single request baseline
	c.bench_function("api_client_single_request", |b| {
		let client = APIClient::new();
		b.iter(|| rt.block_on(async { black_box(client.get("/api/test").await) }));
	});

	// Concurrent requests (5 parallel)
	c.bench_function("api_client_concurrent_5", |b| {
		let client = Arc::new(APIClient::new());
		b.iter(|| {
			rt.block_on(async {
				let handles: Vec<_> = (0..5)
					.map(|_| {
						let c = client.clone();
						tokio::spawn(async move { c.get("/api/test").await })
					})
					.collect();
				black_box(join_all(handles).await)
			})
		});
	});

	// Concurrent requests (10 parallel)
	c.bench_function("api_client_concurrent_10", |b| {
		let client = Arc::new(APIClient::new());
		b.iter(|| {
			rt.block_on(async {
				let handles: Vec<_> = (0..10)
					.map(|_| {
						let c = client.clone();
						tokio::spawn(async move { c.get("/api/test").await })
					})
					.collect();
				black_box(join_all(handles).await)
			})
		});
	});

	// Sequential requests for comparison
	c.bench_function("api_client_sequential_10", |b| {
		let client = APIClient::new();
		b.iter(|| {
			rt.block_on(async {
				for _ in 0..10 {
					let _ = black_box(client.get("/api/test").await);
				}
			})
		});
	});
}

fn benchmark_spy_concurrent(c: &mut Criterion) {
	let rt = Runtime::new().unwrap();

	// Single call baseline
	c.bench_function("spy_single_record", |b| {
		let spy = Spy::<()>::new();
		b.iter(|| {
			rt.block_on(async {
				let _: () = spy
					.record_call(vec![serde_json::Value::String("test".to_string())])
					.await;
				black_box(())
			})
		});
	});

	// Concurrent recording (10 parallel)
	c.bench_function("spy_concurrent_record_10", |b| {
		let spy = Arc::new(Spy::<()>::new());
		b.iter(|| {
			rt.block_on(async {
				let handles: Vec<_> = (0..10)
					.map(|i| {
						let s = spy.clone();
						tokio::spawn(async move {
							s.record_call(vec![serde_json::Value::Number(i.into())])
								.await
						})
					})
					.collect();
				black_box(join_all(handles).await)
			})
		});
	});

	// Concurrent recording (50 parallel)
	c.bench_function("spy_concurrent_record_50", |b| {
		let spy = Arc::new(Spy::<()>::new());
		b.iter(|| {
			rt.block_on(async {
				let handles: Vec<_> = (0..50)
					.map(|i| {
						let s = spy.clone();
						tokio::spawn(async move {
							s.record_call(vec![serde_json::Value::Number(i.into())])
								.await
						})
					})
					.collect();
				black_box(join_all(handles).await)
			})
		});
	});

	// Sequential recording for comparison
	c.bench_function("spy_sequential_record_50", |b| {
		let spy = Spy::<()>::new();
		b.iter(|| {
			rt.block_on(async {
				for i in 0..50 {
					let _: () = spy
						.record_call(vec![serde_json::Value::Number(i.into())])
						.await;
					black_box(());
				}
			})
		});
	});
}

fn benchmark_mock_function_concurrent(c: &mut Criterion) {
	let rt = Runtime::new().unwrap();

	// MockFunction creation
	c.bench_function("mock_function_create", |b| {
		b.iter(|| black_box(MockFunction::<i32>::new()));
	});

	// Single call
	c.bench_function("mock_function_single_call", |b| {
		let mock = MockFunction::<i32>::new();
		rt.block_on(mock.returns(42));
		b.iter(|| rt.block_on(async { black_box(mock.call(vec![]).await) }));
	});

	// Concurrent calls (10 parallel)
	c.bench_function("mock_function_concurrent_call_10", |b| {
		let mock = Arc::new(MockFunction::<i32>::new());
		rt.block_on(mock.returns(42));
		b.iter(|| {
			rt.block_on(async {
				let handles: Vec<_> = (0..10)
					.map(|_| {
						let m = mock.clone();
						tokio::spawn(async move { m.call(vec![]).await })
					})
					.collect();
				black_box(join_all(handles).await)
			})
		});
	});
}

fn benchmark_mixed_concurrent(c: &mut Criterion) {
	let rt = Runtime::new().unwrap();

	// Mixed operations (API + Spy)
	c.bench_function("mixed_api_spy_concurrent", |b| {
		let client = Arc::new(APIClient::new());
		let spy = Arc::new(Spy::<()>::new());
		b.iter(|| {
			rt.block_on(async {
				let api_handles: Vec<_> = (0..5)
					.map(|_| {
						let c = client.clone();
						tokio::spawn(async move { c.get("/api/test").await })
					})
					.collect();

				let spy_handles: Vec<_> = (0..5)
					.map(|i| {
						let s = spy.clone();
						tokio::spawn(async move {
							s.record_call(vec![serde_json::Value::Number(i.into())])
								.await
						})
					})
					.collect();

				let (api_results, spy_results) =
					tokio::join!(join_all(api_handles), join_all(spy_handles));
				black_box((api_results, spy_results))
			})
		});
	});
}

criterion_group!(
	benches,
	benchmark_api_client_concurrent,
	benchmark_spy_concurrent,
	benchmark_mock_function_concurrent,
	benchmark_mixed_concurrent
);
criterion_main!(benches);
