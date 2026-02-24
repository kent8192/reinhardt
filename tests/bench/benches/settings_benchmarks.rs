//! Settings and configuration benchmarks
//!
//! Benchmarks for settings loading and configuration operations:
//! - EnvLoader creation and configuration
//! - Profile parsing and detection
//! - SettingsBuilder operations

use criterion::{Criterion, criterion_group, criterion_main};
use reinhardt_conf::settings::builder::SettingsBuilder;
use reinhardt_conf::settings::env_loader::EnvLoader;
use reinhardt_conf::settings::profile::Profile;
use reinhardt_conf::settings::sources::EnvSource;
use std::hint::black_box;
use std::path::PathBuf;

fn benchmark_env_loader(c: &mut Criterion) {
	// EnvLoader creation
	c.bench_function("env_loader_create", |b| {
		b.iter(|| black_box(EnvLoader::new()));
	});

	// EnvLoader with path configuration
	c.bench_function("env_loader_with_path", |b| {
		b.iter(|| {
			black_box(
				EnvLoader::new()
					.path(PathBuf::from(".env"))
					.overwrite(false)
					.interpolate(true),
			)
		});
	});

	// EnvLoader builder chain
	c.bench_function("env_loader_full_config", |b| {
		b.iter(|| {
			black_box(
				EnvLoader::new()
					.path(PathBuf::from(".env"))
					.path(PathBuf::from(".env.local"))
					.overwrite(false)
					.interpolate(true),
			)
		});
	});
}

fn benchmark_profile(c: &mut Criterion) {
	// Profile parsing
	c.bench_function("profile_parse_development", |b| {
		b.iter(|| black_box(Profile::parse("development")));
	});

	c.bench_function("profile_parse_production", |b| {
		b.iter(|| black_box(Profile::parse("production")));
	});

	c.bench_function("profile_parse_staging", |b| {
		b.iter(|| black_box(Profile::parse("staging")));
	});

	c.bench_function("profile_parse_custom", |b| {
		b.iter(|| black_box(Profile::parse("custom_profile")));
	});

	// Profile from environment
	c.bench_function("profile_from_env", |b| {
		// SAFETY: This benchmark runs in a single-threaded context.
		// Setting APP_ENVIRONMENT is safe as it's only used for Profile::from_env()
		// and is immediately removed after the benchmark iteration.
		unsafe { std::env::set_var("APP_ENVIRONMENT", "development") };
		b.iter(|| black_box(Profile::from_env()));
		// SAFETY: This is paired with the set_var above in the same single-threaded context.
		unsafe { std::env::remove_var("APP_ENVIRONMENT") };
	});

	// Profile comparison
	c.bench_function("profile_is_development", |b| {
		let profile = Profile::Development;
		b.iter(|| black_box(profile.is_development()));
	});

	c.bench_function("profile_is_production", |b| {
		let profile = Profile::Production;
		b.iter(|| black_box(profile.is_production()));
	});
}

fn benchmark_settings_builder(c: &mut Criterion) {
	// SettingsBuilder creation
	c.bench_function("settings_builder_create", |b| {
		b.iter(|| black_box(SettingsBuilder::new()));
	});

	// SettingsBuilder with profile
	c.bench_function("settings_builder_with_profile", |b| {
		b.iter(|| black_box(SettingsBuilder::new().profile(Profile::Development)));
	});

	// SettingsBuilder with strict mode
	c.bench_function("settings_builder_with_strict", |b| {
		b.iter(|| black_box(SettingsBuilder::new().strict(true)));
	});

	// SettingsBuilder with env source
	c.bench_function("settings_builder_with_env_source", |b| {
		b.iter(|| black_box(SettingsBuilder::new().add_source(EnvSource::new())));
	});

	// SettingsBuilder full configuration
	c.bench_function("settings_builder_full_config", |b| {
		b.iter(|| {
			black_box(
				SettingsBuilder::new()
					.profile(Profile::Development)
					.strict(false)
					.add_source(EnvSource::new()),
			)
		});
	});

	// SettingsBuilder build (empty)
	c.bench_function("settings_builder_build_empty", |b| {
		b.iter(|| black_box(SettingsBuilder::new().build()));
	});

	// SettingsBuilder build with profile
	c.bench_function("settings_builder_build_with_profile", |b| {
		b.iter(|| black_box(SettingsBuilder::new().profile(Profile::Development).build()));
	});
}

fn benchmark_env_source(c: &mut Criterion) {
	// EnvSource creation
	c.bench_function("env_source_create", |b| {
		b.iter(|| black_box(EnvSource::new()));
	});

	// EnvSource with prefix
	c.bench_function("env_source_with_prefix", |b| {
		b.iter(|| black_box(EnvSource::new().with_prefix("APP")));
	});
}

criterion_group!(
	benches,
	benchmark_env_loader,
	benchmark_profile,
	benchmark_settings_builder,
	benchmark_env_source
);
criterion_main!(benches);
