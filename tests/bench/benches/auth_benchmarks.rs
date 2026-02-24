//! Authentication and authorization benchmarks
//!
//! Benchmarks for JWT operations and permission checks:
//! - JWT token generation and encoding
//! - JWT token verification and decoding
//! - Claims creation and validation

use chrono::Duration;
use criterion::{Criterion, criterion_group, criterion_main};
use reinhardt_auth::jwt::{Claims, JwtAuth};
use std::hint::black_box;

fn benchmark_jwt_creation(c: &mut Criterion) {
	// JwtAuth instance creation
	c.bench_function("jwt_auth_create", |b| {
		let secret = b"my-super-secret-key-32bytes!!!";
		b.iter(|| black_box(JwtAuth::new(secret)));
	});

	// Claims creation
	c.bench_function("jwt_claims_create", |b| {
		b.iter(|| {
			black_box(Claims::new(
				"user_123".to_string(),
				"testuser".to_string(),
				Duration::hours(24),
			))
		});
	});
}

fn benchmark_jwt_encoding(c: &mut Criterion) {
	let jwt = JwtAuth::new(b"my-super-secret-key-32bytes!!!");

	// Token encoding
	c.bench_function("jwt_encode_token", |b| {
		let claims = Claims::new(
			"user_123".to_string(),
			"testuser".to_string(),
			Duration::hours(24),
		);
		b.iter(|| black_box(jwt.encode(&claims)));
	});

	// Token generation (includes Claims creation)
	c.bench_function("jwt_generate_token", |b| {
		b.iter(|| black_box(jwt.generate_token("user_123".to_string(), "testuser".to_string())));
	});
}

fn benchmark_jwt_decoding(c: &mut Criterion) {
	let jwt = JwtAuth::new(b"my-super-secret-key-32bytes!!!");
	let token = jwt
		.generate_token("user_123".to_string(), "testuser".to_string())
		.unwrap();

	// Token decoding
	c.bench_function("jwt_decode_token", |b| {
		b.iter(|| black_box(jwt.decode(&token)));
	});

	// Token verification (decode + expiration check)
	c.bench_function("jwt_verify_token", |b| {
		b.iter(|| {
			let claims = jwt.decode(&token).unwrap();
			black_box(!claims.is_expired())
		});
	});
}

fn benchmark_jwt_full_cycle(c: &mut Criterion) {
	let jwt = JwtAuth::new(b"my-super-secret-key-32bytes!!!");

	// Full encode-decode cycle
	c.bench_function("jwt_full_cycle", |b| {
		b.iter(|| {
			let token = jwt
				.generate_token("user_123".to_string(), "testuser".to_string())
				.unwrap();
			let claims = jwt.decode(&token).unwrap();
			black_box(!claims.is_expired())
		});
	});

	// Multiple token operations (simulating concurrent verification)
	c.bench_function("jwt_batch_verify_10", |b| {
		let tokens: Vec<_> = (0..10)
			.map(|i| {
				jwt.generate_token(format!("user_{}", i), format!("testuser{}", i))
					.unwrap()
			})
			.collect();

		b.iter(|| {
			for token in &tokens {
				let claims = jwt.decode(token).unwrap();
				black_box(!claims.is_expired());
			}
		});
	});
}

fn benchmark_claims_operations(c: &mut Criterion) {
	// Claims expiration check
	c.bench_function("claims_is_expired", |b| {
		let claims = Claims::new(
			"user_123".to_string(),
			"testuser".to_string(),
			Duration::hours(24),
		);
		b.iter(|| black_box(claims.is_expired()));
	});

	// Claims clone
	c.bench_function("claims_clone", |b| {
		let claims = Claims::new(
			"user_123".to_string(),
			"testuser".to_string(),
			Duration::hours(24),
		);
		b.iter(|| black_box(claims.clone()));
	});
}

criterion_group!(
	benches,
	benchmark_jwt_creation,
	benchmark_jwt_encoding,
	benchmark_jwt_decoding,
	benchmark_jwt_full_cycle,
	benchmark_claims_operations
);
criterion_main!(benches);
