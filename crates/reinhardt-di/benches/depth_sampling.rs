//! Benchmark: Dependency depth sampling

use criterion::{Criterion, criterion_group, criterion_main};
use reinhardt_di::{DiResult, InjectionContext, SingletonScope};
use std::hint::black_box;
use std::sync::Arc;

// Nested dependency service
#[derive(Clone)]
#[allow(dead_code)]
struct Layer {
	depth: usize,
}

impl Layer {
	fn inject_depth_sync(depth: usize) -> DiResult<Self> {
		if depth == 0 {
			return Ok(Layer { depth: 0 });
		}

		// Simulate nested dependency resolution
		let _parent = Self::inject_depth_sync(depth - 1)?;

		Ok(Layer { depth })
	}
}

fn benchmark_depth_50_no_sampling(c: &mut Criterion) {
	c.bench_function("depth_50_no_sampling", |b| {
		b.iter(|| {
			let _singleton = Arc::new(SingletonScope::new());
			let _ctx = InjectionContext::builder(_singleton).build();

			// Inject with depth 50
			black_box(Layer::inject_depth_sync(50).unwrap())
		});
	});
}

fn benchmark_depth_100_with_sampling(c: &mut Criterion) {
	c.bench_function("depth_100_with_sampling", |b| {
		b.iter(|| {
			let _singleton = Arc::new(SingletonScope::new());
			let _ctx = InjectionContext::builder(_singleton).build();

			// Inject with depth 100
			// In practice, sampling would skip intermediate layers
			// Here we just measure full depth for comparison
			black_box(Layer::inject_depth_sync(100).unwrap())
		});
	});
}

criterion_group!(
	benches,
	benchmark_depth_50_no_sampling,
	benchmark_depth_100_with_sampling
);
criterion_main!(benches);
