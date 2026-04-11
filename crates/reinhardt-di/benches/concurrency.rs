//! Benchmark: Concurrent resolution and scope contention

use criterion::{Criterion, criterion_group, criterion_main};
use reinhardt_di::{DiResult, Injectable, InjectionContext, SingletonScope};
use std::hint::black_box;
use std::sync::Arc;

// Concurrent service
#[derive(Clone)]
#[allow(dead_code)]
struct ConcurrentService {
	id: usize,
}

#[async_trait::async_trait]
impl Injectable for ConcurrentService {
	async fn inject(ctx: &InjectionContext) -> DiResult<Self> {
		if let Some(cached) = ctx.get_request::<ConcurrentService>() {
			return Ok((*cached).clone());
		}

		let service = ConcurrentService { id: 1 };
		ctx.set_request(service.clone());
		Ok(service)
	}
}

fn benchmark_concurrent_resolution(c: &mut Criterion) {
	let rt = tokio::runtime::Runtime::new().unwrap();

	c.bench_function("concurrent_resolution_10_tasks", |b| {
		b.to_async(&rt).iter(|| async {
			let singleton = Arc::new(SingletonScope::new());

			// Spawn 10 concurrent tasks
			let mut handles = Vec::new();

			for _ in 0..10 {
				let singleton_clone = singleton.clone();

				let handle = tokio::spawn(async move {
					// Each task gets its own context (with new RequestScope internally)
					let ctx = InjectionContext::builder(singleton_clone).build();

					ConcurrentService::inject(&ctx).await.unwrap()
				});

				handles.push(handle);
			}

			// Wait for all tasks
			let mut results = Vec::new();
			for handle in handles {
				results.push(handle.await.unwrap());
			}

			black_box(results)
		});
	});
}

fn benchmark_scope_contention(c: &mut Criterion) {
	let rt = tokio::runtime::Runtime::new().unwrap();

	c.bench_function("scope_contention_shared_singleton", |b| {
		b.to_async(&rt).iter(|| async {
			// Shared singleton scope (potential contention point)
			let singleton = Arc::new(SingletonScope::new());

			// Spawn 20 tasks accessing same singleton
			let mut handles = Vec::new();

			for _ in 0..20 {
				let singleton_clone = singleton.clone();

				let handle = tokio::spawn(async move {
					// Each task gets its own context
					let ctx = InjectionContext::builder(singleton_clone).build();

					// Inject 5 times per task
					for _ in 0..5 {
						let _ = ConcurrentService::inject(&ctx).await.unwrap();
					}
				});

				handles.push(handle);
			}

			// Wait for all tasks
			for handle in handles {
				handle.await.unwrap();
			}

			black_box(())
		});
	});
}

criterion_group!(
	benches,
	benchmark_concurrent_resolution,
	benchmark_scope_contention
);
criterion_main!(benches);
