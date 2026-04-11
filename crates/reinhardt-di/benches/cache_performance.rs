//! Benchmark: Cache performance (hit vs miss)

use criterion::{Criterion, criterion_group, criterion_main};
use reinhardt_di::{DiResult, Injectable, InjectionContext, SingletonScope};
use std::hint::black_box;
use std::sync::Arc;

// Benchmark service
#[derive(Clone)]
#[allow(dead_code)]
struct BenchService {
	id: usize,
}

#[async_trait::async_trait]
impl Injectable for BenchService {
	async fn inject(ctx: &InjectionContext) -> DiResult<Self> {
		if let Some(cached) = ctx.get_request::<BenchService>() {
			return Ok((*cached).clone());
		}

		let service = BenchService { id: 42 };
		ctx.set_request(service.clone());
		Ok(service)
	}
}

// Heavy computation service (expensive to create)
#[derive(Clone)]
#[allow(dead_code)]
struct HeavyService {
	computed_value: u64,
}

#[async_trait::async_trait]
impl Injectable for HeavyService {
	async fn inject(ctx: &InjectionContext) -> DiResult<Self> {
		if let Some(cached) = ctx.get_request::<HeavyService>() {
			return Ok((*cached).clone());
		}

		// Simulate expensive computation
		let computed_value = (0..1000).fold(0u64, |acc, x| acc.wrapping_add(x));

		let service = HeavyService { computed_value };
		ctx.set_request(service.clone());
		Ok(service)
	}
}

fn benchmark_cache_hit_overhead(c: &mut Criterion) {
	let rt = tokio::runtime::Runtime::new().unwrap();

	c.bench_function("cache_hit", |b| {
		b.to_async(&rt).iter(|| async {
			let singleton = Arc::new(SingletonScope::new());
			let ctx = InjectionContext::builder(singleton).build();

			// First injection (cache miss)
			let _ = BenchService::inject(&ctx).await.unwrap();

			// Second injection (cache hit) - this is measured
			black_box(BenchService::inject(&ctx).await.unwrap())
		});
	});
}

fn benchmark_cache_miss_overhead(c: &mut Criterion) {
	let rt = tokio::runtime::Runtime::new().unwrap();

	c.bench_function("cache_miss", |b| {
		b.to_async(&rt).iter(|| async {
			// Create new context for each iteration (always cache miss)
			let singleton = Arc::new(SingletonScope::new());
			let ctx = InjectionContext::builder(singleton).build();

			// First injection (cache miss) - this is measured
			black_box(BenchService::inject(&ctx).await.unwrap())
		});
	});
}

fn benchmark_mixed_workload(c: &mut Criterion) {
	let rt = tokio::runtime::Runtime::new().unwrap();

	c.bench_function("mixed_hit_miss", |b| {
		b.to_async(&rt).iter(|| async {
			let singleton = Arc::new(SingletonScope::new());
			let ctx = InjectionContext::builder(singleton).build();

			// Cache miss (first injection)
			let _ = HeavyService::inject(&ctx).await.unwrap();

			// 9 cache hits
			for _ in 0..9 {
				let _ = HeavyService::inject(&ctx).await.unwrap();
			}

			// Return last result
			black_box(HeavyService::inject(&ctx).await.unwrap())
		});
	});
}

criterion_group!(
	benches,
	benchmark_cache_hit_overhead,
	benchmark_cache_miss_overhead,
	benchmark_mixed_workload
);
criterion_main!(benches);
